use crate::ui::components::palette::state::{ItemKind, SuggestionItem, ValueProvider};
use heroku_registry::Registry;
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
    registry: Arc<Registry>,
    ttl: Duration,
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
    active_fetches: Arc<Mutex<HashSet<String>>>,
}

impl RegistryBackedProvider {
    /// Creates a new provider with the given registry and cache TTL.
    pub fn new(registry: Arc<Registry>, ttl: Duration) -> Self {
        Self {
            registry,
            ttl,
            cache: Arc::new(Mutex::new(HashMap::new())),
            active_fetches: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Finds the provider ID for a field within a command specified by group and name.
    fn provider_for_field(&self, group: &str, name: &str, field: &str) -> Option<(String, Vec<heroku_types::Bind>)> {
        let spec = self.registry.find_by_group_and_cmd(group, name).ok()?;
        // Check flags first
        if let Some(flag) = spec.flags.iter().find(|f| f.name == field)
            && let Some(heroku_types::ValueProvider::Command { command_id, binds }) = &flag.provider
        {
            return Some((command_id.clone(), binds.clone()));
        }
        // Then positionals
        if let Some(pos) = spec.positional_args.iter().find(|a| a.name == field)
            && let Some(heroku_types::ValueProvider::Command { command_id, binds }) = &pos.provider
        {
            return Some((command_id.clone(), binds.clone()));
        }
        None
    }

    /// Fetches or retrieves cached values for a provider ID.
    ///
    /// Returns cached values if fresh, otherwise spawns a background fetch and returns
    /// an empty vector. Subsequent calls will use cached results after the fetch completes.
    fn list_values_for_provider(
        &self,
        provider_id: &str,
        variables: &serde_json::Map<String, serde_json::Value>,
    ) -> Vec<String> {
        if provider_id.is_empty() {
            return Vec::new();
        }

        // Check cache first
        let now = Instant::now();
        {
            let cache = self.cache.lock().expect("Cache lock poisoned");
            if let Some(entry) = cache.get(provider_id)
                && now.duration_since(entry.fetched_at) < self.ttl
            {
                return entry.items.clone();
            }
        }

        // Parse provider_id as "<group>:list"
        let (group, name) = match provider_id.split_once(':') {
            Some((g, n)) if !g.is_empty() && !n.is_empty() => (g, n),
            _ => return Vec::new(),
        };

        // Get command spec and path
        let Ok(spec) = self.registry.find_by_group_and_cmd(group, name) else {
            return Vec::new();
        };

        // Build resolved path when variables provided and append query for leftover vars
        let mut spec_owned = spec.clone();
        if !variables.is_empty() {
            let path = build_path(&spec_owned.path, variables);
            // Determine which keys were used in path placeholders by checking original path for `{key}`
            let mut leftover: Vec<(String, String)> = Vec::new();
            for (k, v) in variables.iter() {
                let needle = format!("{{{}}}", k);
                if !spec.path.contains(&needle) {
                    let sv = match v {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    leftover.push((k.clone(), sv));
                }
            }
            if !leftover.is_empty() {
                let qp: String = leftover
                    .into_iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("&");
                spec_owned.path = format!("{}?{}", path, qp);
            } else {
                spec_owned.path = path;
            }
        }

        // Skip if fetch is already in progress
        {
            let mut active = self.active_fetches.lock().expect("Active fetches lock poisoned");
            if !active.insert(provider_id.to_string()) {
                return Vec::new(); // Fetch already in progress
            }
        }

        // Spawn background fetch
        let cache = Arc::clone(&self.cache);
        let active_fetches = Arc::clone(&self.active_fetches);
        let provider_id_owned = provider_id.to_string();
        let spec_owned = spec_owned.clone();
        std::thread::spawn(move || {
            let result = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt.block_on(async {
                    fetch_json_array(&spec_owned)
                        .await
                        .map(|values| values.into_iter().filter_map(label_from_value).collect::<Vec<String>>())
                        .unwrap_or_default()
                }),
                Err(_) => Vec::new(),
            };

            // Update cache and clear active fetch
            if !result.is_empty() {
                let mut cache = cache.lock().expect("Cache lock poisoned");
                cache.insert(
                    provider_id_owned.clone(),
                    CacheEntry {
                        fetched_at: Instant::now(),
                        items: result,
                    },
                );
            }
            active_fetches
                .lock()
                .expect("Active fetches lock poisoned")
                .remove(&provider_id_owned);
        });

        Vec::new()
    }
}

impl ValueProvider for RegistryBackedProvider {
    /// Suggests values for a command field based on provider bindings.
    ///
    /// The `command_key` must be in the form "group:name". Returns fuzzy-matched
    /// suggestions for the given `field` and `partial` input, sorted by score.
    fn suggest(
        &self,
        command_key: &str,
        field: &str,
        partial: &str,
        inputs: &std::collections::HashMap<String, String>,
    ) -> Vec<SuggestionItem> {
        let (group, name) = match command_key.split_once(':') {
            Some((g, n)) if !g.is_empty() && !n.is_empty() => (g, n),
            _ => return Vec::new(),
        };

        let Some((provider_id, binds)) = self.provider_for_field(group, name, field) else {
            return Vec::new();
        };

        // Build variables map from bindings
        let mut vars = serde_json::Map::new();
        let mut missing = false;
        for b in &binds {
            if let Some(val) = inputs.get(&b.from) {
                vars.insert(b.provider_key.clone(), serde_json::Value::String(val.clone()));
            } else {
                missing = true;
                break;
            }
        }
        if missing {
            return Vec::new();
        }

        let values = self.list_values_for_provider(&provider_id, &vars);
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

/// Extracts a label from a JSON value, preferring 'name', then 'id', then any string field.
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
