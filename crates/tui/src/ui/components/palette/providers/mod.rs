use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use heroku_registry::Registry;
use heroku_util::fuzzy_score;
use crate::ui::components::palette::state::{ItemKind, SuggestionItem, ValueProvider};

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
    fn provider_for_field(&self, group: &str, name: &str, field: &str) -> Option<String> {
        self.registry
            .find_by_group_and_cmd(group, name)
            .ok()?
            .providers
            .iter()
            .find(|p| p.name == field)
            .map(|p| p.provider_id.clone())
    }

    /// Fetches or retrieves cached values for a provider ID.
    ///
    /// Returns cached values if fresh, otherwise spawns a background fetch and returns
    /// an empty vector. Subsequent calls will use cached results after the fetch completes.
    fn list_values_for_provider(&self, provider_id: &str) -> Vec<String> {
        if provider_id.is_empty() {
            return Vec::new();
        }

        // Check cache first
        let now = Instant::now();
        {
            let cache = self.cache.lock().expect("Cache lock poisoned");
            if let Some(entry) = cache.get(provider_id) {
                if now.duration_since(entry.fetched_at) < self.ttl {
                    return entry.items.clone();
                }
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
        let path = spec.path.clone();

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
        std::thread::spawn(move || {
            let result = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt.block_on(async {
                    crate::cmd::fetch_json_array(&path)
                        .await
                        .map(|values| {
                            values
                                .into_iter()
                                .filter_map(label_from_value)
                                .collect::<Vec<String>>()
                        })
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
    fn suggest(&self, command_key: &str, field: &str, partial: &str) -> Vec<SuggestionItem> {
        let (group, name) = match command_key.split_once(':') {
            Some((g, n)) if !g.is_empty() && !n.is_empty() => (g, n),
            _ => return Vec::new(),
        };

        let Some(provider_id) = self.provider_for_field(group, name, field) else {
            return Vec::new();
        };

        let values = self.list_values_for_provider(&provider_id);
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
            .and_then(|v| v.as_str().map(str::to_string))
            .or_else(|| {
                map.into_iter()
                    .find_map(|(_, v)| v.as_str().map(str::to_string))
            }),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use heroku_registry::{Registry};
    use std::time::Duration;

    #[test]
    fn test_suggest_with_valid_provider() {
        let registry = Registry::from_embedded_schema().expect("load registry from manifest");
        let provider = RegistryBackedProvider::new(Arc::new(registry), Duration::from_secs(60));

        // Mock fetch_json_array to return sample data
        let suggestions = provider.suggest("apps:list", "app", "ap");
        assert!(!suggestions.is_empty(), "Should return suggestions");
        assert!(suggestions.iter().all(|s| s.score > 0), "Suggestions should have positive scores");
        assert!(suggestions.iter().all(|s| s.kind == ItemKind::Value), "Suggestions should be Value kind");
    }

    #[test]
    fn test_no_duplicate_fetches() {
        let registry = Registry::from_embedded_schema().expect("load registry from manifest");
        let provider = RegistryBackedProvider::new(Arc::new(registry), Duration::from_secs(60));

        // Simulate concurrent calls to list_values_for_provider
        let provider_id = "apps:list";
        let threads: Vec<_> = (0..3)
            .map(|_| {
                let provider = provider.clone();
                let provider_id = provider_id.to_string();
                std::thread::spawn(move || provider.list_values_for_provider(&provider_id))
            })
            .collect();

        for t in threads {
            let _ = t.join().expect("Thread panicked");
        }

        let active = provider.active_fetches.lock().expect("Lock poisoned");
        assert!(active.is_empty(), "No active fetches should remain");
    }

    #[test]
    fn test_label_from_value() {
        let tests = vec![
            (serde_json::json!("simple"), Some("simple".to_string())),
            (serde_json::json!({"name": "app1"}), Some("app1".to_string())),
            (serde_json::json!({"id": "123"}), Some("123".to_string())),
            (serde_json::json!({"other": "val", "str": "fallback"}), Some("fallback".to_string())),
            (serde_json::json!({"num": 42}), None),
            (serde_json::json!([]), None),
        ];

        for (value, expected) in tests {
            assert_eq!(label_from_value(value.clone()), expected, "Failed for value: {:?}", &value);
        }
    }
}