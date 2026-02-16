//! Provider suggestion assembly helpers.
//!
//! Encapsulates the logic for turning provider bindings into suggestion
//! results, including cache lookup and fuzzy matching.

use std::collections::HashMap;

use oatty_registry::CommandSpec;
use oatty_types::{
    Bind, ItemKind, ProviderSelectorActionPayload, SuggestionItem, ValueProvider as ProviderBinding, encode_provider_selector_action,
};
use oatty_util::fuzzy_score;
use serde_json::{Map as JsonMap, Value};

use super::identifier::{ProviderIdentifier, canonical_identifier};
use super::registry::ProviderRegistry;
use super::{CacheLookupOutcome, ProviderSuggestionSet, label_from_value};
use crate::provider::SelectionSource;
use crate::resolve::select_path;

/// Helper for building provider-backed suggestions for CLI inputs.
///
/// # Purpose
/// Performs binding resolution and cache-aware suggestion building for a
/// given `(command, field)` tuple.
pub(crate) struct ProviderSuggestionBuilder;

impl ProviderSuggestionBuilder {
    /// Build provider-backed suggestions for a command input.
    ///
    /// # Arguments
    /// - `provider_registry`: Registry for cache lookup and fetch planning.
    /// - `commands`: Full list of registered command specs.
    /// - `command_key`: Canonical `<group> <name>` identifier for the active command.
    /// - `field`: Flag or positional name to resolve.
    /// - `partial`: User-typed prefix for fuzzy matching.
    /// - `inputs`: Current resolved inputs used to satisfy provider bindings.
    ///
    /// # Returns
    /// Returns a suggestion set with immediate items and an optional pending fetch.
    pub(crate) fn build_suggestions(
        provider_registry: &ProviderRegistry,
        commands: &[CommandSpec],
        command_key: &str,
        field: &str,
        partial: &str,
        inputs: &HashMap<String, String>,
    ) -> ProviderSuggestionSet {
        let identifier = match ProviderIdentifier::parse(command_key) {
            Some(identifier) => identifier,
            None => return ProviderSuggestionSet::default(),
        };

        let command_spec = match commands
            .iter()
            .find(|command| command.group == identifier.group && command.name == identifier.name)
        {
            Some(spec) => spec,
            None => return ProviderSuggestionSet::default(),
        };

        let (provider_identifier, bindings) = match binding_for_field(command_spec, field) {
            Some(binding) => binding,
            None => return ProviderSuggestionSet::default(),
        };

        let mut arguments = JsonMap::new();
        for binding in &bindings {
            if let Some(value) = inputs.get(&binding.from) {
                arguments.insert(binding.provider_key.clone(), Value::String(value.clone()));
            } else {
                // Cannot satisfy provider bindings yet; trigger fetch once values are available.
                return ProviderSuggestionSet::default();
            }
        }

        if !provider_required_inputs_satisfied(commands, &provider_identifier, &arguments) {
            return ProviderSuggestionSet::default();
        }

        match provider_registry.cached_values_or_plan(&provider_identifier, arguments) {
            CacheLookupOutcome::Hit(values) => {
                let provider_meta = canonical_identifier(&provider_identifier).unwrap_or_else(|| provider_identifier.clone());
                let selection = provider_registry.resolved_selection_for_provider(&provider_identifier);
                if provider_payload_is_ambiguous(&values, &selection) {
                    let action_payload = ProviderSelectorActionPayload {
                        provider_id: provider_identifier.clone(),
                        command_key: command_key.to_string(),
                        field: field.to_string(),
                        positional: command_spec.positional_args.iter().any(|argument| argument.name == field),
                    };
                    let display = format!("Select value from {provider_meta}...");
                    let score = fuzzy_score(&display, partial).unwrap_or(i64::MAX / 4);
                    return ProviderSuggestionSet::ready(vec![SuggestionItem {
                        display,
                        insert_text: encode_provider_selector_action(&action_payload),
                        kind: ItemKind::Value,
                        meta: Some("selector".to_string()),
                        score,
                    }]);
                }

                let mut items = Vec::with_capacity(values.len());
                for value in values {
                    let Some((insert_text, display)) = provider_row_suggestion_value(&value, &selection) else {
                        continue;
                    };
                    let Some(score) = fuzzy_score(&display, partial) else {
                        continue;
                    };
                    items.push(SuggestionItem {
                        display,
                        insert_text,
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

fn provider_required_inputs_satisfied(commands: &[CommandSpec], provider_identifier: &str, arguments: &JsonMap<String, Value>) -> bool {
    let Some(identifier) = ProviderIdentifier::parse(provider_identifier) else {
        return false;
    };
    let Some(provider_command) = commands
        .iter()
        .find(|command| command.group == identifier.group && command.name == identifier.name)
    else {
        return false;
    };

    if provider_command
        .positional_args
        .iter()
        .any(|positional_argument| !arguments.contains_key(&positional_argument.name))
    {
        return false;
    }

    if provider_command
        .flags
        .iter()
        .filter(|flag| flag.required)
        .any(|flag| !arguments.contains_key(&flag.name))
    {
        return false;
    }

    true
}

fn provider_payload_is_ambiguous(values: &[Value], selection: &crate::provider::FieldSelection) -> bool {
    if matches!(selection.source, SelectionSource::RequiresChoice) {
        return true;
    }

    values.iter().any(|value| !provider_row_is_selectable(value, selection))
}

fn provider_row_is_selectable(value: &Value, selection: &crate::provider::FieldSelection) -> bool {
    match select_path(value, Some(&selection.value_field)) {
        Some(selected_value) => is_scalar_json_value(&selected_value),
        None => false,
    }
}

fn provider_row_suggestion_value(value: &Value, selection: &crate::provider::FieldSelection) -> Option<(String, String)> {
    let selected_value = select_path(value, Some(&selection.value_field))?;
    if !is_scalar_json_value(&selected_value) {
        return None;
    }
    let insert_text = scalar_value_to_string(selected_value);

    let display = select_path(value, Some(&selection.display_field))
        .filter(is_scalar_json_value)
        .map(scalar_value_to_string)
        .or_else(|| label_from_value(value.clone()))
        .unwrap_or_else(|| insert_text.clone());

    Some((insert_text, display))
}

fn is_scalar_json_value(value: &Value) -> bool {
    matches!(value, Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null)
}

fn scalar_value_to_string(value: Value) -> String {
    match value {
        Value::String(value) => value,
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn binding_for_field(command_spec: &CommandSpec, field: &str) -> Option<(String, Vec<Bind>)> {
    if let Some(flag) = command_spec.flags.iter().find(|flag| flag.name == field)
        && let Some(ProviderBinding::Command { command_id, binds }) = &flag.provider
    {
        return Some((command_id.clone(), binds.clone()));
    }
    if let Some(positional) = command_spec.positional_args.iter().find(|arg| arg.name == field)
        && let Some(ProviderBinding::Command { command_id, binds }) = &positional.provider
    {
        return Some((command_id.clone(), binds.clone()));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{provider_payload_is_ambiguous, provider_required_inputs_satisfied, provider_row_suggestion_value};
    use crate::provider::{FieldSelection, SelectionSource};
    use oatty_types::{CommandFlag, CommandSpec, HttpCommandSpec, PositionalArgument};
    use serde_json::json;

    #[test]
    fn payload_ambiguity_detects_any_ambiguous_row() {
        let values = vec![json!({"id":"app-1"}), json!({"slug":"app-a","region":"us-west-2"})];
        let selection = FieldSelection {
            value_field: "id".into(),
            display_field: "name".into(),
            id_field: Some("id".into()),
            source: SelectionSource::ByNames,
        };
        assert!(provider_payload_is_ambiguous(&values, &selection));
    }

    #[test]
    fn required_provider_inputs_must_be_bound_before_palette_suggests() {
        let provider_command = CommandSpec::new_http(
            "apps".to_string(),
            "list".to_string(),
            "List applications".to_string(),
            vec![PositionalArgument {
                name: "owner_id".to_string(),
                help: None,
                provider: None,
            }],
            vec![CommandFlag {
                name: "region".to_string(),
                short_name: Some("r".to_string()),
                required: true,
                r#type: "string".to_string(),
                enum_values: Vec::new(),
                default_value: None,
                description: None,
                provider: None,
            }],
            HttpCommandSpec::new("GET", "/apps", None, None),
            0,
        );
        let commands = vec![provider_command];

        let partial_bindings = serde_json::Map::from_iter([("owner_id".to_string(), json!("team-1"))]);
        assert!(!provider_required_inputs_satisfied(&commands, "apps list", &partial_bindings));

        let complete_bindings = serde_json::Map::from_iter([
            ("owner_id".to_string(), json!("team-1")),
            ("region".to_string(), json!("us-east-1")),
        ]);
        assert!(provider_required_inputs_satisfied(&commands, "apps list", &complete_bindings));
    }

    #[test]
    fn payload_is_not_ambiguous_when_nested_selection_is_deterministic() {
        let values = vec![json!({"cursor":"a","item":{"id":"project-a","name":"Project A"}})];
        let selection = FieldSelection {
            value_field: "item.id".into(),
            display_field: "item.name".into(),
            id_field: Some("item.id".into()),
            source: SelectionSource::ByNames,
        };

        assert!(!provider_payload_is_ambiguous(&values, &selection));
    }

    #[test]
    fn suggestion_uses_nested_value_and_display_fields() {
        let value = json!({"cursor":"a","item":{"id":"project-a","name":"Project A"}});
        let selection = FieldSelection {
            value_field: "item.id".into(),
            display_field: "item.name".into(),
            id_field: Some("item.id".into()),
            source: SelectionSource::ByNames,
        };

        let suggestion = provider_row_suggestion_value(&value, &selection).expect("expected suggestion values");
        assert_eq!(suggestion.0, "project-a");
        assert_eq!(suggestion.1, "Project A");
    }
}
