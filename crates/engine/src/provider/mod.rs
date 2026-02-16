//! Provider registry and value resolution facade.
//!
//! Modules:
//! - `contract`: Contracts describing provider inputs/outputs
//! - `contract_store`: Registry-backed provider contract resolver
//! - `fetch`: Fetcher trait and default HTTP fetcher
//! - `identifier`: Provider identifier parsing and cache key helpers
//! - `registry`: Registry-backed provider implementation with caching
//! - `selection`: Field selection heuristics and coercion helpers
//! - `suggestion_builder`: Suggestion assembly for provider-backed inputs
//! - `null`: No-op provider for tests and disabled scenarios

mod contract_store;
mod fetch;
mod identifier;
mod null;
mod registry;
mod selection;
mod suggestion_builder;
mod value_provider;

use anyhow::Result;
pub use fetch::ProviderValueFetcher;
pub(crate) use identifier::ProviderIdentifier;
pub use identifier::parse_provider_group_and_command;
pub use null::NullProvider;
use oatty_types::ProviderContract;
pub use registry::{CacheLookupOutcome, ProviderRegistry};
pub use selection::{FieldSelection, SelectionSource, coerce_value, infer_selection};
pub use value_provider::{PendingProviderFetch, ProviderFetchPlan, ProviderSuggestionSet, ValueProvider, label_from_value};

use serde_json::Value;

/// Trait defining the interface for provider value resolution.
pub trait ProviderValueResolver: Send + Sync {
    fn fetch_values(&self, provider_id: &str, arguments: &serde_json::Map<String, Value>) -> Result<Vec<Value>>;
    fn get_contract(&self, provider_id: &str) -> Option<ProviderContract>;
}
