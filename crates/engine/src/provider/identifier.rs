//! Provider identifier parsing and cache key helpers.
//!
//! This module centralizes normalization for provider identifiers to ensure
//! callers generate consistent cache keys and command lookups.

use serde_json::{Map as JsonMap, Value};
use std::hash::DefaultHasher;
use std::hash::{Hash, Hasher};
use tracing::warn;

/// Parsed provider identifier in canonical `<group> <name>` form.
///
/// # Purpose
/// Represents a validated provider identifier split into its group and name components.
///
/// # Fields
/// - `group`: Command group name.
/// - `name`: Command name.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub(crate) struct ProviderIdentifier {
    /// Command group name.
    pub group: String,
    /// Command name.
    pub name: String,
}

impl ProviderIdentifier {
    /// Parse a provider identifier in `<group> <name>` format.
    ///
    /// # Arguments
    /// - `identifier`: Raw identifier string to parse.
    ///
    /// # Returns
    /// Returns `Some(ProviderIdentifier)` when the identifier is valid and contains
    /// a non-empty group and name, otherwise `None`.
    ///
    /// # Side effects
    /// Logs a warning when a colon-delimited identifier is supplied.
    pub(crate) fn parse(identifier: &str) -> Option<Self> {
        if let Some((group, name)) = identifier.split_once(char::is_whitespace) {
            let group = group.trim();
            let name = name.trim();
            if !group.is_empty() && !name.is_empty() {
                return Some(Self {
                    group: group.to_string(),
                    name: name.to_string(),
                });
            }
        }
        if identifier.contains(':') {
            warn!(
                "Colon-delimited provider identifiers are no longer supported: '{}'. Use the '<group> <name>' format instead.",
                identifier
            );
        }
        None
    }

    /// Render the identifier in canonical `<group> <name>` form.
    ///
    /// # Returns
    /// Returns a string formatted as `<group> <name>`.
    pub(crate) fn canonical_string(&self) -> String {
        format!("{} {}", self.group, self.name)
    }
}

/// Convert a raw identifier string into canonical `<group> <name>` form.
///
/// # Arguments
/// - `identifier`: Raw identifier string to normalize.
///
/// # Returns
/// Returns `Some(String)` when parsing succeeds, otherwise `None`.
pub(crate) fn canonical_identifier(identifier: &str) -> Option<String> {
    ProviderIdentifier::parse(identifier).map(|parsed| parsed.canonical_string())
}

/// Build a cache key from a canonical identifier and its arguments.
///
/// # Arguments
/// - `canonical_identifier`: Canonical `<group> <name>` provider identifier.
/// - `arguments`: JSON arguments used for the provider call.
///
/// # Returns
/// Returns a cache key string that combines the canonical identifier with a hash of the arguments.
pub(crate) fn cache_key_for_canonical_identifier(canonical_identifier: &str, arguments: &JsonMap<String, Value>) -> String {
    let mut hasher = DefaultHasher::new();
    canonical_identifier.hash(&mut hasher);
    if let Ok(serialized_arguments) = serde_json::to_string(arguments) {
        serialized_arguments.hash(&mut hasher);
    }
    format!("{}:{}", canonical_identifier, hasher.finish())
}

/// Build a cache key from a raw identifier and arguments.
///
/// # Arguments
/// - `identifier`: Raw provider identifier (canonical or otherwise).
/// - `arguments`: JSON arguments used for the provider call.
///
/// # Returns
/// Returns a cache key string based on the canonicalized identifier and arguments.
pub(crate) fn cache_key_for_identifier(identifier: &str, arguments: &JsonMap<String, Value>) -> String {
    let canonical = canonical_identifier(identifier).unwrap_or_else(|| identifier.to_string());
    cache_key_for_canonical_identifier(&canonical, arguments)
}
