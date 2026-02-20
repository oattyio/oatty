//! Configuration IO helpers for MCP plugin configuration.

use crate::config::interpolation::tokenize_config;
use crate::config::{McpConfig, interpolate_config, validate_config};
use dirs_next::config_dir;
use oatty_util::expand_tilde;
use serde_json::Value;
use std::env;
use std::fs;
use std::fs::{create_dir_all, write};
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
    if !path.exists() {
        return Ok(McpConfig::default());
    }

    let content = fs::read_to_string(path)?;
    let raw_config: Value = serde_json::from_str(&content)?;
    let mut config: McpConfig = serde_json::from_value(raw_config)?;
    interpolate_config(&mut config)?;
    validate_config(&config)?;
    Ok(config)
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

    #[test]
    fn default_path_honors_environment_override() {
        let override_path = "~/custom/oatty/mcp.json";
        temp_env::with_var("MCP_CONFIG_PATH", Some(override_path), || {
            let path = default_config_path();
            let expected = expand_tilde(override_path);
            assert_eq!(path, expected);
        });
    }
}
