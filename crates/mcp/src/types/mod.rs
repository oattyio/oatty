//! Core types for MCP plugin management.

pub mod errors;
pub mod tools;

pub use errors::{LogError, McpError, PluginError};
pub use oatty_types::plugin::{
    AuthStatus, EnvSource, EnvVar, HealthStatus, LogLevel, LogSource, McpLogEntry, PluginDetail, PluginStatus, PluginToolSummary,
    TransportStatus,
};
pub use tools::McpToolMetadata;
