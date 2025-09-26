use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow};
use heroku_registry::{CommandSpec, Registry, find_by_group_and_cmd};
use heroku_types::ExecOutcome;
use serde_json::{Map as JsonMap, Value};

use crate::ProviderRegistry;

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
    pub(crate) registry: Arc<Mutex<Registry>>,
    pub(crate) fetcher: Box<dyn ProviderValueFetcher>,
    pub(crate) cache_ttl: Duration,
    cache: Mutex<HashMap<String, CacheEntry>>,
    choices: Mutex<HashMap<String, FieldSelection>>, // persisted user choices
}

impl RegistryProvider {
    pub fn new(registry: Arc<Mutex<Registry>>, fetcher: Box<dyn ProviderValueFetcher>, cache_ttl: Duration) -> Self {
        Self {
            registry,
            fetcher,
            cache_ttl,
            cache: Mutex::new(HashMap::new()),
            choices: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_default_http(registry: Arc<Mutex<Registry>>, cache_ttl: Duration) -> Self {
        Self::new(registry, Box::new(super::fetch::DefaultHttpFetcher), cache_ttl)
    }

    fn resolve_spec(&self, provider_id: &str) -> Option<CommandSpec> {
        let (group, name) = provider_id.split_once(':')?;
        self.registry
            .lock()
            .ok()
            .and_then(|lock| find_by_group_and_cmd(&lock.commands, group, name).ok())
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

impl ProviderRegistry for RegistryProvider {
    fn fetch_values(&self, provider_id: &str, args: &JsonMap<String, Value>) -> Result<Vec<Value>> {
        let key = cache_key(provider_id, args);
        if let Some(entry) = self.cache.lock().expect("cache lock").get(&key).cloned()
            && entry.fetched_at.elapsed() < self.cache_ttl
        {
            return Ok(entry.items);
        }

        let (group, name) = provider_id.split_once(':').ok_or(anyhow!("cannot split {}", provider_id))?;

        let spec_ref = {
            let registry_lock = self.registry.lock().map_err(|e| anyhow!(e.to_string()))?;
            find_by_group_and_cmd(&registry_lock.commands, group, name)?
        };
        let body = args.clone();
        // Resolve path placeholders before exec
        let mut spec_clone = spec_ref.clone();
        if let Some(http_spec) = spec_clone.http_mut() {
            if !args.is_empty() {
                http_spec.path = heroku_util::http_path_resolution::build_path(&http_spec.path, &body);
            }
        } else {
            return Err(anyhow::anyhow!("provider '{}' is not backed by an HTTP command", provider_id));
        }
        let result = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt.block_on(async move { heroku_util::http_exec::exec_remote(&spec_clone, body).await }),
            Err(e) => Err(format!("runtime init failed: {}", e)),
        };

        if let Some(ExecOutcome::Http(_, result, _, _)) = result.ok() {
            if let Some(items) = result.as_array().cloned() {
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
            .ok_or_else(|| anyhow!("unknown provider: {}", provider_id))?;
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
