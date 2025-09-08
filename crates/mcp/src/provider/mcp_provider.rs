//! MCP provider implementation.

use crate::plugin::PluginEngine;
use crate::provider::{McpProviderError, McpProviderOps};
use rmcp::model::{CallToolRequestParam, Tool};
use serde_json::{Map, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
// Note: This would need to be implemented to integrate with the existing engine
// For now, we'll create a local version
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderContract {
    pub args: serde_json::Map<String, serde_json::Value>,
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

    /// Tool metadata.
    tool_metadata: Option<Tool>,

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
            tool_metadata: None,
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

        let client = client.lock().await;
        let peer = client.peer().ok_or_else(|| McpProviderError::PluginNotRunning {
            name: self.plugin_name.clone(),
        })?;

        // List tools to get metadata
        let tools = peer.list_tools(Default::default()).await.map_err(|e| match e {
            rmcp::service::ServiceError::McpError(mcp_err) => McpProviderError::McpError(mcp_err),
            other => McpProviderError::McpError(rmcp::ErrorData::invalid_request(
                format!("Service error: {}", other),
                None,
            )),
        })?;

        // Find the specific tool
        let tool = tools
            .tools
            .iter()
            .find(|tool| tool.name == self.tool_name)
            .ok_or_else(|| McpProviderError::ToolNotFound {
                plugin: self.plugin_name.clone(),
                tool: self.tool_name.clone(),
            })?;

        self.tool_metadata = Some(tool.clone());

        // Build the provider contract from tool metadata
        self.contract = self.build_contract(tool)?;

        Ok(())
    }

    /// Build a provider contract from tool metadata.
    fn build_contract(&self, tool: &Tool) -> Result<ProviderContract, McpProviderError> {
        let mut args = Map::new();

        // Add tool input schema to args
        args.insert(
            "input_schema".to_string(),
            serde_json::to_value(tool.input_schema.as_ref())?,
        );

        // Add tool description
        if let Some(description) = &tool.description {
            args.insert(
                "description".to_string(),
                serde_json::Value::String(description.to_string()),
            );
        }

        Ok(ProviderContract {
            args,
            returns: ProviderReturns {
                fields: vec![ReturnField {
                    name: "result".to_string(),
                    r#type: Some("object".to_string()),
                    tags: vec!["mcp".to_string(), "tool".to_string()],
                }],
            },
        })
    }

    /// Call the MCP tool with the given arguments.
    async fn call_tool(&self, arguments: &Map<String, Value>) -> Result<Value, McpProviderError> {
        // Get the client for the plugin
        let client = self
            .plugin_engine
            .client_manager()
            .get_client(&self.plugin_name)
            .await
            .ok_or_else(|| McpProviderError::PluginNotFound {
                name: self.plugin_name.clone(),
            })?;

        let client = client.lock().await;
        let peer = client.peer().ok_or_else(|| McpProviderError::PluginNotRunning {
            name: self.plugin_name.clone(),
        })?;

        // Prepare the tool call request
        let request = CallToolRequestParam {
            name: self.tool_name.clone().into(),
            arguments: Some(arguments.clone()),
        };

        // Call the tool with timeout
        let result = timeout(Duration::from_secs(30), peer.call_tool(request))
            .await
            .map_err(|_| McpProviderError::TimeoutError {
                operation: "tool_call".to_string(),
            })?;

        let result = result.map_err(|e| match e {
            rmcp::service::ServiceError::McpError(mcp_err) => McpProviderError::McpError(mcp_err),
            other => McpProviderError::McpError(rmcp::ErrorData::invalid_request(
                format!("Service error: {}", other),
                None,
            )),
        })?;

        // Convert the result to JSON
        let result_json = serde_json::to_value(result).map_err(McpProviderError::SerializationError)?;

        Ok(result_json)
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

    #[tokio::test]
    async fn test_mcp_provider_creation() {
        let config = McpConfig::default();
        let plugin_engine = Arc::new(PluginEngine::new(config).unwrap());

        let provider = McpProvider::new("test-plugin", "test-tool", plugin_engine).unwrap();
        assert_eq!(provider.provider_id(), "test-plugin:test-tool");
    }

    #[tokio::test]
    async fn test_mcp_provider_availability() {
        let config = McpConfig::default();
        let plugin_engine = Arc::new(PluginEngine::new(config).unwrap());

        let provider = McpProvider::new("test-plugin", "test-tool", plugin_engine).unwrap();
        assert!(!provider.is_available().await);
    }
}
