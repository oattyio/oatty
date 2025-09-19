//! Model Context Protocol (MCP) plugin infrastructure for Heroku CLI.
//!
//! This crate provides the core infrastructure for managing MCP plugins,
//! including configuration management, client lifecycle, logging, and
//! integration with the existing provider system.

pub mod client;
pub mod config;
pub mod logging;
pub mod plugin;
pub mod provider;
pub mod types;

pub use config::{ConfigError, McpConfig, McpServer};
pub use plugin::{PluginEngine, PluginInfo};
pub use types::{EnvVar, HealthStatus, McpLogEntry, PluginDetail, PluginStatus};

/// Local MCP error type (temporary while migrating to ultrafast-mcp APIs)
#[derive(Debug, thiserror::Error, Clone)]
pub enum McpError {
    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },
    #[error("Internal error: {message}")]
    InternalError { message: String },
}

impl McpError {
    pub fn invalid_request<M: Into<String>>(msg: M, _data: Option<serde_json::Value>) -> Self {
        McpError::InvalidRequest { message: msg.into() }
    }
}
