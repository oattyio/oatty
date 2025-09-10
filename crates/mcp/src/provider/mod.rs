//! MCP provider integration for the engine.

mod adapter;
mod mcp_provider;

pub use adapter::{AdapterError, McpProviderAdapter};
pub use mcp_provider::McpProvider;

use crate::plugin::PluginEngine;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Registry for MCP providers.
pub struct McpProviderRegistry {
    /// Registered providers.
    providers: Arc<Mutex<HashMap<String, Arc<McpProvider>>>>,

    /// Plugin engine.
    plugin_engine: Arc<PluginEngine>,
}

impl McpProviderRegistry {
    /// Create a new MCP provider registry.
    pub fn new(plugin_engine: Arc<PluginEngine>) -> Self {
        Self {
            providers: Arc::new(Mutex::new(HashMap::new())),
            plugin_engine,
        }
    }

    /// Register a provider for a plugin.
    pub async fn register_provider(&self, plugin_name: &str, tool_name: &str) -> Result<(), McpProviderError> {
        let provider = McpProvider::new(plugin_name, tool_name, Arc::clone(&self.plugin_engine))?;

        let provider_id = format!("{}:{}", plugin_name, tool_name);
        let mut providers = self.providers.lock().await;
        providers.insert(provider_id, Arc::new(provider));

        Ok(())
    }

    /// Unregister a provider.
    pub async fn unregister_provider(&self, plugin_name: &str, tool_name: &str) {
        let provider_id = format!("{}:{}", plugin_name, tool_name);
        let mut providers = self.providers.lock().await;
        providers.remove(&provider_id);
    }

    /// Get a provider.
    pub async fn get_provider(&self, plugin_name: &str, tool_name: &str) -> Option<Arc<McpProvider>> {
        let provider_id = format!("{}:{}", plugin_name, tool_name);
        let providers = self.providers.lock().await;
        providers.get(&provider_id).cloned()
    }

    /// List all providers.
    pub async fn list_providers(&self) -> Vec<String> {
        let providers = self.providers.lock().await;
        providers.keys().cloned().collect()
    }

    /// Discover and register providers from all running plugins.
    pub async fn discover_providers(&self) -> Result<(), McpProviderError> {
        let plugins = self.plugin_engine.list_plugins().await;

        for plugin in plugins {
            if plugin.is_running() {
                // TODO: Discover tools from the plugin and register providers
                // This would involve calling the MCP service to list available tools
                tracing::debug!("Discovering providers for plugin: {}", plugin.name);
            }
        }

        Ok(())
    }

    /// Get the plugin engine.
    pub fn plugin_engine(&self) -> &PluginEngine {
        &self.plugin_engine
    }
}

/// Trait for MCP provider operations.
#[async_trait::async_trait]
pub trait McpProviderOps: Send + Sync {
    /// Fetch values from the provider.
    async fn fetch_values(&self, arguments: &Map<String, Value>) -> Result<Vec<Value>, McpProviderError>;

    /// Get the provider contract.
    fn get_contract(&self) -> crate::provider::mcp_provider::ProviderContract;

    /// Check if the provider is available.
    async fn is_available(&self) -> bool;

    /// Get the provider ID.
    fn provider_id(&self) -> &str;
}

/// Errors that can occur with MCP providers.
#[derive(Debug, thiserror::Error)]
pub enum McpProviderError {
    #[error("Plugin not found: {name}")]
    PluginNotFound { name: String },

    #[error("Tool not found: {tool} in plugin {plugin}")]
    ToolNotFound { plugin: String, tool: String },

    #[error("Plugin not running: {name}")]
    PluginNotRunning { name: String },

    #[error("MCP error: {0}")]
    McpError(String),

    #[error("Provider error: {message}")]
    ProviderError { message: String },

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Timeout error: {operation}")]
    TimeoutError { operation: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::McpConfig;

    #[tokio::test]
    async fn test_mcp_provider_registry() {
        let config = McpConfig::default();
        let plugin_engine = Arc::new(PluginEngine::new(config).unwrap());
        let registry = McpProviderRegistry::new(plugin_engine);

        let providers = registry.list_providers().await;
        assert!(providers.is_empty());
    }
}
