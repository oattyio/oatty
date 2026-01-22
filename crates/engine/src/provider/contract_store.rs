//! Provider contract resolution helper.
//!
//! This module isolates contract lookup behavior so the registry can delegate
//! schema-related concerns to a focused helper.

use std::sync::{Arc, Mutex};

use oatty_registry::CommandRegistry;
use oatty_types::{ProviderContract, ProviderFieldContract, ProviderReturnContract};

use super::identifier::ProviderIdentifier;

/// Registry-backed provider contract resolver.
///
/// # Purpose
/// Retrieves provider contracts for canonical `<group> <name>` identifiers and
/// falls back to a default contract when none is registered.
///
/// # Fields
/// - `registry`: Shared command registry used for contract lookup.
#[derive(Debug, Clone)]
pub(crate) struct ProviderContractStore {
    registry: Arc<Mutex<CommandRegistry>>,
}

impl ProviderContractStore {
    /// Create a new contract store bound to a command registry.
    ///
    /// # Arguments
    /// - `registry`: Shared registry containing provider contracts.
    ///
    /// # Returns
    /// Returns a new `ProviderContractStore`.
    pub(crate) fn new(registry: Arc<Mutex<CommandRegistry>>) -> Self {
        Self { registry }
    }

    /// Resolve a provider contract by identifier.
    ///
    /// # Arguments
    /// - `provider_id`: Raw provider identifier string to resolve.
    ///
    /// # Returns
    /// Returns `Some(ProviderContract)` when parsing succeeds and the registry
    /// contains a matching command. Falls back to a default contract only when
    /// the command exists but has no registered contract.
    pub(crate) fn resolve_contract(&self, provider_id: &str) -> Option<ProviderContract> {
        let identifier = ProviderIdentifier::parse(provider_id)?;
        let canonical_key = identifier.canonical_string();
        let registry = self.registry.lock().ok()?;

        if let Some(contract) = registry.provider_contracts.get(&canonical_key).cloned() {
            return Some(contract);
        }
        if registry.commands.iter().any(|command| command.canonical_id() == canonical_key) {
            return Some(default_provider_contract());
        }

        None
    }
}

fn default_provider_contract() -> ProviderContract {
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
    }
}
