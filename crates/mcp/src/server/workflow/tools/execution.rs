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
    ProviderBindingOutcome, RegistryCommandRunner, StepStatus, WorkflowRunState, drive_workflow_run,
    executor::{StepResult, order_steps_for_execution},
    resolve::{RunContext, interpolate_value, resolve_template_expression_value},
    templates::extract_template_expressions,
    workflow::runtime::workflow_spec_from_runtime,
};
use oatty_registry::CommandRegistry;
use oatty_types::workflow::{
    WorkflowRunEvent as EngineWorkflowRunEvent, WorkflowRunRequest as EngineWorkflowRunRequest,
    WorkflowRunStatus as EngineWorkflowRunStatus, WorkflowRunStepStatus as EngineWorkflowRunStepStatus,
};
use std::sync::{Arc, Mutex};

use rmcp::model::ErrorData;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::thread;
use tokio::runtime::RuntimeFlavor;
use tokio::sync::mpsc::unbounded_channel;

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
            "Provide the missing workflow inputs and retry workflow_run.",
        ));
    }

    let registry_snapshot = command_registry
        .lock()
        .map_err(|error| {
            internal_error(
                "WORKFLOW_RUN_REGISTRY_LOCK_FAILED",
                format!("registry lock failed: {error}"),
                serde_json::json!({ "workflow_id": state.workflow.identifier }),
                "Retry workflow_run.",
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
        "Fix listed step run identifiers and catalog configuration, then retry workflow_run.",
    ) {
        return Err(error);
    }

    let run_identifier = format!("run-{}-{}", state.workflow.identifier, chrono::Utc::now().timestamp_millis());
    let engine_run_request = EngineWorkflowRunRequest {
        run_id: run_identifier.clone(),
        workflow: state.workflow.clone(),
        inputs: state.run_context.inputs.clone(),
        environment: state.run_context.environment_variables.clone(),
        step_outputs: state.run_context.steps.clone(),
    };
    let execution_summary = execute_workflow_via_engine_runner(engine_run_request, Arc::new(runner)).map_err(|error| {
        execution_error(
            "WORKFLOW_RUN_FAILED",
            format!("{error:#}"),
            serde_json::json!({ "workflow_id": state.workflow.identifier }),
            false,
            "Inspect run details and command dependencies, then retry.",
        )
    })?;

    state.run_context.steps = execution_summary.output_map.clone();
    let results = execution_summary.results;
    let run_status = match execution_summary.status {
        EngineWorkflowRunStatus::Succeeded => "succeeded",
        EngineWorkflowRunStatus::Canceled => "canceled",
        EngineWorkflowRunStatus::Failed => "failed",
        _ if results.iter().any(|result| result.status == StepStatus::Failed) => "failed",
        _ => "succeeded",
    };
    let output_map = execution_summary.output_map;
    let input_map = state.run_context.inputs.clone();
    let include_results = request.include_results.unwrap_or(true);
    let include_outputs = request.include_outputs.unwrap_or(false);
    append_history_entry(&WorkflowHistoryEntry {
        workflow_id: state.workflow.identifier.clone(),
        run_id: run_identifier.clone(),
        status: run_status.to_string(),
        timestamp: chrono::Utc::now(),
        inputs: Value::Object(input_map.iter().map(|(key, value)| (key.clone(), value.clone())).collect()),
    })
    .map_err(|error| {
        execution_error(
            "WORKFLOW_HISTORY_APPEND_FAILED",
            error.to_string(),
            serde_json::json!({ "workflow_id": state.workflow.identifier, "run_id": run_identifier }),
            true,
            "Retry workflow_run after verifying history directory permissions.",
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
        response.insert("results".to_string(), enrich_step_results_with_failure_reasons(&results));
    }
    if include_outputs {
        response.insert("outputs".to_string(), serde_json::json!(output_map));
    }
    if run_status == "failed" {
        response.insert("failure_summary".to_string(), build_failure_summary(&results, Some(&state)));
    }
    if let Some(final_output) = render_workflow_final_output(&state) {
        response.insert("final_output".to_string(), final_output);
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

#[derive(Debug)]
struct WorkflowExecutionSummary {
    status: EngineWorkflowRunStatus,
    results: Vec<StepResult>,
    output_map: HashMap<String, Value>,
}

fn execute_workflow_via_engine_runner(
    request: EngineWorkflowRunRequest,
    runner: Arc<dyn oatty_engine::CommandRunner + Send + Sync>,
) -> anyhow::Result<WorkflowExecutionSummary> {
    let initial_step_outputs = request.step_outputs.clone();
    let (event_tx, mut event_rx) = unbounded_channel();
    let (_control_tx, control_rx) = unbounded_channel();
    run_drive_workflow_future(request, runner, control_rx, event_tx)?;
    Ok(collect_workflow_execution_summary(&mut event_rx, initial_step_outputs))
}

fn run_drive_workflow_future(
    request: EngineWorkflowRunRequest,
    runner: Arc<dyn oatty_engine::CommandRunner + Send + Sync>,
    control_rx: tokio::sync::mpsc::UnboundedReceiver<oatty_types::workflow::WorkflowRunControl>,
    event_tx: tokio::sync::mpsc::UnboundedSender<EngineWorkflowRunEvent>,
) -> anyhow::Result<()> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => match handle.runtime_flavor() {
            RuntimeFlavor::MultiThread => {
                let drive_future = async move { drive_workflow_run(request, runner, control_rx, event_tx).await };
                tokio::task::block_in_place(|| {
                    handle
                        .block_on(drive_future)
                        .map_err(|error| anyhow::anyhow!("workflow execution failed: {error}"))
                })
            }
            RuntimeFlavor::CurrentThread => run_drive_workflow_on_dedicated_runtime_thread(request, runner, control_rx, event_tx),
            _ => run_drive_workflow_on_dedicated_runtime_thread(request, runner, control_rx, event_tx),
        },
        Err(_) => {
            let drive_future = async move { drive_workflow_run(request, runner, control_rx, event_tx).await };
            let runtime = tokio::runtime::Runtime::new().map_err(|error| anyhow::anyhow!("failed to create runtime: {error}"))?;
            runtime
                .block_on(drive_future)
                .map_err(|error| anyhow::anyhow!("workflow execution failed: {error}"))
        }
    }
}

fn run_drive_workflow_on_dedicated_runtime_thread(
    request: EngineWorkflowRunRequest,
    runner: Arc<dyn oatty_engine::CommandRunner + Send + Sync>,
    control_rx: tokio::sync::mpsc::UnboundedReceiver<oatty_types::workflow::WorkflowRunControl>,
    event_tx: tokio::sync::mpsc::UnboundedSender<EngineWorkflowRunEvent>,
) -> anyhow::Result<()> {
    let join_handle = thread::Builder::new()
        .name("workflow-runner".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| anyhow::anyhow!("failed to create runtime: {error}"))?;
            runtime
                .block_on(async move { drive_workflow_run(request, runner, control_rx, event_tx).await })
                .map_err(|error| anyhow::anyhow!("workflow execution failed: {error}"))
        })
        .map_err(|error| anyhow::anyhow!("failed to spawn workflow runner thread: {error}"))?;

    join_handle
        .join()
        .map_err(|panic_payload| anyhow::anyhow!("workflow runner thread panicked: {panic_payload:?}"))?
}

fn collect_workflow_execution_summary(
    event_rx: &mut tokio::sync::mpsc::UnboundedReceiver<EngineWorkflowRunEvent>,
    initial_step_outputs: HashMap<String, Value>,
) -> WorkflowExecutionSummary {
    let mut status = EngineWorkflowRunStatus::Failed;
    let mut results = Vec::new();
    let mut output_map = initial_step_outputs;

    while let Ok(event) = event_rx.try_recv() {
        match event {
            EngineWorkflowRunEvent::StepFinished {
                step_id,
                status: step_status,
                output,
                logs,
                attempts,
                ..
            } => {
                results.push(StepResult {
                    id: step_id,
                    status: map_engine_step_status(step_status),
                    output,
                    logs,
                    attempts,
                });
            }
            EngineWorkflowRunEvent::RunOutputAccumulated { key, value } => {
                output_map.insert(key, value);
            }
            EngineWorkflowRunEvent::RunCompleted { status: run_status, .. } => {
                status = run_status;
            }
            _ => {}
        }
    }

    WorkflowExecutionSummary {
        status,
        results,
        output_map,
    }
}

fn map_engine_step_status(step_status: EngineWorkflowRunStepStatus) -> StepStatus {
    match step_status {
        EngineWorkflowRunStepStatus::Succeeded => StepStatus::Succeeded,
        EngineWorkflowRunStepStatus::Failed => StepStatus::Failed,
        EngineWorkflowRunStepStatus::Skipped => StepStatus::Skipped,
        EngineWorkflowRunStepStatus::Pending | EngineWorkflowRunStepStatus::Running => StepStatus::Skipped,
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

    let rendered_final_output = render_workflow_final_output(&state);

    Ok(serde_json::json!({
        "workflow_id": runtime_workflow.identifier,
        "rendered_steps": rendered,
        "rendered_final_output": rendered_final_output,
    }))
}

fn render_workflow_final_output(state: &WorkflowRunState) -> Option<Value> {
    state
        .workflow
        .final_output
        .as_ref()
        .map(|final_output| interpolate_value(final_output, &state.run_context))
}

fn enrich_step_results_with_failure_reasons(results: &[StepResult]) -> Value {
    let enriched = results
        .iter()
        .map(|result| {
            let mut value = serde_json::to_value(result).unwrap_or_else(|_| serde_json::json!({}));
            if result.status == StepStatus::Failed
                && let Some(reason) = infer_failure_reason(result)
                && let Value::Object(map) = &mut value
            {
                map.insert("failure_reason".to_string(), serde_json::json!(reason));
            }
            value
        })
        .collect::<Vec<Value>>();
    Value::Array(enriched)
}

fn build_failure_summary(results: &[StepResult], state: Option<&WorkflowRunState>) -> Value {
    let failed_steps = results
        .iter()
        .filter(|result| result.status == StepStatus::Failed)
        .map(|result| {
            serde_json::json!({
                "step_id": result.id,
                "reason": infer_failure_reason(result).unwrap_or_else(|| "step failed without a detailed reason".to_string()),
            })
        })
        .collect::<Vec<Value>>();
    let rendered_failed_steps = state
        .map(|run_state| build_rendered_failed_steps(run_state, results))
        .unwrap_or_default();

    serde_json::json!({
        "failed_step_count": failed_steps.len(),
        "failed_steps": failed_steps,
        "rendered_failed_steps": rendered_failed_steps,
    })
}

fn build_rendered_failed_steps(state: &WorkflowRunState, results: &[StepResult]) -> Vec<Value> {
    let failed_step_identifiers: HashSet<&str> = results
        .iter()
        .filter(|result| result.status == StepStatus::Failed)
        .map(|result| result.id.as_str())
        .collect();
    if failed_step_identifiers.is_empty() {
        return Vec::new();
    }

    let workflow_spec = workflow_spec_from_runtime(&state.workflow);
    let step_specs_by_identifier: HashMap<&str, &_> = workflow_spec.steps.iter().map(|step| (step.id.as_str(), step)).collect();

    let mut diagnostics = Vec::new();
    for failed_step_identifier in failed_step_identifiers {
        let Some(step_specification) = step_specs_by_identifier.get(failed_step_identifier) else {
            continue;
        };
        let prepared = oatty_engine::executor::prepare_step(step_specification, &state.run_context);
        let interpolation_trace = build_step_interpolation_trace(step_specification, &state.run_context);
        diagnostics.push(serde_json::json!({
            "step_id": prepared.id,
            "run": prepared.run,
            "with": prepared.with,
            "body": prepared.body,
            "interpolation_trace": interpolation_trace,
            "replay": {
                "command": prepared.run,
                "with": prepared.with,
                "body": prepared.body,
            }
        }));
    }

    diagnostics
}

fn build_step_interpolation_trace(step_specification: &oatty_engine::model::StepSpec, context: &RunContext) -> Vec<Value> {
    let mut trace_entries = Vec::new();

    if let Some(with_values) = &step_specification.with {
        for (field_name, field_value) in with_values {
            collect_interpolation_trace_from_value(field_value, format!("with.{field_name}").as_str(), context, &mut trace_entries);
        }
    }

    if let Some(body) = &step_specification.body {
        collect_interpolation_trace_from_value(body, "body", context, &mut trace_entries);
    }

    trace_entries
}

fn collect_interpolation_trace_from_value(value: &Value, source_path: &str, context: &RunContext, output: &mut Vec<Value>) {
    match value {
        Value::String(raw_text) => {
            for expression in extract_template_expressions(raw_text) {
                let resolved_value = resolve_template_expression_value(expression.as_str(), context);
                let unresolved = resolved_value.is_none();
                let resolved_type = resolved_value.as_ref().map(json_value_type_name).unwrap_or("unknown");
                output.push(serde_json::json!({
                    "source_path": source_path,
                    "expression": format!("${{{{ {expression} }}}}"),
                    "resolved": resolved_value,
                    "resolved_type": resolved_type,
                    "unresolved": unresolved,
                }));
            }
        }
        Value::Array(entries) => {
            for (index, nested_value) in entries.iter().enumerate() {
                collect_interpolation_trace_from_value(nested_value, format!("{source_path}[{index}]").as_str(), context, output);
            }
        }
        Value::Object(map) => {
            for (key, nested_value) in map {
                collect_interpolation_trace_from_value(nested_value, format!("{source_path}.{key}").as_str(), context, output);
            }
        }
        _ => {}
    }
}

fn json_value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn infer_failure_reason(result: &StepResult) -> Option<String> {
    result
        .logs
        .iter()
        .find_map(|line| {
            line.split_once("failed:")
                .map(|(_, message)| message.trim().to_string())
                .filter(|message| !message.is_empty())
        })
        .or_else(|| result.logs.iter().find(|line| !line.trim().is_empty()).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use serde_json::Map;

    struct TestNoopRunner;

    impl oatty_engine::CommandRunner for TestNoopRunner {
        fn run(
            &self,
            _run: &str,
            _with: Option<&Value>,
            _body: Option<&Value>,
            _context: &oatty_engine::resolve::RunContext,
        ) -> Result<Value> {
            Ok(Value::Null)
        }
    }

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
final_output:
  app: "${{ inputs.app }}"
  region: "${{ inputs.region }}"
  created: "${{ steps.create_app.output.ok }}"
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
        assert_eq!(value["rendered_final_output"]["app"], "demo");
        assert_eq!(value["rendered_final_output"]["region"], "us");
    }

    #[test]
    fn failure_summary_and_reason_are_included_for_failed_steps() {
        let results = vec![
            StepResult {
                id: "ok_step".to_string(),
                status: StepStatus::Succeeded,
                output: serde_json::json!({"ok": true}),
                logs: vec!["step 'ok_step' executed".to_string()],
                attempts: 1,
            },
            StepResult {
                id: "failed_step".to_string(),
                status: StepStatus::Failed,
                output: Value::Null,
                logs: vec!["step 'failed_step' failed: HTTP 403 Forbidden".to_string()],
                attempts: 1,
            },
        ];

        let enriched = enrich_step_results_with_failure_reasons(&results);
        let rows = enriched.as_array().expect("array");
        assert_eq!(rows[1]["failure_reason"], "HTTP 403 Forbidden");

        let summary = build_failure_summary(&results, None);
        assert_eq!(summary["failed_step_count"], 1);
        assert_eq!(summary["failed_steps"][0]["step_id"], "failed_step");
        assert_eq!(summary["failed_steps"][0]["reason"], "HTTP 403 Forbidden");
        assert_eq!(
            summary["rendered_failed_steps"]
                .as_array()
                .expect("rendered_failed_steps array")
                .len(),
            0
        );
    }

    #[test]
    fn failure_summary_includes_rendered_failed_step_diagnostics_with_state() {
        let mut inputs = std::collections::HashMap::new();
        inputs.insert("app".to_string(), serde_json::json!("demo"));
        let request = WorkflowPreviewRenderedRequest {
            workflow_id: None,
            manifest_content: Some(sample_manifest()),
            format: Some("yaml".to_string()),
            inputs: Some(inputs),
        };
        let runtime_workflow =
            resolve_runtime_workflow(None, request.manifest_content.as_deref(), request.format.as_deref()).expect("runtime workflow");
        let mut state = WorkflowRunState::new(runtime_workflow);
        state.set_input_value("app", serde_json::json!("demo"));
        state.apply_input_defaults();
        state.record_step_result("create_app", StepStatus::Succeeded, serde_json::json!({"ok": true}));

        let results = vec![StepResult {
            id: "audit".to_string(),
            status: StepStatus::Failed,
            output: Value::Null,
            logs: vec!["step 'audit' failed: timeout".to_string()],
            attempts: 1,
        }];

        let summary = build_failure_summary(&results, Some(&state));
        let rendered = summary["rendered_failed_steps"].as_array().expect("rendered array");
        assert_eq!(rendered.len(), 1);
        assert_eq!(rendered[0]["step_id"], "audit");
        assert_eq!(rendered[0]["run"], "apps:list");
        assert_eq!(rendered[0]["replay"]["command"], "apps:list");
        let interpolation_trace = rendered[0]["interpolation_trace"].as_array().expect("interpolation_trace array");
        assert_eq!(interpolation_trace.len(), 1);
        assert_eq!(interpolation_trace[0]["expression"], "${{ inputs.app }}");
        assert_eq!(interpolation_trace[0]["resolved"], "demo");
        assert_eq!(interpolation_trace[0]["resolved_type"], "string");
        assert_eq!(interpolation_trace[0]["unresolved"], false);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_drive_workflow_future_supports_current_thread_runtime() {
        let manifest = sample_manifest();
        let runtime_workflow = resolve_runtime_workflow(None, Some(manifest.as_str()), Some("yaml")).expect("runtime workflow");
        let request = EngineWorkflowRunRequest {
            run_id: "run-current-thread".to_string(),
            workflow: runtime_workflow,
            inputs: Map::from_iter([("app".to_string(), serde_json::json!("demo"))]),
            environment: HashMap::new(),
            step_outputs: HashMap::new(),
        };
        let runner: Arc<dyn oatty_engine::CommandRunner + Send + Sync> = Arc::new(TestNoopRunner);
        let (_control_tx, control_rx) = unbounded_channel();
        let (event_tx, _event_rx) = unbounded_channel();

        let result = run_drive_workflow_future(request, runner, control_rx, event_tx);
        assert!(result.is_ok(), "workflow run should execute without runtime panic");
    }
}
