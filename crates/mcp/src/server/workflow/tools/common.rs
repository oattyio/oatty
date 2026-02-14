//! Shared helpers for resolving runtime workflows from tool request payloads.

use crate::server::workflow::errors::{internal_error, validation_error_with_violations};
use crate::server::workflow::services::storage::{find_manifest_record, parse_manifest_content};
use anyhow::Result;
use oatty_engine::RegistryCommandRunner;
use oatty_registry::CommandRegistry;
use oatty_types::workflow::{RuntimeWorkflow, collect_missing_catalog_requirements};
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
    let available_catalogs = registry_snapshot.config.catalogs.clone().unwrap_or_default();
    let runner = RegistryCommandRunner::new(registry_snapshot);

    let violations: Vec<Value> = runner
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

    let missing_catalog_violations = collect_missing_catalog_requirements(workflow.requires.as_ref(), available_catalogs.as_slice())
        .into_iter()
        .map(|missing_requirement| {
            let source_hint = missing_requirement.requirement.source.clone();
            let source_type_hint = missing_requirement.requirement.source_type.map(|source_type| match source_type {
                oatty_types::workflow::WorkflowCatalogRequirementSourceType::Path => "path".to_string(),
                oatty_types::workflow::WorkflowCatalogRequirementSourceType::Url => "url".to_string(),
            });
            let next_step = if let Some(source) = source_hint.as_deref() {
                format!(
                    "Install required catalog '{}' (vendor '{}') from {}{} and retry.",
                    missing_requirement.requirement.title.as_deref().unwrap_or("<untitled>"),
                    missing_requirement.requirement.vendor,
                    source,
                    source_type_hint
                        .as_deref()
                        .map(|source_type| format!(" (source_type={source_type})"))
                        .unwrap_or_default()
                )
            } else {
                format!(
                    "Install or enable a catalog for vendor '{}'{} and retry.",
                    missing_requirement.requirement.vendor,
                    missing_requirement
                        .requirement
                        .title
                        .as_deref()
                        .map(|title| format!(" with title '{}'", title))
                        .unwrap_or_default()
                )
            };

            serde_json::json!({
                "path": format!("$.requires.catalogs[{}]", missing_requirement.index),
                "rule": "catalog_requirement",
                "message": missing_requirement.reason,
                "vendor": missing_requirement.requirement.vendor,
                "title": missing_requirement.requirement.title,
                "source": source_hint,
                "source_type": source_type_hint,
                "next_step": next_step,
            })
        })
        .collect::<Vec<Value>>();

    let mut all_violations = violations;
    all_violations.extend(missing_catalog_violations);

    Ok(all_violations)
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
