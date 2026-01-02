use crate::ProviderValueResolver;
use anyhow::{Result, anyhow};
use oatty_registry::{CommandRegistry, CommandSpec, find_by_group_and_cmd};
use oatty_types::{Bind, ExecOutcome, ItemKind, SuggestionItem, ValueProvider as ProviderBinding};
use oatty_util::{exec_remote_for_provider, fuzzy_score};
use serde_json::{Map as JsonMap, Value};
use std::hash::DefaultHasher;
use std::{
    collections::{HashMap, HashSet},
    fmt,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::runtime::Handle;
use tracing::warn;

use super::{
    PendingProviderFetch, ProviderFetchPlan, ProviderSuggestionSet, ValueProvider,
    contract::{ProviderContract, ProviderReturns, ReturnField},
    fetch::ProviderValueFetcher,
    label_from_value,
    selection::FieldSelection,
};

#[derive(Debug, Clone)]
struct CacheEntry {
    fetched_at: Instant,
    items: Vec<Value>,
}

#[derive(Debug)]
pub enum CacheLookupOutcome {
    Hit(Vec<Value>),
    Pending(PendingProviderFetch),
}

fn cache_key(provider_id: &str, args: &JsonMap<String, Value>) -> String {
    let mut hasher = DefaultHasher::new();
    canonical_identifier(provider_id)
        .unwrap_or_else(|| provider_id.to_string())
        .hash(&mut hasher);
    if let Ok(s) = serde_json::to_string(args) {
        s.hash(&mut hasher);
    }
    format!("{}:{}", provider_id, hasher.finish())
}

fn split_identifier(provider_id: &str) -> Option<(String, String)> {
    if let Some((group, name)) = provider_id.split_once(char::is_whitespace) {
        let group = group.trim();
        let name = name.trim();
        if !group.is_empty() && !name.is_empty() {
            return Some((group.to_string(), name.to_string()));
        }
    }
    if provider_id.contains(':') {
        warn!(
            "Colon-delimited provider identifiers are no longer supported: '{}'. Use the '<group> <name>' format instead.",
            provider_id
        );
    }
    None
}

fn canonical_identifier(provider_id: &str) -> Option<String> {
    split_identifier(provider_id).map(|(group, name)| format!("{group} {name}"))
}

fn split_command_key(command_key: &str) -> Option<(String, String)> {
    let (group, name) = command_key.split_once(char::is_whitespace)?;
    let group = group.trim();
    let name = name.trim();
    if group.is_empty() || name.is_empty() {
        return None;
    }
    Some((group.to_string(), name.to_string()))
}

fn binding_for_field(spec: &CommandSpec, field: &str) -> Option<(String, Vec<Bind>)> {
    if let Some(flag) = spec.flags.iter().find(|flag| flag.name == field)
        && let Some(ProviderBinding::Command { command_id, binds }) = &flag.provider
    {
        return Some((command_id.clone(), binds.clone()));
    }
    if let Some(positional) = spec.positional_args.iter().find(|arg| arg.name == field)
        && let Some(ProviderBinding::Command { command_id, binds }) = &positional.provider
    {
        return Some((command_id.clone(), binds.clone()));
    }
    None
}

fn fetch_and_cache(
    registry: Arc<Mutex<CommandRegistry>>,
    fetcher: Arc<dyn ProviderValueFetcher>,
    handle: Handle,
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
    provider_id: String,
    args: JsonMap<String, Value>,
    cache_key: String,
) -> Result<Vec<Value>> {
    let (group, name) = split_identifier(&provider_id).ok_or_else(|| anyhow!("invalid provider identifier: {}", provider_id))?;

    let (spec, base_url) = {
        let registry_lock = registry.lock().map_err(|error| anyhow!(error.to_string()))?;
        let spec = find_by_group_and_cmd(&registry_lock.commands, &group, &name)?.clone();
        let base_url = registry_lock
            .resolve_base_url_for_command(&spec)
            .or_else(|| spec.http().map(|http| http.base_url.clone()));
        (spec, base_url)
    };

    let spec_for_http = spec.clone();
    if spec_for_http.http().is_some() {
        let body = args.clone();
        let result = handle.block_on(async move { exec_remote_for_provider(&spec_for_http, base_url, body, 0).await });

        if let Ok(ExecOutcome::Http { payload: result_value, .. }) = result
            && let Some(items) = result_value.as_array()
        {
            let items = items.clone();
            cache.lock().expect("cache lock").insert(
                cache_key.clone(),
                CacheEntry {
                    fetched_at: Instant::now(),
                    items: items.clone(),
                },
            );
            return Ok(items);
        }
    }

    let items = fetcher
        .fetch_list(spec, &args)
        .map_err(|error| anyhow!("provider '{}' fetch error: {}", provider_id, error))?;
    cache.lock().expect("cache lock").insert(
        cache_key,
        CacheEntry {
            fetched_at: Instant::now(),
            items: items.clone(),
        },
    );
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
/// - `cache_ttl`: A `Duration` specifying the time-to-live for cached data. Cached data is invalidated after
///   this duration elapses.
///
/// - `cache`: A thread-safe (`Arc` and `Mutex`) `HashMap` used for caching values fetched by providers.
///   The `String` keys represent the identifiers of the cached values, with `CacheEntry` representing
///   the actual cached data and its metadata.
///
/// - `choices`: A thread-safe (`Arc` and `Mutex`) `HashMap` that stores user choices for field selection.
///   The `String` keys represent the choice context, and `FieldSelection` defines the user's persisted selections.
///
/// - `active_fetches`: A thread-safe (`Arc` and `Mutex`) `HashSet` used to track the set of providers
///   or items currently being fetched. The `String` keys represent identifiers of ongoing fetch operations,
///   preventing duplicate fetches for the same identifier.
///
/// - `runtime`: A thread-safe (`Arc`) instance of a `Runtime` that manages asynchronous tasks and operations
///   needed within the `ProviderRegistry`.
///
/// ## Functional Overview
///
/// The `ProviderRegistry` serves as the central coordinating entity for managing provider operations,
/// including value fetching, caching, resolving user preferences, and controlling concurrency with active fetches.
///
/// - **Concurrency Management:** Built with thread-safe primitives (`Arc`, `Mutex`, etc.), allowing concurrent
///   access and modification of critical fields like the `registry`, `cache`, `choices`, and `active_fetches`.
///
/// - **Caching:** Implements a caching mechanism with configurable time-to-live (`cache_ttl`) to optimize
///   fetching operations and reduce redundant computation or retrieval overhead.
///
/// - **User Choices Persistence:** Tracks and persists user preferences through the `choices` field.
///
/// - **Runtime Support:** Utilizes an asynchronous runtime (`Runtime`) to manage asynchronous tasks and provide
///   seamless, non-blocking operations when interacting with providers.
///
/// ## Usage
///
/// Instantiate and manage the `ProviderRegistry` to handle providers and their data within a multi-threaded
/// or asynchronous environment, leveraging caching and runtime facilities to ensure high performance and consistency.
pub struct ProviderRegistry {
    pub(crate) registry: Arc<Mutex<CommandRegistry>>,
    pub(crate) fetcher: Arc<dyn ProviderValueFetcher>,
    pub(crate) cache_ttl: Duration,
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
    choices: Arc<Mutex<HashMap<String, FieldSelection>>>, // persisted user choices
    active_fetches: Arc<Mutex<HashSet<String>>>,
    handle: Handle,
}

impl fmt::Debug for ProviderRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderRegistry").field("cache_ttl", &self.cache_ttl).finish()
    }
}

impl ProviderRegistry {
    pub fn new(registry: Arc<Mutex<CommandRegistry>>, fetcher: Box<dyn ProviderValueFetcher>, cache_ttl: Duration) -> Result<Self> {
        // Expect to be called within an existing Tokio runtime; capture its handle.
        let handle = Handle::try_current().map_err(|e| anyhow!("ProviderRegistry must be created within a Tokio runtime: {e}"))?;

        Ok(Self {
            registry,
            fetcher: Arc::from(fetcher),
            cache_ttl,
            cache: Arc::new(Mutex::new(HashMap::new())),
            choices: Arc::new(Mutex::new(HashMap::new())),
            active_fetches: Arc::new(Mutex::new(HashSet::new())),
            handle,
        })
    }

    pub fn with_default_http(registry: Arc<Mutex<CommandRegistry>>, cache_ttl: Duration) -> Result<Self> {
        Self::new(registry, Box::new(super::fetch::DefaultHttpFetcher), cache_ttl)
    }

    /// Resolves and retrieves a `CommandSpec` based on a given `provider_id`.
    ///
    /// # Arguments
    ///
    /// * `provider_id` - A string slice that identifies the command, formatted as a canonical
    ///   whitespace-separated form `"group name"`. The `group` and `name` must both be
    ///   non-empty after trimming whitespace.
    ///
    /// # Returns
    ///
    /// * `Option<CommandSpec>` - Returns `Some(CommandSpec)` if the `provider_id` is valid and
    ///   the command specification is found in the registry. Returns `None` if:
    ///   - The `provider_id` is not in the required `"group name"` format.
    ///   - Either `group` or `name` is empty after trimming.
    ///   - The registry is unavailable (e.g., a lock on it cannot be obtained).
    ///   - The specified command cannot be found in the registry.
    ///
    /// # Behavior
    ///
    /// This function performs the following steps:
    /// 1. Attempts to split `provider_id` into two parts, `group` and `name`, by the first
    ///    whitespace character.
    /// 2. Trims any surrounding whitespace from both parts.
    /// 3. Returns `None` if the format is invalid, or if either `group` or `name` are empty.
    /// 4. Acquires a lock on the `registry`.
    /// 5. Searches the `registry` for a command matching the provided `group` and `name`.
    ///
    /// If any of these steps fail, the function will return `None`. If the command is found,
    /// it is returned as a `CommandSpec` inside a `Some`.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let provider_id = "utilities compile";
    /// let command_spec = object.resolve_spec(provider_id);
    ///
    /// match command_spec {
    ///     Some(spec) => println!("Command found: {:?}", spec),
    ///     None => println!("Command not found or invalid provider_id"),
    /// }
    /// ```
    ///
    /// # Notes
    ///
    /// This function assumes that the `registry` has a structure that can be accessed with
    /// a lock and includes a `commands` collection that supports `find_by_group_and_cmd`.
    fn resolve_spec(&self, provider_id: &str) -> Option<CommandSpec> {
        let (group, name) = split_identifier(provider_id)?;
        self.registry
            .lock()
            .ok()
            .and_then(|lock| find_by_group_and_cmd(&lock.commands, &group, &name).ok())
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

    pub fn cached_values_or_plan(&self, provider_id: &str, args: JsonMap<String, Value>) -> CacheLookupOutcome {
        let canonical_id = canonical_identifier(provider_id).unwrap_or_else(|| provider_id.to_string());
        let key = cache_key(&canonical_id, &args);

        if let Some(entry) = self.cache.lock().expect("cache lock").get(&key).cloned()
            && entry.fetched_at.elapsed() < self.cache_ttl
        {
            return CacheLookupOutcome::Hit(entry.items);
        }

        let should_dispatch = self.try_begin_fetch(&key);
        let plan = ProviderFetchPlan::new(canonical_id, key.clone(), args);
        let pending = PendingProviderFetch::new(plan, should_dispatch);
        CacheLookupOutcome::Pending(pending)
    }

    pub fn complete_fetch(&self, plan: &ProviderFetchPlan) -> Result<Vec<Value>> {
        let result = fetch_and_cache(
            Arc::clone(&self.registry),
            Arc::clone(&self.fetcher),
            self.handle.clone(),
            Arc::clone(&self.cache),
            plan.provider_id.clone(),
            plan.args.clone(),
            plan.cache_key.clone(),
        );
        self.finish_fetch(&plan.cache_key);
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
    ///      (based on `self.cache_ttl`), the cached data is returned directly.
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
        let key = cache_key(provider_id, args);
        if let Some(entry) = self.cache.lock().expect("cache lock").get(&key).cloned()
            && entry.fetched_at.elapsed() < self.cache_ttl
        {
            return Ok(entry.items);
        }

        fetch_and_cache(
            Arc::clone(&self.registry),
            Arc::clone(&self.fetcher),
            self.handle.clone(),
            Arc::clone(&self.cache),
            canonical_identifier(provider_id).unwrap_or_else(|| provider_id.to_string()),
            args.clone(),
            key,
        )
    }

    fn get_contract(&self, provider_id: &str) -> Option<ProviderContract> {
        // Only accept canonical space-separated identifiers. If parsing fails, return None.
        let spec = self.resolve_spec(provider_id)?;
        let group = spec.group.clone();
        let name = spec.name.clone();
        let canonical_key = format!("{} {}", group, name);
        let legacy_key = format!("{}:{}", group, name);

        // Prefer canonical space-separated key; fall back to a legacy colon key to accommodate
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

impl ValueProvider for ProviderRegistry {
    fn suggest(
        &self,
        commands: &[CommandSpec],
        command_key: &str,
        field: &str,
        partial: &str,
        inputs: &HashMap<String, String>,
    ) -> ProviderSuggestionSet {
        let (group, name) = match split_command_key(command_key) {
            Some(parts) => parts,
            None => return ProviderSuggestionSet::default(),
        };

        let spec = match commands.iter().find(|command| command.group == group && command.name == name) {
            Some(spec) => spec,
            None => return ProviderSuggestionSet::default(),
        };

        let (provider_id, binds) = match binding_for_field(spec, field) {
            Some(binding) => binding,
            None => return ProviderSuggestionSet::default(),
        };

        let mut arguments = JsonMap::new();
        for binding in &binds {
            if let Some(value) = inputs.get(&binding.from) {
                arguments.insert(binding.provider_key.clone(), Value::String(value.clone()));
            } else {
                // Cannot satisfy provider bindings yet; trigger fetch once values are available.
                return ProviderSuggestionSet::default();
            }
        }

        match self.cached_values_or_plan(&provider_id, arguments) {
            CacheLookupOutcome::Hit(values) => {
                let provider_meta = canonical_identifier(&provider_id).unwrap_or(provider_id.clone());
                let mut items = Vec::with_capacity(values.len());
                for value in values {
                    let Some(label) = label_from_value(value) else {
                        continue;
                    };
                    let Some(score) = fuzzy_score(&label, partial) else {
                        continue;
                    };
                    items.push(SuggestionItem {
                        display: label.clone(),
                        insert_text: label,
                        kind: ItemKind::Value,
                        meta: Some(provider_meta.clone()),
                        score,
                    });
                }
                items.sort_by(|a, b| b.score.cmp(&a.score));
                ProviderSuggestionSet::ready(items)
            }
            CacheLookupOutcome::Pending(pending) => ProviderSuggestionSet::with_pending(Vec::new(), pending),
        }
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
        let registry = Arc::new(Mutex::new(CommandRegistry::from_config().unwrap()));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        let _guard = runtime.enter();
        let provider = ProviderRegistry::with_default_http(Arc::clone(&registry), Duration::from_secs(1)).expect("provider");

        // Known command present in the manifest should resolve via space-separated identifier.
        let ok = provider.get_contract("apps list");
        assert!(ok.is_some(), "expected provider contract for 'apps list'");

        // Legacy colon-separated form must be rejected now.
        let bad = provider.get_contract("apps:list");
        assert!(bad.is_none(), "colon form should not be accepted");
    }
}
