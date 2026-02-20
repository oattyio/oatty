//! Configuration management for MCP plugins.
//! This module handles parsing, validation, and interpolation of the
//! ~/.config/oatty/mcp.json configuration file.

mod interpolation;
mod io;
mod model;
mod validation;

pub use interpolation::interpolate_config;
pub use io::{default_config_path, load_config, load_config_from_path, save_config_to_path};
pub use model::{ConfigError, McpAuthConfig, McpConfig, McpServer};
pub use validation::{ValidationError, validate_config, validate_server_name};
