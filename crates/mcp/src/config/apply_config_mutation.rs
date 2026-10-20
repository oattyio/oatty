//! Orchestration for shared configuration mutations.
//!
//! This module owns the single entry point that acquires the operation guard
//! once and retains it across read/merge/save/reload so concurrent tool and TUI
//! mutations cannot interleave. The pure patch helpers live in the sibling
//! [`super::mutation`] module.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use indexmap::IndexSet;
use oatty_types::{EnvSource, EnvVar};
use oatty_util::tokenize_env;
use serde_json::Value;

use super::io::{load_config_from_path, load_raw_config_document_from_path};
use super::model::{McpConfig, McpHttpServerConfig};
use super::mutation::{
    ConfigureServerMutation, ErrorCategory, ErrorStage, McpConfigMutation, McpConfigMutationError, McpConfigMutationOutcome, ReloadPolicy,
    SafeError, ServerStatus, apply_server_patch, deserialize_validated_candidate,
};
use crate::PluginEngine;
use crate::types::McpServerApplyOutcome;

/// Apply a configuration mutation with shared operation guard semantics.
///
/// This is the single entry point for tool and TUI mutation paths. A detached
/// worker task owns the operation guard and every input for the whole
/// read/merge/save/reload sequence. The caller only awaits the worker's join
/// result, so cancelling the caller future cannot release the guard while the
/// mutation is still applying state.
///
/// # Arguments
/// * `engine` — The `PluginEngine` that owns the shared gateway.
/// * `path` — The configuration file path.
/// * `mutation` — The mutation to apply.
/// * `policy` — The reload behavior for runtime application.
///
/// # Returns
/// A [`McpConfigMutationOutcome`] with per-stage success/failure metadata.
pub async fn apply_config_mutation(
    engine: Arc<PluginEngine>,
    path: PathBuf,
    mutation: McpConfigMutation,
    policy: ReloadPolicy,
) -> Result<McpConfigMutationOutcome, McpConfigMutationError> {
    // The worker acquires the guard itself. Nothing inside it may acquire the
    // guard again. Dropping the caller's future detaches this task rather than
    // cancelling it, so persistence and runtime application still finish.
    let worker = tokio::spawn(run_config_mutation_worker(engine, path, mutation, policy));
    worker
        .await
        .map_err(|error| McpConfigMutationError::ValidationFailed(format!("configuration mutation worker failed to complete: {error}")))?
}

/// Complete one mutation while owning the operation guard.
async fn run_config_mutation_worker(
    engine: Arc<PluginEngine>,
    path: PathBuf,
    mutation: McpConfigMutation,
    policy: ReloadPolicy,
) -> Result<McpConfigMutationOutcome, McpConfigMutationError> {
    let guard = engine.acquire_operation_guard().await;

    // Read, merge, validate, tokenize, and persist as one blocking unit under
    // the guard. Tokenization may perform keychain round-trips, so it must not
    // run on an async executor thread.
    let candidate_path = path.clone();
    let (server_name, operation, changed_fields) = tokio::task::spawn_blocking(move || {
        let (server_name, operation, changed_fields, mut document) = build_candidate_document(&candidate_path, &mutation)?;

        // Validate the complete candidate before touching disk so a rejected
        // candidate is never persisted.
        deserialize_validated_candidate(document.clone())?;

        // Tokenize credential values in place. Serializing the typed candidate
        // would discard root extensions and rewrite existing env/header shapes.
        tokenize_document_values(&mut document)?;

        persist_document(&candidate_path, &document)?;
        Ok::<_, McpConfigMutationError>((server_name, operation, changed_fields))
    })
    .await
    .map_err(|error| McpConfigMutationError::ValidationFailed(format!("configuration candidate task failed: {error}")))??;

    // Reloading interpolates credentials and may perform keychain reads, so it
    // is also blocking work. The guard stays owned by this worker across it.
    let reload_path = path.clone();
    let new_config = tokio::task::spawn_blocking(move || {
        load_config_from_path(&reload_path).map_err(|error| McpConfigMutationError::RuntimeLoadFailed(error.to_string()))
    })
    .await
    .map_err(|error| McpConfigMutationError::RuntimeLoadFailed(format!("configuration reload task failed: {error}")))??;

    // Resolve any caller connect intent against the freshly read candidate so a
    // disabled server can never be connected by a stale pre-read.
    let policy = resolve_policy_connect_intent(policy, &new_config);

    let apply_outcome = engine
        .update_config_under_guard(new_config, &policy, &guard)
        .await
        .map_err(|error| McpConfigMutationError::RuntimeApplyFailed(error.to_string()))?;

    let server_outcome = apply_outcome.applied_servers.iter().find(|outcome| outcome.name == server_name);
    Ok(mutation_outcome_from_server_apply(
        server_name,
        operation,
        changed_fields,
        server_outcome,
    ))
}

/// Compute the mutated candidate document without persisting it.
fn build_candidate_document(
    path: &Path,
    mutation: &McpConfigMutation,
) -> Result<(String, String, Vec<String>, Value), McpConfigMutationError> {
    match mutation {
        McpConfigMutation::ConfigureServer { name, patch, mode } => {
            let mut document = load_raw_config_document_from_path(path)?;
            let old_server = document.get("mcpServers").and_then(|servers| servers.get(name)).cloned();
            let mirror = ConfigureServerMutation {
                name: name.clone(),
                patch: patch.clone(),
                mode: *mode,
            };
            apply_server_patch(&mut document, &mirror)?;
            let new_server = document.get("mcpServers").and_then(|servers| servers.get(name)).cloned();
            let changed_fields = diff_server_fields(old_server.as_ref(), new_server.as_ref());
            Ok((name.clone(), "configure".to_string(), changed_fields, document))
        }
        McpConfigMutation::ReplaceServer {
            old_name,
            new_name,
            server,
        } => {
            let mut document = load_raw_config_document_from_path(path)?;
            let root = document.as_object_mut().ok_or(McpConfigMutationError::InvalidRootShape)?;
            let mcp_servers = root
                .get_mut("mcpServers")
                .and_then(Value::as_object_mut)
                .ok_or(McpConfigMutationError::InvalidRootShape)?;

            let old_server = mcp_servers.get(old_name).cloned();
            if old_server.is_none() {
                return Err(McpConfigMutationError::ServerNotFound(old_name.clone()));
            }
            if old_name != new_name && mcp_servers.contains_key(new_name) {
                return Err(McpConfigMutationError::ValidationFailed(format!(
                    "server '{new_name}' already exists; rename target must not collide with an existing server"
                )));
            }

            let server_value = serde_json::to_value(server)
                .map_err(|error| McpConfigMutationError::ValidationFailed(format!("failed to serialize replacement server: {error}")))?;

            // A rename drops the old entry first; a same-name replacement
            // overwrites the entry in place. Both require the source entry to
            // exist, which was checked above.
            if old_name != new_name {
                mcp_servers.remove(old_name);
            }
            mcp_servers.insert(new_name.clone(), server_value.clone());
            let changed_fields = diff_server_fields(old_server.as_ref(), Some(&server_value));
            Ok((new_name.clone(), "replace".to_string(), changed_fields, document))
        }
        McpConfigMutation::RemoveServer(name) => {
            let mut document = load_raw_config_document_from_path(path)?;
            let root = document.as_object_mut().ok_or(McpConfigMutationError::InvalidRootShape)?;
            let mcp_servers = root
                .get_mut("mcpServers")
                .and_then(Value::as_object_mut)
                .ok_or(McpConfigMutationError::InvalidRootShape)?;
            let removed = mcp_servers.remove(name);
            if removed.is_none() {
                return Err(McpConfigMutationError::ServerNotFound(name.clone()));
            }
            // Every persisted field of the entry is gone after removal.
            let changed_fields = removed
                .as_ref()
                .and_then(Value::as_object)
                .map(|entry| entry.keys().cloned().collect())
                .unwrap_or_default();
            Ok((name.clone(), "remove".to_string(), changed_fields, document))
        }
        McpConfigMutation::SetHttpAutostart { auto_start } => {
            let mut document = load_raw_config_document_from_path(path)?;
            let root = document.as_object_mut().ok_or(McpConfigMutationError::InvalidRootShape)?;
            let default_http_server = serde_json::to_value(McpHttpServerConfig::default())
                .map_err(|error| McpConfigMutationError::ValidationFailed(format!("failed to serialize HTTP server defaults: {error}")))?;
            let http_server = root.entry("httpServer").or_insert(default_http_server);
            let http_server = http_server
                .as_object_mut()
                .ok_or_else(|| McpConfigMutationError::ValidationFailed("httpServer must be an object".to_string()))?;
            http_server.insert("autoStart".to_string(), Value::Bool(*auto_start));
            Ok((
                "httpServer".to_string(),
                "setHttpAutostart".to_string(),
                vec!["autoStart".to_string()],
                document,
            ))
        }
    }
}

/// Resolve a caller's connect intent against the freshly loaded candidate.
///
/// [`ReloadPolicy::ConfigureTarget`] carries intent only: a server that is
/// disabled in the resulting configuration is never connected. Other policies
/// pass through unchanged.
fn resolve_policy_connect_intent(policy: ReloadPolicy, config: &McpConfig) -> ReloadPolicy {
    match policy {
        ReloadPolicy::ConfigureTarget { name, connect } => {
            let enabled = config.mcp_servers.get(&name).map(|server| !server.disabled).unwrap_or(false);
            ReloadPolicy::ConfigureTarget {
                name,
                connect: connect && enabled,
            }
        }
        other => other,
    }
}

fn mutation_outcome_from_server_apply(
    server_name: String,
    operation: String,
    changed_fields: Vec<String>,
    server_outcome: Option<&McpServerApplyOutcome>,
) -> McpConfigMutationOutcome {
    let (status, runtime_applied, connection_attempted) = server_outcome
        .map(|o| (o.status, o.runtime_applied, o.connection_attempted))
        .unwrap_or((ServerStatus::Unknown, false, false));

    let error = safe_error_for_server_outcome(server_outcome);

    McpConfigMutationOutcome {
        server_name,
        operation,
        changed_fields,
        persisted: true,
        runtime_applied,
        connection_attempted,
        status,
        error,
    }
}

fn safe_error_for_server_outcome(server_outcome: Option<&McpServerApplyOutcome>) -> Option<SafeError> {
    let outcome = server_outcome?;
    if !outcome.runtime_applied {
        Some(SafeError {
            error_code: ErrorCategory::RuntimeApplyFailed,
            stage: ErrorStage::RuntimeApply,
            next_step: "The server was saved, but runtime updates could not be confirmed. Inspect server status and retry.".to_string(),
        })
    } else if outcome.connection_attempted && outcome.connection_error.is_some() {
        Some(SafeError {
            error_code: ErrorCategory::ConnectionFailed,
            stage: ErrorStage::Connect,
            next_step:
                "The server was persisted but could not be connected. Verify the command or endpoint and retry with mcp_server_set_running."
                    .to_string(),
        })
    } else {
        None
    }
}

/// Tokenize env, header, and auth values without rewriting the unresolved document.
///
/// Env and header entries can be either object maps (`{ KEY: VALUE }`) or
/// arrays of `{ key, value }` rows. Existing `${secret:NAME}` and
/// `${env:NAME}` placeholders pass through unchanged. For all other values, the
/// same secret heuristic used by `tokenize_env` decides whether a value is
/// moved into the configured store; low-entropy values remain plaintext. Auth
/// fields (`username`, `password`, `token`) use `auth.<field>` token keys so
/// their store service names cannot collide with an env or header entry that is
/// literally named e.g. `password`.
fn tokenize_document_values(document: &mut Value) -> Result<(), McpConfigMutationError> {
    let servers = document
        .get_mut("mcpServers")
        .and_then(Value::as_object_mut)
        .ok_or(McpConfigMutationError::InvalidRootShape)?;
    for (name, server) in servers {
        let server = server
            .as_object_mut()
            .ok_or_else(|| value_path_error(format!("mcpServers.{name}"), "an object"))?;
        for field in ["env", "headers"] {
            let Some(collection) = server.get_mut(field) else {
                continue;
            };
            if let Some(entries) = collection.as_object_mut() {
                for (key, value) in entries.iter_mut() {
                    let raw = value
                        .as_str()
                        .ok_or_else(|| value_path_error(format!("mcpServers.{name}.{field}.{key}"), "a string"))?;
                    *value = Value::String(tokenize_value(name, key, raw)?.value);
                }
            } else if let Some(entries) = collection.as_array_mut() {
                for (index, entry) in entries.iter_mut().enumerate() {
                    let item = entry
                        .as_object_mut()
                        .ok_or_else(|| value_path_error(format!("mcpServers.{name}.{field}[{index}]"), "an object"))?;
                    let key = item
                        .get("key")
                        .and_then(Value::as_str)
                        .ok_or_else(|| value_path_error(format!("mcpServers.{name}.{field}[{index}].key"), "a string"))?
                        .to_string();
                    let raw = item
                        .get("value")
                        .and_then(Value::as_str)
                        .ok_or_else(|| value_path_error(format!("mcpServers.{name}.{field}.{key}.value"), "a string"))?
                        .to_string();
                    let tokenized = tokenize_value(name, &key, &raw)?;
                    let persisted_source = item
                        .get("source")
                        .and_then(|source| serde_json::from_value::<EnvSource>(source.clone()).ok());
                    // A plaintext replacement of a previously-secret value must
                    // also refresh the source label; otherwise the runtime keeps
                    // masking a value that is no longer a secret. Only the
                    // masking-relevant Secret -> non-Secret transition is
                    // rewritten, so unrelated File/Raw rows stay untouched.
                    let stale_secret_label = persisted_source
                        .as_ref()
                        .is_some_and(|source| matches!(source, EnvSource::Secret) && !matches!(&tokenized.source, EnvSource::Secret));
                    let update_source =
                        tokenized.value != raw || matches!(&tokenized.source, EnvSource::Secret | EnvSource::Env) || stale_secret_label;
                    item.insert("value".to_string(), Value::String(tokenized.value));
                    if update_source {
                        item.insert(
                            "source".to_string(),
                            serde_json::to_value(tokenized.source).expect("EnvSource is serializable"),
                        );
                    }
                }
            } else {
                return Err(value_path_error(format!("mcpServers.{name}.{field}"), "an object or an array"));
            }
        }

        // Auth fields (`username`, `password`, `token`) support the same
        // `${secret:NAME}` / `${env:NAME}` interpolation as env/header values.
        if let Some(auth) = server.get_mut("auth").and_then(Value::as_object_mut) {
            for field in ["username", "password", "token"] {
                let Some(raw) = auth.get(field).and_then(Value::as_str).map(str::to_string) else {
                    continue;
                };
                let tokenized = tokenize_value(name, &auth_token_key(field), &raw)?;
                auth.insert(field.to_string(), Value::String(tokenized.value));
            }
        }
    }
    Ok(())
}

/// Returns the token key used for an auth field.
///
/// Namespacing keeps keychain service names (`{server}-{key}`) distinct from an
/// env or header entry literally named `username`, `password`, or `token`.
fn auth_token_key(field: &str) -> String {
    format!("auth.{field}")
}

/// Builds a field-path validation error for a malformed raw document value.
fn value_path_error(path: String, expectation: &str) -> McpConfigMutationError {
    McpConfigMutationError::ValidationFailed(format!("{path} must be {expectation}"))
}

/// Apply the same credential policy as `save_config_to_path` to one raw value.
fn tokenize_value(name: &str, key: &str, raw: &str) -> Result<EnvVar, McpConfigMutationError> {
    // References already name their storage source; sending them to tokenize_env
    // would store the placeholder itself as a new secret in keychain mode.
    if raw.contains("${secret:") {
        return Ok(EnvVar::new(key.to_string(), raw.to_string(), EnvSource::Secret));
    }
    if raw.contains("${env:") {
        return Ok(EnvVar::new(key.to_string(), raw.to_string(), EnvSource::Env));
    }
    let mut values = IndexSet::from([EnvVar::new(key.to_string(), raw.to_string(), EnvSource::File)]);
    tokenize_env(&mut values, &name.to_string())
        .map_err(|e| McpConfigMutationError::TokenizationFailed(format!("failed to tokenize {name}.{key}: {e}")))?;
    Ok(values.into_iter().next().expect("singleton env set"))
}

/// Serialize and write the unresolved document, creating parent directories.
fn persist_document(path: &PathBuf, document: &Value) -> Result<(), McpConfigMutationError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(McpConfigMutationError::SaveFailed)?;
    }
    let content =
        serde_json::to_string_pretty(document).map_err(|e| McpConfigMutationError::SaveFailed(std::io::Error::other(e.to_string())))?;
    std::fs::write(path, content).map_err(McpConfigMutationError::SaveFailed)?;
    Ok(())
}

/// Compute the top-level schema field names that differ between two server
/// documents, in stable sorted order.
fn diff_server_fields(old: Option<&Value>, new: Option<&Value>) -> Vec<String> {
    let old_object = old.and_then(Value::as_object);
    let new_object = new.and_then(Value::as_object);

    let mut changed = BTreeSet::new();
    match (old_object, new_object) {
        (None, Some(new_obj)) => {
            changed.extend(new_obj.keys().cloned());
        }
        (Some(old_obj), Some(new_obj)) => {
            for key in old_obj.keys().chain(new_obj.keys()) {
                if old_obj.get(key) != new_obj.get(key) {
                    changed.insert(key.clone());
                }
            }
        }
        (Some(_), None) | (None, None) => {}
    }

    changed.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConfigureMode, McpConfig, McpServer, McpServerPatch, load_config_from_path, load_raw_config_document_from_path};
    use oatty_registry::CommandRegistry;
    use std::sync::Mutex;

    #[test]
    fn saved_mutation_reports_unconfirmed_start_as_runtime_apply_failure() {
        let server_outcome = McpServerApplyOutcome {
            name: "demo".into(),
            status: ServerStatus::Running,
            runtime_applied: false,
            connection_attempted: true,
            connection_error: None,
        };
        let outcome = mutation_outcome_from_server_apply("demo".into(), "configure".into(), vec!["command".into()], Some(&server_outcome));
        assert!(outcome.persisted);
        assert!(!outcome.runtime_applied);
        assert!(outcome.connection_attempted);
        assert_eq!(outcome.status, ServerStatus::Running);
        let error = outcome.error.unwrap();
        assert_eq!(error.error_code, ErrorCategory::RuntimeApplyFailed);
        assert_eq!(error.stage, ErrorStage::RuntimeApply);
        assert!(!error.next_step.contains("secret"));
    }

    #[test]
    fn saved_mutation_keeps_connection_failure_distinct() {
        let server_outcome = McpServerApplyOutcome {
            name: "demo".into(),
            status: ServerStatus::Error,
            runtime_applied: true,
            connection_attempted: true,
            connection_error: Some(crate::plugin::PluginEngineError::ClientManagerError(
                "secret upstream diagnostic".into(),
            )),
        };
        let outcome = mutation_outcome_from_server_apply("demo".into(), "configure".into(), vec![], Some(&server_outcome));
        assert!(outcome.persisted);
        assert!(outcome.runtime_applied);
        assert!(outcome.connection_attempted);
        let error = outcome.error.unwrap();
        assert_eq!(error.error_code, ErrorCategory::ConnectionFailed);
        assert_eq!(error.stage, ErrorStage::Connect);
        assert!(!error.next_step.contains("secret upstream diagnostic"));
    }

    #[tokio::test]
    async fn autostart_first_toggle_creates_default_document_for_either_value() {
        for auto_start in [true, false] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("mcp.json");
            let engine = Arc::new(PluginEngine::new(McpConfig::default(), Arc::new(Mutex::new(CommandRegistry::default()))).unwrap());

            let outcome = apply_config_mutation(
                Arc::clone(&engine),
                path.clone(),
                McpConfigMutation::SetHttpAutostart { auto_start },
                ReloadPolicy::RestorePreviouslyRunning,
            )
            .await
            .unwrap();

            assert!(outcome.persisted);
            assert_eq!(outcome.operation, "setHttpAutostart");
            assert!(!outcome.connection_attempted);
            let persisted = load_raw_config_document_from_path(&path).unwrap();
            assert_eq!(persisted["httpServer"]["autoStart"], auto_start);
            assert_eq!(persisted["httpServer"]["bindAddress"], "127.0.0.1:62889");
            assert!(persisted["mcpServers"].as_object().unwrap().is_empty());
            assert_eq!(engine.config_snapshot().await.http_server.auto_start, auto_start);
        }
    }

    #[tokio::test]
    async fn autostart_adds_missing_http_server_without_rewriting_other_fields() {
        for auto_start in [true, false] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("mcp.json");
            let original = serde_json::json!({
                "mcpServers": {"disabled": {"command": "node", "disabled": true}},
                "extension": {"preserve": [1, 2, 3]}
            });
            std::fs::write(&path, serde_json::to_vec(&original).unwrap()).unwrap();
            let engine = Arc::new(
                PluginEngine::new(
                    load_config_from_path(&path).unwrap(),
                    Arc::new(Mutex::new(CommandRegistry::default())),
                )
                .unwrap(),
            );

            apply_config_mutation(
                engine,
                path.clone(),
                McpConfigMutation::SetHttpAutostart { auto_start },
                ReloadPolicy::RestorePreviouslyRunning,
            )
            .await
            .unwrap();

            let persisted = load_raw_config_document_from_path(&path).unwrap();
            assert_eq!(persisted["mcpServers"], original["mcpServers"]);
            assert_eq!(persisted["extension"], original["extension"]);
            assert_eq!(persisted["httpServer"]["autoStart"], auto_start);
            assert_eq!(persisted["httpServer"]["bindAddress"], "127.0.0.1:62889");
        }
    }

    #[tokio::test]
    async fn autostart_rejects_invalid_existing_document_without_overwrite() {
        for original in ["{bad json", r#"{"mcpServers":{},"httpServer":[]}"#] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("mcp.json");
            std::fs::write(&path, original).unwrap();
            let engine = Arc::new(PluginEngine::new(McpConfig::default(), Arc::new(Mutex::new(CommandRegistry::default()))).unwrap());

            assert!(
                apply_config_mutation(
                    engine,
                    path.clone(),
                    McpConfigMutation::SetHttpAutostart { auto_start: true },
                    ReloadPolicy::RestorePreviouslyRunning,
                )
                .await
                .is_err()
            );
            assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        }
    }

    #[tokio::test]
    async fn configure_adds_server_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");

        let engine = Arc::new(PluginEngine::new(McpConfig::default(), Arc::new(Mutex::new(CommandRegistry::default()))).unwrap());

        let mut patch = McpServerPatch::default();
        patch.configuration.command = Some("node".to_string());

        let outcome = apply_config_mutation(
            Arc::clone(&engine),
            path.clone(),
            McpConfigMutation::ConfigureServer {
                name: "demo".to_string(),
                patch,
                mode: ConfigureMode::Add,
            },
            ReloadPolicy::ConfigureTarget {
                name: "demo".to_string(),
                connect: false,
            },
        )
        .await
        .unwrap();

        assert!(outcome.persisted);
        assert!(outcome.runtime_applied);
        assert!(!outcome.connection_attempted);
        assert_eq!(outcome.server_name, "demo");
        assert_eq!(outcome.operation, "configure");

        let persisted = load_raw_config_document_from_path(&path).unwrap();
        assert_eq!(persisted["mcpServers"]["demo"]["command"], "node");
    }

    #[tokio::test]
    async fn configure_preserves_unrelated_raw_fields_and_collection_shapes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        let original = serde_json::json!({
            "mcpServers": {
                "target": {"command": "node", "disabled": false, "env": {"PLAIN": "value"}},
                "other": {
                    "command": "node",
                    "disabled": false,
                    "headers": [{"key": "X-Keep", "value": "plain", "source": "File", "effective": false}]
                }
            },
            "httpServer": {"autoStart": false, "bindAddress": "127.0.0.1:62889"},
            "extension": {"keep": true}
        });
        std::fs::write(&path, serde_json::to_string(&original).unwrap()).unwrap();
        let config: McpConfig = serde_json::from_value(original.clone()).unwrap();
        let engine = Arc::new(PluginEngine::new(config, Arc::new(Mutex::new(CommandRegistry::default()))).unwrap());

        let mut patch = McpServerPatch::default();
        patch.configuration.tags = Some(vec!["updated".to_string()]);
        apply_config_mutation(
            engine,
            path.clone(),
            McpConfigMutation::ConfigureServer {
                name: "target".to_string(),
                patch,
                mode: ConfigureMode::Edit,
            },
            ReloadPolicy::ConfigureTarget {
                name: "target".to_string(),
                connect: false,
            },
        )
        .await
        .unwrap();

        let persisted = load_raw_config_document_from_path(&path).unwrap();
        assert_eq!(persisted["extension"], original["extension"]);
        assert_eq!(persisted["httpServer"], original["httpServer"]);
        assert_eq!(persisted["mcpServers"]["other"], original["mcpServers"]["other"]);
        assert_eq!(persisted["mcpServers"]["target"]["env"], original["mcpServers"]["target"]["env"]);
        assert_eq!(persisted["mcpServers"]["target"]["tags"], serde_json::json!(["updated"]));
    }

    #[test]
    fn tokenized_map_and_array_secrets_remain_masked_after_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        let mut document = serde_json::json!({
            "mcpServers": {
                "map": {
                    "baseUrl": "https://example.com",
                    "headers": {"Authorization": "${secret:OATTY_REVIEW_TEST_SECRET}"},
                    "disabled": false
                },
                "array": {
                    "command": "node",
                    "env": [{"key": "TOKEN", "value": "${secret:OATTY_REVIEW_TEST_SECRET}", "source": "File"}],
                    "disabled": false
                }
            }
        });

        temp_env::with_var("OATTY_SECRETS_BACKEND", None::<&str>, || {
            tokenize_document_values(&mut document).unwrap();
        });
        assert!(document["mcpServers"]["map"]["headers"].is_object());
        assert_eq!(
            document["mcpServers"]["map"]["headers"]["Authorization"],
            "${secret:OATTY_REVIEW_TEST_SECRET}"
        );
        assert_eq!(
            document["mcpServers"]["array"]["env"][0]["value"],
            "${secret:OATTY_REVIEW_TEST_SECRET}"
        );
        assert_eq!(document["mcpServers"]["array"]["env"][0]["source"], "Secret");
        std::fs::write(&path, serde_json::to_string(&document).unwrap()).unwrap();

        let loaded = temp_env::with_vars(
            [
                ("OATTY_SECRETS_BACKEND", Some("env")),
                ("OATTY_REVIEW_TEST_SECRET", Some("resolved-secret")),
            ],
            || load_config_from_path(&path).unwrap(),
        );
        for (server, field) in [("map", "headers"), ("array", "env")] {
            let value = if field == "headers" {
                loaded.mcp_servers[server].headers.iter().next().unwrap()
            } else {
                loaded.mcp_servers[server].env.iter().next().unwrap()
            };
            assert_eq!(value.value, "resolved-secret");
            assert_eq!(value.source, EnvSource::Secret);
            assert_eq!(value.masked().value, "••••••••••••••••");
        }
    }

    #[test]
    fn auth_fields_are_tokenized_before_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        let mut document = serde_json::json!({
            "mcpServers": {
                "protected": {
                    "baseUrl": "https://example.com",
                    "auth": {
                        "scheme": "basic",
                        "username": "alice",
                        "password": "hunter2",
                        "token": "${secret:OATTY_REVIEW_TEST_SECRET}"
                    },
                    "disabled": false
                }
            }
        });

        temp_env::with_var("OATTY_SECRETS_BACKEND", None::<&str>, || {
            tokenize_document_values(&mut document).unwrap();
        });

        let auth = &document["mcpServers"]["protected"]["auth"];
        // Plain credentials should be persisted (tokenize_env may move them to
        // the configured store, but the value itself is retained as a string).
        assert!(auth["username"].is_string());
        assert!(auth["password"].is_string());
        // A `${secret:NAME}` reference must pass through unchanged — it must
        // NOT be stored as plaintext in the persisted document.
        assert_eq!(auth["token"].as_str().unwrap(), "${secret:OATTY_REVIEW_TEST_SECRET}");
        // And the raw password must not appear in the serialized document
        // when the backend replaces it with a secret reference.
        let serialized = serde_json::to_string(&document).unwrap();
        std::fs::write(&path, &serialized).unwrap();

        // Round-trip through load_config_from_path with the env backend to
        // confirm auth fields reload with the correct source.
        let loaded = temp_env::with_vars(
            [
                ("OATTY_SECRETS_BACKEND", Some("env")),
                ("OATTY_REVIEW_TEST_SECRET", Some("resolved-secret")),
            ],
            || load_config_from_path(&path).unwrap(),
        );
        let auth = loaded.mcp_servers["protected"].auth.as_ref().expect("auth block present");
        assert_eq!(auth.token.as_deref(), Some("resolved-secret"));
    }

    #[tokio::test]
    async fn replace_renames_server_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");

        let mut config = McpConfig::default();
        config.mcp_servers.insert("old".to_string(), McpServer::default());
        config.mcp_servers.get_mut("old").unwrap().command = Some("node".to_string());
        std::fs::write(&path, serde_json::to_string(&config).unwrap()).unwrap();

        let engine = Arc::new(PluginEngine::new(config, Arc::new(Mutex::new(CommandRegistry::default()))).unwrap());

        let replacement = McpServer {
            command: Some("node".to_string()),
            args: Some(vec!["--fast".to_string()]),
            ..Default::default()
        };

        let outcome = apply_config_mutation(
            Arc::clone(&engine),
            path.clone(),
            McpConfigMutation::ReplaceServer {
                old_name: "old".to_string(),
                new_name: "renamed".to_string(),
                server: replacement,
            },
            ReloadPolicy::RestorePreviouslyRunning,
        )
        .await
        .unwrap();

        assert!(outcome.persisted);
        assert!(outcome.runtime_applied);
        assert!(!outcome.connection_attempted);
        assert_eq!(outcome.server_name, "renamed");
        assert_eq!(outcome.operation, "replace");
        assert!(outcome.changed_fields.contains(&"args".to_string()));

        let persisted = load_raw_config_document_from_path(&path).unwrap();
        assert!(persisted["mcpServers"].get("old").is_none());
        assert_eq!(persisted["mcpServers"]["renamed"]["command"], "node");
        assert_eq!(persisted["mcpServers"]["renamed"]["args"][0], "--fast");
    }

    #[tokio::test]
    async fn replace_same_name_overwrites_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");

        let mut config = McpConfig::default();
        config.mcp_servers.insert("demo".to_string(), McpServer::default());
        config.mcp_servers.get_mut("demo").unwrap().command = Some("node".to_string());
        std::fs::write(&path, serde_json::to_string(&config).unwrap()).unwrap();

        let engine = Arc::new(PluginEngine::new(config, Arc::new(Mutex::new(CommandRegistry::default()))).unwrap());

        let replacement = McpServer {
            command: Some("deno".to_string()),
            ..Default::default()
        };

        let outcome = apply_config_mutation(
            Arc::clone(&engine),
            path.clone(),
            McpConfigMutation::ReplaceServer {
                old_name: "demo".to_string(),
                new_name: "demo".to_string(),
                server: replacement,
            },
            ReloadPolicy::RestorePreviouslyRunning,
        )
        .await
        .unwrap();

        assert!(outcome.persisted);
        assert_eq!(outcome.server_name, "demo");
        assert_eq!(outcome.operation, "replace");

        let persisted = load_raw_config_document_from_path(&path).unwrap();
        assert_eq!(persisted["mcpServers"].as_object().unwrap().len(), 1);
        assert_eq!(persisted["mcpServers"]["demo"]["command"], "deno");
    }

    #[tokio::test]
    async fn remove_deletes_server_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");

        let mut config = McpConfig::default();
        config.mcp_servers.insert("demo".to_string(), McpServer::default());
        config.mcp_servers.get_mut("demo").unwrap().command = Some("node".to_string());
        std::fs::write(&path, serde_json::to_string(&config).unwrap()).unwrap();

        let engine = Arc::new(PluginEngine::new(config, Arc::new(Mutex::new(CommandRegistry::default()))).unwrap());

        let outcome = apply_config_mutation(
            Arc::clone(&engine),
            path.clone(),
            McpConfigMutation::RemoveServer("demo".to_string()),
            ReloadPolicy::RestorePreviouslyRunning,
        )
        .await
        .unwrap();

        assert!(outcome.persisted);
        assert!(outcome.runtime_applied);
        assert_eq!(outcome.server_name, "demo");
        assert_eq!(outcome.operation, "remove");
        assert!(outcome.changed_fields.contains(&"command".to_string()));

        let persisted = load_raw_config_document_from_path(&path).unwrap();
        assert!(persisted["mcpServers"].as_object().unwrap().is_empty());
    }

    #[tokio::test]
    async fn remove_missing_server_returns_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");

        let engine = Arc::new(PluginEngine::new(McpConfig::default(), Arc::new(Mutex::new(CommandRegistry::default()))).unwrap());

        let error = apply_config_mutation(
            Arc::clone(&engine),
            path.clone(),
            McpConfigMutation::RemoveServer("ghost".to_string()),
            ReloadPolicy::RestorePreviouslyRunning,
        )
        .await
        .unwrap_err();

        assert!(matches!(error, McpConfigMutationError::ServerNotFound(name) if name == "ghost"));
        // A rejected removal must not create the configuration file.
        assert!(!path.exists());
    }

    #[test]
    fn diff_reports_only_changed_fields() {
        let old = serde_json::json!({ "command": "node", "tags": ["a"], "disabled": false });
        let new = serde_json::json!({ "command": "node", "tags": ["b"], "disabled": false });

        assert_eq!(diff_server_fields(Some(&old), Some(&new)), vec!["tags"]);
    }

    /// Regression test for H1: renaming a server through ReplaceServer must
    /// carry forward unowned fields (cwd, auth, disabled, tags) that the form
    /// does not populate from the old entry.
    #[tokio::test]
    async fn replace_rename_preserves_unowned_fields() {
        use std::path::PathBuf;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");

        // Seed the raw document with a server that has fields the form does not own.
        let seed = serde_json::json!({
            "mcpServers": {
                "old": {
                    "command": "node",
                    "cwd": "/tmp/work",
                    "auth": { "scheme": "basic", "username": "u", "password": "p" },
                    "disabled": true,
                    "tags": ["alpha", "beta"]
                }
            }
        });
        std::fs::write(&path, serde_json::to_string(&seed).unwrap()).unwrap();

        let config = load_config_from_path(&path).unwrap();
        let engine = Arc::new(PluginEngine::new(config, Arc::new(Mutex::new(CommandRegistry::default()))).unwrap());

        // Build a replacement server as the TUI would after carrying forward unowned fields.
        let replacement = McpServer {
            command: Some("node".to_string()),
            cwd: Some(PathBuf::from("/tmp/work")),
            auth: Some(crate::config::McpAuthConfig {
                scheme: "basic".to_string(),
                username: Some("u".to_string()),
                password: Some("p".to_string()),
                token: None,
                header_name: None,
                interactive: None,
            }),
            disabled: true,
            tags: Some(vec!["alpha".to_string(), "beta".to_string()]),
            ..Default::default()
        };

        let outcome = apply_config_mutation(
            Arc::clone(&engine),
            path.clone(),
            McpConfigMutation::ReplaceServer {
                old_name: "old".to_string(),
                new_name: "renamed".to_string(),
                server: replacement,
            },
            ReloadPolicy::RestorePreviouslyRunning,
        )
        .await
        .unwrap();

        assert!(outcome.persisted);
        assert_eq!(outcome.server_name, "renamed");

        let persisted = load_raw_config_document_from_path(&path).unwrap();
        let renamed = &persisted["mcpServers"]["renamed"];
        assert!(persisted["mcpServers"].get("old").is_none());
        assert_eq!(renamed["cwd"].as_str().unwrap(), "/tmp/work");
        assert_eq!(renamed["auth"]["scheme"].as_str().unwrap(), "basic");
        assert_eq!(renamed["auth"]["username"].as_str().unwrap(), "u");
        assert!(renamed["disabled"].as_bool().unwrap());
        assert_eq!(renamed["tags"][0].as_str().unwrap(), "alpha");
        assert_eq!(renamed["tags"][1].as_str().unwrap(), "beta");
    }

    #[tokio::test]
    async fn replace_rename_rejects_collision_with_existing_server() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");

        let seed = serde_json::json!({
            "mcpServers": {
                "old": {"command": "node", "disabled": false},
                "existing": {"command": "deno", "disabled": false}
            }
        });
        std::fs::write(&path, serde_json::to_string(&seed).unwrap()).unwrap();

        let config = load_config_from_path(&path).unwrap();
        let engine = Arc::new(PluginEngine::new(config, Arc::new(Mutex::new(CommandRegistry::default()))).unwrap());

        let error = apply_config_mutation(
            Arc::clone(&engine),
            path.clone(),
            McpConfigMutation::ReplaceServer {
                old_name: "old".to_string(),
                new_name: "existing".to_string(),
                server: McpServer {
                    command: Some("node".to_string()),
                    ..Default::default()
                },
            },
            ReloadPolicy::RestorePreviouslyRunning,
        )
        .await
        .unwrap_err();

        assert!(
            matches!(&error, McpConfigMutationError::ValidationFailed(message) if message.contains("already exists")),
            "got {error:?}"
        );

        let persisted = load_raw_config_document_from_path(&path).unwrap();
        assert_eq!(persisted["mcpServers"]["old"]["command"], "node");
        assert_eq!(persisted["mcpServers"]["existing"]["command"], "deno");
    }

    #[tokio::test]
    async fn replace_nonexistent_server_returns_not_found() {
        for (old_name, new_name) in [("ghost", "renamed"), ("ghost", "ghost")] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("mcp.json");
            let engine = Arc::new(PluginEngine::new(McpConfig::default(), Arc::new(Mutex::new(CommandRegistry::default()))).unwrap());

            let error = apply_config_mutation(
                Arc::clone(&engine),
                path.clone(),
                McpConfigMutation::ReplaceServer {
                    old_name: old_name.to_string(),
                    new_name: new_name.to_string(),
                    server: McpServer {
                        command: Some("node".to_string()),
                        ..Default::default()
                    },
                },
                ReloadPolicy::RestorePreviouslyRunning,
            )
            .await
            .unwrap_err();

            assert!(matches!(error, McpConfigMutationError::ServerNotFound(name) if name == "ghost"));
            assert!(!path.exists(), "a rejected replacement must not create the configuration file");
        }
    }

    #[test]
    fn tokenize_document_values_reports_value_paths_as_validation_errors() {
        let mut map_document = serde_json::json!({
            "mcpServers": {"server": {"command": "node", "env": {"KEY": 42}}}
        });
        let error = tokenize_document_values(&mut map_document).unwrap_err();
        assert!(
            matches!(&error, McpConfigMutationError::ValidationFailed(message) if message.contains("mcpServers.server.env.KEY")),
            "got {error:?}"
        );
        assert!(!matches!(error, McpConfigMutationError::InvalidRootShape));

        let mut array_document = serde_json::json!({
            "mcpServers": {"server": {"command": "node", "env": [{"key": "KEY"}]}}
        });
        let error = tokenize_document_values(&mut array_document).unwrap_err();
        assert!(
            matches!(&error, McpConfigMutationError::ValidationFailed(message) if message.contains("mcpServers.server.env.KEY.value")),
            "got {error:?}"
        );
        assert!(!matches!(error, McpConfigMutationError::InvalidRootShape));
    }

    #[test]
    fn auth_token_keys_are_namespaced_and_round_trip_through_reload() {
        assert_eq!(auth_token_key("username"), "auth.username");
        assert_eq!(auth_token_key("password"), "auth.password");
        assert_eq!(auth_token_key("token"), "auth.token");
        // Auth keys must not collide with an env/header entry named "password".
        assert_ne!(auth_token_key("password"), "password");

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        let document = serde_json::json!({
            "mcpServers": {
                "protected": {
                    "baseUrl": "https://example.com",
                    "auth": {"scheme": "basic", "password": "${secret:protected-auth.password}"},
                    "disabled": false
                }
            }
        });
        std::fs::write(&path, serde_json::to_string(&document).unwrap()).unwrap();

        let loaded = temp_env::with_vars(
            [
                ("OATTY_SECRETS_BACKEND", Some("env")),
                ("protected-auth.password", Some("round-trip-secret")),
            ],
            || load_config_from_path(&path).unwrap(),
        );

        let auth = loaded.mcp_servers["protected"].auth.as_ref().expect("auth block present");
        assert_eq!(auth.password.as_deref(), Some("round-trip-secret"));
    }

    #[test]
    fn plaintext_value_clears_stale_secret_source_label() {
        let mut document = serde_json::json!({
            "mcpServers": {
                "server": {
                    "command": "node",
                    "env": [{"key": "PLAIN", "value": "plain-text-value", "source": "Secret"}]
                }
            }
        });

        temp_env::with_var("OATTY_SECRETS_BACKEND", Some("env"), || {
            tokenize_document_values(&mut document).unwrap();
        });

        let row = &document["mcpServers"]["server"]["env"][0];
        assert_eq!(row["value"], "plain-text-value");
        assert_eq!(row["source"], "Raw", "stale Secret label must not survive a plaintext value");
    }

    #[tokio::test]
    async fn derive_mode_adds_missing_server_and_edits_existing_under_guard() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        let engine = Arc::new(PluginEngine::new(McpConfig::default(), Arc::new(Mutex::new(CommandRegistry::default()))).unwrap());

        let mut add_patch = McpServerPatch::default();
        add_patch.configuration.command = Some("node".to_string());
        apply_config_mutation(
            Arc::clone(&engine),
            path.clone(),
            McpConfigMutation::ConfigureServer {
                name: "derived".to_string(),
                patch: add_patch,
                mode: ConfigureMode::Derive,
            },
            ReloadPolicy::ConfigureTarget {
                name: "derived".to_string(),
                connect: false,
            },
        )
        .await
        .unwrap();

        let mut edit_patch = McpServerPatch::default();
        edit_patch.configuration.args = Some(vec!["--edited".to_string()]);
        let outcome = apply_config_mutation(
            Arc::clone(&engine),
            path.clone(),
            McpConfigMutation::ConfigureServer {
                name: "derived".to_string(),
                patch: edit_patch,
                mode: ConfigureMode::Derive,
            },
            ReloadPolicy::ConfigureTarget {
                name: "derived".to_string(),
                connect: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(outcome.operation, "configure");
        let persisted = load_raw_config_document_from_path(&path).unwrap();
        assert_eq!(persisted["mcpServers"]["derived"]["command"], "node");
        assert_eq!(persisted["mcpServers"]["derived"]["args"], serde_json::json!(["--edited"]));
    }

    #[tokio::test]
    async fn connect_intent_is_resolved_against_the_guarded_disabled_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        let seed = serde_json::json!({
            "mcpServers": {
                "paused": {"command": "node", "disabled": true}
            }
        });
        std::fs::write(&path, serde_json::to_string(&seed).unwrap()).unwrap();
        let config = load_config_from_path(&path).unwrap();
        let engine = Arc::new(PluginEngine::new(config, Arc::new(Mutex::new(CommandRegistry::default()))).unwrap());

        let mut patch = McpServerPatch::default();
        patch.configuration.tags = Some(vec!["updated".to_string()]);
        let outcome = apply_config_mutation(
            Arc::clone(&engine),
            path.clone(),
            McpConfigMutation::ConfigureServer {
                name: "paused".to_string(),
                patch,
                mode: ConfigureMode::Derive,
            },
            ReloadPolicy::ConfigureTarget {
                name: "paused".to_string(),
                connect: true,
            },
        )
        .await
        .unwrap();

        assert!(outcome.persisted);
        assert!(!outcome.connection_attempted, "disabled servers must never be connected");
        assert!(outcome.runtime_applied);
    }

    #[tokio::test]
    async fn cancellation_of_caller_does_not_cancel_the_guarded_worker() {
        use std::time::Duration;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mcp.json");
        let engine = Arc::new(PluginEngine::new(McpConfig::default(), Arc::new(Mutex::new(CommandRegistry::default()))).unwrap());

        let mut patch = McpServerPatch::default();
        patch.configuration.command = Some("node".to_string());
        let caller = tokio::spawn(apply_config_mutation(
            Arc::clone(&engine),
            path.clone(),
            McpConfigMutation::ConfigureServer {
                name: "detached".to_string(),
                patch,
                mode: ConfigureMode::Add,
            },
            ReloadPolicy::RestorePreviouslyRunning,
        ));

        // Give the caller time to spawn its worker, then cancel the caller.
        tokio::time::sleep(Duration::from_millis(20)).await;
        caller.abort();
        let _ = caller.await;

        for _ in 0..100 {
            if path.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(path.exists(), "the detached worker must still persist the configuration");
        let persisted = load_raw_config_document_from_path(&path).unwrap();
        assert_eq!(persisted["mcpServers"]["detached"]["command"], "node");
    }
}
