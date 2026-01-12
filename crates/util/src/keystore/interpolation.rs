//! Configuration interpolation for environment variables and secrets.

use indexmap::{IndexSet, set::MutableValues};
use oatty_types::{EnvSource, EnvVar};
use regex::Regex;
use thiserror::Error;
use tracing::debug;

use crate::is_secret;

static SERVICE: &str = "oatty";

pub fn tokenize_env(envs: &mut IndexSet<EnvVar>, name: &String) -> Result<(), InterpolationError> {
    for i in 0..envs.len() {
        let Some(EnvVar { source, key, value, .. }) = envs.get_index_mut2(i) else {
            continue;
        };
        // Don't trust the source of the incoming env var
        // since the user may have edited the field.
        *source = determine_env_source(value);
        // is_secret call is to determine if we need
        // to store it in the keychain. This avoids storing
        // tokenized secret sources.
        if *source == EnvSource::Secret && is_secret(value) {
            let service = format!("{}-{}", name, key);
            store_secret(service.as_str(), value.as_str())?;
            *value = format!("${{secret:{}}}", service);
        }
    }
    Ok(())
}

/// Interpolate a string value, replacing ${env:NAME} and ${secret:NAME} patterns.
pub fn interpolate_string(value: &str) -> Result<String, InterpolationError> {
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
pub fn resolve_secret(name: &str) -> Result<String, InterpolationError> {
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

/// Determines the source of the env var by its value.
pub fn determine_env_source(value: &str) -> EnvSource {
    if is_secret(value) {
        return EnvSource::Secret;
    }
    if let Some((prefix, _)) = value.trim_start_matches(['{', ' ']).split_once(':') {
        return match prefix {
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
