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
pub use types::{EnvVar, HealthStatus, LogEntry, PluginDetail, PluginStatus};

/// Re-export commonly used types for convenience
pub use rmcp::{
    ErrorData as McpError,
    model::{CallToolResult, Content, Tool},
    service::{Service, ServiceExt},
};
