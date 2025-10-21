//! Asynchronous workflow runner that streams lifecycle events and responds to
//! pause/cancel controls.
//!
//! This module converts the synchronous executor primitives into a cooperative
//! task that emits [`WorkflowRunEvent`]s over a Tokio channel. The caller owns
//! the event receiver and issues control commands (pause, resume, cancel)
//! through the corresponding control channel.

use std::{collections::HashMap, sync::Arc, time::Instant};

use anyhow::{Result, anyhow};
use chrono::Utc;
use heroku_types::workflow::{WorkflowRunControl, WorkflowRunEvent, WorkflowRunRequest, WorkflowRunStatus, WorkflowRunStepStatus};
use serde_json::Value;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, error::TryRecvError};

use crate::{
    RunContext,
    executor::{self, CommandRunner, PreparedStep, StepResult, StepStatus},
    workflow::{runtime::workflow_spec_from_runtime, state::apply_runtime_input_defaults},
};

/// Drives a workflow run to completion while emitting lifecycle events.
pub async fn drive_workflow_run(
    request: WorkflowRunRequest,
    runner: Arc<dyn CommandRunner + Send + Sync>,
    mut control_rx: UnboundedReceiver<WorkflowRunControl>,
    event_tx: UnboundedSender<WorkflowRunEvent>,
) -> Result<()> {
    let mut context = RunContext::default();
    context.inputs = request.inputs.clone();
    context.environment_variables = request.environment.clone();
    context.steps = request.step_outputs.clone();
    apply_runtime_input_defaults(&request.workflow, &mut context);

    if event_tx.send(WorkflowRunEvent::RunStarted { at: Utc::now() }).is_err() {
        return Ok(());
    }

    let spec = workflow_spec_from_runtime(&request.workflow);
    let plan = match executor::prepare_plan(&spec, &context) {
        Ok(plan) => plan,
        Err(error) => {
            let message = error.to_string();
            let _ = event_tx.send(WorkflowRunEvent::RunStatusChanged {
                status: WorkflowRunStatus::Failed,
                message: Some(message.clone()),
            });
            let _ = event_tx.send(WorkflowRunEvent::RunCompleted {
                status: WorkflowRunStatus::Failed,
                finished_at: Utc::now(),
                error: Some(message),
            });
            return Ok(());
        }
    };

    let mut control_state = ControlState::new();
    control_state.emit_status(&event_tx, WorkflowRunStatus::Running, None).ok();

    let labels = step_label_lookup(&request);
    let mut statuses: HashMap<String, WorkflowRunStepStatus> = HashMap::new();
    let mut any_failed = false;

    for (index, step) in plan.steps.iter().enumerate() {
        drain_pending_commands(&mut control_state, &mut control_rx, &event_tx)?;
        if control_state.cancel_requested {
            break;
        }

        if control_state.paused && !control_state.cancel_requested {
            wait_for_resume(&mut control_state, &mut control_rx, &event_tx).await?;
            if control_state.cancel_requested {
                break;
            }
        }

        if let Some(blocked) = dependency_block(step, &statuses) {
            statuses.insert(step.id.clone(), WorkflowRunStepStatus::Skipped);
            emit_step_finished(&event_tx, &blocked, WorkflowRunStepStatus::Skipped, step, 0)?;
            continue;
        }

        let label = labels.get(&step.id).cloned().flatten();
        let _ = event_tx.send(WorkflowRunEvent::StepStarted {
            index,
            step_id: step.id.clone(),
            label: label.clone(),
            started_at: Utc::now(),
        });

        let started_at = Instant::now();
        let result = execute_step(step, &mut context, runner.as_ref());
        let duration_ms = started_at.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
        let status = map_step_status(result.status);
        statuses.insert(step.id.clone(), status);
        if matches!(status, WorkflowRunStepStatus::Failed) {
            any_failed = true;
        }

        emit_step_finished(&event_tx, &result, status, step, duration_ms)?;

        if matches!(status, WorkflowRunStepStatus::Succeeded) {
            let _ = event_tx.send(WorkflowRunEvent::RunOutputAccumulated {
                key: step.id.clone(),
                value: result.output.clone(),
                detail: None,
            });
        }

        drain_pending_commands(&mut control_state, &mut control_rx, &event_tx)?;
        if control_state.cancel_requested {
            break;
        }
        if control_state.paused && !control_state.cancel_requested {
            wait_for_resume(&mut control_state, &mut control_rx, &event_tx).await?;
            if control_state.cancel_requested {
                break;
            }
        }
    }

    let completed_status = if control_state.cancel_requested {
        WorkflowRunStatus::Canceled
    } else if any_failed {
        WorkflowRunStatus::Failed
    } else {
        WorkflowRunStatus::Succeeded
    };

    let _ = event_tx.send(WorkflowRunEvent::RunCompleted {
        status: completed_status,
        finished_at: Utc::now(),
        error: None,
    });
    Ok(())
}

fn execute_step(step: &PreparedStep, context: &mut RunContext, runner: &dyn CommandRunner) -> StepResult {
    if step.repeat.is_some() {
        executor::run_step_repeating_with(step, context, runner)
    } else {
        let result = executor::run_step_with(step, context, runner);
        context.steps.insert(step.id.clone(), result.output.clone());
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heroku_types::workflow::{
        RuntimeWorkflow, WorkflowDefaultSource, WorkflowInputDefault, WorkflowInputDefinition, WorkflowStepDefinition,
    };
    use indexmap::{IndexMap, indexmap};
    use serde_json::{Map as JsonMap, Value};
    use std::collections::HashMap;
    use tokio::sync::mpsc::unbounded_channel;

    fn workflow_with_default_condition() -> RuntimeWorkflow {
        let mut input_definition = WorkflowInputDefinition::default();
        input_definition.default = Some(WorkflowInputDefault {
            from: WorkflowDefaultSource::Literal,
            value: Some(Value::Bool(true)),
        });

        RuntimeWorkflow {
            identifier: "default_condition".into(),
            title: None,
            description: None,
            inputs: indexmap! {
                "flag".into() => input_definition
            },
            steps: vec![WorkflowStepDefinition {
                id: "gate".into(),
                run: "demo run".into(),
                description: None,
                depends_on: Vec::new(),
                with: IndexMap::new(),
                body: Value::Null,
                r#if: Some("${{ inputs.flag }}".into()),
                repeat: None,
                output_contract: None,
            }],
        }
    }

    #[tokio::test]
    async fn drive_workflow_run_respects_literal_defaults() {
        let workflow = workflow_with_default_condition();
        let request = WorkflowRunRequest {
            run_id: "run-1".into(),
            workflow: workflow.clone(),
            inputs: JsonMap::new(),
            environment: HashMap::new(),
            step_outputs: HashMap::new(),
        };

        let (control_tx, control_rx) = unbounded_channel();
        drop(control_tx);
        let (event_tx, mut event_rx) = unbounded_channel();

        let runner: Arc<dyn CommandRunner + Send + Sync> = Arc::new(executor::runner::NoopRunner);
        drive_workflow_run(request, runner, control_rx, event_tx)
            .await
            .expect("drive workflow run");

        let mut saw_success = false;
        while let Ok(event) = event_rx.try_recv() {
            if let WorkflowRunEvent::StepFinished { status, .. } = event {
                saw_success |= status == WorkflowRunStepStatus::Succeeded;
            }
        }

        assert!(
            saw_success,
            "expected gate step to succeed when default renders the condition truthy"
        );
    }
}

fn map_step_status(status: StepStatus) -> WorkflowRunStepStatus {
    match status {
        StepStatus::Skipped => WorkflowRunStepStatus::Skipped,
        StepStatus::Succeeded => WorkflowRunStepStatus::Succeeded,
        StepStatus::Failed => WorkflowRunStepStatus::Failed,
    }
}

fn emit_step_finished(
    event_tx: &UnboundedSender<WorkflowRunEvent>,
    result: &StepResult,
    status: WorkflowRunStepStatus,
    step: &PreparedStep,
    duration_ms: u64,
) -> Result<()> {
    let event = WorkflowRunEvent::StepFinished {
        step_id: step.id.clone(),
        status,
        output: result.output.clone(),
        logs: result.logs.clone(),
        attempts: result.attempts,
        duration_ms,
    };
    event_tx
        .send(event)
        .map_err(|err| anyhow!("failed to emit step finished event: {}", err))?;

    Ok(())
}

fn dependency_block(step: &PreparedStep, statuses: &HashMap<String, WorkflowRunStepStatus>) -> Option<StepResult> {
    for dependency in &step.depends_on {
        match statuses.get(dependency) {
            Some(WorkflowRunStepStatus::Succeeded) => continue,
            Some(WorkflowRunStepStatus::Failed) => return Some(blocked_result(step.id.clone(), dependency, "failed earlier in the run")),
            Some(WorkflowRunStepStatus::Skipped) => {
                return Some(blocked_result(step.id.clone(), dependency, "did not execute successfully"));
            }
            _ => return Some(blocked_result(step.id.clone(), dependency, "has not executed yet")),
        }
    }
    None
}

fn blocked_result(step_id: String, dependency: &str, detail: &str) -> StepResult {
    StepResult {
        id: step_id.clone(),
        status: StepStatus::Skipped,
        output: Value::Null,
        logs: vec![format!("step '{}' skipped because dependency '{}' {}", step_id, dependency, detail)],
        attempts: 0,
    }
}

fn step_label_lookup(request: &WorkflowRunRequest) -> HashMap<String, Option<String>> {
    request
        .workflow
        .steps
        .iter()
        .map(|step| (step.id.clone(), step.description.clone()))
        .collect()
}

fn drain_pending_commands(
    control_state: &mut ControlState,
    control_rx: &mut UnboundedReceiver<WorkflowRunControl>,
    event_tx: &UnboundedSender<WorkflowRunEvent>,
) -> Result<()> {
    loop {
        match control_rx.try_recv() {
            Ok(command) => control_state.process_command(command, event_tx)?,
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => break,
        }
    }
    Ok(())
}

async fn wait_for_resume(
    control_state: &mut ControlState,
    control_rx: &mut UnboundedReceiver<WorkflowRunControl>,
    event_tx: &UnboundedSender<WorkflowRunEvent>,
) -> Result<()> {
    while control_state.paused && !control_state.cancel_requested {
        match control_rx.recv().await {
            Some(command) => control_state.process_command(command, event_tx)?,
            None => break,
        }
    }
    Ok(())
}

struct ControlState {
    paused: bool,
    cancel_requested: bool,
}

impl ControlState {
    fn new() -> Self {
        Self {
            paused: false,
            cancel_requested: false,
        }
    }

    fn process_command(&mut self, command: WorkflowRunControl, event_tx: &UnboundedSender<WorkflowRunEvent>) -> Result<()> {
        match command {
            WorkflowRunControl::Pause => {
                if !self.paused && !self.cancel_requested {
                    self.paused = true;
                    self.emit_status(event_tx, WorkflowRunStatus::Paused, None)?;
                }
            }
            WorkflowRunControl::Resume => {
                if self.paused {
                    self.paused = false;
                    self.emit_status(event_tx, WorkflowRunStatus::Running, None)?;
                }
            }
            WorkflowRunControl::Cancel => {
                if !self.cancel_requested {
                    self.cancel_requested = true;
                    self.paused = false;
                    self.emit_status(event_tx, WorkflowRunStatus::CancelRequested, Some("aborting…".to_string()))?;
                }
            }
        }
        Ok(())
    }

    fn emit_status(
        &mut self,
        event_tx: &UnboundedSender<WorkflowRunEvent>,
        status: WorkflowRunStatus,
        message: Option<String>,
    ) -> Result<()> {
        event_tx
            .send(WorkflowRunEvent::RunStatusChanged { status, message })
            .map_err(|err| anyhow!("failed to emit run status change: {}", err))
    }
}
