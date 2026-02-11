//! Workflow input preview and resolution handlers.

use crate::server::workflow::errors::{execution_error, invalid_params_error, validation_error_with_violations};
use crate::server::workflow::tools::common::resolve_runtime_workflow;
use crate::server::workflow::tools::types::{WorkflowPreviewInputsRequest, WorkflowResolveInputsRequest};
use oatty_engine::{ProviderBindingOutcome, WorkflowRunState};
use oatty_types::workflow::validate_candidate_value;
use rmcp::model::ErrorData;
use serde_json::Value;

pub fn preview_inputs(request: &WorkflowPreviewInputsRequest) -> Result<Value, ErrorData> {
    let runtime_workflow = resolve_runtime_workflow(
        request.workflow_id.as_deref(),
        request.manifest_content.as_deref(),
        request.format.as_deref(),
    )
    .map_err(|error| {
        invalid_params_error(
            "WORKFLOW_INPUT_PREVIEW_FAILED",
            error.to_string(),
            serde_json::json!({
                "workflow_id": request.workflow_id,
                "has_manifest_content": request.manifest_content.is_some()
            }),
            "Provide a valid workflow_id or manifest_content payload.",
        )
    })?;

    let mut state = WorkflowRunState::new(runtime_workflow.clone());
    if let Some(partial_inputs) = request.partial_inputs.as_ref() {
        for (input_name, value) in partial_inputs {
            state.set_input_value(input_name, value.clone());
        }
    }
    state.apply_input_defaults();

    let input_summaries = runtime_workflow
        .inputs
        .iter()
        .map(|(input_name, definition)| {
            let value = state.run_context.inputs.get(input_name).cloned();
            let required = definition.is_required();
            let validation_error = value.as_ref().and_then(|candidate| {
                definition
                    .validate
                    .as_ref()
                    .and_then(|validation| validate_candidate_value(candidate, validation).err().map(|error| error.to_string()))
            });
            let status = if value.is_some() {
                "resolved"
            } else if required {
                "required_missing"
            } else {
                "optional_missing"
            };
            serde_json::json!({
                "input": input_name,
                "required": required,
                "type": definition.r#type,
                "description": definition.description,
                "value": value,
                "status": status,
                "validation_error": validation_error,
            })
        })
        .collect::<Vec<Value>>();

    let required_missing = runtime_workflow
        .inputs
        .iter()
        .filter(|(name, definition)| definition.is_required() && !state.run_context.inputs.contains_key(*name))
        .map(|(name, _)| name.clone())
        .collect::<Vec<String>>();

    Ok(serde_json::json!({
        "workflow_id": runtime_workflow.identifier,
        "inputs": input_summaries,
        "required_missing": required_missing,
    }))
}

pub fn resolve_inputs(request: &WorkflowResolveInputsRequest) -> Result<Value, ErrorData> {
    let runtime_workflow = resolve_runtime_workflow(
        request.workflow_id.as_deref(),
        request.manifest_content.as_deref(),
        request.format.as_deref(),
    )
    .map_err(|error| {
        invalid_params_error(
            "WORKFLOW_INPUT_RESOLUTION_FAILED",
            error.to_string(),
            serde_json::json!({
                "workflow_id": request.workflow_id,
                "has_manifest_content": request.manifest_content.is_some()
            }),
            "Provide a valid workflow_id or manifest_content payload.",
        )
    })?;

    let mut state = WorkflowRunState::new(runtime_workflow.clone());
    if let Some(partial_inputs) = request.partial_inputs.as_ref() {
        for (input_name, value) in partial_inputs {
            state.set_input_value(input_name, value.clone());
        }
    }
    state.apply_input_defaults();
    state.evaluate_input_providers().map_err(|error| {
        execution_error(
            "WORKFLOW_PROVIDER_RESOLUTION_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": runtime_workflow.identifier }),
            false,
            "Inspect provider bindings and retry workflow.resolve_inputs.",
        )
    })?;

    let mut violations = Vec::new();
    for (input_name, definition) in &runtime_workflow.inputs {
        if let Some(validation) = definition.validate.as_ref()
            && let Some(candidate) = state.run_context.inputs.get(input_name)
            && let Err(error) = validate_candidate_value(candidate, validation)
        {
            violations.push(serde_json::json!({
                "path": format!("inputs.{}", input_name),
                "rule": "validation",
                "message": error.to_string(),
                "actual": candidate,
            }));
        }
    }
    if !violations.is_empty() {
        return Err(validation_error_with_violations(
            "WORKFLOW_INPUT_VALIDATION_FAILED",
            "one or more workflow inputs failed validation",
            serde_json::json!({ "workflow_id": runtime_workflow.identifier }),
            "Inspect violations and provide corrected input values.",
            violations,
        ));
    }

    let unresolved_required = runtime_workflow
        .inputs
        .iter()
        .filter(|(name, definition)| definition.is_required() && !state.run_context.inputs.contains_key(*name))
        .map(|(name, _)| name.clone())
        .collect::<Vec<String>>();

    let provider_resolutions = state
        .telemetry()
        .provider_resolution_events()
        .iter()
        .map(|event| {
            serde_json::json!({
                "input": event.input,
                "argument": event.argument,
                "source": match event.source {
                    oatty_engine::ProviderResolutionSource::Automatic => "automatic",
                    oatty_engine::ProviderResolutionSource::Manual => "manual",
                },
                "outcome": match &event.outcome {
                    ProviderBindingOutcome::Resolved(value) => serde_json::json!({"status":"resolved","value":value}),
                    ProviderBindingOutcome::Prompt(prompt) => serde_json::json!({
                        "status":"prompt",
                        "required": prompt.required,
                        "reason": prompt.reason.message,
                        "path": prompt.reason.path
                    }),
                    ProviderBindingOutcome::Skip(skip) => serde_json::json!({"status":"skip","reason": skip.reason.message}),
                    ProviderBindingOutcome::Error(failure) => serde_json::json!({"status":"error","message": failure.message}),
                }
            })
        })
        .collect::<Vec<Value>>();
    let has_blocking_provider_resolution = state
        .telemetry()
        .provider_resolution_events()
        .iter()
        .any(|event| matches!(event.outcome, ProviderBindingOutcome::Prompt(_) | ProviderBindingOutcome::Error(_)));
    let ready = unresolved_required.is_empty() && !has_blocking_provider_resolution;

    Ok(serde_json::json!({
        "workflow_id": runtime_workflow.identifier,
        "resolved_inputs": state.run_context.inputs,
        "required_missing": unresolved_required,
        "ready": ready,
        "provider_resolutions": provider_resolutions,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> String {
        r#"
workflow: demo
inputs:
  app:
    type: string
    validate:
      required: true
      pattern: "^[a-z]+$"
  region:
    default:
      from: literal
      value: us
steps:
  - id: list_apps
    run: apps:list
    with:
      app: "${{ inputs.app }}"
      region: "${{ inputs.region }}"
"#
        .to_string()
    }

    #[test]
    fn preview_inputs_reports_required_missing_values() {
        let request = WorkflowPreviewInputsRequest {
            workflow_id: None,
            manifest_content: Some(sample_manifest()),
            format: Some("yaml".to_string()),
            partial_inputs: None,
        };
        let value = preview_inputs(&request).expect("preview inputs should succeed");
        let required_missing = value["required_missing"].as_array().expect("required_missing array");
        assert_eq!(required_missing.len(), 1);
        assert_eq!(required_missing[0], "app");
    }

    #[test]
    fn resolve_inputs_flags_validation_violations() {
        let mut partial_inputs = std::collections::HashMap::new();
        partial_inputs.insert("app".to_string(), serde_json::json!("BAD-1"));
        let request = WorkflowResolveInputsRequest {
            workflow_id: None,
            manifest_content: Some(sample_manifest()),
            format: Some("yaml".to_string()),
            partial_inputs: Some(partial_inputs),
        };
        let error = resolve_inputs(&request).expect_err("resolve inputs should fail validation");
        let data = error.data.expect("error data");
        let violations = data["violations"].as_array().expect("violations array");
        assert!(!violations.is_empty());
        assert_eq!(violations[0]["path"], "inputs.app");
    }

    #[test]
    fn resolve_inputs_marks_ready_when_required_inputs_are_valid() {
        let mut partial_inputs = std::collections::HashMap::new();
        partial_inputs.insert("app".to_string(), serde_json::json!("demo"));
        let request = WorkflowResolveInputsRequest {
            workflow_id: None,
            manifest_content: Some(sample_manifest()),
            format: Some("yaml".to_string()),
            partial_inputs: Some(partial_inputs),
        };
        let value = resolve_inputs(&request).expect("resolve inputs should succeed");
        assert_eq!(value["ready"], true);
        assert_eq!(
            value["resolved_inputs"]["region"]
                .as_str()
                .expect("region default should be applied"),
            "us"
        );
    }

    #[test]
    fn resolve_inputs_marks_not_ready_when_provider_resolution_requires_prompt() {
        let request = WorkflowResolveInputsRequest {
            workflow_id: None,
            manifest_content: Some(
                r#"
workflow: demo_provider_prompt
inputs:
  target:
    optional: true
    provider: apps:list
    provider_args:
      app:
        from_input: source
        path: name
        required: false
        on_missing: prompt
    depends_on:
      app:
        from_input: source
        path: name
        required: false
        on_missing: prompt
steps:
  - id: list_apps
    run: apps:list
"#
                .to_string(),
            ),
            format: Some("yaml".to_string()),
            partial_inputs: None,
        };
        let value = resolve_inputs(&request).expect("resolve inputs should succeed");
        assert_eq!(value["required_missing"], serde_json::json!([]));
        assert_eq!(value["ready"], false);
        assert_eq!(value["provider_resolutions"][0]["outcome"]["status"], "prompt");
    }
}
