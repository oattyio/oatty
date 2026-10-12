//! Workflow input preview and resolution handlers.
//!
//! This module provides two entry points for inspecting and resolving workflow inputs:
//! - [`preview_inputs`] returns a read-only view of input status without evaluating providers.
//! - [`resolve_inputs`] fully resolves inputs, including provider-driven values, and returns
//!   a readiness signal.
//!
//! Both functions share a common initialization path that builds a [`WorkflowRunState`],
//! applies caller-supplied values and defaults, then performs input-level validation.
//! The shared helpers below keep the two functions focused on their distinct contracts:
//! preview never fails on validation; resolution fails with `WORKFLOW_INPUT_VALIDATION_FAILED`.

use crate::server::workflow::errors::{execution_error, invalid_params_error, validation_error_with_violations};
use crate::server::workflow::tools::common::resolve_runtime_workflow;
use crate::server::workflow::tools::types::{WorkflowPreviewRequest, WorkflowResolveInputsRequest};
use oatty_engine::WorkflowRunState;
use oatty_types::workflow::{RuntimeWorkflow, validate_candidate_value};
use rmcp::model::ErrorData;
use serde_json::Value;

/// A typed validation failure for a single input.
///
/// This struct captures the input name, a human-readable error message, and the
/// candidate value that failed validation. It is intentionally unformatted so that
/// callers can serialize it in the shape they require (JSON violations in errors,
/// detail rows in previews).
struct InputValidationFailure<'a> {
    /// The input name that failed validation.
    input_name: &'a str,
    /// A human-readable description of the validation failure.
    message: String,
    /// The candidate value that failed validation.
    actual_value: &'a Value,
}

/// Initialize a [`WorkflowRunState`] from a workflow and optional caller-provided inputs.
///
/// This helper encapsulates the common initialization sequence shared by preview and
/// resolution paths:
/// 1. Create a fresh workflow run state from the supplied workflow.
/// 2. Apply any caller-provided input values.
/// 3. Apply default values from the workflow definition.
///
/// # Arguments
///
/// * `workflow` — The runtime workflow to initialize state for.
/// * `provided_inputs` — Optional map of input names to candidate values.
///
/// # Returns
///
/// A [`WorkflowRunState`] with provided inputs and defaults applied, ready for
/// further processing such as provider evaluation or validation.
fn initialize_input_state(
    workflow: RuntimeWorkflow,
    provided_inputs: Option<&std::collections::HashMap<String, Value>>,
) -> WorkflowRunState {
    let mut state = WorkflowRunState::new(workflow);
    if let Some(inputs) = provided_inputs {
        for (input_name, value) in inputs {
            state.set_input_value(input_name, value.clone());
        }
    }
    state.apply_input_defaults();
    state
}

/// Collect the names of required inputs that are missing from the state.
///
/// Returns input names in the order they appear in the workflow definition
/// (deterministic iteration order).
///
/// # Arguments
///
/// * `state` — The initialized workflow run state.
///
/// # Returns
///
/// A vector of input names that are declared required but have no value in the state.
fn collect_missing_required_inputs(state: &WorkflowRunState) -> Vec<String> {
    state
        .workflow
        .inputs
        .iter()
        .filter(|(name, definition)| definition.is_required() && !state.run_context.inputs.contains_key(*name))
        .map(|(name, _)| name.clone())
        .collect()
}

/// Collect validation failures for every input that has a candidate value.
///
/// Iterates over all inputs in workflow declaration order, checks each one that
/// currently has a value in the state, and returns typed failures for any that
/// fail the declarative validation rules.
///
/// # Arguments
///
/// * `state` — The initialized workflow run state.
///
/// # Returns
///
/// A vector of [`InputValidationFailure`] for inputs that failed validation.
/// The vector is empty if all inputs with values passed validation.
fn collect_input_validation_failures(state: &WorkflowRunState) -> Vec<InputValidationFailure<'_>> {
    state
        .workflow
        .inputs
        .iter()
        .filter_map(|(input_name, definition)| {
            let candidate = state.run_context.inputs.get(input_name)?;
            definition
                .validate
                .as_ref()
                .and_then(|validation| validate_candidate_value(candidate, validation).err())
                .map(|error| InputValidationFailure {
                    input_name,
                    message: error.to_string(),
                    actual_value: candidate,
                })
        })
        .collect()
}

/// Preview the inputs for a workflow without evaluating provider bindings.
///
/// Returns a read-only view of input status, including whether each input is
/// resolved, required-missing, or optional-missing. If `include_details` is true,
/// per-input validation results are also returned.
///
/// **Important:** Preview never fails on validation — it always returns a successful
/// response with status information.
///
/// # Arguments
///
/// * `request` — The preview request containing workflow identifier/manifest and
///   optional candidate inputs.
///
/// # Returns
///
/// A JSON object with `workflow_id`, `required_missing`, and optionally `inputs`.
pub fn preview_inputs(request: &WorkflowPreviewRequest) -> Result<Value, ErrorData> {
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

    let state = initialize_input_state(runtime_workflow, request.inputs.as_ref());
    let required_missing = collect_missing_required_inputs(&state);
    let include_inputs = request.include_details.unwrap_or(false);

    let mut payload = serde_json::Map::new();
    payload.insert("workflow_id".to_string(), serde_json::json!(state.workflow.identifier));
    payload.insert("required_missing".to_string(), serde_json::json!(required_missing));
    if include_inputs {
        let validation_failures = collect_input_validation_failures(&state);
        let validation_by_name: std::collections::HashMap<&str, String> = validation_failures
            .into_iter()
            .map(|failure| (failure.input_name, failure.message))
            .collect();

        let input_summaries: Vec<Value> = state
            .workflow
            .inputs
            .iter()
            .map(|(input_name, definition)| {
                let value = state.run_context.inputs.get(input_name).cloned();
                let required = definition.is_required();
                let validation_error = validation_by_name.get(input_name.as_str()).cloned();
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
        payload.insert("inputs".to_string(), serde_json::json!(input_summaries));
    }

    Ok(Value::Object(payload))
}

/// Fully resolve inputs for a workflow, including provider-driven values.
///
/// After applying defaults and evaluating provider bindings, this function validates
/// all resolved input values against the workflow's declarative rules. If validation
/// fails, it returns a `WORKFLOW_INPUT_VALIDATION_FAILED` error with typed violations.
///
/// # Arguments
///
/// * `request` — The resolution request containing workflow identifier/manifest,
///   partial inputs, and optional inclusion flags.
///
/// # Returns
///
/// A JSON object with `workflow_id`, `required_missing`, `ready`, and optionally
/// `resolved_inputs` and `provider_resolutions`.
///
/// # Errors
///
/// Returns `WORKFLOW_INPUT_VALIDATION_FAILED` if any input value fails validation
/// after provider resolution.
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

    let mut state = initialize_input_state(runtime_workflow, request.partial_inputs.as_ref());
    state.evaluate_input_providers().map_err(|error| {
        execution_error(
            "WORKFLOW_PROVIDER_RESOLUTION_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": state.workflow.identifier }),
            false,
            "Inspect provider bindings and retry workflow_resolve_inputs.",
        )
    })?;

    let validation_failures = collect_input_validation_failures(&state);
    if !validation_failures.is_empty() {
        return Err(validation_error_with_violations(
            "WORKFLOW_INPUT_VALIDATION_FAILED",
            "one or more workflow inputs failed validation",
            serde_json::json!({ "workflow_id": state.workflow.identifier }),
            "Inspect violations and provide corrected input values.",
            validation_failures
                .into_iter()
                .map(|failure| {
                    serde_json::json!({
                        "path": format!("inputs.{}", failure.input_name),
                        "rule": "validation",
                        "message": failure.message,
                        "actual": failure.actual_value,
                    })
                })
                .collect(),
        ));
    }

    let unresolved_required = collect_missing_required_inputs(&state);

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
                    oatty_engine::ProviderBindingOutcome::Resolved(value) => serde_json::json!({"status":"resolved","value":value}),
                    oatty_engine::ProviderBindingOutcome::Prompt(prompt) => serde_json::json!({
                        "status":"prompt",
                        "required": prompt.required,
                        "reason": prompt.reason.message,
                        "path": prompt.reason.path
                    }),
                    oatty_engine::ProviderBindingOutcome::Skip(skip) => serde_json::json!({"status":"skip","reason": skip.reason.message}),
                    oatty_engine::ProviderBindingOutcome::Error(failure) => serde_json::json!({"status":"error","message": failure.message}),
                }
            })
        })
        .collect::<Vec<Value>>();
    let has_blocking_provider_resolution = state.telemetry().provider_resolution_events().iter().any(|event| {
        matches!(
            event.outcome,
            oatty_engine::ProviderBindingOutcome::Prompt(_) | oatty_engine::ProviderBindingOutcome::Error(_)
        )
    });
    let ready = unresolved_required.is_empty() && !has_blocking_provider_resolution;
    let include_resolved_inputs = request.include_resolved_inputs.unwrap_or(false);
    let include_provider_resolutions = request.include_provider_resolutions.unwrap_or(false);

    let mut payload = serde_json::Map::new();
    payload.insert("workflow_id".to_string(), serde_json::json!(state.workflow.identifier));
    payload.insert("required_missing".to_string(), serde_json::json!(unresolved_required));
    payload.insert("ready".to_string(), serde_json::json!(ready));
    if include_resolved_inputs {
        payload.insert("resolved_inputs".to_string(), serde_json::json!(state.run_context.inputs));
    }
    if include_provider_resolutions {
        payload.insert("provider_resolutions".to_string(), serde_json::json!(provider_resolutions));
    }

    Ok(Value::Object(payload))
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
        let request = WorkflowPreviewRequest {
            view: crate::server::workflow::tools::types::WorkflowPreviewView::Inputs,
            workflow_id: None,
            manifest_content: Some(sample_manifest()),
            format: Some("yaml".to_string()),
            inputs: None,
            include_details: None,
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
            include_resolved_inputs: None,
            include_provider_resolutions: None,
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
            include_resolved_inputs: Some(true),
            include_provider_resolutions: None,
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
            include_resolved_inputs: None,
            include_provider_resolutions: Some(true),
        };
        let value = resolve_inputs(&request).expect("resolve inputs should succeed");
        assert_eq!(value["required_missing"], serde_json::json!([]));
        assert_eq!(value["ready"], false);
        assert_eq!(value["provider_resolutions"][0]["outcome"]["status"], "prompt");
    }

    // Phase 2: Validation parity — both preview and resolution produce identical
    // validation messages for the same invalid candidate value.
    #[test]
    fn preview_and_resolve_produce_identical_validation_messages() {
        let mut partial_inputs = std::collections::HashMap::new();
        partial_inputs.insert("app".to_string(), serde_json::json!("BAD-1"));

        let preview_request = WorkflowPreviewRequest {
            view: crate::server::workflow::tools::types::WorkflowPreviewView::Inputs,
            workflow_id: None,
            manifest_content: Some(sample_manifest()),
            format: Some("yaml".to_string()),
            inputs: Some(partial_inputs.clone()),
            include_details: Some(true),
        };
        let preview_result = preview_inputs(&preview_request).expect("preview should succeed even with invalid input");
        let preview_details = preview_result["inputs"].as_array().expect("preview should have input details");
        let preview_validation = preview_details
            .iter()
            .find(|row| row["input"] == "app")
            .expect("app input should be present")["validation_error"]
            .as_str()
            .expect("app should have a validation_error");

        let resolve_request = WorkflowResolveInputsRequest {
            workflow_id: None,
            manifest_content: Some(sample_manifest()),
            format: Some("yaml".to_string()),
            partial_inputs: Some(partial_inputs),
            include_resolved_inputs: None,
            include_provider_resolutions: None,
        };
        let resolve_error = resolve_inputs(&resolve_request).expect_err("resolve should fail validation");
        let data = resolve_error.data.expect("error should have data");
        let violations = data["violations"].as_array().expect("violations should be an array");
        let resolve_validation = violations
            .iter()
            .find(|v| v["path"] == "inputs.app")
            .expect("app violation should be present")["message"]
            .as_str()
            .expect("app should have a message");

        assert_eq!(
            preview_validation, resolve_validation,
            "preview and resolve validation messages should be identical"
        );
    }

    // Phase 3: Compact mode — invalid input with include_details=false returns
    // successful compact response without exposing details or validation errors.
    #[test]
    fn preview_compact_mode_excludes_details_for_invalid_input() {
        let mut partial_inputs = std::collections::HashMap::new();
        partial_inputs.insert("app".to_string(), serde_json::json!("BAD-1"));

        let request = WorkflowPreviewRequest {
            view: crate::server::workflow::tools::types::WorkflowPreviewView::Inputs,
            workflow_id: None,
            manifest_content: Some(sample_manifest()),
            format: Some("yaml".to_string()),
            inputs: Some(partial_inputs),
            include_details: Some(false),
        };
        let value = preview_inputs(&request).expect("compact preview should succeed even with invalid input");
        let required_missing = value["required_missing"].as_array().expect("required_missing should be present");
        // app has a value ("BAD-1") so it is NOT in required_missing;
        // required_missing only contains inputs with no value at all.
        assert!(required_missing.is_empty(), "app has a value so it cannot be required_missing");
        // No inputs key — compact mode should not include detailed input rows.
        assert!(value.get("inputs").is_none(), "compact mode should not expose inputs");
    }

    // Phase 3: Detail mode — same invalid input with include_details=true has
    // validation_error populated in each input row.
    #[test]
    fn preview_detail_mode_includes_validation_errors() {
        let mut partial_inputs = std::collections::HashMap::new();
        partial_inputs.insert("app".to_string(), serde_json::json!("BAD-1"));

        let request = WorkflowPreviewRequest {
            view: crate::server::workflow::tools::types::WorkflowPreviewView::Inputs,
            workflow_id: None,
            manifest_content: Some(sample_manifest()),
            format: Some("yaml".to_string()),
            inputs: Some(partial_inputs),
            include_details: Some(true),
        };
        let value = preview_inputs(&request).expect("detail preview should succeed");
        let inputs = value["inputs"].as_array().expect("inputs should be an array");
        let app_row = inputs
            .iter()
            .find(|row| row["input"] == "app")
            .expect("app input should be present");
        assert!(
            app_row["validation_error"].is_string(),
            "app should have a validation_error string when include_details is true"
        );
    }
}
