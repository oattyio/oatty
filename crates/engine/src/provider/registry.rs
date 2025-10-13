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
use tokio::runtime::Runtime;

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
    runtime: Arc<Runtime>,
}

impl RegistryProvider {
    pub fn new(registry: Arc<Mutex<Registry>>, fetcher: Box<dyn ProviderValueFetcher>, cache_ttl: Duration) -> Result<Self> {
        let runtime = Runtime::new().map_err(|error| anyhow!("failed to create provider runtime: {error}"))?;

        Ok(Self {
            registry,
            fetcher,
            cache_ttl,
            cache: Mutex::new(HashMap::new()),
            choices: Mutex::new(HashMap::new()),
            runtime: Arc::new(runtime),
        })
    }

    pub fn with_default_http(registry: Arc<Mutex<Registry>>, cache_ttl: Duration) -> Result<Self> {
        Self::new(registry, Box::new(super::fetch::DefaultHttpFetcher), cache_ttl)
    }

    fn resolve_spec(&self, provider_id: &str) -> Option<CommandSpec> {
        // Canonical whitespace-separated form only: "group name".
        let (group, name) = match provider_id.split_once(char::is_whitespace) {
            Some((g, n)) => {
                let g = g.trim();
                let n = n.trim();
                if g.is_empty() || n.is_empty() {
                    return None;
                }
                (g, n)
            }
            None => return None,
        };
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

        let (group, name) = provider_id
            .split_once(char::is_whitespace)
            .ok_or(anyhow!(format!("invalid provider identifier: {}", provider_id)))?;

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
        let result = self
            .runtime
            .block_on(async move { heroku_util::http_exec::exec_remote(&spec_clone, body).await });

        if let Ok(ExecOutcome::Http(_, result_value, _, _)) = result
            && let Some(items) = result_value.as_array()
        {
            let items = items.clone();
            self.cache.lock().expect("cache lock").insert(
                key,
                CacheEntry {
                    fetched_at: Instant::now(),
                    items: items.clone(),
                },
            );
            return Ok(items);
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
        // Only accept canonical space-separated identifiers. If parsing fails, return None.
        let spec = self.resolve_spec(provider_id)?;
        let group = spec.group.clone();
        let name = spec.name.clone();
        let canonical_key = format!("{} {}", group, name);
        let legacy_key = format!("{}:{}", group, name);

        // Prefer canonical space-separated key; fall back to legacy colon key to accommodate
        // older embedded manifests that may still store provider contracts under that format.
        if let Some(contract) = self.registry.lock().ok().and_then(|registry| {
            registry
                .provider_contracts
                .get(&canonical_key)
                .or_else(|| registry.provider_contracts.get(&legacy_key))
                .cloned()
        }) {
            return Some(contract);
        }

        // If no explicit contract is found, return a sensible default.
        Some(default_provider_contract())
    }
}

fn default_provider_contract() -> ProviderContract {
    ProviderContract {
        arguments: Vec::new(),
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[test]
    fn get_contract_accepts_space_form_and_rejects_colon_form() {
        // Build a real registry from the embedded schema; no network calls are made in get_contract.
        let registry = Arc::new(Mutex::new(Registry::from_embedded_schema().expect("embedded schema")));
        let provider = RegistryProvider::with_default_http(Arc::clone(&registry), Duration::from_secs(1)).expect("provider");

        // Known command present in the manifest should resolve via space-separated identifier.
        let ok = provider.get_contract("apps list");
        assert!(ok.is_some(), "expected provider contract for 'apps list'");

        // Legacy colon-separated form must be rejected now.
        let bad = provider.get_contract("apps:list");
        assert!(bad.is_none(), "colon form should not be accepted");
    }
}
