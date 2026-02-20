//! Provider registry types and behaviors.

use crate::plugin::PluginEngine;
use crate::provider::{McpProvider, mcp_provider};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Tracks discovered MCP-backed value providers.
pub struct McpProviderRegistry {
    /// Registered providers keyed by `plugin:tool`.
    providers: Arc<Mutex<HashMap<String, Arc<McpProvider>>>>,
    /// Shared plugin engine used to discover and instantiate providers.
    plugin_engine: Arc<PluginEngine>,
}

impl McpProviderRegistry {
    /// Creates an empty provider registry.
    pub fn new(plugin_engine: Arc<PluginEngine>) -> Self {
        Self {
            providers: Arc::new(Mutex::new(HashMap::new())),
            plugin_engine,
        }
    }

    /// Registers a provider for a plugin tool pair.
    pub async fn register_provider(&self, plugin_name: &str, tool_name: &str) -> Result<(), McpProviderError> {
        let provider = McpProvider::new(plugin_name, tool_name, Arc::clone(&self.plugin_engine))?;
        let provider_identifier = provider_identifier(plugin_name, tool_name);
        let mut providers = self.providers.lock().await;
        providers.insert(provider_identifier, Arc::new(provider));
        Ok(())
    }

    /// Unregisters a provider for a plugin tool pair.
    pub async fn unregister_provider(&self, plugin_name: &str, tool_name: &str) {
        let provider_identifier = provider_identifier(plugin_name, tool_name);
        let mut providers = self.providers.lock().await;
        providers.remove(&provider_identifier);
    }

    /// Retrieves a provider by plugin and tool name.
    pub async fn get_provider(&self, plugin_name: &str, tool_name: &str) -> Option<Arc<McpProvider>> {
        let provider_identifier = provider_identifier(plugin_name, tool_name);
        let providers = self.providers.lock().await;
        providers.get(&provider_identifier).cloned()
    }

    /// Lists all registered provider identifiers.
    pub async fn list_providers(&self) -> Vec<String> {
        let providers = self.providers.lock().await;
        providers.keys().cloned().collect()
    }

    /// Discovers providers from currently running plugins.
    pub async fn discover_providers(&self) -> Result<(), McpProviderError> {
        let plugins = self.plugin_engine.list_plugins().await;
        for plugin in plugins {
            if plugin.is_running() {
                // TODO: Discover tools from each plugin and register providers.
                tracing::debug!("Discovering providers for plugin: {}", plugin.name);
            }
        }
        Ok(())
    }

    /// Exposes the plugin engine used by this registry.
    pub fn plugin_engine(&self) -> &PluginEngine {
        &self.plugin_engine
    }
}

fn provider_identifier(plugin_name: &str, tool_name: &str) -> String {
    format!("{plugin_name}:{tool_name}")
}

/// Contract for MCP provider operations.
#[async_trait::async_trait]
pub trait McpProviderOps: Send + Sync {
    /// Fetches values from a provider using command arguments.
    async fn fetch_values(&self, arguments: &Map<String, Value>) -> Result<Vec<Value>, McpProviderError>;

    /// Returns the provider contract metadata.
    fn get_contract(&self) -> mcp_provider::ProviderContract;

    /// Indicates whether the provider is currently available.
    async fn is_available(&self) -> bool;

    /// Returns provider identifier in `plugin:tool` format.
    fn provider_id(&self) -> &str;
}

/// Errors produced by provider discovery and invocation.
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
    use oatty_registry::CommandRegistry;
    use std::sync::Mutex;

    #[tokio::test]
    async fn provider_registry_starts_empty() {
        let config = McpConfig::default();
        let command_registry = Arc::new(Mutex::new(CommandRegistry::default()));
        let plugin_engine = Arc::new(PluginEngine::new(config, Arc::clone(&command_registry)).unwrap());
        let registry = McpProviderRegistry::new(plugin_engine);

        let providers = registry.list_providers().await;
        assert!(providers.is_empty());
    }
}
