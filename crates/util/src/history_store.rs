//! History persistence utilities for workflows and palette commands.
//!
//! This module exposes abstractions for storing and retrieving history entries,
//! along with a JSON-backed implementation mirroring the ergonomics of the MCP
//! config file (tilde expansion, config directory fallback).

use crate::text_processing::is_secret;
use chrono::serde::ts_seconds;
use chrono::{DateTime, Utc};
use dirs_next::{config_dir, home_dir};
use oatty_types::workflow::{WorkflowDefaultSource, WorkflowInputDefinition};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use thiserror::Error;
use tracing::warn;

/// Environment variable controlling the history file location.
pub const HISTORY_PATH_ENV: &str = "OATTY_HISTORY_PATH";

/// Default filename for the persisted history store.
pub const HISTORY_FILE_NAME: &str = "history.json";

/// Maximum number of entries retained by the store.
pub const DEFAULT_HISTORY_PROFILE: &str = "default_profile";
pub const DEFAULT_HISTORY_LIMIT: usize = 500;

/// Errors surfaced by history store operations.
#[derive(Debug, Error)]
pub enum HistoryStoreError {
    /// I/O failure while reading or writing the history file.
    #[error("history I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Serialization or deserialization failure.
    #[error("history serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Logical grouping for a history entry.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum HistoryScope {
    /// Workflow input entry scoped by workflow + input identifiers.
    WorkflowInput {
        /// Workflow identifier (file stem / runtime id).
        workflow_id: String,
        /// Input identifier inside the workflow.
        input_id: String,
    },
    /// Palette command execution entry.
    PaletteCommand {
        /// Fully-qualified command identifier.
        command_id: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryScopeKind {
    WorkflowInput,
    PaletteCommand,
}

/// Represents a stored history record returned by list operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryRecord {
    pub key: HistoryKey,
    pub value: StoredHistoryValue,
}

/// Uniquely identifies a stored history value.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct HistoryKey {
    /// User identifier (defaults to `default_profile` until auth integration).
    pub user_id: String,
    /// Logical scope for the specific history entry.
    pub scope: HistoryScope,
}

impl HistoryKey {
    /// Build a workflow input history key.
    pub fn workflow_input(user_id: impl Into<String>, workflow_id: impl Into<String>, input_id: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            scope: HistoryScope::WorkflowInput {
                workflow_id: workflow_id.into(),
                input_id: input_id.into(),
            },
        }
    }

    /// Build a palette command history key.
    pub fn palette_command(user_id: impl Into<String>, command_id: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            scope: HistoryScope::PaletteCommand {
                command_id: command_id.into(),
            },
        }
    }

    pub fn scope_kind(&self) -> HistoryScopeKind {
        match &self.scope {
            HistoryScope::WorkflowInput { .. } => HistoryScopeKind::WorkflowInput,
            HistoryScope::PaletteCommand { .. } => HistoryScopeKind::PaletteCommand,
        }
    }
}

/// Stored history value metadata.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct StoredHistoryValue {
    /// Persisted value.
    pub value: Value,
    /// Last time the value was written.
    #[serde(with = "ts_seconds")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Default, Serialize, Deserialize)]
struct HistoryFile {
    entries: VecDeque<HistoryEntry>,
}

impl HistoryFile {
    fn get(&self, key: &HistoryKey) -> Option<StoredHistoryValue> {
        self.entries.iter().find(|entry| entry.key == *key).map(|entry| entry.data.clone())
    }

    fn upsert(&mut self, key: HistoryKey, value: Value, limit: usize) {
        if let Some(position) = self.entries.iter().position(|entry| entry.key == key) {
            self.entries.remove(position);
        }

        let data = StoredHistoryValue {
            value,
            updated_at: Utc::now(),
        };
        self.entries.push_front(HistoryEntry { key, data });
        self.truncate(limit);
    }

    fn truncate(&mut self, limit: usize) {
        while self.entries.len() > limit {
            self.entries.pop_back();
        }
    }

    fn records_for_scope(&self, scope: HistoryScopeKind) -> Vec<HistoryRecord> {
        self.entries
            .iter()
            .filter(|entry| entry.key.scope_kind() == scope)
            .map(|entry| HistoryRecord {
                key: entry.key.clone(),
                value: entry.data.clone(),
            })
            .collect()
    }
}

#[derive(Serialize, Deserialize)]
struct HistoryEntry {
    key: HistoryKey,
    #[serde(flatten)]
    data: StoredHistoryValue,
}

/// Shared trait implemented by history persistence backends.
pub trait HistoryStore: Send + Sync {
    /// Retrieve the latest value associated with the provided key.
    fn get_latest_value(&self, key: &HistoryKey) -> Result<Option<StoredHistoryValue>, HistoryStoreError>;

    /// Store or update the latest value for the provided key.
    fn insert_value(&self, key: HistoryKey, value: Value) -> Result<(), HistoryStoreError>;

    /// List entries belonging to the requested scope, ordered from most recent to oldest.
    fn entries_for_scope(&self, scope: HistoryScopeKind) -> Result<Vec<HistoryRecord>, HistoryStoreError>;

    /// Truncate history to the provided maximum length.
    fn truncate(&self, max_entries: usize) -> Result<(), HistoryStoreError>;
}

/// JSON-backed history store persisted on disk.
pub struct JsonHistoryStore {
    path: PathBuf,
    entries: Mutex<HistoryFile>,
    max_entries: usize,
}

impl JsonHistoryStore {
    /// Create a new store at the provided path (or the default path when omitted).
    pub fn new<P: Into<Option<PathBuf>>>(path: P, max_entries: usize) -> Result<Self, HistoryStoreError> {
        let resolved_path = match path.into() {
            Some(path) => expand_tilde_path(path),
            None => default_history_path(),
        };

        let file = load_history_file(&resolved_path)?;
        Ok(Self {
            path: resolved_path,
            entries: Mutex::new(file),
            max_entries,
        })
    }

    /// Initialize a store using the default settings.
    pub fn with_defaults() -> Result<Self, HistoryStoreError> {
        Self::new(None::<PathBuf>, DEFAULT_HISTORY_LIMIT)
    }

    /// Access the underlying history path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn save_locked(&self, history_file: &HistoryFile) -> Result<(), HistoryStoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(history_file)?;
        fs::write(&self.path, content)?;
        Ok(())
    }
}

impl HistoryStore for JsonHistoryStore {
    fn get_latest_value(&self, key: &HistoryKey) -> Result<Option<StoredHistoryValue>, HistoryStoreError> {
        let entries = self.entries.lock().expect("history lock poisoned");
        Ok(entries.get(key))
    }

    fn insert_value(&self, key: HistoryKey, value: Value) -> Result<(), HistoryStoreError> {
        let mut entries = self.entries.lock().expect("history lock poisoned");
        entries.upsert(key, value, self.max_entries);
        self.save_locked(&entries)
    }

    fn entries_for_scope(&self, scope: HistoryScopeKind) -> Result<Vec<HistoryRecord>, HistoryStoreError> {
        let entries = self.entries.lock().expect("history lock poisoned");
        Ok(entries.records_for_scope(scope))
    }

    fn truncate(&self, max_entries: usize) -> Result<(), HistoryStoreError> {
        let mut entries = self.entries.lock().expect("history lock poisoned");
        entries.truncate(max_entries);
        self.save_locked(&entries)
    }
}

/// In-memory history store primarily used for unit testing.
#[derive(Default)]
pub struct InMemoryHistoryStore {
    entries: Mutex<HistoryFile>,
}

impl InMemoryHistoryStore {
    /// Create an empty in-memory history store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl HistoryStore for InMemoryHistoryStore {
    fn get_latest_value(&self, key: &HistoryKey) -> Result<Option<StoredHistoryValue>, HistoryStoreError> {
        let entries = self.entries.lock().expect("history lock poisoned");
        Ok(entries.get(key))
    }

    fn insert_value(&self, key: HistoryKey, value: Value) -> Result<(), HistoryStoreError> {
        let mut entries = self.entries.lock().expect("history lock poisoned");
        entries.upsert(key, value, DEFAULT_HISTORY_LIMIT);
        Ok(())
    }

    fn entries_for_scope(&self, scope: HistoryScopeKind) -> Result<Vec<HistoryRecord>, HistoryStoreError> {
        let entries = self.entries.lock().expect("history lock poisoned");
        Ok(entries.records_for_scope(scope))
    }

    fn truncate(&self, max_entries: usize) -> Result<(), HistoryStoreError> {
        let mut entries = self.entries.lock().expect("history lock poisoned");
        entries.truncate(max_entries);
        Ok(())
    }
}

fn expand_tilde_path(path: PathBuf) -> PathBuf {
    if let Some(first) = path.components().next()
        && first.as_os_str() != "~"
    {
        return path;
    }

    let input = path.to_string_lossy();
    let trimmed = input.trim();

    if trimmed == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from("~"));
    }

    if let Some(rest) = trimmed.strip_prefix("~/") {
        return home_dir().unwrap_or_else(|| PathBuf::from("~")).join(rest);
    }

    if let Some(rest) = trimmed.strip_prefix("~\\") {
        return home_dir().unwrap_or_else(|| PathBuf::from("~")).join(rest);
    }

    PathBuf::from(trimmed)
}

fn default_history_path() -> PathBuf {
    if let Ok(path) = env::var(HISTORY_PATH_ENV)
        && !path.trim().is_empty()
    {
        return expand_tilde_path(PathBuf::from(path));
    }

    config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("oatty")
        .join(HISTORY_FILE_NAME)
}

fn load_history_file(path: &Path) -> Result<HistoryFile, HistoryStoreError> {
    match fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str::<HistoryFile>(&content) {
            Ok(file) => Ok(file),
            Err(error) => {
                warn!("Failed to parse history file at {}: {}", path.display(), error);
                Ok(HistoryFile::default())
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(HistoryFile::default()),
        Err(error) => Err(HistoryStoreError::Io(error)),
    }
}

/// Returns `true` when the provided value appears to contain sensitive data and should not be persisted.
pub fn value_contains_secret(value: &Value) -> bool {
    match value {
        Value::String(text) => is_secret(text),
        Value::Array(items) => items.iter().any(value_contains_secret),
        Value::Object(map) => map.values().any(value_contains_secret),
        _ => false,
    }
}

/// Returns `true` when the workflow input definition declares a history-based default.
pub fn workflow_input_uses_history(definition: &WorkflowInputDefinition) -> bool {
    matches!(
        definition.default.as_ref().map(|default| &default.from),
        Some(WorkflowDefaultSource::History)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::has_meaningful_value;
    use serde_json::json;
    use std::sync::Arc;
    use std::thread;
    use tempfile::tempdir;

    fn workflow_key() -> HistoryKey {
        HistoryKey::workflow_input("default_profile", "workflow_a", "input_a")
    }

    #[test]
    fn in_memory_store_round_trip() {
        let store = InMemoryHistoryStore::new();
        let key = workflow_key();
        assert!(store.get_latest_value(&key).unwrap().is_none());

        store.insert_value(key.clone(), json!("value")).unwrap();
        let stored = store.get_latest_value(&key).unwrap().unwrap();
        assert_eq!(stored.value, json!("value"));
    }

    #[test]
    fn json_store_persists_entries() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.json");
        let store = JsonHistoryStore::new(Some(path.clone()), 10).unwrap();

        let key = workflow_key();
        store.insert_value(key.clone(), json!("value")).unwrap();

        drop(store);
        let store_reloaded = JsonHistoryStore::new(Some(path.clone()), 10).unwrap();
        let stored = store_reloaded.get_latest_value(&key).unwrap().unwrap();
        assert_eq!(stored.value, json!("value"));
    }

    #[test]
    fn json_store_truncates() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.json");
        let store = JsonHistoryStore::new(Some(path.clone()), 2).unwrap();

        for index in 0..3 {
            let key = HistoryKey::workflow_input("default_profile", format!("wf{}", index), "input");
            store.insert_value(key, json!(index)).unwrap();
        }

        drop(store);
        let store_reloaded = JsonHistoryStore::new(Some(path.clone()), 2).unwrap();
        let len = store_reloaded.entries.lock().unwrap().entries.len();
        assert_eq!(len, 2);
    }

    #[test]
    fn default_path_honors_env_override() {
        let override_path = "~/custom/history.json";
        temp_env::with_var(HISTORY_PATH_ENV, Some(override_path), || {
            let path = default_history_path();
            let expected = expand_tilde_path(PathBuf::from(override_path));
            assert_eq!(path, expected);
        });
    }

    #[test]
    fn invalid_json_returns_empty_store() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.json");
        fs::write(&path, "not json").unwrap();

        let store = JsonHistoryStore::new(Some(path.clone()), 10).unwrap();
        assert!(store.get_latest_value(&workflow_key()).unwrap().is_none());
    }

    #[test]
    fn concurrent_writes_remain_ordered() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.json");
        let store = JsonHistoryStore::new(Some(path.clone()), 10).unwrap();
        let store = Arc::new(store);
        let mut handles = Vec::new();
        for index in 0..5 {
            let handle_store = Arc::clone(&store);
            handles.push(thread::spawn(move || {
                let key = HistoryKey::workflow_input("default_profile", "wf", "input");
                handle_store.insert_value(key, json!(index)).unwrap();
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
        let key = HistoryKey::workflow_input("default_profile", "wf", "input");
        let latest = store.get_latest_value(&key).unwrap();
        assert!(latest.unwrap().value.is_number());
    }

    #[test]
    fn workflow_input_history_detection() {
        let mut definition = WorkflowInputDefinition {
            default: Some(oatty_types::workflow::WorkflowInputDefault {
                from: WorkflowDefaultSource::History,
                value: None,
            }),
            ..WorkflowInputDefinition::default()
        };

        assert!(workflow_input_uses_history(&definition));

        definition.default = Some(oatty_types::workflow::WorkflowInputDefault {
            from: WorkflowDefaultSource::Literal,
            value: None,
        });
        assert!(!workflow_input_uses_history(&definition));
    }

    #[test]
    fn meaningful_value_detection() {
        assert!(!has_meaningful_value(&Value::Null));
        assert!(!has_meaningful_value(&Value::String("   ".into())));
        assert!(has_meaningful_value(&Value::String("data".into())));
        assert!(!has_meaningful_value(&Value::Array(Vec::new())));
        assert!(has_meaningful_value(&Value::Array(vec![Value::Bool(true)])));
        assert!(!has_meaningful_value(&Value::Object(serde_json::Map::new())));
        let mut object = serde_json::Map::new();
        object.insert("key".into(), Value::from(1));
        assert!(has_meaningful_value(&Value::Object(object)));
    }

    #[test]
    fn entries_for_scope_filters_correctly() {
        let store = InMemoryHistoryStore::new();
        let workflow_key = HistoryKey::workflow_input("default_profile", "wf", "input");
        let palette_key = HistoryKey::palette_command("default_profile", "group:cmd");
        store.insert_value(workflow_key.clone(), Value::String("app".into())).unwrap();
        store.insert_value(palette_key.clone(), Value::String("apps info".into())).unwrap();

        let workflow_entries = store.entries_for_scope(HistoryScopeKind::WorkflowInput).unwrap();
        assert_eq!(workflow_entries.len(), 1);
        assert_eq!(workflow_entries[0].key, workflow_key);

        let palette_entries = store.entries_for_scope(HistoryScopeKind::PaletteCommand).unwrap();
        assert_eq!(palette_entries.len(), 1);
        assert_eq!(palette_entries[0].key, palette_key);
    }

    #[test]
    fn detects_secret_in_nested_value() {
        let nested = json!({
            "app": "example",
            "token": "oatty_api_token=abc123def456ghi789",
        });
        assert!(value_contains_secret(&nested));

        let benign = json!({"app": "demo"});
        assert!(!value_contains_secret(&benign));
    }
}
