//! Error types for MCP plugin management.

use thiserror::Error;

/// Main error type for MCP operations.
#[derive(Debug, Error)]
pub enum McpError {
    #[error("Plugin error: {0}")]
    Plugin(#[from] PluginError),

    #[error("Log error: {0}")]
    Log(#[from] LogError),

    #[error("Configuration error: {0}")]
    Config(#[from] crate::config::ConfigError),

    #[error("Transport error: {message}")]
    Transport { message: String },

    #[error("Timeout error: {operation} timed out after {timeout_ms}ms")]
    Timeout { operation: String, timeout_ms: u64 },

    #[error("Handshake error: {message}")]
    Handshake { message: String },

    #[error("Tool invocation error: {tool_name} - {message}")]
    ToolInvocation { tool_name: String, message: String },
}

/// Errors related to plugin lifecycle management.
#[derive(Debug, Error)]
pub enum PluginError {
    #[error("Plugin not found: {name}")]
    NotFound { name: String },

    #[error("Plugin already running: {name}")]
    AlreadyRunning { name: String },

    #[error("Plugin not running: {name}")]
    NotRunning { name: String },

    #[error("Plugin startup failed: {name} - {reason}")]
    StartupFailed { name: String, reason: String },

    #[error("Plugin shutdown failed: {name} - {reason}")]
    ShutdownFailed { name: String, reason: String },

    #[error("Plugin validation failed: {name} - {reason}")]
    ValidationFailed { name: String, reason: String },

    #[error("Plugin configuration error: {name} - {reason}")]
    ConfigurationError { name: String, reason: String },

    #[error("Plugin process error: {name} - {reason}")]
    ProcessError { name: String, reason: String },

    #[error("Plugin communication error: {name} - {reason}")]
    CommunicationError { name: String, reason: String },
}

/// Errors related to logging operations.
#[derive(Debug, Error)]
pub enum LogError {
    #[error("Log buffer full: {plugin_name}")]
    BufferFull { plugin_name: String },

    #[error("Log rotation failed: {reason}")]
    RotationFailed { reason: String },

    #[error("Log export failed: {reason}")]
    ExportFailed { reason: String },

    #[error("Log parsing error: {reason}")]
    ParsingError { reason: String },

    #[error("Log redaction error: {reason}")]
    RedactionError { reason: String },
}

impl McpError {
    /// Create a transport error.
    pub fn transport(message: impl Into<String>) -> Self {
        Self::Transport { message: message.into() }
    }

    /// Create a timeout error.
    pub fn timeout(operation: impl Into<String>, timeout_ms: u64) -> Self {
        Self::Timeout {
            operation: operation.into(),
            timeout_ms,
        }
    }

    /// Create a handshake error.
    pub fn handshake(message: impl Into<String>) -> Self {
        Self::Handshake { message: message.into() }
    }

    /// Create a tool invocation error.
    pub fn tool_invocation(tool_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ToolInvocation {
            tool_name: tool_name.into(),
            message: message.into(),
        }
    }
}

impl PluginError {
    /// Create a plugin not found error.
    pub fn not_found(name: impl Into<String>) -> Self {
        Self::NotFound { name: name.into() }
    }

    /// Create a plugin already running error.
    pub fn already_running(name: impl Into<String>) -> Self {
        Self::AlreadyRunning { name: name.into() }
    }

    /// Create a plugin not running error.
    pub fn not_running(name: impl Into<String>) -> Self {
        Self::NotRunning { name: name.into() }
    }

    /// Create a plugin startup failed error.
    pub fn startup_failed(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::StartupFailed {
            name: name.into(),
            reason: reason.into(),
        }
    }

    /// Create a plugin shutdown failed error.
    pub fn shutdown_failed(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ShutdownFailed {
            name: name.into(),
            reason: reason.into(),
        }
    }

    /// Create a plugin validation failed error.
    pub fn validation_failed(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ValidationFailed {
            name: name.into(),
            reason: reason.into(),
        }
    }

    /// Create a plugin configuration error.
    pub fn configuration_error(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ConfigurationError {
            name: name.into(),
            reason: reason.into(),
        }
    }

    /// Create a plugin process error.
    pub fn process_error(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ProcessError {
            name: name.into(),
            reason: reason.into(),
        }
    }

    /// Create a plugin communication error.
    pub fn communication_error(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::CommunicationError {
            name: name.into(),
            reason: reason.into(),
        }
    }
}

impl LogError {
    /// Create a log buffer full error.
    pub fn buffer_full(plugin_name: impl Into<String>) -> Self {
        Self::BufferFull {
            plugin_name: plugin_name.into(),
        }
    }

    /// Create a log rotation failed error.
    pub fn rotation_failed(reason: impl Into<String>) -> Self {
        Self::RotationFailed { reason: reason.into() }
    }

    /// Create a log export failed error.
    pub fn export_failed(reason: impl Into<String>) -> Self {
        Self::ExportFailed { reason: reason.into() }
    }

    /// Create a log parsing error.
    pub fn parsing_error(reason: impl Into<String>) -> Self {
        Self::ParsingError { reason: reason.into() }
    }

    /// Create a log redaction error.
    pub fn redaction_error(reason: impl Into<String>) -> Self {
        Self::RedactionError { reason: reason.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_error_creation() {
        let err = McpError::transport("Connection failed");
        assert!(matches!(err, McpError::Transport { .. }));

        let err = McpError::timeout("handshake", 5000);
        assert!(matches!(err, McpError::Timeout { .. }));

        let err = McpError::handshake("Invalid protocol version");
        assert!(matches!(err, McpError::Handshake { .. }));
    }

    #[test]
    fn test_plugin_error_creation() {
        let err = PluginError::not_found("test-plugin");
        assert!(matches!(err, PluginError::NotFound { .. }));

        let err = PluginError::startup_failed("test-plugin", "Process crashed");
        assert!(matches!(err, PluginError::StartupFailed { .. }));
    }

    #[test]
    fn test_log_error_creation() {
        let err = LogError::buffer_full("test-plugin");
        assert!(matches!(err, LogError::BufferFull { .. }));

        let err = LogError::rotation_failed("Permission denied");
        assert!(matches!(err, LogError::RotationFailed { .. }));
    }
}
