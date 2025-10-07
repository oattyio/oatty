//! Metadata describing a tool exposed by an MCP plugin.
//!
//! The MCP runtime returns tool descriptions via the `list_tools` RPC. This module converts the
//! `rmcp`-provided model into a serde-friendly representation that downstream components (registry
//! overlay, palette, providers) can use without a direct dependency on `rmcp` internals.

use rmcp::model::Tool as RmcpTool;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Captures the essential metadata for an MCP tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolMetadata {
    /// Tool identifier returned by the MCP server.
    pub name: String,
    /// Optional human-friendly title supplied by the server.
    pub title: Option<String>,
    /// Optional description explaining the tool’s behavior.
    pub description: Option<String>,
    /// JSON schema describing the expected arguments for this tool.
    pub input_schema: Value,
    /// Optional JSON schema describing the structured output produced by the tool.
    pub output_schema: Option<Value>,
    /// Serialized annotations published by the tool (when available).
    pub annotations: Option<Value>,
    /// Optional authentication summary supplied by the CLI (populated during synthesis).
    #[serde(default)]
    pub auth_summary: Option<String>,
}

impl McpToolMetadata {
    /// Build metadata from the raw RMCP tool payload.
    pub fn from_rmcp(tool: &RmcpTool) -> Self {
        let input_schema = Value::Object((tool.input_schema.as_ref()).clone());
        let output_schema = tool.output_schema.as_ref().map(|schema| Value::Object((schema.as_ref()).clone()));
        let annotations = tool.annotations.as_ref().and_then(|ann| serde_json::to_value(ann).ok());

        Self {
            name: tool.name.to_string(),
            title: tool.title.clone(),
            description: tool.description.as_ref().map(|d| d.to_string()),
            input_schema,
            output_schema,
            annotations,
            auth_summary: None,
        }
    }
}

impl From<RmcpTool> for McpToolMetadata {
    fn from(tool: RmcpTool) -> Self {
        Self::from_rmcp(&tool)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{Tool, ToolAnnotations};
    use serde_json::{Map, json};
    use std::sync::Arc;

    #[test]
    fn from_rmcp_copies_core_fields() {
        let mut schema = Map::new();
        schema.insert("type".into(), json!("object"));

        let input_schema = Arc::new(schema.clone());
        let mut tool = Tool::new("demo", "Demo description", input_schema.clone());
        tool.title = Some("Demo".into());
        tool.output_schema = Some(Arc::new(schema));
        tool.annotations = Some(ToolAnnotations::with_title("Demo"));

        let metadata = McpToolMetadata::from_rmcp(&tool);

        assert_eq!(metadata.name, "demo");
        assert_eq!(metadata.title.as_deref(), Some("Demo"));
        assert_eq!(metadata.description.as_deref(), Some("Demo description"));
        assert_eq!(metadata.input_schema["type"], json!("object"));
        assert!(metadata.output_schema.is_some());
        assert!(metadata.annotations.is_some());
        assert!(metadata.auth_summary.is_none());
    }
}
