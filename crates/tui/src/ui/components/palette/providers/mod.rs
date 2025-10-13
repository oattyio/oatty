//! Palette providers for command argument value suggestions.
//!
//! This module contains a `ValueProvider` implementation that sources suggestions
//! from the Heroku command registry and associated HTTP endpoints. Results are
//! cached with a TTL to keep the UI responsive while avoiding excessive network
//! requests. Fuzzy matching is applied to produce relevant, ranked suggestions.

use crate::ui::components::palette::state::ValueProvider;
use heroku_registry::find_by_group_and_cmd;
use heroku_types::CommandSpec;
use heroku_types::{ItemKind, SuggestionItem, command::CommandExecution};
use heroku_util::http_path_resolution::build_path;
use heroku_util::{fetch_json_array, fuzzy_score};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Cache entry for provider results, storing fetched items and timestamp.
#[derive(Debug)]
struct CacheEntry {
    fetched_at: Instant,
    items: Vec<String>,
}

/// A value provider that fetches suggestions from a Heroku API using registry provider bindings.
///
/// Queries the API for a provider command (e.g., "apps:list"), caches results with a TTL,
/// and returns fuzzy-matched suggestions for a given field and partial input.
#[derive(Debug, Clone)]
pub struct RegistryBackedProvider {
    /// Time-to-live duration for cached fetch results
    ttl: Duration,
    /// In-memory cache for provider results keyed by provider id
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
    /// Tracks in-flight fetches to avoid duplicating concurrent work
    active_fetches: Arc<Mutex<HashSet<String>>>,
}

impl RegistryBackedProvider {
    /// Creates a new provider with the given registry and cache TTL.
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            cache: Arc::new(Mutex::new(HashMap::new())),
            active_fetches: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Finds the provider ID for a field within a command specified by group and name.
    fn provider_for_field(
        &self,
        commands: &[CommandSpec],
        group: &str,
        name: &str,
        field: &str,
    ) -> Option<(String, Vec<heroku_types::Bind>)> {
        let command_spec = find_by_group_and_cmd(commands, group, name).ok()?;
        // Check flags first
        if let Some(flag) = command_spec.flags.iter().find(|flag| flag.name == field)
            && let Some(heroku_types::ValueProvider::Command { command_id, binds }) = &flag.provider
        {
            return Some((command_id.clone(), binds.clone()));
        }
        // Then positionals
        if let Some(positional) = command_spec.positional_args.iter().find(|arg| arg.name == field)
            && let Some(heroku_types::ValueProvider::Command { command_id, binds }) = &positional.provider
        {
            return Some((command_id.clone(), binds.clone()));
        }
        None
    }

    /// Returns cached items if present and fresh, otherwise `None`.
    fn get_cached_items_if_fresh(&self, provider_id: &str, current_time: Instant) -> Option<Vec<String>> {
        let cache = self.cache.lock().expect("Cache lock poisoned");
        let entry = cache.get(provider_id)?;
        if current_time.duration_since(entry.fetched_at) < self.ttl {
            Some(entry.items.clone())
        } else {
            None
        }
    }

    /// Parses a provider key into its (group, name) components.
    ///
    /// Accepts the canonical space-separated form ("group name") and, for internal
    /// manifest compatibility, the legacy colon-separated form ("group:name").
    /// User-facing identifiers elsewhere remain space-only; this parser is used
    /// for provider bindings embedded in CommandSpec, which may still carry colon
    /// IDs in unreleased manifests.
    fn parse_provider_key(provider_id: &str) -> Option<(&str, &str)> {
        if let Some((group, name)) = provider_id.split_once(char::is_whitespace) {
            let group = group.trim();
            let name = name.trim();
            if !group.is_empty() && !name.is_empty() {
                return Some((group, name));
            }
        }
        if let Some((group, name)) = provider_id.split_once(':') {
            let group = group.trim();
            let name = name.trim();
            if !group.is_empty() && !name.is_empty() {
                return Some((group, name));
            }
        }
        None
    }

    /// Attempts to mark a provider fetch as active; returns false if already active.
    fn try_begin_fetch(&self, provider_id: &str) -> bool {
        let mut active = self.active_fetches.lock().expect("Active fetches lock poisoned");
        active.insert(provider_id.to_string())
    }

    /// Fetches or retrieves cached values for a provider ID.
    ///
    /// Returns cached values if fresh, otherwise spawns a background fetch and returns
    /// an empty vector. Subsequent calls will use cached results after the fetch completes.
    fn list_values_for_provider(
        &self,
        commands: &[CommandSpec],
        provider_id: &str,
        variables: &serde_json::Map<String, serde_json::Value>,
    ) -> Vec<String> {
        if provider_id.is_empty() {
            return Vec::new();
        }

        // Check cache first
        let current_time = Instant::now();
        if let Some(items) = self.get_cached_items_if_fresh(provider_id, current_time) {
            return items;
        }

        // Parse provider_id as "<group>:list"
        let (group, name) = match Self::parse_provider_key(provider_id) {
            Some((group, name)) => (group, name),
            _ => return Vec::new(),
        };

        let Ok(command_spec) = find_by_group_and_cmd(commands, group, name) else {
            return Vec::new();
        };

        if !matches!(command_spec.execution(), CommandExecution::Http(_)) {
            return Vec::new();
        }

        let Some(command_http) = command_spec.http() else {
            return Vec::new();
        };

        // Build resolved path when variables provided and append query for leftover vars
        let mut resolved_spec = command_spec.clone();
        let Some(resolved_http) = resolved_spec.http_mut() else {
            return Vec::new();
        };
        if !variables.is_empty() {
            let path = build_path(&command_http.path, variables);
            // Determine which keys were used in path placeholders by checking original path for `{key}`
            let mut unused_variables: Vec<(String, String)> = Vec::new();
            for (key, value) in variables.iter() {
                let needle = format!("{{{}}}", key);
                if !command_http.path.contains(&needle) {
                    let string_value = match value {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    unused_variables.push((key.clone(), string_value));
                }
            }
            if !unused_variables.is_empty() {
                let query_string: String = unused_variables
                    .into_iter()
                    .map(|(key, value)| format!("{}={}", key, value))
                    .collect::<Vec<_>>()
                    .join("&");
                resolved_http.path = format!("{}?{}", path, query_string);
            } else {
                resolved_http.path = path;
            }
        }

        // Skip if fetch is already in progress
        if !self.try_begin_fetch(provider_id) {
            return Vec::new(); // Fetch already in progress
        }

        // Spawn background fetch
        let cache = Arc::clone(&self.cache);
        let active_fetches = Arc::clone(&self.active_fetches);
        let provider_id_clone = provider_id.to_string();
        let resolved_spec_clone = resolved_spec.clone();
        std::thread::spawn(move || {
            let fetched_items = match tokio::runtime::Runtime::new() {
                Ok(runtime) => runtime.block_on(async {
                    fetch_json_array(&resolved_spec_clone)
                        .await
                        .map(|values| values.into_iter().filter_map(label_from_value).collect::<Vec<String>>())
                        .unwrap_or_default()
                }),
                Err(_) => Vec::new(),
            };

            // Update cache and clear active fetch
            if !fetched_items.is_empty() {
                let mut cache = cache.lock().expect("Cache lock poisoned");
                cache.insert(
                    provider_id_clone.clone(),
                    CacheEntry {
                        fetched_at: Instant::now(),
                        items: fetched_items,
                    },
                );
            }
            active_fetches
                .lock()
                .expect("Active fetches lock poisoned")
                .remove(&provider_id_clone);
        });

        Vec::new()
    }
}

impl ValueProvider for RegistryBackedProvider {
    /// Suggests values for a command field based on provider bindings.
    ///
    /// The `command_key` must be in the canonical form "group name". Returns fuzzy-matched
    /// suggestions for the given `field` and `partial` input, sorted by score.
    fn suggest(
        &self,
        commands: &[CommandSpec],
        command_key: &str,
        field: &str,
        partial: &str,
        inputs: &std::collections::HashMap<String, String>,
    ) -> Vec<SuggestionItem> {
        let (group, name) = match command_key.split_once(char::is_whitespace) {
            Some((group, name)) => {
                let group = group.trim();
                let name = name.trim();
                if group.is_empty() || name.is_empty() {
                    return Vec::new();
                }
                (group, name)
            }
            _ => return Vec::new(),
        };

        let Some((provider_id, binds)) = self.provider_for_field(commands, group, name, field) else {
            return Vec::new();
        };

        // Build variables map from bindings
        let mut variables = serde_json::Map::new();
        let mut is_missing = false;
        for binding in &binds {
            if let Some(value) = inputs.get(&binding.from) {
                variables.insert(binding.provider_key.clone(), serde_json::Value::String(value.clone()));
            } else {
                is_missing = true;
                break;
            }
        }
        if is_missing {
            return Vec::new();
        }

        let values = self.list_values_for_provider(commands, &provider_id, &variables);
        let mut items: Vec<SuggestionItem> = values
            .into_iter()
            .filter_map(|value| {
                fuzzy_score(&value, partial).map(|score| SuggestionItem {
                    display: value.clone(),
                    insert_text: value,
                    kind: ItemKind::Value,
                    meta: Some(provider_id.clone()),
                    score,
                })
            })
            .collect();

        items.sort_by(|a, b| b.score.cmp(&a.score)); // Sort descending by score
        items
    }
}

/// Extracts a label from a JSON value for display and insertion.
///
/// Preference order:
/// - `name`
/// - `id`
/// - `str`
/// - any first string value found in the object
/// - or the string itself if the value is a JSON string
fn label_from_value(value: serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s),
        serde_json::Value::Object(map) => map
            .get("name")
            .or_else(|| map.get("id"))
            .or_else(|| map.get("str"))
            .and_then(|v| v.as_str().map(str::to_string))
            .or_else(|| map.into_iter().find_map(|(_, v)| v.as_str().map(str::to_string))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::RegistryBackedProvider;

    #[test]
    fn parse_provider_key_accepts_space_and_colon() {
        // Accepts canonical space-separated form
        let parsed = RegistryBackedProvider::parse_provider_key("apps list");
        assert!(parsed.is_some());
        let (g, n) = parsed.unwrap();
        assert_eq!(g, "apps");
        assert_eq!(n, "list");

        // Also accepts legacy colon-separated form for internal bindings
        let parsed_colon = RegistryBackedProvider::parse_provider_key("apps:list");
        assert!(parsed_colon.is_some());
        let (g2, n2) = parsed_colon.unwrap();
        assert_eq!(g2, "apps");
        assert_eq!(n2, "list");

        // Rejects missing name
        assert!(RegistryBackedProvider::parse_provider_key("apps   ").is_none());
        // Rejects missing group
        assert!(RegistryBackedProvider::parse_provider_key("  list").is_none());
    }
}
