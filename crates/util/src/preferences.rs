//! User preference persistence for the Heroku CLI/TUI.
//!
//! This module provides a tiny JSON-backed store that records lightweight
//! configuration such as the user's preferred theme. The file is written to
//! the standard configuration directory (`~/.config/heroku/preferences.json`
//! on most platforms) and is safe to read/write from multiple threads thanks
//! to the internal `Mutex`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use dirs_next::{config_dir, home_dir};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::warn;

/// Environment variable allowing callers to override the preferences file path.
pub const PREFERENCES_PATH_ENV: &str = "HEROKU_PREFERENCES_PATH";

/// Default filename for the JSON payload.
pub const PREFERENCES_FILE_NAME: &str = "preferences.json";

/// Error surfaced when reading or writing preferences fails.
#[derive(Debug, Error)]
pub enum PreferencesError {
    /// I/O failure (for example, permissions or missing directory).
    #[error("preferences I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Serialization or deserialization failure.
    #[error("preferences serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Persisted preference values.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PreferencesPayload {
    /// Canonical identifier of the theme selected via the TUI.
    pub preferred_theme: Option<String>,
}

/// Thread-safe preferences store backed by a JSON file.
pub struct UserPreferences {
    path: PathBuf,
    payload: Mutex<PreferencesPayload>,
    persist_to_disk: bool,
}

impl UserPreferences {
    /// Create a store rooted at the provided path. When `path` is `None`, the
    /// default config directory path is used.
    pub fn new<P: Into<Option<PathBuf>>>(path: P) -> Result<Self, PreferencesError> {
        let resolved_path = match path.into() {
            Some(path) => expand_tilde_path(path),
            None => default_preferences_path(),
        };
        let payload = load_payload(&resolved_path)?;
        Ok(Self {
            path: resolved_path,
            payload: Mutex::new(payload),
            persist_to_disk: true,
        })
    }

    /// Initialize a store with the default settings and file location.
    pub fn with_defaults() -> Result<Self, PreferencesError> {
        Self::new(None::<PathBuf>)
    }

    /// Path to the underlying JSON file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the canonical identifier of the preferred theme, if one was saved.
    pub fn preferred_theme(&self) -> Option<String> {
        self.payload.lock().expect("preferences lock poisoned").preferred_theme.clone()
    }

    /// Persist a new preferred theme identifier.
    pub fn set_preferred_theme(&self, theme_id: Option<String>) -> Result<(), PreferencesError> {
        {
            let mut payload = self.payload.lock().expect("preferences lock poisoned");
            payload.preferred_theme = theme_id;
            if self.persist_to_disk {
                self.save_locked(&payload)?;
            }
        }
        Ok(())
    }

    /// Build an in-memory store used as a fallback when the config directory cannot be accessed.
    pub fn ephemeral() -> Self {
        Self {
            path: PathBuf::new(),
            payload: Mutex::new(PreferencesPayload::default()),
            persist_to_disk: false,
        }
    }

    fn save_locked(&self, payload: &PreferencesPayload) -> Result<(), PreferencesError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(payload)?;
        fs::write(&self.path, data)?;
        Ok(())
    }
}

fn default_preferences_path() -> PathBuf {
    if let Ok(path) = env::var(PREFERENCES_PATH_ENV) {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return expand_tilde_path(PathBuf::from(trimmed));
        }
    }

    config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("heroku")
        .join(PREFERENCES_FILE_NAME)
}

fn expand_tilde_path(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    if text == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from("~"));
    }
    if let Some(rest) = text.strip_prefix("~/") {
        return home_dir().unwrap_or_else(|| PathBuf::from("~")).join(rest);
    }
    if let Some(rest) = text.strip_prefix("~\\") {
        return home_dir().unwrap_or_else(|| PathBuf::from("~")).join(rest);
    }
    path
}

fn load_payload(path: &Path) -> Result<PreferencesPayload, PreferencesError> {
    match fs::read_to_string(path) {
        Ok(data) => match serde_json::from_str(&data) {
            Ok(payload) => Ok(payload),
            Err(error) => {
                warn!(
                    path = %path.display(),
                    error = %error,
                    "Failed to parse preferences file; using defaults"
                );
                Ok(PreferencesPayload::default())
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(PreferencesPayload::default()),
        Err(error) => Err(PreferencesError::Io(error)),
    }
}
