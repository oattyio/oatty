//! Core types for MCP plugin management.

pub mod errors;

pub use errors::{LogError, McpError, PluginError};
pub use heroku_types::plugin::{
    AuthStatus, EnvSource, EnvVar, HealthStatus, LogLevel, LogSource, McpLogEntry, PluginDetail, PluginStatus, TransportStatus,
};
