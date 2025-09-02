use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use heroku_registry::Registry;
use heroku_util::fuzzy_score;

use crate::ui::components::palette::state::{ItemKind, SuggestionItem, ValueProvider};

/// Cache entry for provider results.
#[derive(Debug)]
struct CacheEntry {
    fetched_at: Instant,
    items: Vec<String>,
}

/// A value provider that uses the generated registry's provider bindings
/// to look up a provider command (e.g., "apps:list"), fetch its values
/// via Heroku API, and return suggestions.
#[derive(Debug)]
pub struct RegistryBackedProvider {
    registry: Arc<Registry>,
    ttl: Duration,
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
}

impl RegistryBackedProvider {
    pub fn new(registry: Arc<Registry>, ttl: Duration) -> Self {
        Self {
            registry,
            ttl,
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn provider_for_field(&self, group: &str, name: &str, field: &str) -> Option<String> {
        let spec = self.registry.find_by_group_and_cmd(group, name).ok()?;
        spec.providers
            .iter()
            .find(|p| p.name == field)
            .map(|p| p.provider_id.clone())
    }

    fn list_values_for_provider(&self, provider_id: &str) -> Vec<String> {
        // Cache-by-provider-id: fetch full list without filtering and filter client-side
        let now = Instant::now();
        if let Some(entry) = self.cache.lock().unwrap().get(provider_id) {
            if now.duration_since(entry.fetched_at) < self.ttl {
                return entry.items.clone();
            }
        }

        // Parse provider id as "<group>:list" and locate command spec to extract the path
        let mut parts = provider_id.split(':');
        let group = parts.next().unwrap_or("");
        let name = parts.next().unwrap_or("");
        if group.is_empty() || name.is_empty() {
            return vec![];
        }
        let Ok(spec) = self.registry.find_by_group_and_cmd(group, name) else {
            return vec![];
        };

        // Kick off a background fetch and optimistically return empty on miss.
        // Subsequent calls will return cached results.
        let cache = self.cache.clone();
        let path = spec.path.clone();
        let provider_id_owned = provider_id.to_string();
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(_) => return,
            };
            let result = rt.block_on(async move {
                match crate::cmd::fetch_json_array(&path).await {
                    Ok(values) => values
                        .into_iter()
                        .filter_map(label_from_value)
                        .collect::<Vec<String>>(),
                    Err(_) => Vec::new(),
                }
            });
            if !result.is_empty() {
                let mut guard = cache.lock().unwrap();
                guard.insert(
                    provider_id_owned,
                    CacheEntry {
                        fetched_at: Instant::now(),
                        items: result,
                    },
                );
            }
        });

        vec![]
    }
}

impl ValueProvider for RegistryBackedProvider {
    /// Suggest values by reading provider bindings from the command registry.
    /// The `command_key` must be in the form "group:name" to unambiguously
    /// identify the command.
    fn suggest(&self, command_key: &str, field: &str, partial: &str) -> Vec<SuggestionItem> {
        let (group, name) = match command_key.split_once(':') {
            Some((g, n)) => (g.to_string(), n.to_string()),
            None => return vec![],
        };
        let Some(provider_id) = self.provider_for_field(&group, &name, field) else {
            return vec![];
        };
        let values = self.list_values_for_provider(&provider_id);
        let mut items: Vec<SuggestionItem> = Vec::new();
        for v in values {
            if let Some(score) = fuzzy_score(&v, partial) {
                items.push(SuggestionItem {
                    display: v.clone(),
                    insert_text: v,
                    kind: ItemKind::Value,
                    meta: Some(provider_id.clone()),
                    score,
                });
            }
        }
        items
    }
}

fn label_from_value(v: serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s),
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(name)) = map.get("name") {
                return Some(name.clone());
            }
            if let Some(serde_json::Value::String(id)) = map.get("id") {
                return Some(id.clone());
            }
            // Pick the first string field as a fallback
            for (_k, val) in map.iter() {
                if let serde_json::Value::String(s) = val {
                    return Some(s.clone());
                }
            }
            None
        }
        _ => None,
    }
}
