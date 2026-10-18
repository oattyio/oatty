//! Configuration management for MCP plugins.
//! This module handles parsing, validation, interpolation, and mutation of the
//! ~/.config/oatty/mcp.json configuration file.

mod apply_config_mutation;
mod interpolation;
mod io;
mod model;
mod mutation;
mod validation;

pub use apply_config_mutation::apply_config_mutation;
pub use interpolation::interpolate_config;
pub use io::{default_config_path, load_config, load_config_from_path, load_raw_config_document_from_path, save_config_to_path};
pub use model::{ConfigError, McpAuthConfig, McpConfig, McpServer};
pub use mutation::{ClearField, ConfigureMode, McpConfigMutation, McpServerPatch, ReloadPolicy, ServerStatus};
pub use validation::{ValidationError, validate_config, validate_server_name};
