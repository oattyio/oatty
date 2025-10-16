//! Provider registry and value resolution facade.
//!
//! Modules:
//! - `contract`: Contracts describing provider inputs/outputs
//! - `selection`: Field selection heuristics and coercion helpers
//! - `fetch`: Fetcher trait and default HTTP fetcher
//! - `registry`: Registry-backed provider implementation with caching
//! - `null`: No-op provider for tests and disabled scenarios

mod contract;
mod fetch;
mod null;
mod registry;
mod selection;
mod value_provider;

use anyhow::Result;
pub use contract::{ProviderContract, ProviderReturns, ReturnField};
pub use fetch::ProviderValueFetcher;
pub use null::NullProvider;
pub use registry::ProviderRegistry;
pub use selection::{FieldSelection, SelectionSource, coerce_value, infer_selection};
pub use value_provider::{ValueProvider, label_from_value};

use serde_json::Value;

/// Trait defining the interface for provider value resolution.
pub trait ProviderValueResolver: Send + Sync {
    fn fetch_values(&self, provider_id: &str, arguments: &serde_json::Map<String, Value>) -> Result<Vec<Value>>;
    fn get_contract(&self, provider_id: &str) -> Option<ProviderContract>;
}
