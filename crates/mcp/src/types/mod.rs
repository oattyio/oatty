//! Core types for MCP plugin management.

pub mod errors;
pub mod tools;

pub use errors::{LogError, McpError, PluginError};
pub use oatty_types::plugin::{
    AuthStatus, EnvSource, EnvVar, HealthStatus, LogLevel, LogSource, McpLogEntry, PluginDetail, PluginStatus, PluginToolSummary,
    TransportStatus,
};
pub use tools::McpToolMetadata;

/// Outcome of applying a configuration update across multiple servers.
#[derive(Debug)]
pub(crate) struct McpConfigApplyOutcome {
    pub(crate) applied_servers: Vec<McpServerApplyOutcome>,
}

/// Per-server outcome from a configuration update.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct McpServerApplyOutcome {
    pub(crate) name: String,
    pub(crate) status: crate::config::ServerStatus,
    pub(crate) runtime_applied: bool,
    pub(crate) connection_attempted: bool,
    pub(crate) connection_error: Option<crate::plugin::PluginEngineError>,
}
