//! Configuration interpolation for environment variables and secrets.

use crate::config::{McpAuthConfig, McpConfig, McpServer};
use regex::Regex;
use thiserror::Error;
use tracing::debug;

static SERVICE: &str = "heroku";

/// Interpolate environment variables and secrets in the configuration.
pub fn interpolate_config(config: &mut McpConfig) -> Result<(), InterpolationError> {
    for (name, server) in config.mcp_servers.iter_mut() {
        interpolate_server(server)?;
        debug!("Interpolated configuration for server: {}", name);
    }
    Ok(())
}

/// Interpolate values in a single server configuration.
fn interpolate_server(server: &mut McpServer) -> Result<(), InterpolationError> {
    // Interpolate environment variables
    if let Some(env) = &mut server.env {
        for (_, value) in env.iter_mut() {
            *value = interpolate_string(value)?;
        }
    }

    // Interpolate HTTP headers
    if let Some(headers) = &mut server.headers {
        for (_, value) in headers.iter_mut() {
            *value = interpolate_string(value)?;
        }
    }

    // Interpolate auth config
    if let Some(auth) = &mut server.auth {
        interpolate_auth(auth)?;
    }

    Ok(())
}

/// Interpolate a string value, replacing ${env:NAME} and ${secret:NAME} patterns.
fn interpolate_string(value: &str) -> Result<String, InterpolationError> {
    let env_regex = Regex::new(r"\$\{env:([A-Z_][A-Z0-9_]*)\}")?;
    let secret_regex = Regex::new(r"\$\{secret:([A-Z_][A-Z0-9_]*)\}")?;

    let mut result = value.to_string();

    // Replace environment variables
    for cap in env_regex.captures_iter(value) {
        let var_name = &cap[1];
        let env_value = std::env::var(var_name).map_err(|_| InterpolationError::MissingEnvVar {
            name: var_name.to_string(),
        })?;
        result = result.replace(&cap[0], &env_value);
        debug!("Interpolated env var: {} -> [REDACTED]", var_name);
    }

    // Replace secrets
    for cap in secret_regex.captures_iter(value) {
        let secret_name = &cap[1];
        let secret_value = resolve_secret(secret_name)?;
        result = result.replace(&cap[0], &secret_value);
        debug!("Interpolated secret: {} -> [REDACTED]", secret_name);
    }

    Ok(result)
}

/// Resolve a secret from the OS keychain.
fn resolve_secret(name: &str) -> Result<String, InterpolationError> {
    // Use keyring-rs to resolve secrets
    let keyring = keyring::Entry::new(SERVICE, name).map_err(|e| InterpolationError::KeyringError {
        name: name.to_string(),
        error: e.to_string(),
    })?;

    keyring.get_password().map_err(|e| InterpolationError::MissingSecret {
        name: name.to_string(),
        error: e.to_string(),
    })
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

/// Store a secret in the OS keychain.
#[allow(dead_code)]
pub fn store_secret(name: &str, value: &str) -> Result<(), InterpolationError> {
    let keyring = keyring::Entry::new(SERVICE, name).map_err(|e| InterpolationError::KeyringError {
        name: name.to_string(),
        error: e.to_string(),
    })?;

    keyring.set_password(value).map_err(|e| InterpolationError::KeyringError {
        name: name.to_string(),
        error: e.to_string(),
    })?;

    debug!("Stored secret in keychain: {}", name);
    Ok(())
}

/// Remove a secret from the OS keychain.
#[allow(dead_code)]
pub fn remove_secret(name: &str) -> Result<(), InterpolationError> {
    let keyring = keyring::Entry::new(SERVICE, name).map_err(|e| InterpolationError::KeyringError {
        name: name.to_string(),
        error: e.to_string(),
    })?;

    keyring.delete_credential().map_err(|e| InterpolationError::KeyringError {
        name: name.to_string(),
        error: e.to_string(),
    })?;

    debug!("Removed secret from keychain: {}", name);
    Ok(())
}

/// Errors that can occur during interpolation.
#[derive(Debug, Error)]
pub enum InterpolationError {
    #[error("Missing environment variable: {name}")]
    MissingEnvVar { name: String },

    #[error("Missing secret: {name} - {error}")]
    MissingSecret { name: String, error: String },

    #[error("Keyring error for {name}: {error}")]
    KeyringError { name: String, error: String },

    #[error("Regex compilation error: {0}")]
    Regex(#[from] regex::Error),
}
