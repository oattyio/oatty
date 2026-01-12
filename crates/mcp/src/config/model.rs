//! Data models for MCP configuration.

use indexmap::IndexSet;
use oatty_types::EnvVar;
use oatty_util::InterpolationError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;
use url::Url;

/// MCP configuration containing all configured servers.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpConfig {
    /// Map of server names to server configurations.
    #[serde(rename = "mcpServers")]
    pub mcp_servers: HashMap<String, McpServer>,
}

/// Configuration for a single MCP server.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpServer {
    /// Command to execute for stdio transport (required for stdio).
    pub command: Option<String>,

    /// Arguments to pass to the command.
    pub args: Option<Vec<String>>,

    /// Environment variables to set for the process.
    pub env: IndexSet<EnvVar>,

    /// Working directory for the process.
    pub cwd: Option<PathBuf>,

    /// Base URL for HTTP transport (required for remote servers).
    pub base_url: Option<Url>,

    /// HTTP headers to include in requests.
    pub headers: IndexSet<EnvVar>,

    /// Optional authorization configuration (e.g., Basic credentials).
    pub auth: Option<McpAuthConfig>,

    /// Whether this server is disabled.
    pub disabled: bool,

    /// Optional tags for display/filtering in the UI.
    pub tags: Option<Vec<String>>,

    /// Whether this server is valid.
    pub err: Option<String>,
}

/// Determine the environment variable source, honoring explicitly provided metadata.
impl McpServer {
    /// Check if this server is configured for stdio transport.
    pub fn is_stdio(&self) -> bool {
        self.command.is_some()
    }

    /// Check if this server is configured for HTTP transport.
    pub fn is_http(&self) -> bool {
        self.base_url.is_some()
    }

    /// Check if this server is disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Get the transport type for this server.
    pub fn transport_type(&self) -> TransportType {
        if self.is_stdio() {
            TransportType::Stdio
        } else if self.is_http() {
            TransportType::Http
        } else {
            TransportType::Unknown
        }
    }
}

/// Authorization configuration for MCP servers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpAuthConfig {
    /// Authorization scheme. Currently supports "basic".
    pub scheme: String,
    /// Username (supports interpolation like ${env:NAME} or ${secret:NAME}).
    pub username: Option<String>,
    /// Password (supports interpolation).
    pub password: Option<String>,
    /// Token (supports interpolation). If present without username/password,
    /// constructs Basic auth using "<token>:" as the user:pass pair.
    pub token: Option<String>,
    /// Optional custom header name; defaults to "Authorization" when omitted.
    pub header_name: Option<String>,
    /// Allow interactive prompting on failure.
    pub interactive: Option<bool>,
}

/// Transport type for MCP servers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportType {
    Stdio,
    Http,
    Unknown,
}

impl std::fmt::Display for TransportType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportType::Stdio => write!(f, "stdio"),
            TransportType::Http => write!(f, "http"),
            TransportType::Unknown => write!(f, "unknown"),
        }
    }
}

/// Errors that can occur during configuration operations.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("URL parsing error: {0}")]
    Url(#[from] url::ParseError),

    #[error("Interpolation error: {0}")]
    Interpolation(#[from] InterpolationError),

    #[error("Validation error: {0}")]
    Validation(#[from] crate::config::ValidationError),

    #[error("Configuration error: {message}")]
    Invalid { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_spec_style_config() {
        let json = r#"{
          "mcpServers": {
            "server-name": {
              "command": "node",
              "args": ["-e", "require('@mcp/server').start()"],
              "env": [
                {
                    "key": "FOO",
                    "value": "bar"
                },
                {
                    "key": "OATTY_API_TOKEN",
                    "value": "${env:OATTY_API_TOKEN}"
                }
               ],
              "cwd": "/path/optional",
              "disabled": false,
              "tags": ["code", "gh"]
            },
            "remote-example": {
              "baseUrl": "https://mcp.example.com",
              "headers": {
                "Authorization": "Bearer ${secret:EXAMPLE_TOKEN}"
              },
              "disabled": false
            }
          }
        }"#;

        let cfg: McpConfig = serde_json::from_str(json).expect("config deserializes");
        assert!(cfg.mcp_servers.contains_key("server-name"));
        assert!(cfg.mcp_servers.contains_key("remote-example"));

        let stdio = cfg.mcp_servers.get("server-name").unwrap();
        assert!(stdio.is_stdio());
        assert_eq!(stdio.command.as_deref(), Some("node"));
        assert_eq!(stdio.tags.as_ref().unwrap(), &vec!["code".to_string(), "gh".to_string()]);

        let http = cfg.mcp_servers.get("remote-example").unwrap();
        assert!(http.is_http());
        assert_eq!(http.base_url.as_ref().unwrap().as_str(), "https://mcp.example.com/");
    }

    #[test]
    fn serialize_uses_camel_case_keys() {
        let mut cfg = McpConfig::default();
        let server = McpServer {
            base_url: Some(Url::parse("https://api.example").unwrap()),
            ..Default::default()
        };
        cfg.mcp_servers.insert("svc".to_string(), server);

        let json = serde_json::to_string(&cfg).expect("serialize");
        assert!(json.contains("\"mcpServers\""));
        assert!(json.contains("\"baseUrl\""));
    }
}
