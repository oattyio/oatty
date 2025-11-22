//! Value provider trait shared across the engine and TUI.
//!
//! This module defines the contract for dynamic value providers that can be
//! plugged into the palette as well as workflow runners. Implementations are
//! responsible for sourcing context-aware suggestions for command inputs.

use std::{collections::HashMap, fmt::Debug};

use heroku_types::{CommandSpec, SuggestionItem};
use serde_json::{Map as JsonMap, Value};

/// Specification describing a provider fetch that must be performed before
/// suggestions can be returned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderFetchPlan {
    /// Canonical provider identifier (`group name`).
    pub provider_id: String,
    /// Cache key derived from provider identifier + arguments.
    pub cache_key: String,
    /// Arguments collected from user inputs or bindings.
    pub args: JsonMap<String, Value>,
}

impl ProviderFetchPlan {
    /// Helper to construct a new fetch plan.
    pub fn new(provider_id: String, cache_key: String, args: JsonMap<String, Value>) -> Self {
        Self {
            provider_id,
            cache_key,
            args,
        }
    }
}

/// Pending fetch metadata returned by a provider suggestion call.
#[derive(Clone, Debug)]
pub struct PendingProviderFetch {
    /// Fetch plan describing provider identifier, cache key, and arguments.
    pub plan: ProviderFetchPlan,
    /// Whether the caller should dispatch a new fetch (false when another request is already in flight).
    pub should_dispatch: bool,
}

impl PendingProviderFetch {
    /// Create a new pending fetch record.
    pub fn new(plan: ProviderFetchPlan, should_dispatch: bool) -> Self {
        Self { plan, should_dispatch }
    }
}

/// Result of invoking a value provider.
#[derive(Clone, Debug, Default)]
pub struct ProviderSuggestionSet {
    /// Suggestions immediately available for display.
    pub items: Vec<SuggestionItem>,
    /// Optional fetch that must complete before additional items appear.
    pub pending_fetch: Option<PendingProviderFetch>,
}

impl ProviderSuggestionSet {
    /// Construct a suggestion set containing only ready items.
    pub fn ready(items: Vec<SuggestionItem>) -> Self {
        Self {
            items,
            pending_fetch: None,
        }
    }

    /// Construct a suggestion set with a pending fetch and any existing items.
    pub fn with_pending(items: Vec<SuggestionItem>, fetch: PendingProviderFetch) -> Self {
        Self {
            items,
            pending_fetch: Some(fetch),
        }
    }
}

/// Trait describing a dynamic value provider for command flags and positionals.
///
/// Implementors return context-aware [`SuggestionItem`] entries based on the
/// current command, field, and partially typed user input.
pub trait ValueProvider: Send + Sync + Debug {
    /// Produce suggestions for a given (command, field) tuple and partial input.
    ///
    /// * `commands` — complete registry of CLI commands used for lookup.
    /// * `command_key` — canonical `"group name"` identifier for the active command.
    /// * `field` — flag or positional argument being completed.
    /// * `partial` — user-entered prefix to match against.
    /// * `inputs` — resolved values for other inputs, used to satisfy provider bindings.
    fn suggest(
        &self,
        commands: &[CommandSpec],
        command_key: &str,
        field: &str,
        partial: &str,
        inputs: &HashMap<String, String>,
    ) -> ProviderSuggestionSet;
}

/// Best-effort conversion from a provider JSON payload into a display label.
pub fn label_from_value(value: Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s),
        Value::Object(map) => map
            .get("name")
            .or_else(|| map.get("id"))
            .or_else(|| map.get("str"))
            .and_then(|v| v.as_str().map(str::to_string))
            .or_else(|| map.into_iter().find_map(|(_, v)| v.as_str().map(str::to_string))),
        _ => None,
    }
}
