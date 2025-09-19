//! Core types for MCP plugin management.

pub mod errors;
pub mod plugin;
pub mod status;

pub use errors::{LogError, McpError, PluginError};
pub use plugin::{EnvVar, McpLogEntry, PluginDetail};
pub use status::{HealthStatus, PluginStatus, TransportStatus};
