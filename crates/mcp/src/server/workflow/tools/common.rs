//! Shared helpers for resolving runtime workflows from tool request payloads.

use crate::server::workflow::errors::{internal_error, validation_error_with_violations};
use crate::server::workflow::services::storage::{find_manifest_record, parse_manifest_content};
use anyhow::Result;
use oatty_engine::RegistryCommandRunner;
use oatty_registry::CommandRegistry;
use oatty_types::workflow::RuntimeWorkflow;
use rmcp::model::ErrorData;
use serde_json::Value;
use std::sync::{Arc, Mutex};

pub fn resolve_runtime_workflow(
    workflow_identifier: Option<&str>,
    manifest_content: Option<&str>,
    format_hint: Option<&str>,
) -> Result<RuntimeWorkflow> {
    if let Some(workflow_identifier) = workflow_identifier {
        let Some(record) = find_manifest_record(workflow_identifier)? else {
            return Err(anyhow::anyhow!("workflow '{}' was not found", workflow_identifier));
        };
        return oatty_engine::workflow::document::runtime_workflow_from_definition(&record.definition);
    }

    let manifest_content = manifest_content.ok_or_else(|| anyhow::anyhow!("either workflow_id or manifest_content must be provided"))?;
    let (definition, _) = parse_manifest_content(manifest_content, format_hint)?;
    oatty_engine::workflow::document::runtime_workflow_from_definition(&definition)
}

/// Collects structured preflight violations for workflow command/catalog readiness.
pub fn collect_workflow_preflight_violations(
    workflow: &RuntimeWorkflow,
    command_registry: &Arc<Mutex<CommandRegistry>>,
) -> Result<Vec<Value>, ErrorData> {
    let registry_snapshot = command_registry
        .lock()
        .map_err(|error| {
            internal_error(
                "WORKFLOW_COMMAND_VALIDATION_REGISTRY_LOCK_FAILED",
                format!("registry lock failed: {error}"),
                serde_json::json!({ "workflow_id": workflow.identifier }),
                "Retry workflow validation or run.",
            )
        })?
        .clone();
    let runner = RegistryCommandRunner::new(registry_snapshot);

    let violations = runner
        .validate_workflow_execution_readiness(workflow)
        .into_iter()
        .map(|violation| {
            serde_json::json!({
                "path": format!("steps[{}].run", violation.step_index),
                "rule": violation.code,
                "message": violation.message,
                "step_id": violation.step_id,
                "run": violation.run,
                "next_step": violation.suggested_action,
            })
        })
        .collect();

    Ok(violations)
}

/// Builds a structured invalid-params error when preflight violations exist.
pub fn build_preflight_validation_error(
    workflow_identifier: &str,
    violations: Vec<Value>,
    error_code: &str,
    message: &str,
    suggested_action: &str,
) -> Option<ErrorData> {
    if violations.is_empty() {
        return None;
    }

    Some(validation_error_with_violations(
        error_code,
        message,
        serde_json::json!({
            "workflow_id": workflow_identifier,
            "violation_count": violations.len(),
        }),
        suggested_action,
        violations,
    ))
}
