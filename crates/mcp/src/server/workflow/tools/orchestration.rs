//! Higher-level workflow orchestration MCP tools.

use crate::server::workflow::errors::{invalid_params_error, validation_error_with_violations};
use crate::server::workflow::tools::execution::run_workflow;
use crate::server::workflow::tools::inputs::resolve_inputs;
use crate::server::workflow::tools::manifest::{save_workflow, validate_workflow};
use crate::server::workflow::tools::types::{
    WorkflowAuthorAndRunRequest, WorkflowRepairAndRerunRequest, WorkflowResolveInputsRequest, WorkflowRunRequest, WorkflowSaveRequest,
    WorkflowValidateRequest,
};
use oatty_registry::CommandRegistry;
use rmcp::model::ErrorData;
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// Save, resolve inputs, and execute a workflow from a draft manifest.
pub fn author_and_run(request: &WorkflowAuthorAndRunRequest, command_registry: &Arc<Mutex<CommandRegistry>>) -> Result<Value, ErrorData> {
    let validation = validate_workflow(&WorkflowValidateRequest {
        manifest_content: request.manifest_content.clone(),
        format: request.format.clone(),
    })?;

    let save_summary = save_workflow(
        &WorkflowSaveRequest {
            workflow_id: request.workflow_id.clone(),
            manifest_content: request.manifest_content.clone(),
            format: request.format.clone(),
            overwrite: request.overwrite,
            expected_version: request.expected_version.clone(),
        },
        command_registry,
    )?;

    let workflow_identifier = save_summary
        .get("workflow_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            invalid_params_error(
                "WORKFLOW_AUTHOR_RUN_INVALID_SAVE_RESPONSE",
                "workflow save response did not include workflow_id",
                serde_json::json!({ "save": save_summary }),
                "Retry workflow.author_and_run.",
            )
        })?
        .to_string();

    let resolution = resolve_inputs(&WorkflowResolveInputsRequest {
        workflow_id: Some(workflow_identifier.clone()),
        manifest_content: None,
        format: None,
        partial_inputs: request.inputs.clone(),
    })?;

    let missing = resolution
        .get("required_missing")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !missing.is_empty() {
        return Err(validation_error_with_violations(
            "WORKFLOW_AUTHOR_RUN_MISSING_INPUTS",
            "workflow inputs are incomplete",
            serde_json::json!({ "workflow_id": workflow_identifier, "required_missing": missing }),
            "Provide required inputs and retry workflow.author_and_run.",
            missing
                .into_iter()
                .map(|value| {
                    serde_json::json!({
                        "path": format!("inputs.{}", value.as_str().unwrap_or_default()),
                        "rule": "required",
                        "message": "required input is missing",
                    })
                })
                .collect(),
        ));
    }

    let run_inputs = resolution
        .get("resolved_inputs")
        .and_then(Value::as_object)
        .map(|object| object.iter().map(|(key, value)| (key.clone(), value.clone())).collect())
        .unwrap_or_default();

    let run_result = run_workflow(
        &WorkflowRunRequest {
            workflow_id: Some(workflow_identifier.clone()),
            manifest_content: None,
            format: None,
            inputs: Some(run_inputs),
            execution_mode: None,
        },
        command_registry,
    )?;

    Ok(serde_json::json!({
        "workflow_id": workflow_identifier,
        "validation": validation,
        "save": save_summary,
        "input_resolution": resolution,
        "run": run_result,
    }))
}

/// Validate, persist, and execute a repaired workflow manifest.
pub fn repair_and_rerun(
    request: &WorkflowRepairAndRerunRequest,
    command_registry: &Arc<Mutex<CommandRegistry>>,
) -> Result<Value, ErrorData> {
    let manifest_content = request
        .repaired_manifest_content
        .as_ref()
        .cloned()
        .unwrap_or_else(|| request.manifest_content.clone());

    let author_request = WorkflowAuthorAndRunRequest {
        workflow_id: request.workflow_id.clone(),
        manifest_content,
        format: request.format.clone(),
        overwrite: Some(request.overwrite.unwrap_or(true)),
        expected_version: request.expected_version.clone(),
        inputs: request.inputs.clone(),
    };
    let result = author_and_run(&author_request, command_registry)?;

    Ok(serde_json::json!({
        "repaired": request.repaired_manifest_content.is_some(),
        "result": result,
    }))
}
