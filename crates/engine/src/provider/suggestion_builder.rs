//! Provider suggestion assembly helpers.
//!
//! Encapsulates the logic for turning provider bindings into suggestion
//! results, including cache lookup and fuzzy matching.

use std::collections::HashMap;

use oatty_registry::CommandSpec;
use oatty_types::{Bind, ItemKind, SuggestionItem, ValueProvider as ProviderBinding};
use oatty_util::fuzzy_score;
use serde_json::{Map as JsonMap, Value};

use super::identifier::{ProviderIdentifier, canonical_identifier};
use super::registry::ProviderRegistry;
use super::{CacheLookupOutcome, ProviderSuggestionSet, label_from_value};

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

        match provider_registry.cached_values_or_plan(&provider_identifier, arguments) {
            CacheLookupOutcome::Hit(values) => {
                let provider_meta = canonical_identifier(&provider_identifier).unwrap_or_else(|| provider_identifier.clone());
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
