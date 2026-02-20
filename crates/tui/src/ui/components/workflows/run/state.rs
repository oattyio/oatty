//! State management for the workflow run view.
//!
//! The `RunViewState` structure tracks workflow execution metadata,
//! tabular step/output data, and interaction affordances (focus, layout,
//! detail pane visibility). Rendering is delegated to `RunViewComponent`
//! while this module encapsulates state transitions that components mutate
//! in response to engine events or user actions.

use crate::ui::{components::results::state::ResultsTableState, theme::Theme};
use chrono::{DateTime, Duration, Utc};
use oatty_types::workflow::{WorkflowRunControl, WorkflowRunStatus, WorkflowRunStepStatus, WorkflowStepDefinition};
use oatty_util::format_duration_short;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use serde_json::{Map as JsonMap, Value};
use std::collections::HashMap;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::Instant;

const FINAL_OUTPUT_INTERNAL_STEP_ID: &str = "__oatty_workflow_final_output__";
const FINAL_OUTPUT_DISPLAY_STEP_LABEL: &str = "workflow.final_output";
const INTERNAL_STEP_ID_COLUMN: &str = "_internal_step_id";

/// High-level execution status for a workflow run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunExecutionStatus {
    Pending,
    Running,
    Paused,
    CancelRequested,
    Succeeded,
    Failed,
    Canceled,
}

/// Handle that allows the UI to dispatch control commands to the active workflow run.
#[derive(Debug, Clone)]
pub struct WorkflowRunControlHandle {
    pub sender: UnboundedSender<WorkflowRunControl>,
}

impl RunExecutionStatus {
    /// Returns `true` when the run has reached a terminal state.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Canceled)
    }
}

/// Aggregates metadata captured when a workflow step finishes.
#[derive(Debug)]
pub struct StepFinishedData {
    pub status: WorkflowRunStepStatus,
    pub attempts: u32,
    pub duration_ms: u64,
    pub output: Value,
    pub logs: Vec<String>,
}

/// State container for the workflow run view.
#[derive(Debug)]
pub struct RunViewState {
    pub container_focus: FocusFlag,
    pub cancel_button_focus: FocusFlag,
    pub pause_button_focus: FocusFlag,
    pub view_details_button_focus: FocusFlag,
    pub done_button_focus: FocusFlag,
    pub steps_table: ResultsTableState<'static>,
    run_id: String,
    workflow_identifier: String,
    workflow_title: Option<String>,
    status: RunExecutionStatus,
    status_message: Option<String>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    last_update_at: Option<DateTime<Utc>>,
    step_rows: Vec<Value>,
    outputs: HashMap<String, Value>,
    step_indices: HashMap<String, usize>,
    step_descriptions: HashMap<String, Option<String>>,
    step_repeat_limits: HashMap<String, Option<u32>>,
    running_repeat_steps: HashMap<String, RepeatAnimationState>,
}

impl RunViewState {
    /// Creates a new run view state for the provided workflow metadata.
    pub fn new(run_id: String, identifier: String, title: Option<String>) -> Self {
        let mut steps_table = ResultsTableState::default();
        steps_table.container_focus = FocusFlag::new().with_name("workflow.run.steps");
        steps_table.grid_f = FocusFlag::new().with_name("workflow.run.steps.grid");

        Self {
            run_id,
            workflow_identifier: identifier,
            workflow_title: title,
            status: RunExecutionStatus::Pending,
            status_message: None,
            started_at: None,
            completed_at: None,
            last_update_at: None,
            steps_table,
            container_focus: FocusFlag::new().with_name("workflow.run"),
            cancel_button_focus: FocusFlag::new().with_name("workflow.run.actions.cancel"),
            pause_button_focus: FocusFlag::new().with_name("workflow.run.actions.pause"),
            view_details_button_focus: FocusFlag::new().with_name("workflow.run.actions.view_details"),
            done_button_focus: FocusFlag::new().with_name("workflow.run.actions.done"),
            step_rows: Vec::new(),
            outputs: HashMap::new(),
            step_indices: HashMap::new(),
            step_descriptions: HashMap::new(),
            step_repeat_limits: HashMap::new(),
            running_repeat_steps: HashMap::new(),
        }
    }

    pub fn append_output(&mut self, key: &str, value: Value) {
        self.outputs.insert(key.to_string(), value);
    }

    /// Upserts a pseudo step row representing the workflow-level final output.
    pub fn upsert_final_output_row(&mut self, final_output: Value, theme: &dyn Theme) {
        self.outputs.insert(FINAL_OUTPUT_INTERNAL_STEP_ID.to_string(), final_output.clone());

        let mut row = JsonMap::new();
        row.insert("Step".into(), Value::String(FINAL_OUTPUT_DISPLAY_STEP_LABEL.to_string()));
        row.insert(
            INTERNAL_STEP_ID_COLUMN.into(),
            Value::String(FINAL_OUTPUT_INTERNAL_STEP_ID.to_string()),
        );
        row.insert("Status".into(), Value::String("summary".into()));
        row.insert("Details".into(), Value::String("Workflow final output".into()));
        row.insert("Description".into(), Value::String("Aggregated workflow output".into()));
        row.insert("Output".into(), final_output);

        if let Some(index) = self.step_indices.get(FINAL_OUTPUT_INTERNAL_STEP_ID).copied() {
            self.step_rows[index] = Value::Object(row);
        } else {
            let index = self.step_rows.len();
            self.step_rows.push(Value::Object(row));
            self.step_indices.insert(FINAL_OUTPUT_INTERNAL_STEP_ID.to_string(), index);
            self.step_descriptions.insert(
                FINAL_OUTPUT_INTERNAL_STEP_ID.to_string(),
                Some("Aggregated workflow output".to_string()),
            );
        }

        self.rebuild_steps_table(theme);
    }

    pub fn output_by_index(&self, index: usize) -> Option<Value> {
        let row = self.step_rows.get(index)?;
        let step_id = row
            .get(INTERNAL_STEP_ID_COLUMN)
            .or_else(|| row.get("Step"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        self.outputs.get(&step_id).cloned().or_else(|| row.get("Output").cloned())
    }

    /// Returns the identifier of the first failed workflow step, if one exists.
    ///
    /// Pseudo rows (for example `workflow.final_output`) are excluded.
    pub fn first_failed_step_identifier(&self) -> Option<String> {
        self.step_rows.iter().find_map(|row| {
            let row_object = row.as_object()?;
            let status = row_object.get("Status")?.as_str()?;
            if !status.eq_ignore_ascii_case("failed") {
                return None;
            }

            let step_identifier = row_object
                .get(INTERNAL_STEP_ID_COLUMN)
                .or_else(|| row_object.get("Step"))
                .and_then(Value::as_str)?;
            if step_identifier == FINAL_OUTPUT_INTERNAL_STEP_ID || step_identifier == FINAL_OUTPUT_DISPLAY_STEP_LABEL {
                return None;
            }
            Some(step_identifier.to_string())
        })
    }

    /// Computes a display name favoring the title over the identifier.
    pub fn display_name(&self) -> &str {
        self.workflow_title
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or(&self.workflow_identifier)
    }

    /// Returns the unique run identifier associated with this view.
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    /// Returns any supplemental status message (for example, aborting…).
    pub fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }

    /// Initializes step rows from the workflow definition.
    pub fn initialize_steps(&mut self, steps: &[WorkflowStepDefinition], theme: &dyn Theme) {
        self.step_rows.clear();
        self.step_indices.clear();
        self.step_descriptions.clear();
        self.step_repeat_limits.clear();
        self.running_repeat_steps.clear();

        for (index, step) in steps.iter().enumerate() {
            let mut row = JsonMap::new();
            row.insert("Step".into(), Value::String(step.id.clone()));
            row.insert("Status".into(), Value::String("pending".into()));
            let description = step.description.clone().unwrap_or_default();
            row.insert("Details".into(), Value::String(description.clone()));
            row.insert("Description".into(), Value::String(description));
            self.step_rows.push(Value::Object(row));
            self.step_indices.insert(step.id.clone(), index);
            self.step_descriptions.insert(step.id.clone(), step.description.clone());
            if let Some(repeat) = step.repeat.as_ref() {
                self.step_repeat_limits.insert(step.id.clone(), repeat.max_attempts);
            }
        }

        self.rebuild_steps_table(theme);
    }

    /// Marks a step as running using its index within the workflow definition order.
    pub fn mark_step_running(&mut self, index: usize, step_id: &str, theme: &dyn Theme) {
        if let Some(row) = self.step_rows.get_mut(index).and_then(Value::as_object_mut) {
            row.insert("Status".into(), Value::String("running".into()));
            if self.step_repeat_limits.contains_key(step_id) {
                let limit = self.step_repeat_limits.get(step_id).cloned().flatten();
                let detail_text = {
                    let animation = self
                        .running_repeat_steps
                        .entry(step_id.to_string())
                        .or_insert_with(RepeatAnimationState::new);
                    animation.set_attempt(1);
                    animation.describe(limit)
                };
                row.insert("Details".into(), Value::String(detail_text));
            } else {
                row.insert("Details".into(), Value::String("Running...".into()));
            }
        }
        self.set_last_update_at(Utc::now());
        self.rebuild_steps_table(theme);
    }

    /// Applies the final status and payload for a finished step.
    pub fn mark_step_finished(&mut self, step_id: &str, data: StepFinishedData, theme: &dyn Theme) {
        let Some(&index) = self.step_indices.get(step_id) else {
            return;
        };

        let description = self
            .step_descriptions
            .get(step_id)
            .and_then(|value| value.clone())
            .unwrap_or_default();

        let mut row = JsonMap::new();
        row.insert("Step".into(), Value::String(step_id.to_string()));
        row.insert("Status".into(), Value::String(step_status_label(data.status).to_string()));
        row.insert(
            "Details".into(),
            Value::String(step_summary(data.status, data.attempts, data.duration_ms)),
        );
        row.insert("Description".into(), Value::String(description));
        row.insert("Attempts".into(), Value::Number(serde_json::Number::from(data.attempts)));
        row.insert(
            "Duration".into(),
            Value::String(format_duration_short(Duration::milliseconds(data.duration_ms as i64))),
        );
        row.insert("Logs".into(), Value::Array(data.logs.into_iter().map(Value::String).collect()));
        row.insert("Output".into(), data.output);

        self.step_rows[index] = Value::Object(row);
        self.running_repeat_steps.remove(step_id);
        self.set_last_update_at(Utc::now());
        self.rebuild_steps_table(theme);
    }

    /// Records that the run has started.
    pub fn handle_run_started(&mut self, timestamp: DateTime<Utc>) {
        self.set_started_at(timestamp);
        self.set_status(RunExecutionStatus::Running);
        self.status_message = None;
        self.set_last_update_at(timestamp);
    }

    /// Applies a status change emitted by the engine.
    pub fn apply_status_change(&mut self, status: WorkflowRunStatus, message: Option<String>) {
        let mapped = run_status_to_execution(status);
        self.set_status(mapped);
        self.status_message = message;
        self.set_last_update_at(Utc::now());
    }

    /// Marks the run as completed with the provided terminal status.
    pub fn handle_run_completed(&mut self, status: WorkflowRunStatus, finished_at: DateTime<Utc>, message: Option<String>) {
        self.set_completed_at(finished_at);
        self.apply_status_change(status, message);
    }

    /// Advances the spinner animation for repeating steps that are currently running.
    pub fn advance_repeat_animations(&mut self, theme: &dyn Theme) {
        if self.running_repeat_steps.is_empty() {
            return;
        }

        let mut dirty = false;
        let step_ids: Vec<String> = self.running_repeat_steps.keys().cloned().collect();
        for step_id in step_ids {
            if let Some(state) = self.running_repeat_steps.get_mut(&step_id) {
                state.advance();
                let limit = self.step_repeat_limits.get(&step_id).copied().flatten();
                let detail_text = state.describe(limit);
                dirty |= self.update_running_repeat_row(&step_id, detail_text);
            }
        }

        if dirty {
            self.rebuild_steps_table(theme);
        }
    }

    /// Updates the visible attempt counter for an in-flight repeating step.
    pub fn update_repeat_attempt(&mut self, step_id: &str, attempt: u32, max_attempts: Option<u32>, theme: &dyn Theme) {
        if let Some(limit) = max_attempts {
            self.step_repeat_limits.insert(step_id.to_string(), Some(limit));
        }

        let limit = self.step_repeat_limits.get(step_id).copied().flatten();
        let detail_text = {
            let animation = self
                .running_repeat_steps
                .entry(step_id.to_string())
                .or_insert_with(RepeatAnimationState::new);
            animation.set_attempt(attempt);
            animation.describe(limit)
        };

        if self.update_running_repeat_row(step_id, detail_text) {
            self.rebuild_steps_table(theme);
        }
    }

    fn update_running_repeat_row(&mut self, step_id: &str, detail_text: String) -> bool {
        let Some(&index) = self.step_indices.get(step_id) else {
            return false;
        };
        let Some(row) = self.step_rows.get_mut(index).and_then(Value::as_object_mut) else {
            return false;
        };
        row.insert("Details".into(), Value::String(detail_text));
        true
    }

    fn rebuild_steps_table(&mut self, theme: &dyn Theme) {
        let selected_step_identifier = self.selected_step_identifier();
        let payload = Value::Array(self.step_rows.clone());
        self.steps_table.apply_result_json(Some(payload), theme, false);
        self.restore_selected_step_by_identifier(selected_step_identifier);
    }

    fn selected_step_identifier(&self) -> Option<String> {
        let selected_index = self.steps_table.table_state.selected()?;
        self.step_rows
            .get(selected_index)
            .and_then(Value::as_object)
            .and_then(|row| row.get(INTERNAL_STEP_ID_COLUMN).or_else(|| row.get("Step")))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    }

    fn restore_selected_step_by_identifier(&mut self, selected_step_identifier: Option<String>) {
        let Some(selected_step_identifier) = selected_step_identifier else {
            return;
        };
        let selected_index = self.step_rows.iter().position(|row| {
            row.as_object()
                .and_then(|row| row.get(INTERNAL_STEP_ID_COLUMN).or_else(|| row.get("Step")))
                .and_then(Value::as_str)
                .is_some_and(|step_id| step_id == selected_step_identifier)
        });
        self.steps_table.table_state.select(selected_index);
    }

    /// Returns the current execution status.
    pub fn status(&self) -> RunExecutionStatus {
        self.status
    }

    /// Sets the execution status.
    pub fn set_status(&mut self, status: RunExecutionStatus) {
        self.status = status;
    }

    /// Records the start timestamp for elapsed-time calculations.
    pub fn set_started_at(&mut self, timestamp: DateTime<Utc>) {
        self.started_at = Some(timestamp);
    }

    /// Records the completion timestamp.
    pub fn set_completed_at(&mut self, timestamp: DateTime<Utc>) {
        self.completed_at = Some(timestamp);
    }

    /// Updates the last update timestamp for freshness tracking.
    pub fn set_last_update_at(&mut self, timestamp: DateTime<Utc>) {
        self.last_update_at = Some(timestamp);
    }

    /// Calculates the elapsed duration since `started_at`.
    pub fn elapsed_since_start(&self, now: DateTime<Utc>) -> Option<Duration> {
        self.started_at.map(|start| now - start)
    }

    /// Provides mutable access to the step results state.
    pub fn steps_table_mut(&mut self) -> &mut ResultsTableState<'static> {
        &mut self.steps_table
    }
}

impl HasFocus for RunViewState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.widget(&self.steps_table);
        if matches!(self.status, RunExecutionStatus::Running | RunExecutionStatus::Paused) {
            builder.leaf_widget(&self.pause_button_focus);
        }
        builder.leaf_widget(&self.cancel_button_focus);
        if self.steps_table.table_state.selected().is_some() {
            builder.leaf_widget(&self.view_details_button_focus);
        }
        if matches!(self.status, RunExecutionStatus::Succeeded | RunExecutionStatus::Failed) {
            builder.leaf_widget(&self.done_button_focus);
        }

        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}

fn run_status_to_execution(status: WorkflowRunStatus) -> RunExecutionStatus {
    match status {
        WorkflowRunStatus::Pending => RunExecutionStatus::Pending,
        WorkflowRunStatus::Running => RunExecutionStatus::Running,
        WorkflowRunStatus::Paused => RunExecutionStatus::Paused,
        WorkflowRunStatus::CancelRequested => RunExecutionStatus::CancelRequested,
        WorkflowRunStatus::Succeeded => RunExecutionStatus::Succeeded,
        WorkflowRunStatus::Failed => RunExecutionStatus::Failed,
        WorkflowRunStatus::Canceled => RunExecutionStatus::Canceled,
    }
}

fn step_status_label(status: WorkflowRunStepStatus) -> &'static str {
    match status {
        WorkflowRunStepStatus::Pending => "pending",
        WorkflowRunStepStatus::Running => "running",
        WorkflowRunStepStatus::Succeeded => "succeeded",
        WorkflowRunStepStatus::Failed => "failed",
        WorkflowRunStepStatus::Skipped => "skipped",
    }
}

fn step_summary(status: WorkflowRunStepStatus, attempts: u32, duration_ms: u64) -> String {
    let label = step_status_label(status);
    if status == WorkflowRunStepStatus::Skipped {
        label.to_string()
    } else {
        let attempts = attempts.max(1);
        format!("{} ({} attempt(s), {} ms)", label, attempts, duration_ms)
    }
}

const RUNNING_SPINNER_FRAMES: [&str; 3] = [".  ", ".. ", "..."];

#[derive(Debug, Clone)]
struct RepeatAnimationState {
    attempt: u32,
    spinner_index: usize,
    last_tick: Instant,
}

impl RepeatAnimationState {
    fn new() -> Self {
        Self {
            attempt: 1,
            spinner_index: 0,
            last_tick: Instant::now(),
        }
    }

    fn advance(&mut self) {
        // Advance one frame per call; the UI drive controls cadence.
        // This makes animation deterministic in tests without relying on wall time.
        self.spinner_index = (self.spinner_index + 1) % RUNNING_SPINNER_FRAMES.len();
        self.last_tick = Instant::now();
    }

    fn set_attempt(&mut self, attempt: u32) {
        self.attempt = attempt.max(1);
        self.spinner_index = 0;
    }

    fn describe(&self, limit: Option<u32>) -> String {
        let frame = RUNNING_SPINNER_FRAMES[self.spinner_index];
        let limit_label = limit.map(|value| value.to_string()).unwrap_or_else(|| "∞".to_string());
        format!("Running{} (attempt {}/{})", frame, self.attempt, limit_label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::dracula::DraculaTheme;
    use indexmap::IndexMap;
    use oatty_types::workflow::{WorkflowRepeat, WorkflowStepDefinition};
    use serde_json::{Value, json};

    fn make_step(id: &str, description: Option<&str>) -> WorkflowStepDefinition {
        WorkflowStepDefinition {
            id: id.into(),
            run: "cmd".into(),
            description: description.map(|text| text.into()),
            depends_on: Vec::new(),
            r#if: None,
            with: IndexMap::new(),
            body: Value::Null,
            repeat: None,
            output_contract: None,
        }
    }

    #[test]
    fn initialize_steps_sets_pending_status() {
        let theme = DraculaTheme::new();
        let mut state = RunViewState::new("run-1".into(), "workflow".into(), Some("Example".into()));
        state.initialize_steps(&[make_step("alpha", Some("first step"))], &theme);

        let row = state.steps_table.selected_data(0).cloned().expect("row exists");
        assert_eq!(row["Step"], Value::String("alpha".into()));
        assert_eq!(row["Status"], Value::String("pending".into()));
        assert_eq!(row["Details"], Value::String("first step".into()));
    }

    #[test]
    fn mark_step_finished_updates_status_and_output() {
        let theme = DraculaTheme::new();
        let mut state = RunViewState::new("run-1".into(), "workflow".into(), None);
        state.initialize_steps(&[make_step("alpha", None)], &theme);

        state.mark_step_finished(
            "alpha",
            StepFinishedData {
                status: WorkflowRunStepStatus::Succeeded,
                attempts: 2,
                duration_ms: 150,
                output: json!({"result": "ok"}),
                logs: vec!["completed".into()],
            },
            &theme,
        );

        let row = state.steps_table.selected_data(0).cloned().expect("row exists");
        assert_eq!(row["Status"], Value::String("succeeded".into()));
        let summary = row["Details"].as_str().expect("summary");
        assert!(summary.contains("succeeded"));
        assert_eq!(row["Output"], json!({"result": "ok"}));
        assert_eq!(state.output_by_index(0), Some(json!({"result": "ok"})));
    }

    #[test]
    fn repeat_steps_show_animated_running_status() {
        let theme = DraculaTheme::new();
        let mut repeating_step = make_step("alpha", None);
        repeating_step.repeat = Some(WorkflowRepeat {
            until: Some("steps.alpha.output.status == \"ready\"".into()),
            every: Some("5s".into()),
            timeout: None,
            max_attempts: None,
        });

        let mut state = RunViewState::new("run-1".into(), "workflow".into(), None);
        state.initialize_steps(&[repeating_step], &theme);

        state.mark_step_running(0, "alpha", &theme);
        let row = state.steps_table.selected_data(0).cloned().expect("row exists");
        assert_eq!(row["Details"], Value::String("Running.   (attempt 1/∞)".into()));

        state.update_repeat_attempt("alpha", 3, Some(5), &theme);
        let row = state.steps_table.selected_data(0).cloned().expect("row exists");
        assert_eq!(row["Details"], Value::String("Running.   (attempt 3/5)".into()));

        state.advance_repeat_animations(&theme);
        let row = state.steps_table.selected_data(0).cloned().expect("row exists");
        assert_eq!(row["Details"], Value::String("Running..  (attempt 3/5)".into()));
    }

    #[test]
    fn repeat_updates_preserve_user_selected_row() {
        let theme = DraculaTheme::new();
        let mut alpha = make_step("alpha", None);
        alpha.repeat = Some(WorkflowRepeat {
            until: Some("steps.alpha.output.status == \"ready\"".into()),
            every: Some("5s".into()),
            timeout: None,
            max_attempts: Some(5),
        });
        let beta = make_step("beta", Some("completed"));

        let mut state = RunViewState::new("run-1".into(), "workflow".into(), None);
        state.initialize_steps(&[alpha, beta], &theme);

        state.mark_step_finished(
            "beta",
            StepFinishedData {
                status: WorkflowRunStepStatus::Succeeded,
                attempts: 1,
                duration_ms: 10,
                output: json!({"ok": true}),
                logs: vec!["ok".into()],
            },
            &theme,
        );
        state.steps_table.table_state.select(Some(1));

        state.mark_step_running(0, "alpha", &theme);
        state.update_repeat_attempt("alpha", 2, Some(5), &theme);
        state.advance_repeat_animations(&theme);

        assert_eq!(state.steps_table.table_state.selected(), Some(1));
        let selected_row = state.steps_table.selected_data(1).cloned().expect("row exists");
        assert_eq!(selected_row["Step"], Value::String("beta".into()));
    }

    #[test]
    fn final_output_row_does_not_overwrite_real_step_with_same_identifier() {
        let theme = DraculaTheme::new();
        let mut state = RunViewState::new("run-1".into(), "workflow".into(), None);
        state.initialize_steps(&[make_step("final_output", None)], &theme);
        state.mark_step_finished(
            "final_output",
            StepFinishedData {
                status: WorkflowRunStepStatus::Succeeded,
                attempts: 1,
                duration_ms: 10,
                output: json!({"real": true}),
                logs: vec!["ok".into()],
            },
            &theme,
        );

        state.upsert_final_output_row(json!({"summary": true}), &theme);

        assert_eq!(state.steps_table.num_rows(), 2);
        let real_step_index = state
            .step_rows
            .iter()
            .position(|row| row["Step"] == Value::String("final_output".into()))
            .expect("real step row present");
        let summary_index = state
            .step_rows
            .iter()
            .position(|row| row["Step"] == Value::String("workflow.final_output".into()))
            .expect("summary row present");

        assert_eq!(state.output_by_index(real_step_index), Some(json!({"real": true})));
        assert_eq!(state.output_by_index(summary_index), Some(json!({"summary": true})));
    }
}
