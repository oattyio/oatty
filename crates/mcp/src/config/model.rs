//! Data models for MCP configuration.

use heroku_types::{EnvSource, EnvVar};
use serde::de::Deserializer;
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpServer {
    /// Command to execute for stdio transport (required for stdio).
    pub command: Option<String>,

    /// Arguments to pass to the command.
    pub args: Option<Vec<String>>,

    /// Environment variables to set for the process.
    #[serde(
        default,
        deserialize_with = "deserialize_environment_variables",
        skip_serializing_if = "Option::is_none"
    )]
    pub env: Option<Vec<EnvVar>>,

    /// Working directory for the process.
    pub cwd: Option<PathBuf>,

    /// Base URL for HTTP transport (required for remote servers).
    pub base_url: Option<Url>,

    /// HTTP headers to include in requests.
    #[serde(
        default,
        deserialize_with = "deserialize_environment_variables",
        skip_serializing_if = "Option::is_none"
    )]
    pub headers: Option<Vec<EnvVar>>,

    /// Optional authorization configuration (e.g., Basic credentials).
    pub auth: Option<McpAuthConfig>,

    /// Whether this server is disabled.
    pub disabled: Option<bool>,

    /// Optional tags for display/filtering in the UI.
    pub tags: Option<Vec<String>>,

    /// Whether this server is valid.
    pub err: Option<String>,
}

/// Default `effective` flag for configuration entries.
fn default_effective_flag() -> bool {
    true
}

/// Deserialize environment variables supporting both list and map formats.
fn deserialize_environment_variables<'de, D>(deserializer: D) -> Result<Option<Vec<EnvVar>>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw_collection = Option::<RawEnvironmentVariableCollection>::deserialize(deserializer)?;
    Ok(raw_collection.map(|collection| collection.into_environment_variables()))
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawEnvironmentVariableCollection {
    List(Vec<RawEnvironmentVariable>),
    Map(HashMap<String, RawEnvironmentVariableValue>),
}

impl RawEnvironmentVariableCollection {
    fn into_environment_variables(self) -> Vec<EnvVar> {
        match self {
            RawEnvironmentVariableCollection::List(list) => {
                list.into_iter().map(RawEnvironmentVariable::into_environment_variable).collect()
            }
            RawEnvironmentVariableCollection::Map(map) => {
                let mut variables: Vec<EnvVar> = map.into_iter().map(|(key, value)| value.into_environment_variable(key)).collect();
                variables.sort_by(|a, b| a.key.cmp(&b.key));
                variables
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawEnvironmentVariable {
    key: String,
    value: String,
    #[serde(default)]
    source: Option<EnvSource>,
    #[serde(default)]
    effective: Option<bool>,
}

impl RawEnvironmentVariable {
    fn into_environment_variable(self) -> EnvVar {
        let RawEnvironmentVariable {
            key,
            value,
            source,
            effective,
        } = self;

        let environment_source = compute_environment_source(source, &value);
        EnvVar {
            key,
            value,
            source: environment_source,
            effective: effective.unwrap_or_else(default_effective_flag),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawEnvironmentVariableValue {
    Simple(String),
    Detailed(RawEnvironmentVariableDetail),
}

impl RawEnvironmentVariableValue {
    fn into_environment_variable(self, key: String) -> EnvVar {
        match self {
            RawEnvironmentVariableValue::Simple(value) => {
                let environment_source = compute_environment_source(None, &value);
                EnvVar {
                    key,
                    value,
                    source: environment_source,
                    effective: default_effective_flag(),
                }
            }
            RawEnvironmentVariableValue::Detailed(detail) => detail.into_environment_variable(key),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawEnvironmentVariableDetail {
    value: String,
    #[serde(default)]
    source: Option<EnvSource>,
    #[serde(default)]
    effective: Option<bool>,
}

impl RawEnvironmentVariableDetail {
    fn into_environment_variable(self, key: String) -> EnvVar {
        let environment_source = compute_environment_source(self.source, &self.value);
        EnvVar {
            key,
            value: self.value,
            source: environment_source,
            effective: self.effective.unwrap_or_else(default_effective_flag),
        }
    }
}

/// Determine the environment variable source, honoring explicitly provided metadata.
fn compute_environment_source(provided_source: Option<EnvSource>, value: &str) -> EnvSource {
    if let Some(source) = provided_source {
        return source;
    }

    super::interpolation::determine_env_source(value)
}

impl Default for McpServer {
    fn default() -> Self {
        Self {
            command: None,
            args: None,
            env: None,
            cwd: None,
            base_url: None,
            headers: None,
            auth: None,
            disabled: Some(false),
            tags: None,
            err: None,
        }
    }
}

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
        self.disabled.unwrap_or(false)
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
    Interpolation(#[from] crate::config::InterpolationError),

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
                    "key": "HEROKU_API_TOKEN",
                    "value": "${env:HEROKU_API_TOKEN}"
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
