use crate::ui::components::common::TextInputState;
use crate::ui::components::workflows::input::WorkflowInputViewState;
use anyhow::{Result, anyhow};
use heroku_engine::workflow::document::build_runtime_catalog;
use heroku_engine::{ProviderBindingOutcome, WorkflowRunState};
use heroku_registry::{CommandRegistry, feat_gate::feature_workflows};
use heroku_types::workflow::{RuntimeWorkflow, WorkflowInputDefinition, WorkflowValueProvider};
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::{layout::Rect, widgets::ListState};
use std::sync::{Arc, Mutex};
use heroku_types::{WorkflowInputValidation, WorkflowProviderErrorPolicy};
use crate::ui::components::table::TableState;

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
    pub fn filtered_identifier_width(&self) -> usize {
        self.filtered_indices
            .iter()
            .filter_map(|index| self.workflows.get(*index))
            .map(|workflow| workflow.identifier.len())
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

/// Aggregates workflow list state with execution metadata, modal visibility, and provider cache snapshots.
#[derive(Debug, Default)]
pub struct WorkflowState {
    pub list: WorkflowListState,
    active_run_state: Option<WorkflowRunState>,
    selected_metadata: Option<WorkflowSelectionMetadata>,
    input_view: Option<WorkflowInputViewState>,
    /// Manual entry modal state (when open); None when not editing.
    pub manual_entry: Option<ManualEntryViewState>,
    /// Provider-backed selector state (when open); None when not selecting.
    pub selector: Option<WorkflowSelectorViewState<'static>>,
    /// The focus flags for the workflow view.
    pub container_focus: FocusFlag,
    pub f_search: FocusFlag,
}

impl WorkflowState {
    /// Creates a new workflow view state with the default list configuration.
    pub fn new() -> Self {
        Self {
            list: WorkflowListState::new(),
            active_run_state: None,
            selected_metadata: None,
            input_view: None,
            manual_entry: None,
            selector: None,
            container_focus: FocusFlag::named("workflow.container"),
            f_search: FocusFlag::named("workflow.search"),
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
    pub fn filtered_identifier_width(&self) -> usize {
        self.list.filtered_identifier_width()
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
    }

    /// Ends any active inputs session and drops the stored run state.
    pub fn end_inputs_session(&mut self) {
        self.input_view = None;
        self.active_run_state = None;
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

impl HasFocus for WorkflowState {
    fn build(&self, builder: &mut FocusBuilder) {
        if let Some(view) = &self.input_view {
            view.build(builder);
        } else {
            let tag = builder.start(self);
            builder.leaf_widget(&self.f_search);
            builder.widget(&self.list);
            builder.end(tag);
        }
    }

    fn focus(&self) -> FocusFlag {
        if let Some(view) = &self.input_view {
            view.focus()
        } else {
            self.container_focus.clone()
        }
    }

    fn area(&self) -> Rect {
        if let Some(view) = &self.input_view {
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

// ---- Manual entry modal state ----
#[derive(Debug, Clone, Default)]
pub struct ManualEntryViewState {
    /// Label for the value being entered (input name)
    pub label: String,
    /// UTF-8 safe text input buffer and cursor
    pub text: TextInputState,
    /// Latest validation error shown inline
    pub error: Option<String>,
    /// Optional validation rules from the workflow definition
    pub validation: Option<WorkflowInputValidation>,
    /// Optional placeholder shown when empty
    pub placeholder: Option<String>,
}

/// Provider-backed selector state rendered in the Workflow Collector modal.
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
    /// Lightweight inline filter buffer.
    pub filter: TextInputState,
    /// Whether the filter input is currently focused/active.
    pub filter_active: bool,
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
        let mut state = ManualEntryViewState::default();
        state.label = name.to_string();
        state.validation = def.validate.clone();
        state.placeholder = def.placeholder.clone();
        if let Some(existing) = run_state
            .run_context
            .inputs
            .get(name)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
        {
            state.text.set_input(existing);
            // place cursor at end
            state.text.set_cursor(state.text.input().len());
        }
        self.manual_entry = Some(state);
    }

    /// Returns an immutable reference to the manual entry state when present.
    pub fn manual_entry_state(&self) -> Option<&ManualEntryViewState> {
        self.manual_entry.as_ref()
    }

    /// Returns a mutable reference to the manual entry state when present.
    pub fn manual_entry_state_mut(&mut self) -> Option<&mut ManualEntryViewState> {
        self.manual_entry.as_mut()
    }

    /// Returns the currently active input name, if any.
    pub fn active_input_name(&self) -> Option<String> {
        let run_state = self.active_run_state()?;
        let idx = self.input_view_state()?.selected();
        run_state.workflow.inputs.get_index(idx).map(|(k, _)| k.clone())
    }

    /// Initializes the Provider-backed selector for the currently active input.
    pub fn open_selector_for_active_input(&mut self) {
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
            filter_active: false,
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
