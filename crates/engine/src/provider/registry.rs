use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::Mutex,
    time::{Duration, Instant},
};

use heroku_registry::{CommandSpec, Registry};
use serde_json::{Map as JsonMap, Value};

use super::{
    contract::{ProviderContract, ProviderReturns, ReturnField},
    fetch::ProviderValueFetcher,
    selection::FieldSelection,
};

#[derive(Debug, Clone)]
struct CacheEntry {
    fetched_at: Instant,
    items: Vec<Value>,
}

fn cache_key(provider_id: &str, args: &JsonMap<String, Value>) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    provider_id.hash(&mut hasher);
    if let Ok(s) = serde_json::to_string(args) {
        s.hash(&mut hasher);
    }
    format!("{}:{}", provider_id, hasher.finish())
}

pub struct RegistryProvider {
    pub(crate) registry: Registry,
    pub(crate) fetcher: Box<dyn ProviderValueFetcher>,
    pub(crate) cache_ttl: Duration,
    cache: Mutex<HashMap<String, CacheEntry>>,
    choices: Mutex<HashMap<String, FieldSelection>>, // persisted user choices
}

impl RegistryProvider {
    pub fn new(registry: Registry, fetcher: Box<dyn ProviderValueFetcher>, cache_ttl: Duration) -> Self {
        Self {
            registry,
            fetcher,
            cache_ttl,
            cache: Mutex::new(HashMap::new()),
            choices: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_default_http(registry: Registry, cache_ttl: Duration) -> Self {
        Self::new(registry, Box::new(super::fetch::DefaultHttpFetcher), cache_ttl)
    }

    fn resolve_spec(&self, provider_id: &str) -> Option<&CommandSpec> {
        let (group, name) = provider_id.split_once(':')?;
        self.registry.find_by_group_and_cmd(group, name).ok()
    }

    pub fn persist_choice(&self, provider_id: &str, selection: FieldSelection) {
        self.choices
            .lock()
            .expect("choices lock")
            .insert(provider_id.to_string(), selection);
    }
    pub fn choice_for(&self, provider_id: &str) -> Option<FieldSelection> {
        self.choices.lock().expect("choices lock").get(provider_id).cloned()
    }
}

impl crate::provider::ProviderRegistry for RegistryProvider {
    fn fetch_values(&self, provider_id: &str, args: &JsonMap<String, Value>) -> anyhow::Result<Vec<Value>> {
        let key = cache_key(provider_id, args);
        if let Some(entry) = self.cache.lock().expect("cache lock").get(&key).cloned()
            && entry.fetched_at.elapsed() < self.cache_ttl
        {
            return Ok(entry.items);
        }

        if let Some((group, name)) = provider_id.split_once(':')
            && let Ok(spec_ref) = self.registry.find_by_group_and_cmd(group, name)
        {
            let body = args.clone();
            // Resolve path placeholders before exec
            let mut spec_clone = spec_ref.clone();
            if !args.is_empty() {
                spec_clone.path = heroku_util::http_path_resolution::build_path(&spec_clone.path, &body);
            }
            let result = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt.block_on(async move { heroku_util::http_exec::exec_remote(&spec_clone, body).await }),
                Err(e) => Err(format!("runtime init failed: {}", e)),
            };
            if let Ok(outcome) = result
                && let Some(Value::Array(arr)) = outcome.result_json
            {
                let items = arr;
                self.cache.lock().expect("cache lock").insert(
                    key,
                    CacheEntry {
                        fetched_at: Instant::now(),
                        items: items.clone(),
                    },
                );
                return Ok(items);
            }
        }

        let resolved_spec = self
            .resolve_spec(provider_id)
            .ok_or_else(|| anyhow::anyhow!("unknown provider: {}", provider_id))?;
        let items = self.fetcher.fetch_list(resolved_spec, args)?;
        self.cache.lock().expect("cache lock").insert(
            key,
            CacheEntry {
                fetched_at: Instant::now(),
                items: items.clone(),
            },
        );
        Ok(items)
    }

    fn get_contract(&self, provider_id: &str) -> Option<ProviderContract> {
        self.resolve_spec(provider_id).map(|_| ProviderContract {
            args: JsonMap::new(),
            returns: ProviderReturns {
                fields: vec![
                    ReturnField {
                        name: "id".into(),
                        r#type: Some("string".into()),
                        tags: vec!["id".into(), "identifier".into()],
                    },
                    ReturnField {
                        name: "name".into(),
                        r#type: Some("string".into()),
                        tags: vec!["display".into(), "name".into()],
                    },
                ],
            },
        })
    }
}
