//! Audit logging for MCP plugin lifecycle events.

use chrono::{DateTime, Utc};
use heroku_util::redact_sensitive_with;
use serde::{Deserialize, Serialize};
use tokio::fs::OpenOptions;
use std::path::PathBuf;
use thiserror::Error;
use tracing::debug;
use tokio::io::AsyncWriteExt;

/// Audit logger for tracking plugin lifecycle events.
#[derive(Debug)]
pub struct AuditLogger {
    /// Path to the audit log file.
    log_path: PathBuf,

    /// Maximum size of the audit log file before rotation.
    max_size: u64,

    /// Maximum age of audit log entries before rotation.
    max_age_days: u64,
}

/// An audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Timestamp of the event.
    pub timestamp: DateTime<Utc>,

    /// Plugin name.
    pub plugin_name: String,

    /// Action performed.
    pub action: AuditAction,

    /// Additional metadata about the action.
    pub metadata: serde_json::Map<String, serde_json::Value>,

    /// Result of the action (success, failure, etc.).
    pub result: AuditResult,
}

/// Actions that can be audited.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuditAction {
    /// Plugin was started.
    Start,
    /// Plugin was stopped.
    Stop,
    /// Plugin was restarted.
    Restart,
    /// Plugin configuration was updated.
    ConfigUpdate,
    /// Plugin was installed.
    Install,
    /// Plugin was uninstalled.
    Uninstall,
    /// Plugin was enabled.
    Enable,
    /// Plugin was disabled.
    Disable,
    /// Tool was invoked.
    ToolInvoke,
    /// Health check was performed.
    HealthCheck,
    /// Secret was accessed.
    SecretAccess,
}

/// Result of an audited action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuditResult {
    /// Action succeeded.
    Success,
    /// Action failed.
    Failure,
    /// Action was skipped.
    Skipped,
    /// Action is in progress.
    InProgress,
}

impl AuditLogger {
    /// Create a new audit logger.
    pub fn new() -> anyhow::Result<Self> {
        let log_path = crate::logging::default_audit_log_path();

        // Ensure the directory exists
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        Ok(Self {
            log_path,
            max_size: 10 * 1024 * 1024, // 10MB
            max_age_days: 7,
        })
    }

    /// Create a new audit logger with custom settings.
    pub fn with_settings(log_path: PathBuf, max_size: u64, max_age_days: u64) -> Self {
        Self {
            log_path,
            max_size,
            max_age_days,
        }
    }

    /// Log an audit entry.
    pub async fn log(&self, entry: AuditEntry) -> Result<(), AuditError> {
        // Check if we need to rotate the log file
        if self.should_rotate().await? {
            self.rotate_log().await?;
        }

        // Redact sensitive fields in the audit entry before writing
        let redacted_entry = redact_audit_entry(entry);
        let json_line = serde_json::to_string(&redacted_entry).map_err(|e| AuditError::SerializationError(e.to_string()))?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .await
            .map_err(AuditError::IoError)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o600);
            tokio::fs::set_permissions(&self.log_path, permissions)
                .await
                .map_err(AuditError::IoError)?;
        }

        file.write_all(json_line.as_bytes()).await.map_err(AuditError::IoError)?;
        file.write_all(b"\n").await.map_err(AuditError::IoError)?;

        debug!(
            "Audit log entry: {} {} {}",
            redacted_entry.plugin_name,
            serde_json::to_string(&redacted_entry.action).unwrap_or_default(),
            serde_json::to_string(&redacted_entry.result).unwrap_or_default()
        );

        Ok(())
    }

    /// Check if the log file should be rotated.
    async fn should_rotate(&self) -> Result<bool, AuditError> {
        if !self.log_path.exists() {
            return Ok(false);
        }

        // Check file size
        let metadata = tokio::fs::metadata(&self.log_path).await.map_err(AuditError::IoError)?;

        if metadata.len() > self.max_size {
            return Ok(true);
        }

        // Check file age
        let modified = metadata.modified().map_err(AuditError::IoError)?;

        let age = std::time::SystemTime::now()
            .duration_since(modified)
            .map_err(|e| AuditError::IoError(std::io::Error::other(e)))?;

        if age.as_secs() > self.max_age_days * 24 * 60 * 60 {
            return Ok(true);
        }

        Ok(false)
    }

    /// Rotate the log file.
    async fn rotate_log(&self) -> Result<(), AuditError> {
        if !self.log_path.exists() {
            return Ok(());
        }

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let rotated_path = self.log_path.with_extension(format!("{}.jsonl", timestamp));

        tokio::fs::rename(&self.log_path, &rotated_path)
            .await
            .map_err(AuditError::IoError)?;

        debug!("Rotated audit log: {} -> {}", self.log_path.display(), rotated_path.display());

        Ok(())
    }

    /// Read recent audit entries.
    pub async fn read_recent(&self, count: usize) -> Result<Vec<AuditEntry>, AuditError> {
        if !self.log_path.exists() {
            return Ok(Vec::new());
        }

        let content = tokio::fs::read_to_string(&self.log_path).await.map_err(AuditError::IoError)?;

        let mut entries = Vec::new();

        for line in content.lines().rev().take(count) {
            if let Ok(entry) = serde_json::from_str::<AuditEntry>(line) {
                entries.push(entry);
            }
        }

        // Reverse to get chronological order
        entries.reverse();
        Ok(entries)
    }

    /// Get the path to the audit log file.
    pub fn log_path(&self) -> &PathBuf {
        &self.log_path
    }
}

/// Redact sensitive values in an AuditEntry's metadata and any string fields.
fn redact_audit_entry(mut entry: AuditEntry) -> AuditEntry {
    // Redact metadata string values and known sensitive keys
    let mut redacted = serde_json::Map::new();
    for (k, v) in entry.metadata.into_iter() {
        redacted.insert(k, redact_json_value(v));
    }
    entry.metadata = redacted;
    entry
}

/// Recursively redact strings in JSON values and mask values for sensitive keys.
fn redact_json_value(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::String(s) => serde_json::Value::String(redact_sensitive_with(&s, "[REDACTED]")),
        serde_json::Value::Object(mut map) => {
            let sensitive_keys = [
                "authorization",
                "auth",
                "token",
                "access_token",
                "id_token",
                "secret",
                "password",
                "api_key",
                "apikey",
                "x-api-key",
                "cookie",
                "set-cookie",
            ];
            for (k, val) in map.clone().into_iter() {
                if sensitive_keys.iter().any(|sk| k.eq_ignore_ascii_case(sk)) {
                    map.insert(k, serde_json::Value::String("[REDACTED]".to_string()));
                } else {
                    map.insert(k, redact_json_value(val));
                }
            }
            serde_json::Value::Object(map)
        }
        serde_json::Value::Array(arr) => serde_json::Value::Array(arr.into_iter().map(redact_json_value).collect()),
        other => other,
    }
}

/// Errors that can occur during audit logging.
#[derive(Debug, Error)]
#[allow(clippy::enum_variant_names)]
pub enum AuditError {
    #[error("IO error: {0}")]
    IoError(std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),
}

impl From<std::io::Error> for AuditError {
    fn from(error: std::io::Error) -> Self {
        Self::IoError(error)
    }
}

/// Helper functions for creating common audit entries.
impl AuditEntry {
    /// Create a plugin start audit entry.
    pub fn plugin_start(plugin_name: String, metadata: serde_json::Map<String, serde_json::Value>) -> Self {
        Self {
            timestamp: Utc::now(),
            plugin_name,
            action: AuditAction::Start,
            metadata,
            result: AuditResult::Success,
        }
    }

    /// Create a plugin stop audit entry.
    pub fn plugin_stop(plugin_name: String, metadata: serde_json::Map<String, serde_json::Value>) -> Self {
        Self {
            timestamp: Utc::now(),
            plugin_name,
            action: AuditAction::Stop,
            metadata,
            result: AuditResult::Success,
        }
    }

    /// Create a tool invocation audit entry.
    pub fn tool_invoke(plugin_name: String, tool_name: String, result: AuditResult) -> Self {
        let mut metadata = serde_json::Map::new();
        metadata.insert("tool_name".to_string(), serde_json::Value::String(tool_name));

        Self {
            timestamp: Utc::now(),
            plugin_name,
            action: AuditAction::ToolInvoke,
            metadata,
            result,
        }
    }

    /// Create a health check audit entry.
    pub fn health_check(plugin_name: String, healthy: bool, latency_ms: Option<u64>) -> Self {
        let mut metadata = serde_json::Map::new();
        metadata.insert("healthy".to_string(), serde_json::Value::Bool(healthy));
        if let Some(latency) = latency_ms {
            metadata.insert("latency_ms".to_string(), serde_json::Value::Number(latency.into()));
        }

        Self {
            timestamp: Utc::now(),
            plugin_name,
            action: AuditAction::HealthCheck,
            metadata,
            result: if healthy { AuditResult::Success } else { AuditResult::Failure },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_audit_logger() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("audit.jsonl");

        let logger = AuditLogger::with_settings(log_path.clone(), 1024, 1);

        let entry = AuditEntry::plugin_start("test-plugin".to_string(), serde_json::Map::new());

        logger.log(entry).await.unwrap();

        let entries = logger.read_recent(10).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].plugin_name, "test-plugin");
        assert_eq!(entries[0].action, AuditAction::Start);
    }

    #[test]
    fn test_audit_entry_creation() {
        let entry = AuditEntry::tool_invoke("test-plugin".to_string(), "test-tool".to_string(), AuditResult::Success);

        assert_eq!(entry.plugin_name, "test-plugin");
        assert_eq!(entry.action, AuditAction::ToolInvoke);
        assert_eq!(entry.result, AuditResult::Success);
        assert_eq!(entry.metadata["tool_name"], "test-tool");
    }
}
