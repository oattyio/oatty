use crate::ui::components::common::TextInputState;
use crate::ui::components::table::TableState;
use crate::ui::components::workflows::collector::manual_entry::ManualEntryState;
use crate::ui::components::workflows::input::WorkflowInputViewState;
use crate::ui::components::workflows::run::RunViewState;
use crate::ui::theme::Theme;
use anyhow::{Result, anyhow};
use heroku_engine::workflow::document::build_runtime_catalog;
use heroku_engine::{ProviderBindingOutcome, WorkflowRunState};
use heroku_registry::{CommandRegistry, feat_gate::feature_workflows, utils::find_by_group_and_cmd};
use heroku_types::WorkflowProviderErrorPolicy;
use heroku_types::{
    command::SchemaProperty,
    workflow::{
        RuntimeWorkflow, WorkflowInputDefinition, WorkflowRunControl, WorkflowRunEvent, WorkflowRunStatus, WorkflowRunStepStatus,
        WorkflowValueProvider,
    },
};
use indexmap::IndexMap;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::{layout::Rect, widgets::ListState};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;

/// Maintains the workflow catalogue, filtered view, and list selection state for the picker UI.
#[derive(Debug, Default)]
pub struct WorkflowListState {
    pub selected: usize,
    pub f_list: FocusFlag,

    workflows: Vec<RuntimeWorkflow>,
    filtered_indices: Vec<usize>,
    search_input: TextInputState,
    list_state: ListState,
    container_focus: FocusFlag,
}

impl WorkflowListState {
    /// Creates a new workflow list state with default focus and selection values.
    pub fn new() -> Self {
        Self {
            workflows: Vec::new(),
            filtered_indices: Vec::new(),
            search_input: TextInputState::new(),
            selected: 0,
            list_state: ListState::default(),
            container_focus: FocusFlag::named("root.workflows"),
            f_list: FocusFlag::named("root.workflows.list"),
        }
    }

    /// Loads workflow definitions from the registry when the feature flag is enabled.
    ///
    /// The list is populated once, and later calls are inexpensive no-ops.
    pub fn ensure_loaded(&mut self, registry: &Arc<Mutex<CommandRegistry>>) -> Result<()> {
        if !feature_workflows() {
            self.workflows.clear();
            self.filtered_indices.clear();
            self.search_input.set_input("");
            self.search_input.set_cursor(0);
            self.list_state.select(None);
            return Ok(());
        }

        if self.workflows.is_empty() {
            let definitions = &registry.lock().map_err(|_| anyhow!("could not lock registry"))?.workflows;

            let catalog = build_runtime_catalog(definitions)?;
            self.workflows = catalog.into_values().collect();
            self.rebuild_filter();
        }

        Ok(())
    }

    /// Returns the selected workflow from the filtered list, if one is available.
    pub fn selected_workflow(&self) -> Option<&RuntimeWorkflow> {
        self.filtered_indices
            .get(self.selected)
            .and_then(|index| self.workflows.get(*index))
    }

    /// Advances the selection to the next visible workflow, wrapping cyclically.
    pub fn select_next(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.filtered_indices.len();
        self.list_state.select(Some(self.selected));
    }

    /// Moves the selection to the previous visible workflow, wrapping cyclically.
    pub fn select_prev(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        if self.selected == 0 {
            self.selected = self.filtered_indices.len() - 1;
        } else {
            self.selected -= 1;
        }
        self.list_state.select(Some(self.selected));
    }

    /// Exposes the internal list state for Ratatui rendering.
    pub fn list_state(&mut self) -> &mut ListState {
        &mut self.list_state
    }

    /// Returns the search query currently active for filtering.
    pub fn search_query(&self) -> &str {
        self.search_input.input()
    }

    /// Returns the current cursor (byte index) within the search query.
    pub fn search_cursor(&self) -> usize {
        self.search_input.cursor()
    }

    /// Move search cursor one character to the left (UTF‑8 safe).
    pub fn move_search_left(&mut self) {
        self.search_input.move_left();
        // moving cursor alone does not affect filtering
    }

    /// Move search cursor one character to the right (UTF‑8 safe).
    pub fn move_search_right(&mut self) {
        self.search_input.move_right();
    }

    /// Appends a character to the search query and rebuilds the filtered view.
    pub fn append_search_char(&mut self, character: char) {
        self.search_input.insert_char(character);
        self.rebuild_filter();
    }

    /// Removes the character before the cursor and rebuilds the filter.
    pub fn pop_search_char(&mut self) {
        self.search_input.backspace();
        self.rebuild_filter();
    }

    /// Clears the search query and shows all workflows.
    pub fn clear_search(&mut self) {
        if self.search_input.is_empty() {
            return;
        }
        self.search_input.set_input("");
        self.search_input.set_cursor(0);
        self.rebuild_filter();
    }

    /// Returns the number of workflows matching the current filter.
    pub fn filtered_count(&self) -> usize {
        self.filtered_indices.len()
    }

    /// Returns the total number of workflows loaded from the registry.
    pub fn total_count(&self) -> usize {
        self.workflows.len()
    }

    /// Provides the filtered indices for callers that need to inspect visible workflows.
    pub fn filtered_indices(&self) -> &[usize] {
        &self.filtered_indices
    }

    /// Returns the workflow stored at a specific absolute index.
    pub fn workflow_by_index(&self, index: usize) -> Option<&RuntimeWorkflow> {
        self.workflows.get(index)
    }

    /// Calculates the width required for the identifier column using the filtered set.
    pub fn filtered_title_width(&self) -> usize {
        self.filtered_indices
            .iter()
            .filter_map(|index| self.workflows.get(*index))
            .map(|workflow| {
                workflow
                    .title
                    .as_ref()
                    .and_then(|t| Some(t.len()))
                    .unwrap_or(workflow.identifier.len())
            })
            .max()
            .unwrap_or(0)
    }

    fn rebuild_filter(&mut self) {
        if self.workflows.is_empty() {
            self.filtered_indices.clear();
            self.selected = 0;
            self.list_state.select(None);
            return;
        }

        let query = self.search_query().trim().to_lowercase();
        if query.is_empty() {
            self.filtered_indices = (0..self.workflows.len()).collect();
        } else {
            self.filtered_indices = self
                .workflows
                .iter()
                .enumerate()
                .filter(|(_, workflow)| Self::matches_search(workflow, &query))
                .map(|(index, _)| index)
                .collect();
        }

        if self.filtered_indices.is_empty() {
            self.selected = 0;
            self.list_state.select(None);
        } else {
            if self.selected >= self.filtered_indices.len() {
                self.selected = 0;
            }
            self.list_state.select(Some(self.selected));
        }
    }

    fn matches_search(workflow: &RuntimeWorkflow, lower_query: &str) -> bool {
        let identifier_matches = workflow.identifier.to_lowercase().contains(lower_query);
        let title_matches = workflow
            .title
            .as_deref()
            .map(|title| title.to_lowercase().contains(lower_query))
            .unwrap_or(false);
        let description_matches = workflow
            .description
            .as_deref()
            .map(|description| description.to_lowercase().contains(lower_query))
            .unwrap_or(false);

        identifier_matches || title_matches || description_matches
    }
}

impl HasFocus for WorkflowListState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.leaf_widget(&self.f_list);
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}

/// Handle that allows the UI to dispatch control commands to the active workflow run.
#[derive(Debug, Clone)]
pub struct WorkflowRunControlHandle {
    pub run_id: String,
    pub sender: UnboundedSender<WorkflowRunControl>,
}

/// Aggregates workflow list state with execution metadata, modal visibility, and provider cache snapshots.
#[derive(Debug, Default)]
pub struct WorkflowState {
    pub list: WorkflowListState,
    active_run_state: Option<WorkflowRunState>,
    selected_metadata: Option<WorkflowSelectionMetadata>,
    input_view: Option<WorkflowInputViewState>,
    run_view: Option<RunViewState>,
    /// Manual entry modal state (when open); None when not editing.
    pub manual_entry: Option<ManualEntryState>,
    /// Provider-backed selector state (when open); None when not selecting.
    pub selector: Option<WorkflowSelectorViewState<'static>>,
    /// The focus flags for the workflow view.
    pub container_focus: FocusFlag,
    pub f_search: FocusFlag,
    active_run_id: Option<String>,
    run_control: Option<WorkflowRunControlHandle>,
}

impl WorkflowState {
    /// Creates a new workflow view state with the default list configuration.
    pub fn new() -> Self {
        Self {
            list: WorkflowListState::new(),
            active_run_state: None,
            selected_metadata: None,
            input_view: None,
            run_view: None,
            manual_entry: None,
            selector: None,
            container_focus: FocusFlag::named("workflow.container"),
            f_search: FocusFlag::named("workflow.search"),
            active_run_id: None,
            run_control: None,
        }
    }

    /// Lazily loads workflows from the registry and refreshes derived metadata.
    pub fn ensure_loaded(&mut self, registry: &Arc<Mutex<CommandRegistry>>) -> Result<()> {
        self.list.ensure_loaded(registry)?;
        self.refresh_selection_metadata();
        Ok(())
    }

    /// Returns the number of workflows after applying the active search filter.
    pub fn filtered_count(&self) -> usize {
        self.list.filtered_count()
    }

    /// Returns the total number of workflows available from the registry.
    pub fn total_count(&self) -> usize {
        self.list.total_count()
    }

    /// Provides read access to the active search query.
    pub fn search_query(&self) -> &str {
        self.list.search_query()
    }

    /// Provides the current cursor position (byte index) in the search input.
    pub fn search_cursor(&self) -> usize {
        self.list.search_cursor()
    }

    /// Move search cursor one character to the left (UTF‑8 safe).
    pub fn move_search_left(&mut self) {
        self.list.move_search_left();
    }

    /// Move search cursor one character to the right (UTF‑8 safe).
    pub fn move_search_right(&mut self) {
        self.list.move_search_right();
    }

    /// Updates the search query and recalculates the filtered list.
    pub fn append_search_char(&mut self, character: char) {
        self.list.append_search_char(character);
        self.refresh_selection_metadata();
    }

    /// Removes the trailing character from the search query.
    pub fn pop_search_char(&mut self) {
        self.list.pop_search_char();
        self.refresh_selection_metadata();
    }

    /// Clears any search filters currently applied to the workflow catalogue.
    pub fn clear_search(&mut self) {
        self.list.clear_search();
        self.refresh_selection_metadata();
    }

    /// Advances the selection to the next workflow and updates metadata.
    pub fn select_next(&mut self) {
        self.list.select_next();
        self.refresh_selection_metadata();
    }

    /// Moves the selection to the previous workflow and updates metadata.
    pub fn select_prev(&mut self) {
        self.list.select_prev();
        self.refresh_selection_metadata();
    }

    /// Exposes the Ratatui list state for rendering.
    pub fn list_state(&mut self) -> &mut ListState {
        self.list.list_state()
    }

    /// Provides the indices for the filtered workflows in display order.
    pub fn filtered_indices(&self) -> &[usize] {
        self.list.filtered_indices()
    }

    /// Returns a workflow by its absolute index in the catalogue.
    pub fn workflow_by_index(&self, index: usize) -> Option<&RuntimeWorkflow> {
        self.list.workflow_by_index(index)
    }

    /// Returns the currently selected workflow from the filtered view.
    pub fn selected_workflow(&self) -> Option<&RuntimeWorkflow> {
        self.list.selected_workflow()
    }

    /// Computes the identifier column width for the filtered set.
    pub fn filtered_title_width(&self) -> usize {
        self.list.filtered_title_width()
    }

    /// Provides the derived metadata for the currently selected workflow, if any.
    pub fn selected_metadata(&self) -> Option<&WorkflowSelectionMetadata> {
        self.selected_metadata.as_ref()
    }

    /// Stores an active workflow run state for interaction with the collector.
    pub fn set_active_run_state(&mut self, state: WorkflowRunState) {
        self.active_run_state = Some(state);
    }

    /// Retrieves the active run state immutably.
    pub fn active_run_state(&self) -> Option<&WorkflowRunState> {
        self.active_run_state.as_ref()
    }

    /// Retrieves the active run state mutably.
    pub fn active_run_state_mut(&mut self) -> Option<&mut WorkflowRunState> {
        self.active_run_state.as_mut()
    }

    /// Returns a mutable reference to the input view state when active.
    pub fn input_view_state_mut(&mut self) -> Option<&mut WorkflowInputViewState> {
        self.input_view.as_mut()
    }

    /// Returns an immutable reference to the input view state when active.
    pub fn input_view_state(&self) -> Option<&WorkflowInputViewState> {
        self.input_view.as_ref()
    }

    /// Begins an inputs session by storing the prepared run state and initializing view state.
    pub fn begin_inputs_session(&mut self, run_state: WorkflowRunState) {
        self.active_run_state = Some(run_state);
        self.input_view = Some(WorkflowInputViewState::new());
        self.run_view = None;
        self.active_run_id = None;
        self.run_control = None;
    }

    /// Ends any active inputs session and drops the stored run state.
    pub fn end_inputs_session(&mut self) {
        self.input_view = None;
        self.active_run_state = None;
        self.run_view = None;
        self.active_run_id = None;
        self.run_control = None;
    }

    /// Hides the input view while keeping the prepared run state available.
    pub fn close_input_view(&mut self) {
        self.input_view = None;
        self.manual_entry = None;
        self.selector = None;
    }

    /// Retrieves the currently active input definition from the application's workflows.
    ///
    /// This function is used to get the definition of the input that is currently selected in the
    /// workflow's input view. It first determines the active run state of the workflow and the index
    /// of the selected input in the input view state. Using that index, it fetches the corresponding
    /// input definition from the workflow's inputs.
    ///
    /// # Arguments
    ///
    /// * `app` - A reference to the `App` struct containing workflows and their associated state.
    ///
    /// # Returns
    ///
    /// * `Option<WorkflowInputDefinition>`:
    ///     - `Some(WorkflowInputDefinition)` - The active input definition if it exists and is
    ///       accessible.
    ///     - `None` - If no active run state, input view state, or valid input at the given index is found.
    ///
    /// # Example
    ///
    /// ```
    /// let input_def = active_input_definition(&app);
    /// if let Some(def) = input_def {
    ///     println!("Active input definition: {:?}", def);
    /// } else {
    ///     println!("No active input definition available");
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// This function may return `None` if:
    /// - There's no active run state for the workflow.
    /// - The input view state is unavailable.
    /// - The selected input index does not correspond to a valid input.
    ///
    /// Note: Ownership and cloning of the input definition are handled internally to safely provide
    /// the output.
    pub fn active_input_definition(&self) -> Option<WorkflowInputDefinition> {
        let run_state = self.active_run_state()?;
        let idx = self.input_view_state()?.selected();
        run_state.workflow.inputs.get_index(idx).map(|(_, def)| def.clone())
    }

    /// Counts unresolved required inputs.
    ///
    /// This aligns with the InputStatus used by the Input Collection View:
    /// an input is considered "resolved" once it has a value present in the
    /// run context, regardless of provider argument binding states. Optional
    /// inputs that are not filled do not block readiness and are excluded.
    pub fn unresolved_item_count(&self) -> usize {
        let Some(run_state) = self.active_run_state.as_ref() else {
            return 0;
        };

        run_state
            .workflow
            .inputs
            .iter()
            .filter(|(name, def)| {
                let required = def.is_required();
                required && run_state.run_context.inputs.get(*name).is_none()
            })
            .count()
    }

    fn refresh_selection_metadata(&mut self) {
        self.selected_metadata = self.list.selected_workflow().map(WorkflowSelectionMetadata::from_runtime);
    }
}

fn describe_step_status(status: WorkflowRunStepStatus) -> &'static str {
    match status {
        WorkflowRunStepStatus::Pending => "pending",
        WorkflowRunStepStatus::Running => "running",
        WorkflowRunStepStatus::Succeeded => "succeeded",
        WorkflowRunStepStatus::Failed => "failed",
        WorkflowRunStepStatus::Skipped => "skipped",
    }
}

fn describe_run_status(status: WorkflowRunStatus) -> &'static str {
    match status {
        WorkflowRunStatus::Pending => "pending",
        WorkflowRunStatus::Running => "running",
        WorkflowRunStatus::Paused => "paused",
        WorkflowRunStatus::CancelRequested => "cancel requested",
        WorkflowRunStatus::Succeeded => "succeeded",
        WorkflowRunStatus::Failed => "failed",
        WorkflowRunStatus::Canceled => "canceled",
    }
}

#[cfg(test)]
mod workflow_run_tests {
    use super::*;
    use crate::ui::components::workflows::run::RunViewState;
    use crate::ui::components::workflows::run::state::RunExecutionStatus;
    use crate::ui::theme::dracula::DraculaTheme;
    use chrono::Utc;
    use heroku_types::workflow::{
        RuntimeWorkflow, WorkflowDefaultSource, WorkflowInputDefault, WorkflowInputDefinition, WorkflowStepDefinition,
    };
    use indexmap::IndexMap;
    use serde_json::{Value, json};

    fn sample_workflow() -> RuntimeWorkflow {
        RuntimeWorkflow {
            identifier: "wf".into(),
            title: Some("Workflow".into()),
            description: None,
            inputs: IndexMap::new(),
            steps: vec![WorkflowStepDefinition {
                id: "first".into(),
                run: "cmd".into(),
                description: Some("step".into()),
                depends_on: Vec::new(),
                r#if: None,
                with: IndexMap::new(),
                body: Value::Null,
                repeat: None,
                output_contract: None,
            }],
        }
    }

    #[test]
    fn unresolved_count_ignores_inputs_populated_by_defaults() {
        let mut inputs = IndexMap::new();

        let mut region = WorkflowInputDefinition::default();
        region.default = Some(WorkflowInputDefault {
            from: WorkflowDefaultSource::Literal,
            value: Some(json!("us")),
        });
        inputs.insert("region".into(), region);

        inputs.insert("app".into(), WorkflowInputDefinition::default());

        let workflow = RuntimeWorkflow {
            identifier: "with_defaults".into(),
            title: None,
            description: None,
            inputs,
            steps: vec![WorkflowStepDefinition {
                id: "first".into(),
                run: "cmd".into(),
                description: None,
                depends_on: Vec::new(),
                r#if: None,
                with: IndexMap::new(),
                body: Value::Null,
                repeat: None,
                output_contract: None,
            }],
        };

        let mut run_state = WorkflowRunState::new(workflow.clone());
        run_state.apply_input_defaults();

        let mut state = WorkflowState::new();
        state.begin_inputs_session(run_state);

        assert_eq!(state.unresolved_item_count(), 1);
        let stored_state = state.active_run_state().expect("active run state");
        assert_eq!(stored_state.run_context.inputs.get("region"), Some(&json!("us")));
    }

    #[test]
    fn apply_run_event_updates_status_and_logs() {
        let theme = DraculaTheme::new();
        let workflow = sample_workflow();
        let run_id = "run-1".to_string();
        let run_state = WorkflowRunState::new(workflow.clone());
        let mut view_state = RunViewState::new(run_id.clone(), workflow.identifier.clone(), workflow.title.clone());
        view_state.initialize_steps(&workflow.steps, &theme);

        let mut state = WorkflowState::new();
        state.begin_run_session(run_id.clone(), run_state, view_state);

        let logs = state.apply_run_event(&run_id, WorkflowRunEvent::RunStarted { at: Utc::now() }, &theme);
        assert!(logs.iter().any(|entry| entry.contains("started")));
        assert_eq!(state.run_view_state().unwrap().status(), RunExecutionStatus::Running);

        let logs = state.apply_run_event(
            &run_id,
            WorkflowRunEvent::RunStatusChanged {
                status: WorkflowRunStatus::CancelRequested,
                message: Some("aborting...".into()),
            },
            &theme,
        );
        assert!(logs.iter().any(|entry| entry.contains("aborting")));
        let view = state.run_view_state().unwrap();
        assert_eq!(view.status(), RunExecutionStatus::CancelRequested);
        assert_eq!(view.status_message(), Some("aborting..."));

        let logs = state.apply_run_event(
            &run_id,
            WorkflowRunEvent::StepFinished {
                step_id: "first".into(),
                status: WorkflowRunStepStatus::Succeeded,
                output: json!({"ok": true}),
                logs: vec!["done".into()],
                attempts: 1,
                duration_ms: 20,
            },
            &theme,
        );
        assert!(logs.iter().any(|entry| entry.contains("Step 'first'")));
        let row = state.run_view_state().unwrap().steps_table().selected_data().cloned().expect("row");
        assert_eq!(row["Status"], Value::String("succeeded".into()));
    }
}

impl HasFocus for WorkflowState {
    fn build(&self, builder: &mut FocusBuilder) {
        if let Some(run_view) = &self.run_view {
            run_view.build(builder);
        } else if let Some(view) = &self.input_view {
            view.build(builder);
        } else {
            let tag = builder.start(self);
            builder.leaf_widget(&self.f_search);
            builder.widget(&self.list);
            builder.end(tag);
        }
    }

    fn focus(&self) -> FocusFlag {
        if let Some(run_view) = &self.run_view {
            run_view.focus()
        } else if let Some(view) = &self.input_view {
            view.focus()
        } else {
            self.container_focus.clone()
        }
    }

    fn area(&self) -> Rect {
        if let Some(run_view) = &self.run_view {
            run_view.area()
        } else if let Some(view) = &self.input_view {
            view.area()
        } else {
            self.list.area()
        }
    }
}

/// Derived metadata describing the currently selected workflow for quick access in the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowSelectionMetadata {
    /// Canonical workflow identifier.
    pub identifier: String,
    /// Optional human-friendly title supplied by authors.
    pub title: Option<String>,
    /// Optional description summarizing workflow responsibilities.
    pub description: Option<String>,
    /// Number of declared inputs in the workflow definition.
    pub input_count: usize,
    /// Number of steps that will execute when the workflow runs.
    pub step_count: usize,
}

fn provider_identifier(definition: &WorkflowInputDefinition) -> Option<String> {
    definition.provider.as_ref().map(|provider| match provider {
        WorkflowValueProvider::Id(id) => id.clone(),
        WorkflowValueProvider::Detailed(detail) => detail.id.clone(),
    })
}

impl WorkflowSelectionMetadata {
    fn from_runtime(workflow: &RuntimeWorkflow) -> Self {
        Self {
            identifier: workflow.identifier.clone(),
            title: workflow.title.clone(),
            description: workflow.description.clone(),
            input_count: workflow.inputs.len(),
            step_count: workflow.steps.len(),
        }
    }
}

/// Enriched schema metadata used to annotate selector rows and detail entries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkflowSelectorFieldMetadata {
    /// JSON type hint reported by the upstream schema.
    pub json_type: Option<String>,
    /// Semantic tags associated with the field (for example, `app_id`).
    pub tags: Vec<String>,
    /// Enumerated literals declared for the field, when available.
    pub enum_values: Vec<String>,
    /// Indicates whether the field is required for the provider payload.
    pub required: bool,
}

/// A staged workflow selector choice that is ready to be applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowSelectorStagedSelection {
    /// JSON value that will be written into the workflow input slot.
    pub value: Value,
    /// Human-readable representation of the value for status messaging.
    pub display_value: String,
    /// Source field used to extract the value from the provider row.
    pub source_field: Option<String>,
    /// Full JSON row returned by the provider.
    pub row: Value,
}

impl WorkflowSelectorStagedSelection {
    /// Constructs a new staged selection with the provided value metadata.
    pub fn new(value: Value, display_value: String, source_field: Option<String>, row: Value) -> Self {
        Self {
            value,
            display_value,
            source_field,
            row,
        }
    }
}

/// Focus targets available within the workflow selector modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowCollectorFocus {
    /// Provider results table focus, arrow navigation enabled.
    Table,
    /// Inline filter input focus, text editing enabled.
    Filter,
    /// Confirmation buttons focus, Enter and arrow keys interact with buttons.
    Buttons(SelectorButtonFocus),
}

impl Default for WorkflowCollectorFocus {
    fn default() -> Self {
        Self::Table
    }
}

/// Identifies which selector confirmation button currently holds focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorButtonFocus {
    /// Cancel button.
    Cancel,
    /// Apply button.
    Apply,
}

/// Retained layout metadata capturing screen regions for pointer hit-testing.
#[derive(Debug, Clone, Default)]
pub struct WorkflowSelectorLayoutState {
    /// Rect covering the overall selector content area.
    pub container_area: Option<Rect>,
    /// Rect covering the filter input at the top of the modal.
    pub filter_area: Option<Rect>,
    /// Rect covering the results table.
    pub table_area: Option<Rect>,
    /// Rect covering the detail pane.
    pub detail_area: Option<Rect>,
    /// Rect covering the footer (buttons + hints).
    pub footer_area: Option<Rect>,
    /// Rect for the Cancel button inside the footer.
    pub cancel_button_area: Option<Rect>,
    /// Rect for the Apply button inside the footer.
    pub apply_button_area: Option<Rect>,
}

#[derive(Debug)]
pub struct WorkflowSelectorViewState<'a> {
    /// Canonical provider identifier (e.g., "apps list").
    pub provider_id: String,
    /// Arguments resolved for the provider (from prior inputs/steps).
    pub resolved_args: serde_json::Map<String, serde_json::Value>,
    /// Backing table state used for rendering results.
    pub table: TableState<'a>,
    /// Optional value_field from `select` used to extract the workflow value.
    pub value_field: Option<String>,
    /// Optional display_field used as primary for filtering.
    pub display_field: Option<String>,
    /// Provider on_error policy to drive fallback behavior.
    pub on_error: Option<WorkflowProviderErrorPolicy>,
    /// Current status label ("loading…", "loaded", or error text).
    pub status: SelectorStatus,
    /// Optional error message to surface inline.
    pub error_message: Option<String>,
    /// Original unfiltered provider items (array of rows).
    pub original_items: Option<Vec<serde_json::Value>>,
    /// Cache key currently awaiting asynchronous fetch completion.
    pub pending_cache_key: Option<String>,
    /// Lightweight inline filter buffer.
    pub filter: TextInputState,
    /// Current focus target within the selector modal.
    pub focus: WorkflowCollectorFocus,
    /// Cached metadata derived from the provider schema.
    pub field_metadata: IndexMap<String, WorkflowSelectorFieldMetadata>,
    /// Currently staged selection awaiting confirmation.
    pub staged_selection: Option<WorkflowSelectorStagedSelection>,
    /// Layout metadata from the most recent render pass.
    pub layout: WorkflowSelectorLayoutState,
}

impl<'a> WorkflowSelectorViewState<'a> {
    pub fn set_items(&mut self, items: Vec<serde_json::Value>) {
        self.original_items = Some(items);
        self.status = SelectorStatus::Loaded;
        self.error_message = None;
        self.pending_cache_key = None;
        self.clear_staged_selection();
    }

    pub fn set_error(&mut self, message: String) {
        self.status = SelectorStatus::Error;
        self.error_message = Some(message);
        self.pending_cache_key = None;
        self.clear_staged_selection();
    }

    pub fn refresh_table(&mut self, theme: &dyn Theme) {
        let Some(items) = self.original_items.clone() else {
            return;
        };
        let query = self.filter.input().trim().to_lowercase();
        let dataset: Vec<Value> = if query.is_empty() {
            items
        } else {
            items
                .into_iter()
                .filter(|item| match item {
                    Value::Object(map) => {
                        if let Some(display_field) = self.display_field.as_deref() {
                            if let Some(value) = map.get(display_field) {
                                if let Some(text) = value.as_str() {
                                    return text.to_lowercase().starts_with(&query);
                                }
                            }
                        }
                        map.values()
                            .any(|value| value.as_str().map(|text| text.to_lowercase().contains(&query)).unwrap_or(false))
                    }
                    Value::String(text) => text.to_lowercase().contains(&query),
                    _ => false,
                })
                .collect()
        };
        let json = Value::Array(dataset);
        self.table.apply_result_json(Some(json), theme);
        self.table.normalize();
        self.clear_staged_selection();
    }

    pub fn clear_staged_selection(&mut self) {
        self.staged_selection = None;
    }

    pub fn set_staged_selection(&mut self, selection: Option<WorkflowSelectorStagedSelection>) {
        self.staged_selection = selection;
    }

    pub fn staged_selection(&self) -> Option<&WorkflowSelectorStagedSelection> {
        self.staged_selection.as_ref()
    }

    pub fn take_staged_selection(&mut self) -> Option<WorkflowSelectorStagedSelection> {
        self.staged_selection.take()
    }

    pub fn focus_filter(&mut self) {
        self.focus = WorkflowCollectorFocus::Filter;
        self.filter.set_cursor(self.filter.input().len());
    }

    pub fn focus_table(&mut self) {
        self.focus = WorkflowCollectorFocus::Table;
    }

    pub fn focus_buttons(&mut self, button: SelectorButtonFocus) {
        self.focus = WorkflowCollectorFocus::Buttons(button);
    }

    pub fn is_filter_focused(&self) -> bool {
        matches!(self.focus, WorkflowCollectorFocus::Filter)
    }

    pub fn apply_enabled(&self) -> bool {
        self.staged_selection.is_some()
    }

    pub fn set_layout(&mut self, layout: WorkflowSelectorLayoutState) {
        self.layout = layout;
    }

    pub fn layout(&self) -> &WorkflowSelectorLayoutState {
        &self.layout
    }

    pub fn button_focus(&self) -> SelectorButtonFocus {
        match self.focus {
            WorkflowCollectorFocus::Buttons(button) => button,
            _ => SelectorButtonFocus::Apply,
        }
    }

    pub fn next_focus(&mut self) {
        match self.focus {
            WorkflowCollectorFocus::Table => self.focus_filter(),
            WorkflowCollectorFocus::Filter => self.focus_buttons(SelectorButtonFocus::Cancel),
            WorkflowCollectorFocus::Buttons(SelectorButtonFocus::Cancel) => self.focus_buttons(SelectorButtonFocus::Apply),
            WorkflowCollectorFocus::Buttons(SelectorButtonFocus::Apply) => self.focus_table(),
        }
    }

    pub fn prev_focus(&mut self) {
        match self.focus {
            WorkflowCollectorFocus::Table => self.focus_buttons(SelectorButtonFocus::Apply),
            WorkflowCollectorFocus::Filter => self.focus_table(),
            WorkflowCollectorFocus::Buttons(SelectorButtonFocus::Cancel) => self.focus_filter(),
            WorkflowCollectorFocus::Buttons(SelectorButtonFocus::Apply) => self.focus_buttons(SelectorButtonFocus::Cancel),
        }
    }

    pub fn sync_stage_with_selection(&mut self) {
        let Some(staged) = self.staged_selection.as_ref() else {
            return;
        };
        let Some(current_row) = self.table.selected_data() else {
            self.clear_staged_selection();
            return;
        };
        if staged.row != *current_row {
            self.clear_staged_selection();
        }
    }
}

/// Loading status for the selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorStatus {
    Loading,
    Loaded,
    Error,
}

impl WorkflowState {
    /// Opens the Manual Entry modal for the currently selected input.
    pub fn open_manual_for_active_input(&mut self) {
        let Some(run_state) = self.active_run_state() else {
            return;
        };
        let Some(view) = self.input_view_state() else {
            return;
        };
        let idx = view.selected();
        let Some((name, def)) = run_state.workflow.inputs.get_index(idx) else {
            return;
        };
        let existing_value = run_state.run_context.inputs.get(name);
        let state = ManualEntryState::from_definition(def, name, existing_value);
        self.manual_entry = Some(state);
    }

    /// Returns an immutable reference to the manual entry state when present.
    pub fn manual_entry_state(&self) -> Option<&ManualEntryState> {
        self.manual_entry.as_ref()
    }

    /// Returns a mutable reference to the manual entry state when present.
    pub fn manual_entry_state_mut(&mut self) -> Option<&mut ManualEntryState> {
        self.manual_entry.as_mut()
    }

    /// Begins a run session by persisting the run identifier, engine state, and view state.
    pub fn begin_run_session(&mut self, run_id: String, run_state: WorkflowRunState, run_view: RunViewState) {
        self.active_run_state = Some(run_state);
        self.run_view = Some(run_view);
        self.active_run_id = Some(run_id);
        self.run_control = None;
    }

    /// Stores a freshly prepared run view state.
    pub fn begin_run_view(&mut self, state: RunViewState) {
        self.run_view = Some(state);
    }

    /// Returns the active run view state, if present.
    pub fn run_view_state(&self) -> Option<&RunViewState> {
        self.run_view.as_ref()
    }

    /// Returns the active run view state mutably, if present.
    pub fn run_view_state_mut(&mut self) -> Option<&mut RunViewState> {
        self.run_view.as_mut()
    }

    /// Clears the active run view state.
    pub fn close_run_view(&mut self) {
        self.run_view = None;
    }

    /// Registers the control channel for the currently active run.
    pub fn register_run_control(&mut self, run_id: &str, sender: UnboundedSender<WorkflowRunControl>) {
        if self.active_run_id.as_deref() == Some(run_id) {
            self.run_control = Some(WorkflowRunControlHandle {
                run_id: run_id.to_string(),
                sender,
            });
        }
    }

    /// Returns the control sender for the specified run, if available.
    pub fn run_control_sender(&self, run_id: &str) -> Option<&UnboundedSender<WorkflowRunControl>> {
        if self.active_run_id.as_deref() == Some(run_id) {
            self.run_control.as_ref().map(|handle| &handle.sender)
        } else {
            None
        }
    }

    /// Applies a workflow run event to the state, returning log messages to surface.
    pub fn apply_run_event(&mut self, run_id: &str, event: WorkflowRunEvent, theme: &dyn Theme) -> Vec<String> {
        if self.active_run_id.as_deref() != Some(run_id) {
            return Vec::new();
        }

        let mut log_messages = Vec::new();
        let Some(run_view) = self.run_view.as_mut() else {
            return Vec::new();
        };

        match event {
            WorkflowRunEvent::RunStarted { at } => {
                run_view.handle_run_started(at);
                log_messages.push(format!("Workflow run '{}' started.", run_id));
            }
            WorkflowRunEvent::RunStatusChanged { status, message } => {
                run_view.apply_status_change(status, message.clone());
                if let Some(text) = message {
                    log_messages.push(text);
                }
            }
            WorkflowRunEvent::StepStarted { index, .. } => {
                run_view.mark_step_running(index, theme);
            }
            WorkflowRunEvent::StepFinished {
                step_id,
                status,
                output,
                logs,
                attempts,
                duration_ms,
            } => {
                if let Some(state) = self.active_run_state.as_mut() {
                    state.run_context.steps.insert(step_id.clone(), output.clone());
                }
                run_view.mark_step_finished(&step_id, status, attempts, duration_ms, output, logs.clone(), theme);
                log_messages.push(format!("Step '{}' {}", step_id, describe_step_status(status)));
                if status == WorkflowRunStepStatus::Failed {
                    self.run_control = None;
                }
            }
            WorkflowRunEvent::RunOutputAccumulated { key, value, detail } => {
                run_view.append_output(&key, value, detail, theme);
            }
            WorkflowRunEvent::RunCompleted {
                status,
                finished_at,
                error,
            } => {
                run_view.handle_run_completed(status, finished_at, error.clone());
                self.run_control = None;
                if let Some(text) = error {
                    log_messages.push(text);
                } else {
                    log_messages.push(format!("Workflow run '{}' completed with {}.", run_id, describe_run_status(status)));
                }
            }
            WorkflowRunEvent::StepOutputProduced { .. } => {
                // Future enhancement: stream intermediate outputs into the detail view.
            }
        }

        log_messages
    }

    /// Returns the currently active input name, if any.
    pub fn active_input_name(&self) -> Option<String> {
        let run_state = self.active_run_state()?;
        let idx = self.input_view_state()?.selected();
        run_state.workflow.inputs.get_index(idx).map(|(k, _)| k.clone())
    }

    /// Initializes the provider-backed selector for the currently active input.
    pub fn open_selector_for_active_input(&mut self, registry: &Arc<Mutex<CommandRegistry>>) {
        let Some(run_state) = self.active_run_state() else {
            return;
        };
        let Some(view) = self.input_view_state() else {
            return;
        };
        let idx = view.selected();
        let Some((name, def)) = run_state.workflow.inputs.get_index(idx) else {
            return;
        };

        let provider_id = match provider_identifier(def) {
            Some(id) => id,
            None => return,
        };
        let field_metadata = resolve_selector_field_metadata(registry, &provider_id);

        // Collect resolved provider args from the binding outcomes.
        let mut args = serde_json::Map::new();
        if let Some(pstate) = run_state.provider_state_for(name) {
            for (arg, outcome) in &pstate.argument_outcomes {
                if let ProviderBindingOutcome::Resolved(value) = &outcome.outcome {
                    args.insert(arg.clone(), value.clone());
                }
            }
        }

        let value_field = def.select.as_ref().and_then(|s| s.value_field.clone());

        let table: TableState<'static> = Default::default();

        self.selector = Some(WorkflowSelectorViewState {
            provider_id,
            resolved_args: args,
            table,
            value_field,
            display_field: def.select.as_ref().and_then(|s| s.display_field.clone()),
            on_error: def.on_error.clone(),
            status: SelectorStatus::Loading,
            error_message: None,
            original_items: None,
            filter: TextInputState::new(),
            pending_cache_key: None,
            focus: WorkflowCollectorFocus::Table,
            field_metadata,
            staged_selection: None,
            layout: WorkflowSelectorLayoutState::default(),
        });
    }

    /// Returns the selector state if present.
    pub fn selector_state(&self) -> Option<&WorkflowSelectorViewState<'static>> {
        self.selector.as_ref()
    }
    /// Returns a mutable selector state if present.
    pub fn selector_state_mut(&mut self) -> Option<&mut WorkflowSelectorViewState<'static>> {
        self.selector.as_mut()
    }
}

fn resolve_selector_field_metadata(
    registry: &Arc<Mutex<CommandRegistry>>,
    provider_id: &str,
) -> IndexMap<String, WorkflowSelectorFieldMetadata> {
    let Some((group, name)) = split_provider_identifier(provider_id) else {
        return IndexMap::new();
    };

    let spec = {
        let Ok(guard) = registry.lock() else {
            return IndexMap::new();
        };
        find_by_group_and_cmd(&guard.commands, &group, &name).ok()
    };

    let Some(spec) = spec else {
        return IndexMap::new();
    };

    spec.http()
        .and_then(|http| http.output_schema.as_ref())
        .map(build_field_metadata_from_schema)
        .unwrap_or_default()
}

fn split_provider_identifier(provider_id: &str) -> Option<(String, String)> {
    let (group, name) = provider_id.split_once(char::is_whitespace)?;
    let group = group.trim();
    let name = name.trim();
    if group.is_empty() || name.is_empty() {
        return None;
    }
    Some((group.to_string(), name.to_string()))
}

fn build_field_metadata_from_schema(schema: &SchemaProperty) -> IndexMap<String, WorkflowSelectorFieldMetadata> {
    match schema.r#type.as_str() {
        "object" => collect_object_field_metadata(schema),
        "array" => schema.items.as_deref().map(build_field_metadata_from_schema).unwrap_or_default(),
        _ => IndexMap::new(),
    }
}

fn collect_object_field_metadata(schema: &SchemaProperty) -> IndexMap<String, WorkflowSelectorFieldMetadata> {
    let mut metadata = IndexMap::new();
    let Some(properties) = &schema.properties else {
        return metadata;
    };

    let mut keys: Vec<_> = properties.keys().cloned().collect();
    keys.sort();

    for key in keys {
        let Some(property) = properties.get(&key) else {
            continue;
        };
        let property = property.as_ref();
        metadata.insert(
            key.clone(),
            WorkflowSelectorFieldMetadata {
                json_type: sanitize_schema_type(&property.r#type),
                tags: property.tags.clone(),
                enum_values: property.enum_values.clone(),
                required: schema.required.iter().any(|required| required == &key),
            },
        );
    }

    metadata
}

fn sanitize_schema_type(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}
