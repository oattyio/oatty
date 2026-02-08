//! Persistent log writer for TUI log entries.
//!
//! This module stores log entries as newline-delimited JSON (JSONL) so
//! diagnostics tooling can consume logs independently of the in-memory TUI
//! state.

use std::env;
use std::fs::{File, OpenOptions, create_dir_all};
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use chrono::{SecondsFormat, Utc};
use oatty_util::{redact_json, redact_sensitive};
use serde_json::json;

use crate::ui::components::logs::state::LogEntry;

/// Environment variable used to override the persistent TUI log file path.
pub const TUI_LOG_PATH_ENV: &str = "OATTY_TUI_LOG_PATH";
/// Environment variable used to override the max on-disk size before rotating.
pub const TUI_LOG_MAX_BYTES_ENV: &str = "OATTY_TUI_LOG_MAX_BYTES";
/// Environment variable used to override how many rotated files are retained.
pub const TUI_LOG_MAX_FILES_ENV: &str = "OATTY_TUI_LOG_MAX_FILES";

const DEFAULT_TUI_LOG_MAX_BYTES: u64 = 10 * 1024 * 1024;
const DEFAULT_TUI_LOG_MAX_FILES: usize = 5;
const DEFAULT_QUEUE_CAPACITY: usize = 2048;
const FLUSH_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy)]
struct RotationSettings {
    max_bytes: u64,
    max_files: usize,
}

#[derive(Debug, Clone)]
pub struct PersistentLogConfig {
    pub path: PathBuf,
    rotation_settings: RotationSettings,
}

/// Background log worker handle used by the UI thread.
#[derive(Debug)]
pub struct PersistentLogWorker {
    sender: SyncSender<LogEntry>,
    join_handle: Option<JoinHandle<()>>,
    path: PathBuf,
}

impl PersistentLogWorker {
    pub fn from_environment() -> anyhow::Result<Self> {
        let config = PersistentLogConfig::from_environment();
        Self::start(config)
    }

    pub fn start(config: PersistentLogConfig) -> anyhow::Result<Self> {
        let writer = PersistentLogWriter::new(config.path.clone(), config.rotation_settings)?;
        let path = writer.path().clone();
        let (sender, receiver) = sync_channel(DEFAULT_QUEUE_CAPACITY);
        let join_handle = std::thread::Builder::new()
            .name("oatty-tui-log-writer".to_string())
            .spawn(move || run_log_writer_loop(writer, receiver))
            .map_err(|error| anyhow::anyhow!("failed to spawn log writer thread: {error}"))?;

        Ok(Self {
            sender,
            join_handle: Some(join_handle),
            path,
        })
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn enqueue_entry(&self, entry: &LogEntry) -> anyhow::Result<bool> {
        match self.sender.try_send(entry.clone()) {
            Ok(()) => Ok(true),
            Err(TrySendError::Full(_)) => Ok(false),
            Err(TrySendError::Disconnected(_)) => Err(anyhow::anyhow!("persistent log writer channel disconnected")),
        }
    }
}

impl Drop for PersistentLogWorker {
    fn drop(&mut self) {
        let (replacement_sender, _replacement_receiver) = sync_channel(1);
        let sender = std::mem::replace(&mut self.sender, replacement_sender);
        drop(sender);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

impl PersistentLogConfig {
    fn from_environment() -> Self {
        Self {
            path: resolve_log_path(),
            rotation_settings: resolve_rotation_settings(),
        }
    }
}

#[derive(Debug)]
struct PersistentLogWriter {
    file: File,
    path: PathBuf,
    rotation_settings: RotationSettings,
}

impl PersistentLogWriter {
    fn new(path: PathBuf, rotation_settings: RotationSettings) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self {
            file,
            path,
            rotation_settings,
        })
    }

    fn path(&self) -> &PathBuf {
        &self.path
    }

    fn append_entry(&mut self, entry: &LogEntry) -> anyhow::Result<()> {
        self.rotate_if_needed()?;
        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let record = match entry {
            LogEntry::Text { level, msg } => json!({
                "timestamp": timestamp,
                "source": "tui",
                "entry_type": "text",
                "level": level.as_ref().map(|value| value.to_string()).unwrap_or_else(|| "info".to_string()),
                "message": redact_sensitive(msg),
            }),
            LogEntry::Api { status, raw, json } => json!({
                "timestamp": timestamp,
                "source": "api",
                "entry_type": "http",
                "status": *status,
                "message": redact_sensitive(raw),
                "payload": json.as_ref().map(redact_json),
            }),
            LogEntry::Mcp { raw, json } => json!({
                "timestamp": timestamp,
                "source": "mcp",
                "entry_type": "mcp",
                "message": redact_sensitive(raw),
                "payload": json.as_ref().map(redact_json),
            }),
        };
        serde_json::to_writer(&mut self.file, &record)?;
        self.file.write_all(b"\n")?;
        Ok(())
    }

    fn flush(&mut self) -> anyhow::Result<()> {
        self.file.flush()?;
        Ok(())
    }

    fn rotate_if_needed(&mut self) -> anyhow::Result<()> {
        let current_size = self.file.metadata().map(|metadata| metadata.len()).unwrap_or_default();
        if current_size < self.rotation_settings.max_bytes {
            return Ok(());
        }

        self.rotate_files()?;
        self.file = OpenOptions::new().create(true).append(true).open(&self.path)?;
        Ok(())
    }

    fn rotate_files(&self) -> anyhow::Result<()> {
        if self.rotation_settings.max_files == 0 {
            if self.path.exists() {
                std::fs::remove_file(&self.path)?;
            }
            return Ok(());
        }

        for index in (1..=self.rotation_settings.max_files).rev() {
            let source = if index == 1 {
                self.path.clone()
            } else {
                rotated_path(&self.path, index - 1)
            };
            if !source.exists() {
                continue;
            }
            let destination = rotated_path(&self.path, index);
            if destination.exists() {
                std::fs::remove_file(&destination)?;
            }
            std::fs::rename(source, destination)?;
        }
        Ok(())
    }
}

fn run_log_writer_loop(mut writer: PersistentLogWriter, receiver: Receiver<LogEntry>) {
    let mut has_pending_writes = false;
    let mut last_flush = Instant::now();

    loop {
        match receiver.recv_timeout(FLUSH_INTERVAL) {
            Ok(entry) => {
                if let Err(error) = writer.append_entry(&entry) {
                    tracing::warn!(error = %error, path = %writer.path().display(), "Failed to append persistent TUI log entry.");
                } else {
                    has_pending_writes = true;
                }
                if has_pending_writes && last_flush.elapsed() >= FLUSH_INTERVAL {
                    if let Err(error) = writer.flush() {
                        tracing::warn!(error = %error, path = %writer.path().display(), "Failed to flush persistent TUI logs.");
                    }
                    has_pending_writes = false;
                    last_flush = Instant::now();
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if has_pending_writes {
                    if let Err(error) = writer.flush() {
                        tracing::warn!(error = %error, path = %writer.path().display(), "Failed to flush persistent TUI logs.");
                    }
                    has_pending_writes = false;
                    last_flush = Instant::now();
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                if has_pending_writes && let Err(error) = writer.flush() {
                    tracing::warn!(error = %error, path = %writer.path().display(), "Failed to flush persistent TUI logs.");
                }
                break;
            }
        }
    }
}

fn resolve_log_path() -> PathBuf {
    if let Some(path) = env::var_os(TUI_LOG_PATH_ENV)
        && !path.is_empty()
    {
        return PathBuf::from(path);
    }

    let registry_config_path = oatty_registry::default_config_path();
    let base_directory = registry_config_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base_directory.join("logs").join("tui.jsonl")
}

fn resolve_rotation_settings() -> RotationSettings {
    RotationSettings {
        max_bytes: parse_u64_from_environment(TUI_LOG_MAX_BYTES_ENV).unwrap_or(DEFAULT_TUI_LOG_MAX_BYTES),
        max_files: parse_usize_from_environment(TUI_LOG_MAX_FILES_ENV).unwrap_or(DEFAULT_TUI_LOG_MAX_FILES),
    }
}

fn parse_u64_from_environment(key: &str) -> Option<u64> {
    env::var(key).ok()?.parse::<u64>().ok().filter(|value| *value > 0)
}

fn parse_usize_from_environment(key: &str) -> Option<usize> {
    env::var(key).ok()?.parse::<usize>().ok()
}

fn rotated_path(base: &std::path::Path, index: usize) -> PathBuf {
    PathBuf::from(format!("{}.{}", base.display(), index))
}

#[cfg(test)]
mod tests {
    use super::{parse_u64_from_environment, parse_usize_from_environment, rotated_path};
    use std::path::Path;

    #[test]
    fn rotated_path_appends_index_suffix() {
        let path = rotated_path(Path::new("/tmp/tui.jsonl"), 3);
        assert_eq!(path.to_string_lossy(), "/tmp/tui.jsonl.3");
    }

    #[test]
    fn parse_u64_from_environment_ignores_missing_or_invalid_values() {
        assert!(parse_u64_from_environment("MISSING_TEST_ENV_KEY").is_none());
    }

    #[test]
    fn parse_usize_from_environment_ignores_missing_values() {
        assert!(parse_usize_from_environment("MISSING_TEST_ENV_KEY").is_none());
    }
}
