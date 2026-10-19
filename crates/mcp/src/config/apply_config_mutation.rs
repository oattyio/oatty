//! Orchestration for shared configuration mutations.
//!
//! This module owns the single entry point that acquires the operation guard
//! once and retains it across read/merge/save/reload so concurrent tool and TUI
//! mutations cannot interleave. The pure patch helpers live in the sibling
//! [`super::mutation`] module.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use super::interpolation::tokenize_config;
use super::io::{load_config_from_path, load_raw_config_document_from_path};
use super::mutation::{
    ConfigureServerMutation, ErrorCategory, ErrorStage, McpConfigMutation, McpConfigMutationError, McpConfigMutationOutcome, ReloadPolicy,
    SafeError, ServerStatus, apply_server_patch, deserialize_validated_candidate,
};
use crate::PluginEngine;

/// Apply a configuration mutation with shared operation guard semantics.
///
/// This is the single entry point for tool and TUI mutation paths. It acquires
/// the operation guard once and retains it across read, merge, save, and
/// runtime reload.
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
    let guard = engine.acquire_operation_guard().await;

    let (server_name, operation, changed_fields, document) = match mutation {
        McpConfigMutation::ConfigureServer { name, patch, mode } => {
            let mut document = load_raw_config_document_from_path(&path)?;
            let old_server = document.get("mcpServers").and_then(|m| m.get(&name)).cloned();
            let mirror = ConfigureServerMutation {
                name: name.clone(),
                patch,
                mode,
            };
            apply_server_patch(&mut document, &mirror)?;
            let new_server = document.get("mcpServers").and_then(|m| m.get(&name)).cloned();
            let changed_fields = diff_server_fields(old_server.as_ref(), new_server.as_ref());
            (name, "configure".to_string(), changed_fields, document)
        }
        McpConfigMutation::ReplaceServer {
            old_name,
            new_name,
            server,
        } => {
            let mut document = load_raw_config_document_from_path(&path)?;
            let old_server = document.get("mcpServers").and_then(|m| m.get(&old_name)).cloned();
            let server_value = serde_json::to_value(&server)
                .map_err(|e| McpConfigMutationError::ValidationFailed(format!("failed to serialize replacement server: {e}")))?;
            let root = document.as_object_mut().ok_or(McpConfigMutationError::InvalidRootShape)?;
            let mcp_servers = root
                .get_mut("mcpServers")
                .and_then(Value::as_object_mut)
                .ok_or(McpConfigMutationError::InvalidRootShape)?;
            // A rename drops the old entry first; a same-name replacement
            // overwrites the entry in place.
            if old_name != new_name {
                mcp_servers.remove(&old_name);
            }
            mcp_servers.insert(new_name.clone(), server_value.clone());
            let changed_fields = diff_server_fields(old_server.as_ref(), Some(&server_value));
            (new_name, "replace".to_string(), changed_fields, document)
        }
        McpConfigMutation::RemoveServer(name) => {
            let mut document = load_raw_config_document_from_path(&path)?;
            let root = document.as_object_mut().ok_or(McpConfigMutationError::InvalidRootShape)?;
            let mcp_servers = root
                .get_mut("mcpServers")
                .and_then(Value::as_object_mut)
                .ok_or(McpConfigMutationError::InvalidRootShape)?;
            let removed = mcp_servers.remove(&name);
            if removed.is_none() {
                return Err(McpConfigMutationError::ServerNotFound(name));
            }
            // Every persisted field of the entry is gone after removal.
            let changed_fields = removed
                .as_ref()
                .and_then(Value::as_object)
                .map(|entry| entry.keys().cloned().collect())
                .unwrap_or_default();
            (name, "remove".to_string(), changed_fields, document)
        }
        other => {
            return Err(McpConfigMutationError::ValidationFailed(format!(
                "unsupported mutation variant: {other:?}"
            )));
        }
    };

    // Validate the complete candidate before touching disk so a rejected
    // candidate is never persisted.
    let mut document = document;
    deserialize_validated_candidate(document.clone())?;

    // Tokenize any secret/env values before persisting so credentials are not
    // written to disk in clear text.
    let mut candidate = deserialize_validated_candidate(document.clone())?;
    tokenize_config(&mut candidate)
        .map_err(|e| McpConfigMutationError::ValidationFailed(format!("failed to tokenize configuration: {e}")))?;
    document = serde_json::to_value(candidate)
        .map_err(|e| McpConfigMutationError::ValidationFailed(format!("failed to serialize tokenized configuration: {e}")))?;

    persist_document(&path, &document)?;

    // Reload the now-persisted configuration and apply it under the guard we
    // already hold. Runtime application must not reacquire the guard.
    // Run the reload in a spawned task so cancellation of the requesting
    // future cannot interrupt it mid-flight or drop the operation guard
    // before application completes.
    let new_config = load_config_from_path(&path).map_err(|e| McpConfigMutationError::RuntimeLoadFailed(e.to_string()))?;
    let apply_task = tokio::spawn({
        let engine = Arc::clone(&engine);
        let policy = policy.clone();
        async move {
            engine
                .update_config_under_guard(new_config, &policy, &guard)
                .await
                .map_err(|e| McpConfigMutationError::RuntimeApplyFailed(e.to_string()))
        }
    });
    let apply_outcome = apply_task
        .await
        .map_err(|e| McpConfigMutationError::RuntimeApplyFailed(format!("runtime application task failed: {e}")))??;

    let server_outcome = apply_outcome.applied_servers.iter().find(|o| o.name == server_name);
    let (status, runtime_applied, connection_attempted) = server_outcome
        .map(|o| (o.status, o.runtime_applied, o.connection_attempted))
        .unwrap_or((ServerStatus::Unknown, false, false));

    let error = if connection_attempted && server_outcome.is_some_and(|o| o.connection_error.is_some()) {
        Some(SafeError {
            error_code: ErrorCategory::ConnectionFailed,
            stage: ErrorStage::Connect,
            next_step:
                "The server was persisted but could not be connected. Verify the command or endpoint and retry with mcp_server_set_running."
                    .to_string(),
        })
    } else {
        None
    };

    Ok(McpConfigMutationOutcome {
        server_name,
        operation,
        changed_fields,
        persisted: true,
        runtime_applied,
        connection_attempted,
        status,
        error,
    })
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
    use crate::config::{ConfigureMode, McpConfig, McpServer, McpServerPatch, load_raw_config_document_from_path};
    use oatty_registry::CommandRegistry;
    use std::sync::Mutex;

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
}
