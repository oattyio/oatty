//! Configuration interpolation for environment variables and secrets.

use crate::config::{McpAuthConfig, McpConfig, McpServer};
use indexmap::set::MutableValues;
use oatty_types::EnvVar;
use oatty_util::{InterpolationError, interpolate_string, tokenize_env};
use tracing::debug;

/// Interpolate environment variables and secrets in the configuration.
pub fn interpolate_config(config: &mut McpConfig) -> Result<(), InterpolationError> {
    for (name, server) in config.mcp_servers.iter_mut() {
        if let Err(e) = interpolate_server(server) {
            server.err = Some(e.to_string());
            server.disabled = true;
        }
        debug!("Interpolated configuration for server: {}", name);
    }
    Ok(())
}

/// pull any secrets and env vars out and store in the keyring
/// then replace the values with tokenized strings before saving.
/// e.g., ${env:NAME} or ${secret:github.NAME}
pub fn tokenize_config(config: &mut McpConfig) -> Result<(), InterpolationError> {
    for (name, mcp_server) in config.mcp_servers.iter_mut() {
        if !mcp_server.env.is_empty() {
            tokenize_env(&mut mcp_server.env, name)?;
        }
        if !mcp_server.headers.is_empty() {
            tokenize_env(&mut mcp_server.headers, name)?;
        }
    }
    Ok(())
}

/// Interpolate values in a single server configuration.
fn interpolate_server(server: &mut McpServer) -> Result<(), InterpolationError> {
    // Interpolate environment variables
    for i in 0..server.env.len() {
        let Some(EnvVar { value, .. }) = server.env.get_index_mut2(i) else {
            continue;
        };
        *value = interpolate_string(value)?;
    }

    // Interpolate HTTP headers
    for i in 0..server.headers.len() {
        let Some(EnvVar { value, .. }) = server.headers.get_index_mut2(i) else {
            continue;
        };

        *value = interpolate_string(value)?;
    }

    // Interpolate auth config
    if let Some(auth) = &mut server.auth {
        interpolate_auth(auth)?;
    }

    Ok(())
}

fn interpolate_auth(auth: &mut McpAuthConfig) -> Result<(), InterpolationError> {
    // Scheme is literal; normalize to lowercase
    auth.scheme = auth.scheme.to_lowercase();
    if let Some(u) = &mut auth.username {
        *u = interpolate_string(u)?;
    }
    if let Some(p) = &mut auth.password {
        *p = interpolate_string(p)?;
    }
    if let Some(t) = &mut auth.token {
        *t = interpolate_string(t)?;
    }
    if let Some(h) = &mut auth.header_name {
        *h = interpolate_string(h)?;
    }
    Ok(())
}
