use crate::provider::{McpProvider, McpProviderError, McpProviderOps};
use serde_json::{Map, Value};
use std::sync::Arc;
// Note: This would need to be implemented to integrate with the existing engine
// For now, we'll create a local version
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderContract {
    pub args: Map<String, Value>,
    pub returns: ProviderReturns,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderReturns {
    pub fields: Vec<ReturnField>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReturnField {
    pub name: String,
    pub r#type: Option<String>,
    pub tags: Vec<String>,
}

#[allow(dead_code)]
pub trait ProviderRegistry: Send + Sync {
    fn fetch_values(&self, provider_id: &str, arguments: &Map<String, Value>) -> anyhow::Result<Vec<Value>>;
    fn get_contract(&self, provider_id: &str) -> Option<ProviderContract>;
}

/// Adapter that implements the engine's ProviderRegistry trait for MCP providers.
pub struct McpProviderAdapter {
    /// The underlying MCP provider.
    provider: Arc<McpProvider>,
}

impl McpProviderAdapter {
    /// Create a new MCP provider adapter.
    pub fn new(provider: Arc<McpProvider>) -> Self {
        Self { provider }
    }

    /// Get the underlying MCP provider.
    pub fn provider(&self) -> &Arc<McpProvider> {
        &self.provider
    }
}

impl ProviderRegistry for McpProviderAdapter {
    fn fetch_values(&self, provider_id: &str, arguments: &Map<String, Value>) -> anyhow::Result<Vec<Value>> {
        // Check if the provider ID matches
        if provider_id != self.provider.provider_id() {
            return Err(anyhow::anyhow!(
                "Provider ID mismatch: expected {}, got {}",
                self.provider.provider_id(),
                provider_id
            ));
        }

        let fetch_future = async {
            self.provider
                .fetch_values(arguments)
                .await
                .map_err(|e| anyhow::anyhow!("MCP provider error: {}", e))
        };

        match tokio::runtime::Handle::try_current() {
            Ok(handle) => handle.block_on(fetch_future),
            Err(_) => {
                let runtime = tokio::runtime::Runtime::new().map_err(|error| anyhow::anyhow!("failed to create runtime: {}", error))?;
                runtime.block_on(fetch_future)
            }
        }
    }

    fn get_contract(&self, provider_id: &str) -> Option<ProviderContract> {
        // Check if the provider ID matches
        if provider_id != self.provider.provider_id() {
            return None;
        }

        let contract = self.provider.get_contract();
        Some(ProviderContract {
            args: contract.args,
            returns: ProviderReturns {
                fields: contract
                    .returns
                    .fields
                    .into_iter()
                    .map(|f| ReturnField {
                        name: f.name,
                        r#type: f.r#type,
                        tags: f.tags,
                    })
                    .collect(),
            },
        })
    }
}

/// Registry for managing multiple MCP provider adapters.
#[allow(dead_code)]
pub struct McpProviderAdapterRegistry {
    /// Registered adapters.
    adapters: std::collections::HashMap<String, Arc<McpProviderAdapter>>,
}

impl McpProviderAdapterRegistry {
    /// Create a new adapter registry.
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            adapters: std::collections::HashMap::new(),
        }
    }

    /// Register an adapter.
    #[allow(dead_code)]
    pub fn register_adapter(&mut self, provider_id: String, adapter: Arc<McpProviderAdapter>) {
        self.adapters.insert(provider_id, adapter);
    }

    /// Unregister an adapter.
    #[allow(dead_code)]
    pub fn unregister_adapter(&mut self, provider_id: &str) {
        self.adapters.remove(provider_id);
    }

    /// Get an adapter.
    #[allow(dead_code)]
    pub fn get_adapter(&self, provider_id: &str) -> Option<&Arc<McpProviderAdapter>> {
        self.adapters.get(provider_id)
    }

    /// List all registered provider IDs.
    #[allow(dead_code)]
    pub fn list_providers(&self) -> Vec<String> {
        self.adapters.keys().cloned().collect()
    }

    /// Check if a provider is registered.
    #[allow(dead_code)]
    pub fn is_registered(&self, provider_id: &str) -> bool {
        self.adapters.contains_key(provider_id)
    }

    /// Get the number of registered providers.
    #[allow(dead_code)]
    pub fn count(&self) -> usize {
        self.adapters.len()
    }
}

impl Default for McpProviderAdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur in the adapter.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("Provider ID mismatch: expected {expected}, got {actual}")]
    ProviderIdMismatch { expected: String, actual: String },

    #[error("Provider not found: {provider_id}")]
    ProviderNotFound { provider_id: String },

    #[error("MCP provider error: {0}")]
    McpProviderError(#[from] McpProviderError),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Adapter error: {message}")]
    AdapterError { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::McpConfig;
    use crate::plugin::PluginEngine;
    use oatty_registry::CommandRegistry;
    use std::sync::Mutex;

    #[test]
    fn test_mcp_provider_adapter_creation() {
        let config = McpConfig::default();
        let command_registry = Arc::new(Mutex::new(CommandRegistry {
            commands: Vec::new(),
            workflows: vec![],
            provider_contracts: Default::default(),
        }));
        let plugin_engine = Arc::new(PluginEngine::new(config, Arc::clone(&command_registry)).unwrap());
        let provider = Arc::new(McpProvider::new("test-plugin", "test-tool", plugin_engine).unwrap());
        let adapter = McpProviderAdapter::new(provider);

        assert_eq!(adapter.provider().provider_id(), "test-plugin:test-tool");
    }

    #[test]
    fn test_mcp_provider_adapter_registry() {
        let mut adapter_registry = McpProviderAdapterRegistry::new();

        let config = McpConfig::default();
        let command_registry = Arc::new(Mutex::new(CommandRegistry {
            commands: Vec::new(),
            workflows: vec![],
            provider_contracts: Default::default(),
        }));
        let plugin_engine = Arc::new(PluginEngine::new(config, Arc::clone(&command_registry)).unwrap());
        let provider = Arc::new(McpProvider::new("test-plugin", "test-tool", plugin_engine).unwrap());
        let adapter = Arc::new(McpProviderAdapter::new(provider));

        adapter_registry.register_adapter("test-plugin:test-tool".to_string(), adapter);

        assert!(adapter_registry.is_registered("test-plugin:test-tool"));
        assert_eq!(adapter_registry.count(), 1);

        let providers = adapter_registry.list_providers();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0], "test-plugin:test-tool");

        adapter_registry.unregister_adapter("test-plugin:test-tool");
        assert!(!adapter_registry.is_registered("test-plugin:test-tool"));
        assert_eq!(adapter_registry.count(), 0);
    }

    #[test]
    fn test_provider_registry_implementation() {
        let config = McpConfig::default();
        let command_registry = Arc::new(Mutex::new(CommandRegistry {
            commands: Vec::new(),
            workflows: vec![],
            provider_contracts: Default::default(),
        }));
        let plugin_engine = Arc::new(PluginEngine::new(config, Arc::clone(&command_registry)).unwrap());
        let provider = Arc::new(McpProvider::new("test-plugin", "test-tool", plugin_engine).unwrap());
        let adapter = McpProviderAdapter::new(provider);

        // Test get_contract
        let contract = adapter.get_contract("test-plugin:test-tool");
        assert!(contract.is_some());

        let contract = adapter.get_contract("wrong-id");
        assert!(contract.is_none());

        // Test fetch_values (this will fail because the plugin isn't running, but we can test the ID check)
        let args = Map::new();
        let result = adapter.fetch_values("wrong-id", &args);
        assert!(result.is_err());
    }
}
