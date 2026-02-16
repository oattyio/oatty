use crate::ProviderValueResolver;
use anyhow::{Result, anyhow};
use oatty_registry::{CommandRegistry, CommandSpec};
use oatty_types::ProviderContract;
use serde_json::{Map as JsonMap, Value};
use std::{
    collections::{HashMap, HashSet},
    fmt,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tracing::{debug, info, warn};

use super::{
    PendingProviderFetch, ProviderFetchPlan, ProviderSuggestionSet, ValueProvider,
    contract_store::ProviderContractStore,
    fetch::ProviderValueFetcher,
    identifier::{ProviderIdentifier, cache_key_for_canonical_identifier, cache_key_for_identifier, canonical_identifier},
    selection::{FieldSelection, infer_selection},
    suggestion_builder::ProviderSuggestionBuilder,
};

#[derive(Debug, Clone)]
struct CacheEntry {
    fetched_at: Instant,
    items: Vec<Value>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProviderCacheStore {
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
    cache_time_to_live: Duration,
    active_fetches: Arc<Mutex<HashSet<String>>>,
}

impl ProviderCacheStore {
    fn new(cache_time_to_live: Duration) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            cache_time_to_live,
            active_fetches: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    fn lookup_fresh(&self, cache_key: &str) -> Option<Vec<Value>> {
        let entry = self.cache.lock().expect("cache lock").get(cache_key).cloned()?;
        if entry.fetched_at.elapsed() < self.cache_time_to_live {
            Some(entry.items)
        } else {
            None
        }
    }

    fn store_items(&self, cache_key: String, items: Vec<Value>) {
        self.cache.lock().expect("cache lock").insert(
            cache_key,
            CacheEntry {
                fetched_at: Instant::now(),
                items,
            },
        );
    }

    fn try_begin_fetch(&self, key: &str) -> bool {
        let mut active = self.active_fetches.lock().expect("active fetches lock");
        if active.contains(key) {
            return false;
        }
        active.insert(key.to_string());
        true
    }

    fn finish_fetch(&self, key: &str) {
        self.active_fetches.lock().expect("active fetches lock").remove(key);
    }
}

#[derive(Debug)]
pub enum CacheLookupOutcome {
    Hit(Vec<Value>),
    Pending(PendingProviderFetch),
}

fn fetch_and_cache(
    registry: Arc<Mutex<CommandRegistry>>,
    fetcher: Arc<dyn ProviderValueFetcher>,
    cache_store: &ProviderCacheStore,
    provider_id: String,
    args: JsonMap<String, Value>,
    cache_key: String,
) -> Result<Vec<Value>> {
    debug!(
        provider_id = %provider_id,
        argument_count = args.len(),
        "provider fetch started"
    );
    let identifier = ProviderIdentifier::parse(&provider_id).ok_or_else(|| anyhow!("invalid provider identifier: {}", provider_id))?;

    let (spec, base_url, headers) = {
        let registry_lock = registry.lock().map_err(|error| anyhow!(error.to_string()))?;
        let spec = registry_lock
            .find_by_group_and_cmd_cloned(&identifier.group, &identifier.name)?
            .clone();
        let base_url = registry_lock
            .resolve_base_url_for_command(&spec)
            .ok_or_else(|| anyhow!("missing base URL for command '{}'", spec.name))?;
        let headers = registry_lock
            .resolve_headers_for_command(&spec)
            .ok_or_else(|| anyhow!("could not determine headers for command: {}", &spec.canonical_id()))?
            .clone();
        debug!(
            provider_id = %provider_id,
            command = %spec.canonical_id(),
            base_url = %base_url,
            header_count = headers.len(),
            "provider fetch resolved command settings"
        );
        (spec, base_url, headers)
    };

    let items = fetcher
        .fetch_list(spec, &args, base_url.as_str(), &headers)
        .map_err(|error| anyhow!("provider '{}' fetch error: {}", provider_id, error))?;
    info!(
        provider_id = %provider_id,
        item_count = items.len(),
        "provider fetch completed"
    );
    cache_store.store_items(cache_key, items.clone());
    Ok(items)
}
/// A structure representing the ProviderRegistry, responsible for managing data providers, handling caching,
/// and maintaining runtime state for fetching values and storing user choices.
///
/// ## Fields
///
/// - `registry`: A thread-safe (using `Arc` and `Mutex`) instance of `CommandRegistry` which serves as the
///   central registry for commands.
///
/// - `fetcher`: A thread-safe (`Arc`) trait object (`dyn ProviderValueFetcher`) used to fetch provider data. This
///   enables polymorphism, allowing different types of fetchers to be utilized at runtime.
///
/// - `cache_store`: A cache coordinator that manages cached values, time-to-live tracking, and
///   in-flight fetch bookkeeping to avoid duplicate provider requests.
///
/// - `contract_store`: A contract resolver that looks up provider schemas and applies
///   default contract fallbacks when none are registered.
///
/// - `choices`: A thread-safe (`Arc` and `Mutex`) `HashMap` that stores user choices for field selection.
///   The `String` keys represent the choice context, and `FieldSelection` defines the user's persisted selections.
///
/// ## Functional Overview
///
/// The `ProviderRegistry` serves as the central coordinating entity for managing provider operations,
/// including value fetching, caching, resolving user preferences, and controlling concurrency with active fetches.
///
/// - **Concurrency Management:** Built with thread-safe primitives (`Arc`, `Mutex`, etc.), allowing concurrent
///   access and modification of critical fields like the `registry`, `cache_store`, and `choices`.
///
/// - **Caching:** Implements a caching mechanism with configurable time-to-live to optimize
///   fetching operations and reduce redundant computation or retrieval overhead.
///
/// - **User Choices Persistence:** Tracks and persists user preferences through the `choices` field.
///
/// ## Usage
///
/// Instantiate and manage the `ProviderRegistry` to handle providers and their data within a multi-threaded
/// or asynchronous environment, leveraging caching and runtime facilities to ensure high performance and consistency.
pub struct ProviderRegistry {
    pub(crate) registry: Arc<Mutex<CommandRegistry>>,
    pub(crate) fetcher: Arc<dyn ProviderValueFetcher>,
    pub(crate) cache_store: ProviderCacheStore,
    pub(crate) contract_store: ProviderContractStore,
    choices: Arc<Mutex<HashMap<String, FieldSelection>>>, // persisted user choices
}

impl fmt::Debug for ProviderRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderRegistry")
            .field("cache_ttl", &self.cache_store.cache_time_to_live)
            .finish()
    }
}

impl ProviderRegistry {
    pub fn new(registry: Arc<Mutex<CommandRegistry>>, fetcher: Box<dyn ProviderValueFetcher>, cache_ttl: Duration) -> Result<Self> {
        Ok(Self {
            contract_store: ProviderContractStore::new(Arc::clone(&registry)),
            registry,
            fetcher: Arc::from(fetcher),
            cache_store: ProviderCacheStore::new(cache_ttl),
            choices: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn with_default_http(registry: Arc<Mutex<CommandRegistry>>, cache_ttl: Duration) -> Result<Self> {
        Self::new(registry, Box::new(super::fetch::DefaultHttpFetcher), cache_ttl)
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

    pub(crate) fn resolved_selection_for_provider(&self, provider_id: &str) -> FieldSelection {
        if let Some(selection) = self.choice_for(provider_id) {
            return selection;
        }

        let contract = self.contract_store.resolve_contract(provider_id);
        infer_selection(None, contract.as_ref())
    }

    pub fn cached_values_or_plan(&self, provider_id: &str, args: JsonMap<String, Value>) -> CacheLookupOutcome {
        let canonical_id = canonical_identifier(provider_id).unwrap_or_else(|| provider_id.to_string());
        let key = cache_key_for_canonical_identifier(&canonical_id, &args);

        if let Some(items) = self.cache_store.lookup_fresh(&key) {
            debug!(
                provider_id = %canonical_id,
                cache_key = %key,
                item_count = items.len(),
                "provider cache hit"
            );
            return CacheLookupOutcome::Hit(items);
        }

        let should_dispatch = self.cache_store.try_begin_fetch(&key);
        debug!(
            provider_id = %canonical_id,
            cache_key = %key,
            should_dispatch,
            "provider cache miss"
        );
        let plan = ProviderFetchPlan::new(canonical_id, key.clone(), args);
        let pending = PendingProviderFetch::new(plan, should_dispatch);
        CacheLookupOutcome::Pending(pending)
    }

    pub fn complete_fetch(&self, plan: &ProviderFetchPlan) -> Result<Vec<Value>> {
        debug!(
            provider_id = %plan.provider_id,
            cache_key = %plan.cache_key,
            "provider fetch dispatch"
        );
        let result = fetch_and_cache(
            Arc::clone(&self.registry),
            Arc::clone(&self.fetcher),
            &self.cache_store,
            plan.provider_id.clone(),
            plan.args.clone(),
            plan.cache_key.clone(),
        );
        self.cache_store.finish_fetch(&plan.cache_key);
        if let Err(ref error) = result {
            warn!("provider fetch failed: {}", error);
        }
        result
    }
}

impl ProviderValueResolver for ProviderRegistry {
    /// Fetches a list of values associated with a specified `provider_id` and provided arguments.
    ///
    /// This function first checks if the requested data is available in a cache and if the cached
    /// data is still valid (based on the configured cache TTL). If valid cached data is found, it is
    /// returned directly. Otherwise, it fetches the data either through an HTTP command or
    /// a custom fetcher defined for the provider.
    ///
    /// ### Arguments
    ///
    /// * `&self` - A reference to the current instance of the object containing the cache and other
    ///   necessary components.
    /// * `provider_id: &str` - A string identifier that represents the provider. It must follow a
    ///   format that includes a group and a command, separated by whitespace.
    /// * `args: &JsonMap<String, Value>` - A map of arguments to be used while resolving the request.
    ///   Passed to the provider during HTTP execution.
    ///
    /// ### Returns
    ///
    /// A `Result` object is returned:
    /// - `Ok(Vec<Value>)` - A vector of `Value` objects fetched from the provider or cache.
    /// - `Err(anyhow::Error)` - If an error occurs during provider resolution, execution, or unexpected
    ///   scenarios (e.g., invalid identifiers, unsupported providers, etc.).
    ///
    /// ### Errors
    ///
    /// - Returns an error if the `provider_id` is invalid (does not contain both group and command).
    /// - Returns an error if the provider is not backed by an HTTP command.
    /// - Returns an error if fetching the required data fails (e.g., invalid registry, network errors, etc.).
    ///
    /// ### Implementation Details
    ///
    /// 1. **Caching**:
    ///    - Generates a `cache_key` using `provider_id` and `args`.
    ///    - Checks if there is a valid cache entry. If the entry exists and hasn't expired
    ///      (based on the configured cache time-to-live), the cached data is returned directly.
    ///
    /// 2. **Provider Resolution**:
    ///    - Splits and validates the `provider_id` to determine the group and command.
    ///    - Resolves the provider's execution specification through the `self.registry`.
    ///
    /// 3. **HTTP Command Execution**:
    ///    - If the provider is backed by an HTTP command (checked using the `spec_ref` object):
    ///        - Resolves HTTP path placeholders with the provided arguments.
    ///        - Executes the HTTP command using `oatty_util::http_exec::exec_remote`.
    ///        - If successful, caches the result and returns the obtained list of values.
    ///
    /// 4. **Fallback to Fetcher**:
    ///    - If HTTP execution is not supported or fails, it attempts to resolve the provider and
    ///      fetches the list of values using the `self.fetcher` fallback mechanism.
    ///    - The fetched values are stored in the cache and returned.
    ///
    /// ### Example Usage
    ///
    /// ```rust,ignore
    /// let provider_id = "group_name command_name";
    /// let args = JsonMap::new();
    /// let result = my_instance.fetch_values(provider_id, &args);
    ///
    /// match result {
    ///     Ok(values) => {
    ///         for value in values {
    ///             println!("Fetched Value: {:?}", value);
    ///         }
    ///     }
    ///     Err(err) => eprintln!("Error fetching values: {}", err),
    /// }
    /// ```
    ///
    /// ### Notes
    ///
    /// - The function ensures thread-safe access by locking both the cache and the registry when needed.
    /// - The presence of an HTTP command is mandatory for successful execution.
    /// - Provider identifiers must be properly formatted, or an error will be returned.
    fn fetch_values(&self, provider_id: &str, args: &JsonMap<String, Value>) -> Result<Vec<Value>> {
        let key = cache_key_for_identifier(provider_id, args);
        if let Some(items) = self.cache_store.lookup_fresh(&key) {
            debug!(
                provider_id = %provider_id,
                cache_key = %key,
                item_count = items.len(),
                "provider cache hit"
            );
            return Ok(items);
        }

        debug!(
            provider_id = %provider_id,
            cache_key = %key,
            "provider cache miss"
        );
        fetch_and_cache(
            Arc::clone(&self.registry),
            Arc::clone(&self.fetcher),
            &self.cache_store,
            canonical_identifier(provider_id).unwrap_or_else(|| provider_id.to_string()),
            args.clone(),
            key,
        )
    }

    fn get_contract(&self, provider_id: &str) -> Option<ProviderContract> {
        self.contract_store.resolve_contract(provider_id)
    }
}

impl ValueProvider for ProviderRegistry {
    fn suggest(
        &self,
        commands: &[CommandSpec],
        command_key: &str,
        field: &str,
        partial: &str,
        inputs: &HashMap<String, String>,
    ) -> ProviderSuggestionSet {
        ProviderSuggestionBuilder::build_suggestions(self, commands, command_key, field, partial, inputs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oatty_types::{CommandExecution, ProviderFieldContract, ProviderReturnContract};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    fn build_registry_with_apps_list() -> CommandRegistry {
        let command_spec = CommandSpec {
            group: "apps".into(),
            name: "list".into(),
            catalog_identifier: 0,
            summary: "List applications".into(),
            positional_args: Vec::new(),
            flags: Vec::new(),
            execution: CommandExecution::default(),
        };
        let mut registry = CommandRegistry::default().with_commands(vec![command_spec]);
        registry.provider_contracts.insert(
            "apps list".into(),
            ProviderContract {
                arguments: Vec::new(),
                returns: ProviderReturnContract {
                    fields: vec![
                        ProviderFieldContract {
                            name: "id".into(),
                            r#type: Some("string".into()),
                            tags: vec!["id".into(), "identifier".into()],
                        },
                        ProviderFieldContract {
                            name: "name".into(),
                            r#type: Some("string".into()),
                            tags: vec!["display".into(), "name".into()],
                        },
                    ],
                },
            },
        );
        registry
    }

    fn build_registry_with_apps_list_without_contract() -> CommandRegistry {
        let command_spec = CommandSpec {
            group: "apps".into(),
            name: "list".into(),
            catalog_identifier: 0,
            summary: "List applications".into(),
            positional_args: Vec::new(),
            flags: Vec::new(),
            execution: CommandExecution::default(),
        };
        CommandRegistry::default().with_commands(vec![command_spec])
    }

    #[test]
    fn get_contract_accepts_space_form_and_rejects_colon_form() {
        let registry = Arc::new(Mutex::new(build_registry_with_apps_list()));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        let _guard = runtime.enter();
        let provider = ProviderRegistry::with_default_http(Arc::clone(&registry), Duration::from_secs(1)).expect("provider");

        // Known command present in the manifest should resolve via space-separated identifier.
        let known_contract = provider.get_contract("apps list");
        assert!(known_contract.is_some(), "expected provider contract for 'apps list'");

        // Legacy colon-separated form must be rejected now.
        let colon_form_contract = provider.get_contract("apps:list");
        assert!(colon_form_contract.is_none(), "colon form should not be accepted");
    }

    #[test]
    fn get_contract_returns_none_for_unknown_command() {
        let registry = Arc::new(Mutex::new(CommandRegistry::default()));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        let _guard = runtime.enter();
        let provider = ProviderRegistry::with_default_http(Arc::clone(&registry), Duration::from_secs(1)).expect("provider");

        let contract = provider.get_contract("apps list");
        assert!(contract.is_none(), "expected no contract for unknown command");
    }

    #[test]
    fn get_contract_falls_back_for_known_command_without_contract() {
        let registry = Arc::new(Mutex::new(build_registry_with_apps_list_without_contract()));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        let _guard = runtime.enter();
        let provider = ProviderRegistry::with_default_http(Arc::clone(&registry), Duration::from_secs(1)).expect("provider");

        let contract = provider.get_contract("apps list");
        assert!(contract.is_some(), "expected default contract for known command");
    }

    #[test]
    fn parse_identifier_parses_whitespace_form() {
        let parsed = ProviderIdentifier::parse("apps list").expect("identifier");
        assert_eq!(parsed.group, "apps");
        assert_eq!(parsed.name, "list");
    }

    #[test]
    fn parse_identifier_rejects_invalid_forms() {
        assert!(ProviderIdentifier::parse("apps").is_none());
        assert!(ProviderIdentifier::parse("apps:list").is_none());
        assert!(ProviderIdentifier::parse("  ").is_none());
    }

    #[test]
    fn cache_key_normalizes_whitespace() {
        let args = JsonMap::new();
        let first_key = cache_key_for_identifier("apps list", &args);
        let second_key = cache_key_for_identifier("apps   list", &args);
        assert_eq!(first_key, second_key);
    }
}
