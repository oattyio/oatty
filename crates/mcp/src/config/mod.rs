//! Configuration management for MCP plugins.
//! This module handles parsing, validation, and interpolation of the
//! ~/.config/oatty/mcp.json configuration file.

mod interpolation;
mod model;
mod validation;

use oatty_util::expand_tilde;
pub use interpolation::{InterpolationError, determine_env_source, interpolate_config};
pub use model::{ConfigError, McpAuthConfig, McpConfig, McpServer};
pub use validation::{ValidationError, validate_config, validate_server_name};

use crate::config::interpolation::tokenize_config;
use dirs_next::config_dir;
use serde_json::Value;
use std::env;
use std::fs;
use std::fs::create_dir_all;
use std::fs::write;
use std::path::PathBuf;

/// Get the default path for the MCP configuration file.
pub fn default_config_path() -> PathBuf {
    if let Ok(path) = env::var("MCP_CONFIG_PATH")
        && !path.trim().is_empty()
    {
        return expand_tilde(&path);
    }

    config_dir().unwrap_or_else(|| PathBuf::from(".")).join("oatty").join("mcp.json")
}

/// Load and parse the MCP configuration from the default location.
pub fn load_config() -> anyhow::Result<McpConfig> {
    let path = default_config_path();
    load_config_from_path(&path)
}

/// Load and parse the MCP configuration from a specific path.
pub fn load_config_from_path(path: &std::path::Path) -> anyhow::Result<McpConfig> {
    if !path.exists() {
        return Ok(McpConfig::default());
    }

    let content = fs::read_to_string(path)?;
    let raw_config: Value = serde_json::from_str(&content)?;
    let mut config: McpConfig = serde_json::from_value(raw_config)?;

    // Interpolate environment variables and secrets
    interpolate_config(&mut config)?;

    // Validate the configuration
    validate_config(&config)?;

    Ok(config)
}

/// Save the MCP configuration to a specific path.
pub fn save_config_to_path(config: &mut McpConfig, path: &std::path::Path) -> anyhow::Result<()> {
    // Ensure the directory exists
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
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
    fn default_path_honors_env_override() {
        let override_path = "~/custom/heroku/mcp.json";
        temp_env::with_var("MCP_CONFIG_PATH", Some(override_path), || {
            let path = default_config_path();
            let expected = expand_tilde(override_path);
            assert_eq!(path, expected);
        });
    }
}
