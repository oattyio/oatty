//! Configuration IO helpers for MCP plugin configuration.

use crate::config::interpolation::tokenize_config;
use crate::config::mutation::McpConfigMutationError;
use crate::config::{McpConfig, interpolate_config, validate_config};
use dirs_next::config_dir;
use oatty_util::expand_tilde;
use serde_json::Value;
use std::env;
use std::fs;
use std::fs::{create_dir_all, write};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Returns the default path for the MCP configuration file.
pub fn default_config_path() -> PathBuf {
    if let Ok(path) = env::var("MCP_CONFIG_PATH")
        && !path.trim().is_empty()
    {
        return expand_tilde(&path);
    }

    config_dir().unwrap_or_else(|| PathBuf::from(".")).join("oatty").join("mcp.json")
}

/// Loads and parses MCP configuration from the default path.
pub fn load_config() -> anyhow::Result<McpConfig> {
    let path = default_config_path();
    load_config_from_path(&path)
}

/// Loads and parses MCP configuration from a specific path.
pub fn load_config_from_path(path: &Path) -> anyhow::Result<McpConfig> {
    let value = read_raw_document(path).map_err(anyhow::Error::from)?;
    let mut config: McpConfig = serde_json::from_value(value)?;
    interpolate_config(&mut config)?;
    validate_config(&config)?;
    Ok(config)
}

/// Loads the unresolved MCP configuration document from a specific path.
///
/// Returns a serialized [`McpConfig::default`] document when the file is missing.
/// Existing files must contain an object root with an object `mcpServers` key;
/// malformed JSON or an invalid root shape are returned as typed errors.
pub fn load_raw_config_document_from_path(path: &Path) -> Result<Value, McpConfigMutationError> {
    read_raw_document(path)
}

/// Shared file-read and JSON-parse helper used by both the runtime loader and
/// the unresolved document reader.
///
/// Missing files are represented by the default configuration document.
/// Other I/O errors, malformed JSON, or invalid root shapes are propagated.
fn read_raw_document(path: &Path) -> Result<Value, McpConfigMutationError> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return serde_json::to_value(McpConfig::default())
                .map_err(|_| McpConfigMutationError::ValidationFailed("failed to serialize default configuration document".to_string()));
        }
        Err(error) => return Err(McpConfigMutationError::ReadFailed(error)),
    };

    let value: Value = serde_json::from_str(&content).map_err(McpConfigMutationError::ParseFailed)?;
    validate_root_shape(&value)?;
    Ok(value)
}

/// Validates that the document has an object root and an object `mcpServers` key.
fn validate_root_shape(value: &Value) -> Result<(), McpConfigMutationError> {
    let root = value.as_object().ok_or(McpConfigMutationError::InvalidRootShape)?;
    let mcp_servers = root.get("mcpServers").ok_or(McpConfigMutationError::InvalidRootShape)?;
    if !mcp_servers.is_object() {
        return Err(McpConfigMutationError::InvalidRootShape);
    }
    Ok(())
}

/// Saves MCP configuration to a specific path.
pub fn save_config_to_path(config: &mut McpConfig, path: &Path) -> anyhow::Result<()> {
    if let Some(parent_directory) = path.parent() {
        create_dir_all(parent_directory)?;
    }

    tokenize_config(config)?;
    let content = serde_json::to_string_pretty(config)?;
    write(path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn default_path_honors_environment_override() {
        let override_path = "~/custom/oatty/mcp.json";
        temp_env::with_var("MCP_CONFIG_PATH", Some(override_path), || {
            let path = default_config_path();
            let expected = expand_tilde(override_path);
            assert_eq!(path, expected);
        });
    }

    #[test]
    fn missing_file_returns_default_document() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.json");

        let value = load_raw_config_document_from_path(&path).unwrap();
        let config: McpConfig = serde_json::from_value(value).unwrap();
        assert!(config.mcp_servers.is_empty());
        assert_eq!(config.http_server.bind_address, Some("127.0.0.1:62889".to_string()));

        let loaded = load_config_from_path(&path).unwrap();
        assert!(loaded.mcp_servers.is_empty());
    }

    #[test]
    fn malformed_json_returns_parse_failed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        fs::write(&path, "not json").unwrap();

        let err = load_raw_config_document_from_path(&path).unwrap_err();
        assert!(matches!(err, McpConfigMutationError::ParseFailed(_)));
    }

    #[test]
    fn non_object_root_returns_invalid_root_shape() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        fs::write(&path, "\"string\"").unwrap();

        let err = load_raw_config_document_from_path(&path).unwrap_err();
        assert!(matches!(err, McpConfigMutationError::InvalidRootShape));
    }

    #[test]
    fn non_object_mcp_servers_returns_invalid_root_shape() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        fs::write(&path, r#"{"mcpServers": []}"#).unwrap();

        let err = load_raw_config_document_from_path(&path).unwrap_err();
        assert!(matches!(err, McpConfigMutationError::InvalidRootShape));
    }

    #[test]
    fn typed_invalid_entry_propagates_in_typed_loader() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        fs::write(&path, r#"{"mcpServers": {"bad": {"baseUrl": "not-a-url"}}}"#).unwrap();

        // Raw reader intentionally does not validate types.
        let raw = load_raw_config_document_from_path(&path).unwrap();
        assert!(raw.get("mcpServers").unwrap().get("bad").is_some());

        // Typed loader fails on deserialization or validation.
        let result = load_config_from_path(&path);
        assert!(result.is_err());
    }

    #[test]
    fn fresh_interpolation_clears_stale_diagnostics_preserving_disabled_setting() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        fs::write(
            &path,
            r#"{"mcpServers":{"enabled":{"command":"node","disabled":false,"err":"stale"},"disabled":{"command":"node","disabled":true,"err":"stale"}}}"#,
        )
        .unwrap();

        let config = load_config_from_path(&path).unwrap();
        let enabled = config.mcp_servers.get("enabled").unwrap();
        assert!(enabled.err.is_none());
        assert!(!enabled.disabled);

        let disabled = config.mcp_servers.get("disabled").unwrap();
        assert!(disabled.err.is_none());
        assert!(disabled.disabled);
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_file_returns_read_failed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        fs::write(&path, r#"{"mcpServers": {}}"#).unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o000);
        fs::set_permissions(&path, permissions).unwrap();

        let err = load_raw_config_document_from_path(&path).unwrap_err();
        assert!(matches!(err, McpConfigMutationError::ReadFailed(_)));

        // Restore permissions so the temp dir can be cleaned up.
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o644);
        fs::set_permissions(&path, permissions).unwrap();
    }
}
