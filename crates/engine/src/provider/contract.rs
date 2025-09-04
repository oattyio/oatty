use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Metadata describing a provider's capabilities and interface.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderContract {
    #[serde(default)]
    pub args: serde_json::Map<String, Value>,
    #[serde(default)]
    pub returns: ProviderReturns,
}

/// Description of the values that a provider can return.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderReturns {
    #[serde(default)]
    pub fields: Vec<ReturnField>,
}

/// Definition of a single field within a provider's return value.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReturnField {
    pub name: String,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}
