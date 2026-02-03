//! MCP provider implementation.

use crate::plugin::PluginEngine;
use crate::provider::{McpProviderError, McpProviderOps};

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

/// MCP provider that bridges MCP tools to the engine's provider system.
pub struct McpProvider {
    /// Plugin name.
    plugin_name: String,

    /// Tool name.
    tool_name: String,

    /// Provider ID.
    provider_id: String,

    /// Plugin engine.
    plugin_engine: Arc<PluginEngine>,

    /// Provider contract.
    contract: ProviderContract,
}

impl McpProvider {
    /// Create a new MCP provider.
    pub fn new(plugin_name: &str, tool_name: &str, plugin_engine: Arc<PluginEngine>) -> Result<Self, McpProviderError> {
        let provider_id = format!("{}:{}", plugin_name, tool_name);

        Ok(Self {
            plugin_name: plugin_name.to_string(),
            tool_name: tool_name.to_string(),
            provider_id,
            plugin_engine,
            contract: ProviderContract::default(),
        })
    }

    /// Initialize the provider by fetching tool metadata.
    pub async fn initialize(&mut self) -> Result<(), McpProviderError> {
        // Get the client for the plugin
        let client = self
            .plugin_engine
            .client_manager()
            .get_client(&self.plugin_name)
            .await
            .ok_or_else(|| McpProviderError::PluginNotFound {
                name: self.plugin_name.clone(),
            })?;

        let _client = client.lock().await;
        Ok(())
    }

    /// Call the MCP tool with the given arguments.
    async fn call_tool(&self, _arguments: &Map<String, Value>) -> Result<Value, McpProviderError> {
        // Get the client for the plugin
        let _client = self
            .plugin_engine
            .client_manager()
            .get_client(&self.plugin_name)
            .await
            .ok_or_else(|| McpProviderError::PluginNotFound {
                name: self.plugin_name.clone(),
            })?;

        Err(McpProviderError::ProviderError {
            message: format!("MCP tool '{}' execution not yet wired", self.tool_name),
        })
    }
}

#[async_trait::async_trait]
impl McpProviderOps for McpProvider {
    async fn fetch_values(&self, arguments: &Map<String, Value>) -> Result<Vec<Value>, McpProviderError> {
        // Call the MCP tool
        let result = self.call_tool(arguments).await?;

        // Convert the result to a vector of values
        // The exact format depends on the tool's output
        let values = match result {
            Value::Array(arr) => arr,
            Value::Object(obj) => {
                // If the result is an object, try to extract an array from it
                if let Some(Value::Array(arr)) = obj.get("items") {
                    arr.clone()
                } else if let Some(Value::Array(arr)) = obj.get("results") {
                    arr.clone()
                } else {
                    // If no array is found, wrap the object in an array
                    vec![Value::Object(obj)]
                }
            }
            other => vec![other],
        };

        Ok(values)
    }

    fn get_contract(&self) -> ProviderContract {
        self.contract.clone()
    }

    async fn is_available(&self) -> bool {
        // Check if the plugin is running
        self.plugin_engine.is_plugin_running(&self.plugin_name).await
    }

    fn provider_id(&self) -> &str {
        &self.provider_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::McpConfig;
    use oatty_registry::CommandRegistry;
    use std::sync::Mutex;

    #[tokio::test]
    async fn test_mcp_provider_creation() {
        let config = McpConfig::default();
        let registry = Arc::new(Mutex::new(CommandRegistry::default()));
        let plugin_engine = Arc::new(PluginEngine::new(config, Arc::clone(&registry)).unwrap());

        let provider = McpProvider::new("test-plugin", "test-tool", plugin_engine).unwrap();
        assert_eq!(provider.provider_id(), "test-plugin:test-tool");
        assert_eq!(provider.plugin_name, "test-plugin");
        assert_eq!(provider.tool_name, "test-tool");
    }

    #[tokio::test]
    async fn test_mcp_provider_availability() {
        let config = McpConfig::default();
        let registry = Arc::new(Mutex::new(CommandRegistry::default()));
        let plugin_engine = Arc::new(PluginEngine::new(config, Arc::clone(&registry)).unwrap());

        let provider = McpProvider::new("test-plugin", "test-tool", plugin_engine).unwrap();
        assert!(!provider.is_available().await);
    }
}
