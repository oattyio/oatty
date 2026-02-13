//! Manifest-oriented workflow tool handlers.

use crate::server::workflow::errors::{
    conflict_error, execution_error, internal_error, invalid_params_error, not_found_error, validation_error_with_violations,
};
use crate::server::workflow::services::storage::{
    compute_version, find_manifest_record, list_manifest_records, parse_manifest_content, sanitize_workflow_identifier,
    serialize_definition, write_manifest,
};
use crate::server::workflow::services::sync::synchronize_runtime_workflows;
use crate::server::workflow::tools::common::{build_preflight_validation_error, collect_workflow_preflight_violations};
use crate::server::workflow::tools::types::{
    WorkflowDeleteRequest, WorkflowExportRequest, WorkflowGetRequest, WorkflowImportRequest, WorkflowRenameRequest, WorkflowSaveRequest,
    WorkflowValidateRequest,
};
use oatty_registry::CommandRegistry;
use oatty_types::workflow::RuntimeWorkflow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rmcp::model::ErrorData;
use serde_json::Value;

pub fn list_workflows() -> Result<Value, ErrorData> {
    let records = list_manifest_records().map_err(|error| {
        internal_error(
            "WORKFLOW_LIST_FAILED",
            error.to_string(),
            serde_json::json!({}),
            "Verify runtime workflow directory accessibility and retry.",
        )
    })?;

    let payload = records
        .iter()
        .map(|record| {
            serde_json::json!({
                "workflow_id": record.definition.workflow,
                "title": record.definition.title,
                "description": record.definition.description,
                "path": record.path.to_string_lossy().to_string(),
                "format": record.format.as_str(),
                "version": record.version,
            })
        })
        .collect::<Vec<Value>>();

    Ok(serde_json::json!(payload))
}

pub fn get_workflow(request: &WorkflowGetRequest) -> Result<Value, ErrorData> {
    sanitize_workflow_identifier(&request.workflow_id).map_err(|error| {
        invalid_params_error(
            "WORKFLOW_IDENTIFIER_INVALID",
            error.to_string(),
            serde_json::json!({ "workflow_id": request.workflow_id }),
            "Provide a workflow identifier containing only letters, numbers, underscores, or hyphens.",
        )
    })?;

    let Some(record) = find_manifest_record(&request.workflow_id).map_err(|error| {
        internal_error(
            "WORKFLOW_GET_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": request.workflow_id }),
            "Inspect runtime workflow directory and retry.",
        )
    })?
    else {
        return Err(not_found_error(
            "WORKFLOW_NOT_FOUND",
            format!("workflow '{}' was not found", request.workflow_id),
            serde_json::json!({ "workflow_id": request.workflow_id }),
            "Use workflow.list to inspect available workflow identifiers.",
        ));
    };

    let include_content = request.include_content.unwrap_or(true);
    let include_parsed = request.include_parsed.unwrap_or(false);
    let mut payload = serde_json::Map::new();
    payload.insert("workflow_id".to_string(), serde_json::json!(record.definition.workflow));
    payload.insert("path".to_string(), serde_json::json!(record.path.to_string_lossy().to_string()));
    payload.insert("format".to_string(), serde_json::json!(record.format.as_str()));
    payload.insert("version".to_string(), serde_json::json!(record.version));
    if include_content {
        payload.insert("content".to_string(), serde_json::json!(record.content));
    }
    if include_parsed {
        payload.insert("parsed".to_string(), serde_json::json!(record.definition));
    }

    Ok(Value::Object(payload))
}

pub fn validate_workflow(request: &WorkflowValidateRequest, command_registry: &Arc<Mutex<CommandRegistry>>) -> Result<Value, ErrorData> {
    let (definition, _) = parse_manifest_content(&request.manifest_content, request.format.as_deref()).map_err(|error| {
        validation_error_with_violations(
            "WORKFLOW_PARSE_FAILED",
            error.to_string(),
            serde_json::json!({ "format": request.format }),
            "Ensure the manifest content is valid YAML or JSON and retry.",
            vec![serde_json::json!({
                "path": "$",
                "rule": "parse",
                "message": error.to_string(),
                "expected": request.format.clone().unwrap_or_else(|| "yaml|json".to_string()),
            })],
        )
    })?;

    let runtime = oatty_engine::workflow::document::runtime_workflow_from_definition(&definition).map_err(|error| {
        validation_error_with_violations(
            "WORKFLOW_VALIDATION_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": definition.workflow }),
            "Fix reported workflow schema errors and retry.",
            vec![serde_json::json!({
                "path": "$",
                "rule": "schema",
                "message": error.to_string(),
            })],
        )
    })?;

    validate_workflow_command_readiness(&runtime, command_registry)?;

    Ok(serde_json::json!({
        "valid": true,
        "workflow_id": runtime.identifier,
        "warnings": [],
    }))
}

pub fn save_workflow(request: &WorkflowSaveRequest, command_registry: &Arc<Mutex<CommandRegistry>>) -> Result<Value, ErrorData> {
    let (mut definition, format) = parse_manifest_content(&request.manifest_content, request.format.as_deref()).map_err(|error| {
        validation_error_with_violations(
            "WORKFLOW_PARSE_FAILED",
            error.to_string(),
            serde_json::json!({ "format": request.format }),
            "Ensure the manifest content is valid YAML or JSON and retry.",
            vec![serde_json::json!({
                "path": "$",
                "rule": "parse",
                "message": error.to_string(),
                "expected": request.format.clone().unwrap_or_else(|| "yaml|json".to_string()),
            })],
        )
    })?;

    let workflow_identifier = resolve_workflow_identifier(request, &mut definition)?;
    let runtime = oatty_engine::workflow::document::runtime_workflow_from_definition(&definition).map_err(|error| {
        validation_error_with_violations(
            "WORKFLOW_VALIDATION_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": workflow_identifier }),
            "Fix reported workflow schema errors and retry.",
            vec![serde_json::json!({
                "path": "$",
                "rule": "schema",
                "message": error.to_string(),
            })],
        )
    })?;
    validate_workflow_command_readiness(&runtime, command_registry)?;

    let existing = find_manifest_record(&workflow_identifier).map_err(|error| {
        internal_error(
            "WORKFLOW_LOOKUP_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": workflow_identifier }),
            "Inspect runtime workflow directory and retry.",
        )
    })?;
    let overwrite = request.overwrite.unwrap_or(false);
    if existing.is_some() && !overwrite {
        return Err(conflict_error(
            "WORKFLOW_ALREADY_EXISTS",
            format!("workflow '{}' already exists", workflow_identifier),
            serde_json::json!({ "workflow_id": workflow_identifier }),
            "Set overwrite=true or choose a different workflow identifier.",
        ));
    }
    if let (Some(expected_version), Some(current)) = (request.expected_version.as_ref(), existing.as_ref())
        && *expected_version != current.version
    {
        return Err(conflict_error(
            "WORKFLOW_VERSION_CONFLICT",
            format!(
                "workflow '{}' version mismatch (expected {}, actual {})",
                workflow_identifier, expected_version, current.version
            ),
            serde_json::json!({
                "workflow_id": workflow_identifier,
                "expected_version": expected_version,
                "actual_version": current.version
            }),
            "Refresh with workflow.get and retry save using the latest version.",
        ));
    }

    let serialized = serialize_definition(&definition, format).map_err(|error| {
        internal_error(
            "WORKFLOW_SERIALIZE_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": workflow_identifier, "format": format.as_str() }),
            "Inspect workflow schema content and retry.",
        )
    })?;
    let path = write_manifest(&workflow_identifier, &serialized, format).map_err(|error| {
        internal_error(
            "WORKFLOW_PERSIST_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": workflow_identifier }),
            "Verify write permissions for runtime workflow directory and retry.",
        )
    })?;
    let sync_summary = synchronize_runtime_workflows(command_registry).map_err(|error| {
        execution_error(
            "WORKFLOW_SYNC_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": workflow_identifier }),
            true,
            "Retry to refresh registry state.",
        )
    })?;

    Ok(serde_json::json!({
        "workflow_id": workflow_identifier,
        "path": path.to_string_lossy().to_string(),
        "format": format.as_str(),
        "version": compute_version(&serialized),
        "sync": {
            "workflow_count": sync_summary.workflow_count,
            "synthetic_command_count": sync_summary.synthetic_command_count,
        }
    }))
}

fn validate_workflow_command_readiness(
    workflow: &RuntimeWorkflow,
    command_registry: &Arc<Mutex<CommandRegistry>>,
) -> Result<(), ErrorData> {
    let violations = collect_workflow_preflight_violations(workflow, command_registry)?;
    if let Some(error) = build_preflight_validation_error(
        &workflow.identifier,
        violations,
        "WORKFLOW_COMMAND_VALIDATION_FAILED",
        "workflow references commands or catalog configuration that are not ready",
        "Fix listed command/catalog issues, then rerun workflow.validate.",
    ) {
        return Err(error);
    }
    Ok(())
}

pub fn delete_workflow(request: &WorkflowDeleteRequest, command_registry: &Arc<Mutex<CommandRegistry>>) -> Result<Value, ErrorData> {
    sanitize_workflow_identifier(&request.workflow_id).map_err(|error| {
        invalid_params_error(
            "WORKFLOW_IDENTIFIER_INVALID",
            error.to_string(),
            serde_json::json!({ "workflow_id": request.workflow_id }),
            "Provide a workflow identifier containing only letters, numbers, underscores, or hyphens.",
        )
    })?;

    let Some(record) = find_manifest_record(&request.workflow_id).map_err(|error| {
        internal_error(
            "WORKFLOW_DELETE_LOOKUP_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": request.workflow_id }),
            "Inspect runtime workflow directory and retry.",
        )
    })?
    else {
        return Err(not_found_error(
            "WORKFLOW_NOT_FOUND",
            format!("workflow '{}' was not found", request.workflow_id),
            serde_json::json!({ "workflow_id": request.workflow_id }),
            "Use workflow.list to inspect available workflow identifiers.",
        ));
    };

    crate::server::workflow::services::storage::remove_manifest(&record.path).map_err(|error| {
        internal_error(
            "WORKFLOW_DELETE_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": request.workflow_id, "path": record.path }),
            "Verify file permissions and retry workflow deletion.",
        )
    })?;
    let sync_summary = synchronize_runtime_workflows(command_registry).map_err(|error| {
        execution_error(
            "WORKFLOW_SYNC_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": request.workflow_id }),
            true,
            "Retry to refresh registry state.",
        )
    })?;

    Ok(serde_json::json!({
        "deleted": true,
        "workflow_id": request.workflow_id,
        "path": record.path.to_string_lossy().to_string(),
        "sync": {
            "workflow_count": sync_summary.workflow_count,
            "synthetic_command_count": sync_summary.synthetic_command_count,
        }
    }))
}

pub fn rename_workflow(request: &WorkflowRenameRequest, command_registry: &Arc<Mutex<CommandRegistry>>) -> Result<Value, ErrorData> {
    let source_identifier = sanitize_workflow_identifier(&request.workflow_id).map_err(|error| {
        invalid_params_error(
            "WORKFLOW_IDENTIFIER_INVALID",
            error.to_string(),
            serde_json::json!({ "workflow_id": request.workflow_id }),
            "Provide a workflow identifier containing only letters, numbers, underscores, or hyphens.",
        )
    })?;
    let destination_identifier = sanitize_workflow_identifier(&request.new_id).map_err(|error| {
        invalid_params_error(
            "WORKFLOW_IDENTIFIER_INVALID",
            error.to_string(),
            serde_json::json!({ "new_id": request.new_id }),
            "Provide a workflow identifier containing only letters, numbers, underscores, or hyphens.",
        )
    })?;
    if source_identifier == destination_identifier {
        return Err(conflict_error(
            "WORKFLOW_RENAME_NOOP",
            "source and destination workflow identifiers are identical",
            serde_json::json!({ "workflow_id": source_identifier }),
            "Provide a new workflow identifier that differs from the current id.",
        ));
    }

    let Some(source_record) = find_manifest_record(&source_identifier).map_err(|error| {
        internal_error(
            "WORKFLOW_RENAME_LOOKUP_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": source_identifier }),
            "Inspect runtime workflow directory and retry.",
        )
    })?
    else {
        return Err(not_found_error(
            "WORKFLOW_NOT_FOUND",
            format!("workflow '{}' was not found", source_identifier),
            serde_json::json!({ "workflow_id": source_identifier }),
            "Use workflow.list to inspect available workflow identifiers.",
        ));
    };

    let destination_exists = find_manifest_record(&destination_identifier).map_err(|error| {
        internal_error(
            "WORKFLOW_RENAME_LOOKUP_FAILED",
            error.to_string(),
            serde_json::json!({ "new_id": destination_identifier }),
            "Inspect runtime workflow directory and retry.",
        )
    })?;
    if destination_exists.is_some() && !request.overwrite.unwrap_or(false) {
        return Err(conflict_error(
            "WORKFLOW_ALREADY_EXISTS",
            format!("workflow '{}' already exists", destination_identifier),
            serde_json::json!({ "new_id": destination_identifier }),
            "Set overwrite=true or choose a different workflow identifier.",
        ));
    }

    let mut definition = source_record.definition.clone();
    definition.workflow = destination_identifier.clone();
    let serialized = serialize_definition(&definition, source_record.format).map_err(|error| {
        internal_error(
            "WORKFLOW_SERIALIZE_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": source_identifier, "new_id": destination_identifier }),
            "Inspect workflow schema content and retry.",
        )
    })?;
    let new_path = write_manifest(&destination_identifier, &serialized, source_record.format).map_err(|error| {
        internal_error(
            "WORKFLOW_PERSIST_FAILED",
            error.to_string(),
            serde_json::json!({ "new_id": destination_identifier }),
            "Verify write permissions for runtime workflow directory and retry.",
        )
    })?;

    if source_record.path != new_path {
        crate::server::workflow::services::storage::remove_manifest(&source_record.path).map_err(|error| {
            internal_error(
                "WORKFLOW_RENAME_CLEANUP_FAILED",
                error.to_string(),
                serde_json::json!({ "workflow_id": source_identifier, "path": source_record.path }),
                "Manual cleanup may be required; retry rename after cleanup.",
            )
        })?;
    }

    let sync_summary = synchronize_runtime_workflows(command_registry).map_err(|error| {
        execution_error(
            "WORKFLOW_SYNC_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": destination_identifier }),
            true,
            "Retry to refresh registry state.",
        )
    })?;

    Ok(serde_json::json!({
        "workflow_id": destination_identifier,
        "path": new_path.to_string_lossy().to_string(),
        "format": source_record.format.as_str(),
        "version": compute_version(&serialized),
        "renamed_from": source_identifier,
        "sync": {
            "workflow_count": sync_summary.workflow_count,
            "synthetic_command_count": sync_summary.synthetic_command_count,
        }
    }))
}

pub fn export_workflow(request: &WorkflowExportRequest) -> Result<Value, ErrorData> {
    sanitize_workflow_identifier(&request.workflow_id).map_err(|error| {
        invalid_params_error(
            "WORKFLOW_IDENTIFIER_INVALID",
            error.to_string(),
            serde_json::json!({ "workflow_id": request.workflow_id }),
            "Provide a workflow identifier containing only letters, numbers, underscores, or hyphens.",
        )
    })?;

    let Some(record) = find_manifest_record(&request.workflow_id).map_err(|error| {
        internal_error(
            "WORKFLOW_EXPORT_LOOKUP_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": request.workflow_id }),
            "Inspect runtime workflow directory and retry.",
        )
    })?
    else {
        return Err(not_found_error(
            "WORKFLOW_NOT_FOUND",
            format!("workflow '{}' was not found", request.workflow_id),
            serde_json::json!({ "workflow_id": request.workflow_id }),
            "Use workflow.list to inspect available workflow identifiers.",
        ));
    };

    let output_format = if let Some(format_hint) = request.format.as_deref() {
        crate::server::workflow::services::storage::WorkflowManifestFormat::from_hint(Some(format_hint)).map_err(|error| {
            invalid_params_error(
                "WORKFLOW_EXPORT_FORMAT_INVALID",
                error.to_string(),
                serde_json::json!({ "format": request.format }),
                "Use one of: yaml, yml, json.",
            )
        })?
    } else {
        record.format
    };

    let output_path = resolve_project_relative_path(&request.output_path)?;
    if output_path.exists() && !request.overwrite.unwrap_or(false) {
        return Err(conflict_error(
            "WORKFLOW_EXPORT_PATH_EXISTS",
            format!("destination '{}' already exists", output_path.to_string_lossy()),
            serde_json::json!({ "output_path": request.output_path }),
            "Set overwrite=true or choose a different output path.",
        ));
    }

    if request.create_directories.unwrap_or(false)
        && let Some(parent) = output_path.parent()
    {
        std::fs::create_dir_all(parent).map_err(|error| {
            internal_error(
                "WORKFLOW_EXPORT_CREATE_DIR_FAILED",
                error.to_string(),
                serde_json::json!({ "output_path": output_path }),
                "Verify write permissions for the target project directory and retry.",
            )
        })?;
    }

    let serialized = serialize_definition(&record.definition, output_format).map_err(|error| {
        internal_error(
            "WORKFLOW_EXPORT_SERIALIZE_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": request.workflow_id, "format": output_format.as_str() }),
            "Inspect workflow schema content and retry.",
        )
    })?;

    std::fs::write(&output_path, &serialized).map_err(|error| {
        internal_error(
            "WORKFLOW_EXPORT_WRITE_FAILED",
            error.to_string(),
            serde_json::json!({ "output_path": output_path }),
            "Verify file permissions for the project path and retry.",
        )
    })?;

    Ok(serde_json::json!({
        "workflow_id": record.definition.workflow,
        "output_path": output_path.to_string_lossy().to_string(),
        "format": output_format.as_str(),
        "version": compute_version(&serialized),
        "bytes_written": serialized.len(),
    }))
}

pub fn import_workflow(request: &WorkflowImportRequest, command_registry: &Arc<Mutex<CommandRegistry>>) -> Result<Value, ErrorData> {
    let input_path = resolve_project_relative_path(&request.input_path)?;
    if !input_path.exists() {
        return Err(not_found_error(
            "WORKFLOW_IMPORT_PATH_NOT_FOUND",
            format!("input path '{}' does not exist", input_path.to_string_lossy()),
            serde_json::json!({ "input_path": request.input_path }),
            "Provide a valid project-relative input path and retry.",
        ));
    }

    let content = std::fs::read_to_string(&input_path).map_err(|error| {
        internal_error(
            "WORKFLOW_IMPORT_READ_FAILED",
            error.to_string(),
            serde_json::json!({ "input_path": input_path }),
            "Verify read permissions for the source file and retry.",
        )
    })?;

    let save_summary = save_workflow(
        &WorkflowSaveRequest {
            workflow_id: request.workflow_id.clone(),
            manifest_content: content,
            format: request.format.clone(),
            overwrite: request.overwrite,
            expected_version: request.expected_version.clone(),
        },
        command_registry,
    )?;

    Ok(serde_json::json!({
        "source_path": input_path.to_string_lossy().to_string(),
        "saved": save_summary,
    }))
}

fn resolve_workflow_identifier(
    request: &WorkflowSaveRequest,
    definition: &mut oatty_types::workflow::WorkflowDefinition,
) -> Result<String, ErrorData> {
    let requested_identifier = request.workflow_id.as_deref().map(str::trim).filter(|value| !value.is_empty());
    let parsed_identifier = definition.workflow.trim();

    let resolved = match (requested_identifier, parsed_identifier.is_empty()) {
        (Some(identifier), true) => {
            definition.workflow = identifier.to_string();
            identifier.to_string()
        }
        (Some(identifier), false) if identifier == parsed_identifier => identifier.to_string(),
        (Some(identifier), false) => {
            return Err(conflict_error(
                "WORKFLOW_IDENTIFIER_MISMATCH",
                format!(
                    "workflow identifier mismatch between request '{}' and manifest '{}'",
                    identifier, parsed_identifier
                ),
                serde_json::json!({
                    "requested_workflow_id": identifier,
                    "manifest_workflow_id": parsed_identifier
                }),
                "Use a matching workflow_id or update the manifest workflow field.",
            ));
        }
        (None, true) => {
            return Err(invalid_params_error(
                "WORKFLOW_IDENTIFIER_MISSING",
                "workflow identifier is required in request.workflow_id or manifest.workflow",
                serde_json::json!({}),
                "Set workflow_id in the request or provide a manifest.workflow value.",
            ));
        }
        (None, false) => parsed_identifier.to_string(),
    };

    sanitize_workflow_identifier(&resolved).map_err(|error| {
        invalid_params_error(
            "WORKFLOW_IDENTIFIER_INVALID",
            error.to_string(),
            serde_json::json!({ "workflow_id": resolved }),
            "Provide a workflow identifier containing only letters, numbers, underscores, or hyphens.",
        )
    })?;
    Ok(resolved)
}

fn resolve_project_relative_path(project_relative_path: &str) -> Result<PathBuf, ErrorData> {
    let trimmed = project_relative_path.trim();
    if trimmed.is_empty() {
        return Err(invalid_params_error(
            "WORKFLOW_PATH_INVALID",
            "project path cannot be empty",
            serde_json::json!({ "path": project_relative_path }),
            "Provide a non-empty project-relative path.",
        ));
    }

    let relative_path = Path::new(trimmed);
    if relative_path.is_absolute() {
        return Err(invalid_params_error(
            "WORKFLOW_PATH_INVALID",
            "absolute paths are not supported; provide a project-relative path",
            serde_json::json!({ "path": project_relative_path }),
            "Use a project-relative path (for example, workflows/my_workflow.yaml).",
        ));
    }

    let project_root = std::env::current_dir().map_err(|error| {
        internal_error(
            "WORKFLOW_PROJECT_ROOT_RESOLVE_FAILED",
            error.to_string(),
            serde_json::json!({}),
            "Retry after ensuring process working directory is available.",
        )
    })?;
    let resolved_path = project_root.join(relative_path);

    let parent = resolved_path.parent().unwrap_or(project_root.as_path());
    if let Ok(canonical_parent) = parent.canonicalize()
        && let Ok(canonical_project_root) = project_root.canonicalize()
        && !canonical_parent.starts_with(&canonical_project_root)
    {
        return Err(invalid_params_error(
            "WORKFLOW_PATH_OUTSIDE_PROJECT",
            "path resolves outside the current project directory",
            serde_json::json!({ "path": project_relative_path }),
            "Use a project-relative path within the current repository.",
        ));
    }

    Ok(resolved_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oatty_registry::CommandRegistry;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    fn create_temp_directory() -> PathBuf {
        let mut directory = std::env::temp_dir();
        directory.push(format!(
            "oatty-workflow-rename-test-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&directory).expect("create temp directory");
        directory
    }

    fn sample_manifest(identifier: &str) -> String {
        format!(
            r#"
workflow: {identifier}
inputs:
  app:
    validate:
      required: true
steps:
  - id: list_apps
    run: apps list
    with:
      app: "${{{{ inputs.app }}}}"
"#
        )
    }

    #[test]
    fn rename_workflow_moves_manifest_identifier() {
        let temp_directory = create_temp_directory();
        temp_env::with_var(
            "REGISTRY_WORKFLOWS_PATH",
            Some(temp_directory.to_string_lossy().to_string()),
            || {
                let registry = Arc::new(Mutex::new(CommandRegistry::default()));

                let (definition, format) =
                    parse_manifest_content(&sample_manifest("source_workflow"), Some("yaml")).expect("parse source manifest");
                let serialized = serialize_definition(&definition, format).expect("serialize source manifest");
                write_manifest("source_workflow", &serialized, format).expect("persist source manifest");

                let rename_request = WorkflowRenameRequest {
                    workflow_id: "source_workflow".to_string(),
                    new_id: "renamed_workflow".to_string(),
                    overwrite: Some(false),
                };
                let renamed = rename_workflow(&rename_request, &registry).expect("rename workflow");
                assert_eq!(renamed["workflow_id"], "renamed_workflow");

                let source = find_manifest_record("source_workflow").expect("source lookup");
                let renamed = find_manifest_record("renamed_workflow").expect("renamed lookup");
                assert!(source.is_none());
                assert!(renamed.is_some());
            },
        );
    }
}
