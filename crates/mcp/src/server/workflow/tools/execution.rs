//! Workflow execution tool handlers.

use crate::server::workflow::errors::{execution_error, internal_error, invalid_params_error};
use crate::server::workflow::services::history::{WorkflowHistoryEntry, append_history_entry};
use crate::server::workflow::tools::common::{
    build_preflight_validation_error, collect_workflow_preflight_violations, resolve_runtime_workflow,
};
use crate::server::workflow::tools::types::{
    WorkflowPreviewRenderedRequest, WorkflowRunExecutionMode, WorkflowRunRequest, WorkflowStepPlanRequest,
};
use oatty_engine::{
    ProviderBindingOutcome, RegistryCommandRunner, StepStatus, WorkflowRunState, executor::order_steps_for_execution,
    workflow::runtime::workflow_spec_from_runtime,
};
use oatty_registry::CommandRegistry;
use std::sync::{Arc, Mutex};

use rmcp::model::ErrorData;
use serde_json::Value;

pub fn run_workflow(request: &WorkflowRunRequest, command_registry: &Arc<Mutex<CommandRegistry>>) -> Result<Value, ErrorData> {
    let runtime_workflow = resolve_runtime_workflow(
        request.workflow_id.as_deref(),
        request.manifest_content.as_deref(),
        request.format.as_deref(),
    )
    .map_err(|error| {
        invalid_params_error(
            "WORKFLOW_RUN_INVALID_REQUEST",
            error.to_string(),
            serde_json::json!({
                "workflow_id": request.workflow_id,
                "has_manifest_content": request.manifest_content.as_ref().is_some()
            }),
            "Provide a valid workflow_id or manifest_content payload.",
        )
    })?;

    let execution_mode = request.execution_mode.unwrap_or(WorkflowRunExecutionMode::Auto);
    let should_recommend_task_mode = runtime_workflow.steps.len() > 5 || runtime_workflow.steps.iter().any(|step| step.repeat.is_some());
    let task_recommendation_reason = if should_recommend_task_mode {
        Some("workflow has multiple or repeating steps and may exceed synchronous tool-call limits")
    } else {
        None
    };

    let mut state = WorkflowRunState::new(runtime_workflow);
    if let Some(inputs) = request.inputs.as_ref() {
        for (input_name, value) in inputs {
            state.set_input_value(input_name, value.clone());
        }
    }
    state.apply_input_defaults();
    state.evaluate_input_providers().map_err(|error| {
        execution_error(
            "WORKFLOW_PROVIDER_RESOLUTION_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": state.workflow.identifier }),
            false,
            "Inspect provider bindings and input defaults, then retry.",
        )
    })?;

    if let Some(blocked) = state
        .telemetry()
        .provider_resolution_events()
        .iter()
        .find(|event| matches!(event.outcome, ProviderBindingOutcome::Prompt(_) | ProviderBindingOutcome::Error(_)))
    {
        return Err(invalid_params_error(
            "WORKFLOW_INPUTS_INCOMPLETE",
            format!(
                "provider argument {}.{} requires manual resolution",
                blocked.input, blocked.argument
            ),
            serde_json::json!({
                "workflow_id": state.workflow.identifier,
                "input": blocked.input,
                "argument": blocked.argument,
            }),
            "Provide the missing workflow inputs and retry workflow.run.",
        ));
    }

    let registry_snapshot = command_registry
        .lock()
        .map_err(|error| {
            internal_error(
                "WORKFLOW_RUN_REGISTRY_LOCK_FAILED",
                format!("registry lock failed: {error}"),
                serde_json::json!({ "workflow_id": state.workflow.identifier }),
                "Retry workflow.run.",
            )
        })?
        .clone();
    let runner = RegistryCommandRunner::new(registry_snapshot);
    let violations = collect_workflow_preflight_violations(&state.workflow, command_registry)?;
    if let Some(error) = build_preflight_validation_error(
        &state.workflow.identifier,
        violations,
        "WORKFLOW_RUN_PRECHECK_FAILED",
        "workflow run blocked by command/catalog preflight validation",
        "Fix listed step run identifiers and catalog configuration, then retry workflow.run.",
    ) {
        return Err(error);
    }

    let results = state.execute_with_runner(&runner).map_err(|error| {
        execution_error(
            "WORKFLOW_RUN_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": state.workflow.identifier }),
            false,
            "Inspect run details and command dependencies, then retry.",
        )
    })?;

    let run_status = if results.iter().all(|result| result.status != StepStatus::Failed) {
        "succeeded"
    } else {
        "failed"
    };
    let run_identifier = format!("run-{}-{}", state.workflow.identifier, chrono::Utc::now().timestamp_millis());
    let output_map = state.run_context.steps.clone();
    let input_map = state.run_context.inputs.clone();
    let include_results = request.include_results.unwrap_or(true);
    let include_outputs = request.include_outputs.unwrap_or(false);
    append_history_entry(&WorkflowHistoryEntry {
        workflow_id: state.workflow.identifier.clone(),
        run_id: run_identifier.clone(),
        status: run_status.to_string(),
        timestamp: chrono::Utc::now(),
        inputs: serde_json::Value::Object(input_map.iter().map(|(key, value)| (key.clone(), value.clone())).collect()),
    })
    .map_err(|error| {
        execution_error(
            "WORKFLOW_HISTORY_APPEND_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": state.workflow.identifier, "run_id": run_identifier }),
            true,
            "Retry workflow.run after verifying history directory permissions.",
        )
    })?;

    let mut response = serde_json::Map::new();
    response.insert("run_id".to_string(), serde_json::json!(run_identifier));
    response.insert("workflow_id".to_string(), serde_json::json!(state.workflow.identifier));
    response.insert("status".to_string(), serde_json::json!(run_status));
    response.insert(
        "execution_mode_requested".to_string(),
        serde_json::json!(execution_mode_label(execution_mode)),
    );
    response.insert("execution_mode_used".to_string(), serde_json::json!("sync"));
    response.insert("task_mode_supported".to_string(), serde_json::json!(true));
    response.insert("task_recommended".to_string(), serde_json::json!(should_recommend_task_mode));
    response.insert(
        "task_recommendation_reason".to_string(),
        serde_json::json!(task_recommendation_reason),
    );
    response.insert("inputs".to_string(), serde_json::json!(input_map));
    if include_results {
        response.insert("results".to_string(), serde_json::json!(results));
    }
    if include_outputs {
        response.insert("outputs".to_string(), serde_json::json!(output_map));
    }

    Ok(Value::Object(response))
}

pub fn run_with_task_capability_guard(
    request: &WorkflowRunRequest,
    command_registry: &Arc<Mutex<CommandRegistry>>,
) -> Result<Value, ErrorData> {
    // This currently executes synchronously when called directly.
    // When clients invoke this tool with a `task` request envelope, rmcp's task handler enqueues
    // and tracks the execution automatically through the shared OperationProcessor.
    run_workflow(request, command_registry)
}

fn execution_mode_label(execution_mode: WorkflowRunExecutionMode) -> &'static str {
    match execution_mode {
        WorkflowRunExecutionMode::Sync => "sync",
        WorkflowRunExecutionMode::Auto => "auto",
        WorkflowRunExecutionMode::Task => "task",
    }
}

pub fn step_plan(request: &WorkflowStepPlanRequest) -> Result<Value, ErrorData> {
    let runtime_workflow = resolve_runtime_workflow(
        request.workflow_id.as_deref(),
        request.manifest_content.as_deref(),
        request.format.as_deref(),
    )
    .map_err(|error| {
        invalid_params_error(
            "WORKFLOW_STEP_PLAN_FAILED",
            error.to_string(),
            serde_json::json!({
                "workflow_id": request.workflow_id,
                "has_manifest_content": request.manifest_content.is_some()
            }),
            "Provide a valid workflow_id or manifest_content payload.",
        )
    })?;

    let mut state = WorkflowRunState::new(runtime_workflow.clone());
    if let Some(inputs) = request.inputs.as_ref() {
        for (input_name, value) in inputs {
            state.set_input_value(input_name, value.clone());
        }
    }
    state.apply_input_defaults();

    let workflow_spec = workflow_spec_from_runtime(&runtime_workflow);
    let ordered_steps = order_steps_for_execution(&workflow_spec.steps).map_err(|error| {
        invalid_params_error(
            "WORKFLOW_STEP_ORDER_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": runtime_workflow.identifier }),
            "Fix step dependency cycles or invalid dependencies and retry.",
        )
    })?;

    let plan = ordered_steps
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let should_run = step
                .r#if
                .as_deref()
                .map(|condition| oatty_engine::resolve::eval_condition(condition, &state.run_context))
                .unwrap_or(true);
            serde_json::json!({
                "index": index,
                "step_id": step.id,
                "run": step.run,
                "depends_on": step.depends_on,
                "condition": step.r#if,
                "will_run": should_run,
            })
        })
        .collect::<Vec<Value>>();

    Ok(serde_json::json!({
        "workflow_id": runtime_workflow.identifier,
        "steps": plan,
    }))
}

pub fn preview_rendered(request: &WorkflowPreviewRenderedRequest) -> Result<Value, ErrorData> {
    let runtime_workflow = resolve_runtime_workflow(
        request.workflow_id.as_deref(),
        request.manifest_content.as_deref(),
        request.format.as_deref(),
    )
    .map_err(|error| {
        invalid_params_error(
            "WORKFLOW_PREVIEW_RENDERED_FAILED",
            error.to_string(),
            serde_json::json!({
                "workflow_id": request.workflow_id,
                "has_manifest_content": request.manifest_content.is_some()
            }),
            "Provide a valid workflow_id or manifest_content payload.",
        )
    })?;

    let mut state = WorkflowRunState::new(runtime_workflow.clone());
    if let Some(inputs) = request.inputs.as_ref() {
        for (input_name, value) in inputs {
            state.set_input_value(input_name, value.clone());
        }
    }
    state.apply_input_defaults();

    let workflow_spec = workflow_spec_from_runtime(&runtime_workflow);
    let ordered_steps = order_steps_for_execution(&workflow_spec.steps).map_err(|error| {
        invalid_params_error(
            "WORKFLOW_STEP_ORDER_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": runtime_workflow.identifier }),
            "Fix step dependency cycles or invalid dependencies and retry.",
        )
    })?;

    let rendered = ordered_steps
        .iter()
        .map(|step| {
            let prepared = oatty_engine::executor::prepare_step(step, &state.run_context);
            serde_json::json!({
                "step_id": prepared.id,
                "run": prepared.run,
                "depends_on": prepared.depends_on,
                "condition": prepared.r#if,
                "with": prepared.with,
                "body": prepared.body,
            })
        })
        .collect::<Vec<Value>>();

    Ok(serde_json::json!({
        "workflow_id": runtime_workflow.identifier,
        "rendered_steps": rendered,
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
    validate:
      required: true
  region:
    default:
      from: literal
      value: us
steps:
  - id: create_app
    run: apps:create
    if: inputs.app == "demo"
    with:
      app: "${{ inputs.app }}"
      region: "${{ inputs.region }}"
    body:
      app: "${{ inputs.app }}"
  - id: audit
    run: apps:list
    depends_on: [create_app]
    with:
      app: "${{ inputs.app }}"
"#
        .to_string()
    }

    #[test]
    fn step_plan_evaluates_conditions() {
        let mut inputs = std::collections::HashMap::new();
        inputs.insert("app".to_string(), serde_json::json!("demo"));
        let request = WorkflowStepPlanRequest {
            workflow_id: None,
            manifest_content: Some(sample_manifest()),
            format: Some("yaml".to_string()),
            inputs: Some(inputs),
        };
        let value = step_plan(&request).expect("step plan should succeed");
        let steps = value["steps"].as_array().expect("steps array");
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0]["step_id"], "create_app");
        assert_eq!(steps[0]["will_run"], true);
        assert_eq!(steps[1]["step_id"], "audit");
    }

    #[test]
    fn preview_rendered_interpolates_step_values() {
        let mut inputs = std::collections::HashMap::new();
        inputs.insert("app".to_string(), serde_json::json!("demo"));
        let request = WorkflowPreviewRenderedRequest {
            workflow_id: None,
            manifest_content: Some(sample_manifest()),
            format: Some("yaml".to_string()),
            inputs: Some(inputs),
        };
        let value = preview_rendered(&request).expect("preview rendered should succeed");
        let rendered_steps = value["rendered_steps"].as_array().expect("rendered_steps array");
        assert_eq!(rendered_steps.len(), 2);
        assert_eq!(rendered_steps[0]["with"]["app"], "demo");
        assert_eq!(rendered_steps[0]["with"]["region"], "us");
        assert_eq!(rendered_steps[0]["body"]["app"], "demo");
    }
}
