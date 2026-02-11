//! Configuration interpolation for environment variables and secrets.

use indexmap::{IndexSet, set::MutableValues};
use oatty_types::{EnvSource, EnvVar};
use regex::Regex;
use thiserror::Error;
use tracing::debug;

use crate::is_secret;

static SERVICE: &str = "oatty";
/// Environment variable used to select the secret resolution backend.
pub const SECRETS_BACKEND_ENV_VAR: &str = "OATTY_SECRETS_BACKEND";

/// Secret resolution backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretsBackend {
    /// Resolve `${secret:NAME}` values via OS keychain (`keyring-rs`).
    Keychain,
    /// Resolve `${secret:NAME}` values from process environment variable `NAME`.
    Environment,
}

impl SecretsBackend {
    fn from_env_var(raw: Option<String>) -> Self {
        match raw.unwrap_or_default().trim().to_ascii_lowercase().as_str() {
            "env" => Self::Environment,
            _ => Self::Keychain,
        }
    }
}

/// Determine the currently configured secrets backend.
pub fn secrets_backend() -> SecretsBackend {
    let configured_value = std::env::var(SECRETS_BACKEND_ENV_VAR).ok();
    SecretsBackend::from_env_var(configured_value)
}

pub fn tokenize_env(envs: &mut IndexSet<EnvVar>, name: &String) -> Result<(), InterpolationError> {
    let configured_backend = secrets_backend();
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
            if configured_backend == SecretsBackend::Environment {
                // In env backend mode, preserve inline values/placeholders and avoid
                // keychain writes to keep local and CI usage keychain-free.
                continue;
            }
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

/// Resolve a secret using the configured secrets backend.
pub fn resolve_secret(name: &str) -> Result<String, InterpolationError> {
    match secrets_backend() {
        SecretsBackend::Environment => std::env::var(name).map_err(|error| InterpolationError::MissingSecret {
            name: name.to_string(),
            error: error.to_string(),
        }),
        SecretsBackend::Keychain => {
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
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_backend_defaults_to_keychain_when_env_var_is_missing() {
        temp_env::with_var(SECRETS_BACKEND_ENV_VAR, None::<&str>, || {
            assert_eq!(secrets_backend(), SecretsBackend::Keychain);
        });
    }

    #[test]
    fn secrets_backend_uses_environment_when_configured() {
        temp_env::with_var(SECRETS_BACKEND_ENV_VAR, Some("env"), || {
            assert_eq!(secrets_backend(), SecretsBackend::Environment);
        });
    }

    #[test]
    fn resolve_secret_reads_process_environment_in_environment_mode() {
        temp_env::with_vars(
            [
                (SECRETS_BACKEND_ENV_VAR, Some("env")),
                ("INTERPOLATION_TEST_SECRET", Some("test-secret-value")),
            ],
            || {
                let resolved = resolve_secret("INTERPOLATION_TEST_SECRET").expect("secret resolves from environment");
                assert_eq!(resolved, "test-secret-value");
            },
        );
    }
}
