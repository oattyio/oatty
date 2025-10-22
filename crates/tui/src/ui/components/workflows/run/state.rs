//! State management for the workflow run view.
//!
//! The `RunViewState` structure tracks workflow execution metadata,
//! tabular step/output data, and interaction affordances (focus, layout,
//! detail pane visibility). Rendering is delegated to `RunViewComponent`
//! while this module encapsulates state transitions that components mutate
//! in response to engine events or user actions.

use std::{cmp::min, collections::HashMap};

use chrono::{DateTime, Duration, Utc};
use heroku_types::workflow::{WorkflowRunStatus, WorkflowRunStepStatus, WorkflowStepDefinition};
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use serde_json::{Map as JsonMap, Value};

use crate::ui::{
    components::table::state::{KeyValueEntry, TableState, build_key_value_entries},
    theme::Theme,
};

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

impl RunExecutionStatus {
    /// Returns `true` when the run has reached a terminal state.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Canceled)
    }
}

/// Identifies the source backing the detail pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunDetailSource {
    Steps,
    Outputs,
}

/// Captures mouse hit-testing targets for the run view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunViewMouseTarget {
    StepsTable,
    OutputsTable,
    DetailPane,
    CancelButton,
    PauseButton,
}

/// Layout metadata captured during the most recent render pass.
#[derive(Debug, Default, Clone)]
pub struct RunViewLayout {
    last_area: Option<Rect>,
    header_area: Option<Rect>,
    steps_area: Option<Rect>,
    outputs_area: Option<Rect>,
    detail_area: Option<Rect>,
    footer_area: Option<Rect>,
    cancel_button_area: Option<Rect>,
    pause_button_area: Option<Rect>,
    mouse_target_areas: Vec<Rect>,
    mouse_target_roles: Vec<RunViewMouseTarget>,
}

impl RunViewLayout {
    /// Stores the container area used during the most recent render.
    pub fn set_last_area(&mut self, area: Rect) {
        self.last_area = Some(area);
    }

    /// Records the header region.
    pub fn set_header_area(&mut self, area: Rect) {
        self.header_area = Some(area);
    }

    /// Records the steps table region.
    pub fn set_steps_area(&mut self, area: Rect) {
        self.steps_area = Some(area);
    }

    /// Records the outputs table region.
    pub fn set_outputs_area(&mut self, area: Rect) {
        self.outputs_area = Some(area);
    }

    /// Records the detail pane region.
    pub fn set_detail_area(&mut self, area: Option<Rect>) {
        self.detail_area = area;
    }

    /// Records the footer region.
    pub fn set_footer_area(&mut self, area: Rect) {
        self.footer_area = Some(area);
    }

    /// Records the cancel button region.
    pub fn set_cancel_button_area(&mut self, area: Option<Rect>) {
        self.cancel_button_area = area;
    }

    /// Records the pause button region.
    pub fn set_pause_button_area(&mut self, area: Option<Rect>) {
        self.pause_button_area = area;
    }

    /// Updates mouse hit-testing targets.
    pub fn set_mouse_targets(&mut self, targets: Vec<(Rect, RunViewMouseTarget)>) {
        self.mouse_target_areas = targets.iter().map(|(area, _)| *area).collect();
        self.mouse_target_roles = targets.iter().map(|(_, role)| *role).collect();
    }

    /// Returns the outer area recorded during the last render pass.
    pub fn last_area(&self) -> Option<Rect> {
        self.last_area
    }

    /// Returns the most recent steps table area.
    pub fn steps_area(&self) -> Option<Rect> {
        self.steps_area
    }

    /// Returns the most recent outputs table area.
    pub fn outputs_area(&self) -> Option<Rect> {
        self.outputs_area
    }

    /// Returns the most recent detail pane area.
    pub fn detail_area(&self) -> Option<Rect> {
        self.detail_area
    }

    /// Returns the most recent cancel button area.
    pub fn cancel_button_area(&self) -> Option<Rect> {
        self.cancel_button_area
    }

    /// Returns the most recent pause button area.
    pub fn pause_button_area(&self) -> Option<Rect> {
        self.pause_button_area
    }

    /// Returns the configured mouse target rectangles.
    pub fn mouse_target_areas(&self) -> &[Rect] {
        &self.mouse_target_areas
    }

    /// Returns the roles associated with the configured mouse targets.
    pub fn mouse_target_roles(&self) -> &[RunViewMouseTarget] {
        &self.mouse_target_roles
    }
}

/// Detail pane state tracking the active source and selection.
#[derive(Debug, Clone)]
pub struct RunDetailState {
    source: RunDetailSource,
    selection: Option<usize>,
    offset: usize,
}

impl RunDetailState {
    fn new(source: RunDetailSource) -> Self {
        Self {
            source,
            selection: None,
            offset: 0,
        }
    }

    /// Returns the current source driving the detail pane.
    pub fn source(&self) -> RunDetailSource {
        self.source
    }

    /// Sets the selection within the detail pane.
    pub fn set_selection(&mut self, selection: Option<usize>, entry_count: usize) {
        self.selection = selection.map(|index| index.min(entry_count.saturating_sub(1)));
        if let Some(index) = self.selection {
            self.offset = min(index, entry_count.saturating_sub(1));
        } else {
            self.offset = 0;
        }
    }

    /// Adjusts the selection using a signed delta.
    pub fn adjust_selection(&mut self, entry_count: usize, delta: isize) {
        if entry_count == 0 {
            self.selection = None;
            self.offset = 0;
            return;
        }
        let current = self.selection.unwrap_or(0);
        let next = if delta.is_positive() {
            current.saturating_add(delta as usize).min(entry_count.saturating_sub(1))
        } else {
            current.saturating_sub(delta.unsigned_abs())
        };
        self.selection = Some(next);
        self.offset = min(next, entry_count.saturating_sub(1));
    }

    /// Resets scroll offset when entry count shrinks.
    pub fn clamp_offset(&mut self, entry_count: usize) {
        if entry_count == 0 {
            self.offset = 0;
            self.selection = None;
            return;
        }
        self.offset = self.offset.min(entry_count.saturating_sub(1));
        if let Some(selection) = self.selection {
            self.selection = Some(selection.min(entry_count.saturating_sub(1)));
        }
    }

    /// Returns the current selection.
    pub fn selection(&self) -> Option<usize> {
        self.selection
    }

    /// Returns the current offset.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Updates the source while preserving selection metadata.
    pub fn set_source(&mut self, source: RunDetailSource) {
        if self.source != source {
            self.source = source;
            self.selection = None;
            self.offset = 0;
        }
    }
}

/// State container for the workflow run view.
#[derive(Debug)]
pub struct RunViewState {
    run_id: String,
    workflow_identifier: String,
    workflow_title: Option<String>,
    status: RunExecutionStatus,
    status_message: Option<String>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    last_update_at: Option<DateTime<Utc>>,
    is_wide_mode: bool,
    steps_table: TableState<'static>,
    outputs_table: TableState<'static>,
    detail: Option<RunDetailState>,
    container_focus: FocusFlag,
    detail_focus: FocusFlag,
    cancel_button_focus: FocusFlag,
    pause_button_focus: FocusFlag,
    layout: RunViewLayout,
    step_rows: Vec<Value>,
    step_indices: HashMap<String, usize>,
    step_descriptions: HashMap<String, Option<String>>,
    output_rows: Vec<Value>,
}

impl RunViewState {
    /// Creates a new run view state for the provided workflow metadata.
    pub fn new(run_id: String, identifier: String, title: Option<String>) -> Self {
        let mut steps_table = TableState::default();
        steps_table.container_focus = FocusFlag::named("workflow.run.steps");
        steps_table.grid_f = FocusFlag::named("workflow.run.steps.grid");

        let mut outputs_table = TableState::default();
        outputs_table.container_focus = FocusFlag::named("workflow.run.outputs");
        outputs_table.grid_f = FocusFlag::named("workflow.run.outputs.grid");

        Self {
            run_id,
            workflow_identifier: identifier,
            workflow_title: title,
            status: RunExecutionStatus::Pending,
            status_message: None,
            started_at: None,
            completed_at: None,
            last_update_at: None,
            is_wide_mode: false,
            steps_table,
            outputs_table,
            detail: None,
            container_focus: FocusFlag::named("workflow.run"),
            detail_focus: FocusFlag::named("workflow.run.detail"),
            cancel_button_focus: FocusFlag::named("workflow.run.actions.cancel"),
            pause_button_focus: FocusFlag::named("workflow.run.actions.pause"),
            layout: RunViewLayout::default(),
            step_rows: Vec::new(),
            step_indices: HashMap::new(),
            step_descriptions: HashMap::new(),
            output_rows: Vec::new(),
        }
    }

    /// Returns the workflow title when provided.
    pub fn workflow_title(&self) -> Option<&str> {
        self.workflow_title.as_deref()
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

    /// Sets the supplemental status message.
    pub fn set_status_message(&mut self, message: Option<String>) {
        self.status_message = message;
    }

    /// Initializes step rows from the workflow definition.
    pub fn initialize_steps(&mut self, steps: &[WorkflowStepDefinition], theme: &dyn Theme) {
        self.step_rows.clear();
        self.step_indices.clear();
        self.step_descriptions.clear();

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
        }

        self.rebuild_steps_table(theme);
        self.output_rows.clear();
        self.rebuild_outputs_table(theme);
    }

    /// Marks a step as running using its index within the workflow definition order.
    pub fn mark_step_running(&mut self, index: usize, theme: &dyn Theme) {
        if let Some(row) = self.step_rows.get_mut(index).and_then(Value::as_object_mut) {
            row.insert("Status".into(), Value::String("running".into()));
            row.insert("Details".into(), Value::String("Running...".into()));
        }
        self.set_last_update_at(Utc::now());
        self.rebuild_steps_table(theme);
    }

    /// Applies the final status and payload for a finished step.
    pub fn mark_step_finished(
        &mut self,
        step_id: &str,
        status: WorkflowRunStepStatus,
        attempts: u32,
        duration_ms: u64,
        output: Value,
        logs: Vec<String>,
        theme: &dyn Theme,
    ) {
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
        row.insert("Status".into(), Value::String(step_status_label(status).to_string()));
        row.insert("Details".into(), Value::String(step_summary(status, attempts, duration_ms)));
        row.insert("Description".into(), Value::String(description));
        row.insert("Attempts".into(), Value::Number(serde_json::Number::from(attempts)));
        row.insert("DurationMs".into(), Value::Number(serde_json::Number::from(duration_ms)));
        row.insert("Logs".into(), Value::Array(logs.into_iter().map(Value::String).collect()));
        row.insert("Output".into(), output);

        self.step_rows[index] = Value::Object(row);
        self.set_last_update_at(Utc::now());
        self.rebuild_steps_table(theme);
    }

    /// Appends a new output row for display.
    pub fn append_output(&mut self, key: &str, value: Value, detail: Option<Value>, theme: &dyn Theme) {
        let mut row = JsonMap::new();
        row.insert("Key".into(), Value::String(key.to_string()));
        row.insert("Value".into(), Value::String(summarize_value(&value)));
        row.insert("RawValue".into(), value);
        if let Some(detail_value) = detail {
            row.insert("Detail".into(), detail_value);
        }
        self.output_rows.push(Value::Object(row));
        self.rebuild_outputs_table(theme);
        self.outputs_table.normalize();
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

    fn rebuild_steps_table(&mut self, theme: &dyn Theme) {
        let payload = Value::Array(self.step_rows.clone());
        self.steps_table.apply_result_json(Some(payload), theme);
        self.steps_table.normalize();
    }

    fn rebuild_outputs_table(&mut self, theme: &dyn Theme) {
        let payload = Value::Array(self.output_rows.clone());
        self.outputs_table.apply_result_json(Some(payload), theme);
        self.outputs_table.normalize();
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

    /// Returns the start timestamp, if recorded.
    pub fn started_at(&self) -> Option<DateTime<Utc>> {
        self.started_at
    }

    /// Records the completion timestamp.
    pub fn set_completed_at(&mut self, timestamp: DateTime<Utc>) {
        self.completed_at = Some(timestamp);
    }

    /// Returns the completion timestamp, if recorded.
    pub fn completed_at(&self) -> Option<DateTime<Utc>> {
        self.completed_at
    }

    /// Updates the last update timestamp for freshness tracking.
    pub fn set_last_update_at(&mut self, timestamp: DateTime<Utc>) {
        self.last_update_at = Some(timestamp);
    }

    /// Returns the last update timestamp, if recorded.
    pub fn last_update_at(&self) -> Option<DateTime<Utc>> {
        self.last_update_at
    }

    /// Calculates the elapsed duration since `started_at`.
    pub fn elapsed_since_start(&self, now: DateTime<Utc>) -> Option<Duration> {
        self.started_at.map(|start| now - start)
    }

    /// Returns whether wide mode is currently enabled.
    pub fn is_wide_mode(&self) -> bool {
        self.is_wide_mode
    }

    /// Enables or disables wide mode explicitly.
    pub fn set_wide_mode(&mut self, wide: bool) {
        self.is_wide_mode = wide;
    }

    /// Toggles the wide mode flag.
    pub fn toggle_wide_mode(&mut self) {
        self.is_wide_mode = !self.is_wide_mode;
    }

    /// Provides immutable access to the steps table state.
    pub fn steps_table(&self) -> &TableState<'static> {
        &self.steps_table
    }

    /// Provides mutable access to the steps table state.
    pub fn steps_table_mut(&mut self) -> &mut TableState<'static> {
        &mut self.steps_table
    }

    /// Provides immutable access to the outputs table state.
    pub fn outputs_table(&self) -> &TableState<'static> {
        &self.outputs_table
    }

    /// Provides mutable access to the outputs table state.
    pub fn outputs_table_mut(&mut self) -> &mut TableState<'static> {
        &mut self.outputs_table
    }

    /// Returns `true` when the detail pane is visible.
    pub fn is_detail_visible(&self) -> bool {
        self.detail.is_some()
    }

    /// Sets the detail pane to display the provided source.
    pub fn show_detail(&mut self, source: RunDetailSource) {
        match &mut self.detail {
            Some(state) => state.set_source(source),
            None => self.detail = Some(RunDetailState::new(source)),
        }
    }

    /// Hides the detail pane and resets selection metadata.
    pub fn hide_detail(&mut self) {
        self.detail = None;
    }

    /// Returns the current detail state, if visible.
    pub fn detail(&self) -> Option<&RunDetailState> {
        self.detail.as_ref()
    }

    /// Returns the current detail state mutably, if visible.
    pub fn detail_mut(&mut self) -> Option<&mut RunDetailState> {
        self.detail.as_mut()
    }

    /// Returns the focus flag used for the detail pane.
    pub fn detail_focus_flag(&self) -> &FocusFlag {
        &self.detail_focus
    }

    /// Returns the focus flag used for the cancel button.
    pub fn cancel_button_focus_flag(&self) -> &FocusFlag {
        &self.cancel_button_focus
    }

    /// Returns the focus flag used for the pause button.
    pub fn pause_button_focus_flag(&self) -> &FocusFlag {
        &self.pause_button_focus
    }

    /// Returns the focus flag for the steps table container.
    pub fn steps_focus_flag(&self) -> &FocusFlag {
        &self.steps_table.container_focus
    }

    /// Returns the focus flag for the outputs table container.
    pub fn outputs_focus_flag(&self) -> &FocusFlag {
        &self.outputs_table.container_focus
    }

    /// Records the latest render layout information.
    pub fn set_layout(&mut self, layout: RunViewLayout) {
        self.layout = layout;
    }

    /// Returns the most recently captured layout.
    pub fn layout(&self) -> &RunViewLayout {
        &self.layout
    }

    /// Updates the steps table with new rows sourced from JSON.
    pub fn apply_steps_json(&mut self, value: Option<Value>, theme: &dyn Theme) {
        if let Some(Value::Array(rows)) = &value {
            self.step_rows = rows.clone();
        }
        self.steps_table.apply_result_json(value, theme);
    }

    /// Updates the outputs table with new rows sourced from JSON.
    pub fn apply_outputs_json(&mut self, value: Option<Value>, theme: &dyn Theme) {
        if let Some(Value::Array(rows)) = &value {
            self.output_rows = rows.clone();
        }
        self.outputs_table.apply_result_json(value, theme);
    }

    /// Returns the JSON payload backing the current detail pane, if visible.
    pub fn current_detail_payload(&self) -> Option<&Value> {
        let detail_state = self.detail.as_ref()?;
        match detail_state.source {
            RunDetailSource::Steps => self.steps_table.selected_data(),
            RunDetailSource::Outputs => self.outputs_table.selected_data(),
        }
    }

    /// Builds key-value entries for the currently selected detail payload.
    pub fn current_detail_entries(&self) -> Option<Vec<KeyValueEntry>> {
        self.current_detail_payload().map(build_key_value_entries)
    }

    /// Ensures the detail selection and offset remain within bounds after updates.
    pub fn clamp_detail_entries(&mut self) {
        let entry_count = self.current_detail_entries().map(|entries| entries.len()).unwrap_or(0);
        if let Some(detail_state) = self.detail.as_mut() {
            detail_state.clamp_offset(entry_count);
        }
    }

    /// Sets the detail selection explicitly.
    pub fn set_detail_selection(&mut self, selection: Option<usize>) {
        let entry_count = self.current_detail_entries().map(|entries| entries.len()).unwrap_or(0);
        if let Some(detail_state) = self.detail.as_mut() {
            detail_state.set_selection(selection, entry_count);
        }
    }

    /// Adjusts the detail selection with a signed delta.
    pub fn adjust_detail_selection(&mut self, delta: isize) {
        let entry_count = self.current_detail_entries().map(|entries| entries.len()).unwrap_or(0);
        if let Some(detail_state) = self.detail.as_mut() {
            detail_state.adjust_selection(entry_count, delta);
        }
    }

    /// Exposes the container focus flag used by the focus tree.
    pub fn container_focus_flag(&self) -> &FocusFlag {
        &self.container_focus
    }
}

impl HasFocus for RunViewState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.widget(&self.steps_table);
        builder.widget(&self.outputs_table);
        builder.leaf_widget(&self.detail_focus);
        builder.leaf_widget(&self.pause_button_focus);
        builder.leaf_widget(&self.cancel_button_focus);
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        self.layout.last_area().unwrap_or_default()
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

fn summarize_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::dracula::DraculaTheme;
    use heroku_types::workflow::WorkflowStepDefinition;
    use indexmap::IndexMap;
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

        let row = state.steps_table().selected_data().cloned().expect("row exists");
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
            WorkflowRunStepStatus::Succeeded,
            2,
            150,
            json!({"result": "ok"}),
            vec!["completed".into()],
            &theme,
        );

        let row = state.steps_table().selected_data().cloned().expect("row exists");
        assert_eq!(row["Status"], Value::String("succeeded".into()));
        let summary = row["Details"].as_str().expect("summary");
        assert!(summary.contains("succeeded"));
        assert_eq!(row["Output"], json!({"result": "ok"}));
    }

    #[test]
    fn append_output_records_rows() {
        let theme = DraculaTheme::new();
        let mut state = RunViewState::new("run-1".into(), "workflow".into(), None);
        state.initialize_steps(&[make_step("alpha", None)], &theme);

        state.append_output("alpha", json!(42), None, &theme);

        let row = state.outputs_table().selected_data().cloned().expect("row exists");
        assert_eq!(row["Key"], Value::String("alpha".into()));
        assert_eq!(row["Value"], Value::String("42".into()));
        assert_eq!(row["RawValue"], json!(42));
    }
}
