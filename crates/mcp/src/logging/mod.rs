//! Logging system for MCP plugins.

mod audit;
mod formatter;
mod ring_buffer;

pub use audit::{AuditAction, AuditEntry, AuditLogger, AuditResult};
pub use formatter::{LogFormatter, RedactionRules};
pub use ring_buffer::LogRingBuffer;

use crate::types::{LogError, McpLogEntry};
use dirs_next::config_dir;
use oatty_util::redact_sensitive_with;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Manager for all plugin logs.
#[derive(Debug)]
pub struct LogManager {
    /// Ring buffers for each plugin.
    buffers: Arc<Mutex<HashMap<String, LogRingBuffer>>>,

    /// Audit logger for tracking plugin lifecycle events.
    audit_logger: AuditLogger,

    /// Log formatter with redaction rules.
    formatter: LogFormatter,

    /// Maximum number of log entries per plugin.
    max_entries_per_plugin: usize,
}

impl LogManager {
    /// Create a new log manager.
    pub fn new() -> anyhow::Result<Self> {
        let audit_logger = AuditLogger::new()?;
        let formatter = LogFormatter::new();

        Ok(Self {
            buffers: Arc::new(Mutex::new(HashMap::new())),
            audit_logger,
            formatter,
            max_entries_per_plugin: 1000,
        })
    }

    /// Add a log entry for a plugin.
    pub async fn add_log(&self, plugin_name: &str, entry: McpLogEntry) -> Result<(), LogError> {
        let mut buffers = self.buffers.lock().await;

        // Get or create the buffer for this plugin
        let buffer = buffers
            .entry(plugin_name.to_string())
            .or_insert_with(|| LogRingBuffer::new(self.max_entries_per_plugin));

        // Add the entry to the buffer
        buffer.add_entry(entry.clone())?;

        // Format for potential external sinks (kept internal only to avoid TUI overlay)
        let _formatted = self.formatter.format(&entry);

        Ok(())
    }

    /// Get recent log entries for a plugin.
    pub async fn get_recent_logs(&self, plugin_name: &str, count: usize) -> Vec<McpLogEntry> {
        let buffers = self.buffers.lock().await;

        if let Some(buffer) = buffers.get(plugin_name) {
            buffer.get_recent(count)
        } else {
            Vec::new()
        }
    }

    /// Get all log entries for a plugin.
    pub async fn get_all_logs(&self, plugin_name: &str) -> Vec<McpLogEntry> {
        let buffers = self.buffers.lock().await;

        if let Some(buffer) = buffers.get(plugin_name) {
            buffer.get_all()
        } else {
            Vec::new()
        }
    }

    /// Clear logs for a plugin.
    pub async fn clear_logs(&self, plugin_name: &str) {
        let mut buffers = self.buffers.lock().await;
        if let Some(buffer) = buffers.get_mut(plugin_name) {
            buffer.clear();
        }
    }

    /// Export logs for a plugin to a file (redacted by default).
    pub async fn export_logs(&self, plugin_name: &str, path: &std::path::Path) -> Result<(), LogError> {
        self.export_logs_with_redaction(plugin_name, path, true).await
    }

    /// Export logs with optional redaction.
    pub async fn export_logs_with_redaction(&self, plugin_name: &str, path: &std::path::Path, redact: bool) -> Result<(), LogError> {
        let logs = self.get_all_logs(plugin_name).await;

        let mut content = String::new();
        for log in logs {
            let line = if redact {
                self.formatter.format(&log)
            } else {
                self.formatter.format_for_export(&log)
            };
            content.push_str(&line);
            content.push('\n');
        }

        tokio::fs::write(path, content)
            .await
            .map_err(|e| LogError::export_failed(e.to_string()))?;

        Ok(())
    }

    /// Log an audit event.
    pub async fn log_audit(&self, entry: AuditEntry) -> Result<(), LogError> {
        self.audit_logger
            .log(entry)
            .await
            .map_err(|e| LogError::export_failed(e.to_string()))
    }

    /// Get the audit logger.
    pub fn audit_logger(&self) -> &AuditLogger {
        &self.audit_logger
    }

    /// Get the log formatter.
    pub fn formatter(&self) -> &LogFormatter {
        &self.formatter
    }
}

impl Default for LogManager {
    fn default() -> Self {
        Self::new().expect("Failed to create log manager")
    }
}

/// Get the default path for audit logs.
pub fn default_audit_log_path() -> PathBuf {
    config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("heroku")
        .join("mcp-audit.jsonl")
}

/// Sanitize text for safe logging by redacting sensitive substrings.
pub fn sanitize_log_text(text: &str) -> String {
    redact_sensitive_with(text, "[REDACTED]")
}

#[cfg(test)]
mod tests {
    use crate::types::{LogLevel, LogSource};

    use super::*;

    #[tokio::test]
    async fn test_log_manager() {
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
    async fn test_export_logs_with_and_without_redaction() {
        let manager = LogManager::new().unwrap();

        let secret_msg = "API key: abc123def456".to_string();
        let entry = McpLogEntry::new(LogLevel::Info, secret_msg.clone(), LogSource::System, "p".to_string());
        manager.add_log("p", entry).await.unwrap();

        // Paths
        let mut redacted_path = std::env::temp_dir();
        redacted_path.push("mcp_log_redacted.txt");
        let mut raw_path = std::env::temp_dir();
        raw_path.push("mcp_log_raw.txt");

        manager.export_logs_with_redaction("p", &redacted_path, true).await.unwrap();
        manager.export_logs_with_redaction("p", &raw_path, false).await.unwrap();

        let redacted = tokio::fs::read_to_string(&redacted_path).await.unwrap();
        let raw = tokio::fs::read_to_string(&raw_path).await.unwrap();

        assert!(redacted.contains("[REDACTED]"));
        assert!(raw.contains(&secret_msg));

        // Cleanup best-effort
        let _ = tokio::fs::remove_file(redacted_path).await;
        let _ = tokio::fs::remove_file(raw_path).await;
    }
}
