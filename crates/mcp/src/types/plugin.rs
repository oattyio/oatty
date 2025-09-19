//! Plugin-related data structures.

use crate::types::{HealthStatus, PluginStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Authentication status for a plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthStatus {
    /// Authentication status is unknown (not yet checked).
    Unknown,
    /// Plugin is successfully authenticated.
    Authorized,
    /// Authentication is required but not provided.
    Required,
    /// Authentication failed with an error.
    Failed,
}

impl std::fmt::Display for AuthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthStatus::Unknown => write!(f, "Unknown"),
            AuthStatus::Authorized => write!(f, "Authorized"),
            AuthStatus::Required => write!(f, "Required"),
            AuthStatus::Failed => write!(f, "Failed"),
        }
    }
}

impl Default for AuthStatus {
    fn default() -> Self {
        AuthStatus::Unknown
    }
}

/// Detailed information about a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDetail {
    /// Plugin name.
    pub name: String,

    /// Current status of the plugin.
    pub status: PluginStatus,

    /// Command or base URL for the plugin.
    pub command_or_url: String,

    /// Environment variables for the plugin.
    pub env: Vec<EnvVar>,

    /// Recent logs from the plugin.
    pub logs: Vec<McpLogEntry>,

    /// Health metrics for the plugin.
    pub health: HealthStatus,

    /// Tags associated with the plugin.
    pub tags: Vec<String>,

    /// Last start time.
    pub last_start: Option<DateTime<Utc>>,

    /// Handshake latency in milliseconds.
    pub handshake_latency: Option<u64>,

    /// Authentication status for the plugin.
    pub auth_status: AuthStatus,
}

impl PluginDetail {
    /// Create a new plugin detail with default values.
    pub fn new(name: String, command_or_url: String) -> Self {
        Self {
            name,
            status: PluginStatus::Stopped,
            command_or_url,
            env: Vec::new(),
            logs: Vec::new(),
            health: HealthStatus::default(),
            tags: Vec::new(),
            last_start: None,
            handshake_latency: None,
            auth_status: AuthStatus::default(),
        }
    }

    /// Add a log entry to the plugin.
    pub fn add_log(&mut self, entry: McpLogEntry) {
        self.logs.push(entry);
        // Keep only the last 1000 log entries
        if self.logs.len() > 1000 {
            self.logs.remove(0);
        }
    }

    /// Get the most recent log entries.
    pub fn recent_logs(&self, count: usize) -> Vec<&McpLogEntry> {
        self.logs.iter().rev().take(count).collect()
    }

    /// Check if the plugin is running.
    pub fn is_running(&self) -> bool {
        matches!(self.status, PluginStatus::Running)
    }

    /// Check if the plugin is healthy.
    pub fn is_healthy(&self) -> bool {
        self.is_running() && self.health.is_healthy()
    }
}

/// Environment variable for a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    /// Environment variable key.
    pub key: String,

    /// Environment variable value (masked for secrets).
    pub value: String,

    /// Source of the environment variable.
    pub source: EnvSource,

    /// Whether the value is effectively resolved.
    pub effective: bool,
}

impl EnvVar {
    /// Create a new environment variable.
    pub fn new(key: String, value: String, source: EnvSource) -> Self {
        Self {
            key,
            value,
            source,
            effective: true,
        }
    }

    /// Create a masked version of the environment variable for display.
    pub fn masked(&self) -> Self {
        let masked_value = if self.is_secret() {
            "••••••••••••••••".to_string()
        } else {
            self.value.clone()
        };

        Self {
            key: self.key.clone(),
            value: masked_value,
            source: self.source.clone(),
            effective: self.effective,
        }
    }

    /// Check if this environment variable contains a secret.
    pub fn is_secret(&self) -> bool {
        matches!(self.source, EnvSource::Secret)
    }
}

/// Source of an environment variable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EnvSource {
    /// From the configuration file.
    File,
    /// From a secret in the keychain.
    Secret,
    /// From the process environment.
    Env,
}

impl std::fmt::Display for EnvSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnvSource::File => write!(f, "file"),
            EnvSource::Secret => write!(f, "secret"),
            EnvSource::Env => write!(f, "env"),
        }
    }
}

/// A log entry from a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpLogEntry {
    /// Timestamp of the log entry.
    pub timestamp: DateTime<Utc>,

    /// Log level.
    pub level: LogLevel,

    /// Log message.
    pub message: String,

    /// Source of the log (stdout, stderr, or system).
    pub source: LogSource,

    /// Plugin name that generated this log.
    pub plugin_name: String,
}

impl McpLogEntry {
    /// Create a new log entry.
    pub fn new(level: LogLevel, message: String, source: LogSource, plugin_name: String) -> Self {
        Self {
            timestamp: Utc::now(),
            level,
            message,
            source,
            plugin_name,
        }
    }

    /// Create a system log entry.
    pub fn system(message: String, plugin_name: String) -> Self {
        Self::new(LogLevel::Info, message, LogSource::System, plugin_name)
    }

    /// Create an error log entry.
    pub fn error(message: String, source: LogSource, plugin_name: String) -> Self {
        Self::new(LogLevel::Error, message, source, plugin_name)
    }

    /// Format the log entry for display.
    pub fn format(&self) -> String {
        format!(
            "[{}] {} {}: {}",
            self.timestamp.format("%H:%M:%S"),
            self.level,
            self.source,
            self.message
        )
    }
}

/// Log level for plugin logs.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "err"),
        }
    }
}

/// Source of a log entry.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LogSource {
    /// Standard output from the plugin.
    Stdout,
    /// Standard error from the plugin.
    Stderr,
    /// System-generated log.
    System,
}

impl std::fmt::Display for LogSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogSource::Stdout => write!(f, "stdout"),
            LogSource::Stderr => write!(f, "stderr"),
            LogSource::System => write!(f, "system"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_detail_creation() {
        let plugin = PluginDetail::new("test".to_string(), "node test.js".to_string());
        assert_eq!(plugin.name, "test");
        assert_eq!(plugin.status, PluginStatus::Stopped);
        assert_eq!(plugin.command_or_url, "node test.js");
        assert!(plugin.logs.is_empty());
    }

    #[test]
    fn test_env_var_masking() {
        let env_var = EnvVar::new("GITHUB_TOKEN".to_string(), "secret123".to_string(), EnvSource::Secret);

        let masked = env_var.masked();
        assert_eq!(masked.value, "••••••••••••••••");
        assert!(masked.is_secret());
    }

    #[test]
    fn test_log_entry_formatting() {
        let log = McpLogEntry::new(
            LogLevel::Info,
            "Plugin started".to_string(),
            LogSource::System,
            "test".to_string(),
        );

        let formatted = log.format();
        assert!(formatted.contains("info"));
        assert!(formatted.contains("system"));
        assert!(formatted.contains("Plugin started"));
    }
}
