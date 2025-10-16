//! Value provider trait shared across the engine and TUI.
//!
//! This module defines the contract for dynamic value providers that can be
//! plugged into the palette as well as workflow runners. Implementations are
//! responsible for sourcing context-aware suggestions for command inputs.

use std::{collections::HashMap, fmt::Debug};

use heroku_types::{CommandSpec, SuggestionItem};
use serde_json::Value;

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
    ) -> Vec<SuggestionItem>;
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
