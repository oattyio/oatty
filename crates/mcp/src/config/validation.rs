//! Configuration validation for MCP servers.

use crate::config::model::TransportType;
use crate::config::{McpConfig, McpServer};
use heroku_types::EnvVar;
use once_cell::sync::Lazy;
use regex::Regex;
use thiserror::Error;
use tracing::debug;

/// Validate the entire MCP configuration.
pub fn validate_config(config: &McpConfig) -> Result<(), ValidationError> {
    for (name, server) in &config.mcp_servers {
        validate_server_name(name)?;
        validate_server(server)?;
        debug!("Validated server configuration: {}", name);
    }
    Ok(())
}

/// Validate a server name.
pub fn validate_server_name(name: &str) -> Result<(), ValidationError> {
    let name_regex = Regex::new(r"^[a-z0-9._-]+$")?;

    if !name_regex.is_match(name) {
        return Err(ValidationError::InvalidServerName {
            name: name.to_string(),
            reason: "Server name must contain only lowercase letters, numbers, dots, underscores, and hyphens".to_string(),
        });
    }

    if name.is_empty() {
        return Err(ValidationError::InvalidServerName {
            name: name.to_string(),
            reason: "Server name cannot be empty".to_string(),
        });
    }

    Ok(())
}

/// Validate a single server configuration.
pub fn validate_server(server: &McpServer) -> Result<(), ValidationError> {
    match server.transport_type() {
        TransportType::Stdio => validate_stdio_server(server),
        TransportType::Http => validate_http_server(server),
        TransportType::Unknown => Err(ValidationError::InvalidTransport {
            reason: "Server must have either 'command' (stdio) or 'baseUrl' (http)".to_string(),
        }),
    }
}

/// Validate a stdio server configuration.
fn validate_stdio_server(server: &McpServer) -> Result<(), ValidationError> {
    // Command is required for stdio
    if server.command.is_none() {
        return Err(ValidationError::MissingRequiredField {
            field: "command".to_string(),
            transport: "stdio".to_string(),
        });
    }

    // Validate environment variable names
    if let Some(env) = &server.env {
        for EnvVar { key, .. } in env {
            validate_env_key(key)?;
        }
    }

    Ok(())
}

/// Validate an HTTP server configuration.
fn validate_http_server(server: &McpServer) -> Result<(), ValidationError> {
    // Base URL is required for HTTP
    if server.base_url.is_none() {
        return Err(ValidationError::MissingRequiredField {
            field: "baseUrl".to_string(),
            transport: "http".to_string(),
        });
    }

    // Ensure scheme is http or https
    if let Some(url) = &server.base_url {
        let scheme = url.scheme();
        if scheme != "http" && scheme != "https" {
            return Err(ValidationError::InvalidTransport {
                reason: format!("Unsupported URL scheme: {} (expected http/https)", scheme),
            });
        }
    }

    // Validate HTTP headers
    if let Some(headers) = &server.headers {
        for EnvVar { key, .. } in headers {
            validate_header_name(key)?;
        }
    }

    Ok(())
}

/// Validate an environment variable key.
static ENV_KEY_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[A-Z_][A-Z0-9_]*$").expect("env key regex should compile"));

fn validate_env_key(key: &str) -> Result<(), ValidationError> {
    if !ENV_KEY_REGEX.is_match(key) {
        return Err(ValidationError::InvalidEnvKey {
            key: key.to_string(),
            reason: "Environment variable keys must start with uppercase letter or underscore, followed by uppercase letters, numbers, or underscores".to_string(),
        });
    }

    Ok(())
}

/// Validate an HTTP header name.
fn validate_header_name(name: &str) -> Result<(), ValidationError> {
    if name.is_empty() {
        return Err(ValidationError::InvalidHeaderName {
            name: name.to_string(),
            reason: "Header name cannot be empty".to_string(),
        });
    }

    // Basic validation - header names should not contain control characters
    if name.chars().any(|c| c.is_control()) {
        return Err(ValidationError::InvalidHeaderName {
            name: name.to_string(),
            reason: "Header name cannot contain control characters".to_string(),
        });
    }

    Ok(())
}

/// Errors that can occur during validation.
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Invalid server name '{name}': {reason}")]
    InvalidServerName { name: String, reason: String },

    #[error("Invalid transport configuration: {reason}")]
    InvalidTransport { reason: String },

    #[error("Missing required field '{field}' for {transport} transport")]
    MissingRequiredField { field: String, transport: String },

    #[error("Invalid environment variable key '{key}': {reason}")]
    InvalidEnvKey { key: String, reason: String },

    #[error("Invalid HTTP header name '{name}': {reason}")]
    InvalidHeaderName { name: String, reason: String },

    #[error("Regex compilation error: {0}")]
    Regex(#[from] regex::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::McpServer;
    use url::Url;

    #[test]
    fn test_validate_server_name_valid() {
        assert!(validate_server_name("github").is_ok());
        assert!(validate_server_name("my-server").is_ok());
        assert!(validate_server_name("server_1").is_ok());
        assert!(validate_server_name("test.server").is_ok());
    }

    #[test]
    fn test_validate_server_name_invalid() {
        assert!(validate_server_name("").is_err());
        assert!(validate_server_name("GitHub").is_err()); // uppercase
        assert!(validate_server_name("my server").is_err()); // space
        assert!(validate_server_name("server@example").is_err()); // special char
    }

    #[test]
    fn test_validate_stdio_server() {
        let server = McpServer {
            command: Some("node".to_string()),
            args: Some(vec!["-e".to_string(), "console.log('hello')".to_string()]),
            ..Default::default()
        };

        assert!(validate_server(&server).is_ok());
    }

    #[test]
    fn test_validate_stdio_server_missing_command() {
        let server = McpServer::default();
        assert!(validate_server(&server).is_err());
    }

    #[test]
    fn test_validate_http_server() {
        let server = McpServer {
            base_url: Some(Url::parse("https://example.com").unwrap()),
            ..Default::default()
        };

        assert!(validate_server(&server).is_ok());
    }

    #[test]
    fn test_validate_http_server_rejects_non_http() {
        let server = McpServer {
            base_url: Some(Url::parse("ws://example.com").unwrap()),
            ..Default::default()
        };

        assert!(validate_server(&server).is_err());
    }

    #[test]
    fn test_validate_env_key() {
        assert!(validate_env_key("GITHUB_TOKEN").is_ok());
        assert!(validate_env_key("_PRIVATE_KEY").is_ok());
        assert!(validate_env_key("API_KEY_123").is_ok());

        assert!(validate_env_key("github_token").is_err()); // lowercase
        assert!(validate_env_key("123API_KEY").is_err()); // starts with number
        assert!(validate_env_key("API-KEY").is_err()); // hyphen
    }
}
