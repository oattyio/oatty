//! Core log manager behavior for MCP plugin logging.

use crate::logging::{AuditEntry, AuditLogger, LogFormatter, LogRingBuffer};
use crate::types::{LogError, McpLogEntry};
use dirs_next::config_dir;
use oatty_util::redact_sensitive_with;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

const DEFAULT_MAX_LOG_ENTRIES_PER_PLUGIN: usize = 1000;

/// Stores and manages logs for all active plugins.
#[derive(Debug)]
pub struct LogManager {
    /// Ring buffers keyed by plugin name.
    buffers: Arc<Mutex<HashMap<String, LogRingBuffer>>>,
    /// Audit logger for plugin lifecycle events.
    audit_logger: AuditLogger,
    /// Formatter used for serialization and redaction.
    formatter: LogFormatter,
    /// Maximum number of buffered entries per plugin.
    max_entries_per_plugin: usize,
}

impl LogManager {
    /// Constructs a new log manager with default buffer sizing and formatter rules.
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            buffers: Arc::new(Mutex::new(HashMap::new())),
            audit_logger: AuditLogger::new()?,
            formatter: LogFormatter::new(),
            max_entries_per_plugin: DEFAULT_MAX_LOG_ENTRIES_PER_PLUGIN,
        })
    }

    /// Adds a log entry to the plugin-specific ring buffer.
    pub async fn add_log(&self, plugin_name: &str, entry: McpLogEntry) -> Result<(), LogError> {
        let mut buffers = self.buffers.lock().await;
        let buffer = buffers
            .entry(plugin_name.to_string())
            .or_insert_with(|| LogRingBuffer::new(self.max_entries_per_plugin));
        buffer.add_entry(entry.clone())?;

        // Keep formatting path warm for export sinks without showing it in UI.
        let _formatted = self.formatter.format(&entry);
        Ok(())
    }

    /// Returns up to `count` most recent log entries for `plugin_name`.
    pub async fn get_recent_logs(&self, plugin_name: &str, count: usize) -> Vec<McpLogEntry> {
        let buffers = self.buffers.lock().await;
        buffers.get(plugin_name).map_or_else(Vec::new, |buffer| buffer.get_recent(count))
    }

    /// Returns all buffered log entries for `plugin_name`.
    pub async fn get_all_logs(&self, plugin_name: &str) -> Vec<McpLogEntry> {
        let buffers = self.buffers.lock().await;
        buffers.get(plugin_name).map_or_else(Vec::new, LogRingBuffer::get_all)
    }

    /// Clears all buffered entries for `plugin_name`.
    pub async fn clear_logs(&self, plugin_name: &str) {
        let mut buffers = self.buffers.lock().await;
        if let Some(buffer) = buffers.get_mut(plugin_name) {
            buffer.clear();
        }
    }

    /// Exports all logs for `plugin_name` with default redaction.
    pub async fn export_logs(&self, plugin_name: &str, path: &Path) -> Result<(), LogError> {
        self.export_logs_with_redaction(plugin_name, path, true).await
    }

    /// Exports all logs for `plugin_name` with optional redaction.
    pub async fn export_logs_with_redaction(&self, plugin_name: &str, path: &Path, redact: bool) -> Result<(), LogError> {
        let logs = self.get_all_logs(plugin_name).await;
        let content = self.build_export_content(logs, redact);
        tokio::fs::write(path, content)
            .await
            .map_err(|error| LogError::export_failed(error.to_string()))?;
        Ok(())
    }

    fn build_export_content(&self, logs: Vec<McpLogEntry>, redact: bool) -> String {
        let mut content = String::new();
        for log_entry in logs {
            let line = if redact {
                self.formatter.format(&log_entry)
            } else {
                self.formatter.format_for_export(&log_entry)
            };
            content.push_str(&line);
            content.push('\n');
        }
        content
    }

    /// Writes an audit log entry.
    pub async fn log_audit(&self, entry: AuditEntry) -> Result<(), LogError> {
        self.audit_logger
            .log(entry)
            .await
            .map_err(|error| LogError::export_failed(error.to_string()))
    }

    /// Returns the underlying audit logger.
    pub fn audit_logger(&self) -> &AuditLogger {
        &self.audit_logger
    }

    /// Returns the formatter used by this manager.
    pub fn formatter(&self) -> &LogFormatter {
        &self.formatter
    }
}

impl Default for LogManager {
    fn default() -> Self {
        Self::new().expect("Failed to create log manager")
    }
}

/// Returns the default path for MCP audit logs.
pub fn default_audit_log_path() -> PathBuf {
    config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("oatty")
        .join("mcp-audit.jsonl")
}

/// Redacts sensitive text for safe log display and export.
pub fn sanitize_log_text(text: &str) -> String {
    redact_sensitive_with(text, "[REDACTED]")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{LogLevel, LogSource};

    #[tokio::test]
    async fn log_manager_stores_recent_entries() {
        let manager = LogManager::new().unwrap();
        let entry = McpLogEntry::new(
            LogLevel::Info,
            "Test message".to_string(),
            LogSource::System,
            "test-plugin".to_string(),
        );

        manager.add_log("test-plugin", entry).await.unwrap();

        let logs = manager.get_recent_logs("test-plugin", 10).await;
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].message, "Test message");
    }

    #[tokio::test]
    async fn export_logs_respects_redaction_flag() {
        let manager = LogManager::new().unwrap();
        let secret_message = "API key: abc123def456".to_string();
        let entry = McpLogEntry::new(LogLevel::Info, secret_message.clone(), LogSource::System, "plugin".to_string());
        manager.add_log("plugin", entry).await.unwrap();

        let mut redacted_path = std::env::temp_dir();
        redacted_path.push("mcp_log_redacted.txt");
        let mut raw_path = std::env::temp_dir();
        raw_path.push("mcp_log_raw.txt");

        manager.export_logs_with_redaction("plugin", &redacted_path, true).await.unwrap();
        manager.export_logs_with_redaction("plugin", &raw_path, false).await.unwrap();

        let redacted = tokio::fs::read_to_string(&redacted_path).await.unwrap();
        let raw = tokio::fs::read_to_string(&raw_path).await.unwrap();

        assert!(redacted.contains("[REDACTED]"));
        assert!(raw.contains(&secret_message));

        let _ = tokio::fs::remove_file(redacted_path).await;
        let _ = tokio::fs::remove_file(raw_path).await;
    }
}
