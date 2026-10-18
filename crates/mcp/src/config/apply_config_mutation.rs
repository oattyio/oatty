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
        other => {
            return Err(McpConfigMutationError::ValidationFailed(format!(
                "unsupported mutation variant: {other:?}"
            )));
        }
    };

    // Validate the complete candidate before touching disk so a rejected
    // candidate is never persisted.
    deserialize_validated_candidate(document.clone())?;

    persist_document(&path, &document)?;

    // Reload the now-persisted configuration and apply it under the guard we
    // already hold. Runtime application must not reacquire the guard.
    let new_config = load_config_from_path(&path).map_err(|e| McpConfigMutationError::RuntimeLoadFailed(e.to_string()))?;
    let apply_outcome = engine
        .update_config_under_guard(new_config, &policy, &guard)
        .await
        .map_err(|e| McpConfigMutationError::RuntimeApplyFailed(e.to_string()))?;

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
    use crate::config::{ConfigureMode, McpConfig, McpServerPatch, load_raw_config_document_from_path};
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

    #[test]
    fn diff_reports_only_changed_fields() {
        let old = serde_json::json!({ "command": "node", "tags": ["a"], "disabled": false });
        let new = serde_json::json!({ "command": "node", "tags": ["b"], "disabled": false });

        assert_eq!(diff_server_fields(Some(&old), Some(&new)), vec!["tags"]);
    }
}
