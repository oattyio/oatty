//! Configuration interpolation for environment variables and secrets.

use crate::config::{McpAuthConfig, McpConfig, McpServer};
use heroku_types::{EnvSource, EnvVar};
use heroku_util::is_secret;
use regex::Regex;
use thiserror::Error;
use tracing::debug;

static SERVICE: &str = "heroku";

/// Interpolate environment variables and secrets in the configuration.
pub fn interpolate_config(config: &mut McpConfig) -> Result<(), InterpolationError> {
    for (name, server) in config.mcp_servers.iter_mut() {
        if let Err(e) = interpolate_server(server) {
            server.err = Some(e.to_string());
            server.disabled = Some(true);
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
        if let Some(envs) = mcp_server.env.as_mut() {
            tokenize_env(envs, name)?;
        }
        if let Some(headers) = mcp_server.headers.as_mut() {
            tokenize_env(headers, name)?;
        }
    }
    Ok(())
}

fn tokenize_env(envs: &mut Vec<EnvVar>, name: &String) -> Result<(), InterpolationError> {
    for EnvVar { source, key, value, .. } in envs {
        if *source == EnvSource::Secret {
            let service = format!("{}-{}", name, key);
            store_secret(service.as_str(), value.as_str())?;
            *value = format!("${{secret:{}}}", service);
        }
    }
    Ok(())
}

/// Interpolate values in a single server configuration.
fn interpolate_server(server: &mut McpServer) -> Result<(), InterpolationError> {
    // Interpolate environment variables
    if let Some(env) = &mut server.env {
        for EnvVar { value, .. } in env.iter_mut() {
            *value = interpolate_string(value)?;
        }
    }

    // Interpolate HTTP headers
    if let Some(headers) = &mut server.headers {
        for EnvVar { value, .. } in headers.iter_mut() {
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
    let env_regex = Regex::new(r"\$\{env:([\w+_-]*)}")?;
    let secret_regex = Regex::new(r"\$\{secret:([\w+_-]*)}")?;

    let mut resolved_env = Vec::new();
    for cap in env_regex.captures_iter(value) {
        let var_name = cap[1].to_string();
        let env_value = std::env::var(&var_name).map_err(|_| InterpolationError::MissingEnvVar { name: var_name.clone() })?;
        debug!("Interpolated env var: {} -> [REDACTED]", var_name);
        resolved_env.push((cap[0].to_string(), env_value));
    }

    let mut resolved_secrets = Vec::new();
    for cap in secret_regex.captures_iter(value) {
        let secret_name = cap[1].to_string();
        let secret_value = resolve_secret(&secret_name)?;
        debug!("Interpolated secret: {} -> [REDACTED]", secret_name);
        resolved_secrets.push((cap[0].to_string(), secret_value));
    }

    let mut result = value.to_string();
    for (placeholder, env_value) in resolved_env {
        result = result.replace(&placeholder, &env_value);
    }
    for (placeholder, secret_value) in resolved_secrets {
        result = result.replace(&placeholder, &secret_value);
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

/// Determines the source of the env var by its value.
pub fn determine_env_source(value: &str) -> EnvSource {
    if is_secret(value) {
        return EnvSource::Secret;
    }
    if let Some(splits) = value.trim_start_matches(&['{', ' ']).split_once(":") {
        return match splits.0 {
            "env" => EnvSource::Env,
            "secret" => EnvSource::Secret,
            "file" => EnvSource::File,
            _ => EnvSource::Raw,
        };
    }
    EnvSource::Raw
}

/// Store a secret in the OS keychain.
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
#[derive(Debug, Error, Clone)]
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
