use crate::ui::components::common::TextInputState;
use crate::ui::components::workflows::input::WorkflowInputViewState;
use anyhow::{anyhow, Result};
use heroku_engine::workflow::document::build_runtime_catalog;
use heroku_engine::{BindingSource, ProviderBindingOutcome, WorkflowRunState};
use heroku_registry::{feat_gate::feature_workflows, Registry};
use heroku_types::workflow::{RuntimeWorkflow, WorkflowInputDefinition, WorkflowValueProvider};
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::{layout::Rect, widgets::ListState};
use serde_json::Value as JsonValue;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

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
    /// The list is populated once and subsequent calls are inexpensive no-ops.
    pub fn ensure_loaded(&mut self, registry: &Arc<Mutex<Registry>>) -> Result<()> {
        if !feature_workflows() {
            self.workflows.clear();
            self.filtered_indices.clear();
            self.search_input.set_input("");
            self.search_input.set_cursor(0);
            self.list_state.select(None);
            return Ok(());
        }

        if self.workflows.is_empty() {
            let definitions = registry.lock().map_err(|_| anyhow!("could not lock registry"))?.workflows.clone();

            let catalog = build_runtime_catalog(&definitions)?;
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
    collector_visible: bool,
    active_run_state: Option<WorkflowRunState>,
    selected_metadata: Option<WorkflowSelectionMetadata>,
    provider_cache: WorkflowProviderCache,
    input_view: Option<WorkflowInputViewState>,
    field_picker_target: Option<WorkflowBindingTarget>,
    /// The focus flags for the workflow view.
    pub container_focus: FocusFlag,
    pub f_search: FocusFlag,
}

impl WorkflowState {
    /// Creates a new workflow view state with the default list configuration.
    pub fn new() -> Self {
        Self {
            list: WorkflowListState::new(),
            collector_visible: false,
            active_run_state: None,
            selected_metadata: None,
            provider_cache: WorkflowProviderCache::default(),
            input_view: None,
            field_picker_target: None,
            container_focus: FocusFlag::named("workflow.container"),
            f_search: FocusFlag::named("workflow.search"),
        }
    }

    /// Lazily loads workflows from the registry and refreshes derived metadata.
    pub fn ensure_loaded(&mut self, registry: &Arc<Mutex<Registry>>) -> Result<()> {
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

    /// Marks the guided input collector as visible or hidden.
    ///
    /// When the collector closes, provider cache snapshots are cleared so fresh
    /// data is fetched on the next run.
    pub fn set_collector_visible(&mut self, visible: bool) {
        self.collector_visible = visible;
        if !visible {
            self.provider_cache.clear();
            self.field_picker_target = None;
        }
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

    /// Consumes and returns the active run state, clearing it from memory.
    pub fn take_run_state(&mut self) -> Option<WorkflowRunState> {
        self.field_picker_target = None;
        self.active_run_state.take()
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
        run_state
            .workflow
            .inputs
            .get_index(idx)
            .map(|(_, def)| def.clone())
    }


    /// Records a provider cache snapshot associated with the current workflow selection or active run.
    ///
    /// This helper is reserved for future workflow UX iterations that capture cache metadata during manual
    /// provider refresh flows.
    pub fn record_provider_snapshot(&mut self, provider_key: impl Into<String>, item_count: Option<usize>, ttl: Option<Duration>) {
        let key: String = provider_key.into();

        let workflow_identifier = self.selected_metadata().map(|metadata| metadata.identifier.clone()).or_else(|| {
            self.active_run_state
                .as_ref()
                .map(|run_state| run_state.workflow.identifier.clone())
        });

        if let Some(workflow_identifier) = workflow_identifier {
            self.record_provider_snapshot_for(&workflow_identifier, key, item_count, ttl);
        }
    }

    /// Returns cached provider information for the current workflow.
    pub fn provider_snapshot(&self, provider_key: &str) -> Option<&WorkflowProviderSnapshot> {
        if let Some(metadata) = self.selected_metadata()
            && let Some(snapshot) = self.provider_snapshot_for(&metadata.identifier, provider_key)
        {
            return Some(snapshot);
        }

        if let Some(run_state) = &self.active_run_state {
            return self.provider_snapshot_for(&run_state.workflow.identifier, provider_key);
        }

        None
    }

    /// Records provider refresh times and TTL metadata after evaluation.
    pub fn observe_provider_refresh(&mut self, run_state: &WorkflowRunState) {
        let workflow_identifier = run_state.workflow.identifier.clone();
        let updates = collect_provider_updates(run_state);
        for update in updates {
            self.record_provider_snapshot_for(&workflow_identifier, update.key, update.item_count, update.ttl);
        }
    }

    /// Convenience wrapper that records refresh metadata using the active run state, if present.
    pub fn observe_provider_refresh_current(&mut self) {
        let Some(run_state) = self.active_run_state.as_ref() else {
            return;
        };
        let workflow_identifier = run_state.workflow.identifier.clone();
        let updates = collect_provider_updates(run_state);
        for update in updates {
            self.record_provider_snapshot_for(&workflow_identifier, update.key, update.item_count, update.ttl);
        }
    }

    fn record_provider_snapshot_for(
        &mut self,
        workflow_identifier: &str,
        provider_key: impl Into<String>,
        item_count: Option<usize>,
        ttl: Option<Duration>,
    ) {
        let key = format!("{}::{}", workflow_identifier, provider_key.into());
        self.provider_cache.record_snapshot(key, item_count, ttl, Instant::now());
    }

    /// Stores binding metadata used when the field picker is launched.
    pub fn set_field_picker_target(&mut self, target: WorkflowBindingTarget) {
        self.field_picker_target = Some(target);
    }

    /// Returns the currently staged field picker target, if any.
    pub fn field_picker_target(&self) -> Option<&WorkflowBindingTarget> {
        self.field_picker_target.as_ref()
    }

    /// Clears any pending field picker target.
    pub fn clear_field_picker_target(&mut self) {
        self.field_picker_target = None;
    }

    /// Applies a concrete value to the run state for the given binding target and re-evaluates providers.
    pub fn apply_binding_value(&mut self, target: &WorkflowBindingTarget, value: JsonValue) -> Result<()> {
        let run_state = self
            .active_run_state
            .as_mut()
            .ok_or_else(|| anyhow!("no active workflow run state available"))?;
        apply_binding_value_to_state(run_state, target, value)?;
        run_state.evaluate_input_providers()?;
        self.observe_provider_refresh_current();
        Ok(())
    }

    /// Applies a value captured via the field picker using the staged target.
    pub fn apply_field_picker_value(&mut self, value: JsonValue) -> Result<()> {
        let target = self
            .field_picker_target
            .clone()
            .ok_or_else(|| anyhow!("field picker target not set"))?;
        self.apply_binding_value(&target, value)
    }

    /// Counts unresolved provider prompts and errors that still need input.
    pub fn unresolved_item_count(&self) -> usize {
        let Some(run_state) = self.active_run_state.as_ref() else {
            return 0;
        };

        let mut count = 0;
        for (input_name, _) in run_state.workflow.inputs.iter() {
            if let Some(provider_state) = run_state.provider_state_for(input_name) {
                for outcome_state in provider_state.argument_outcomes.values() {
                    match &outcome_state.outcome {
                        ProviderBindingOutcome::Prompt(_) | ProviderBindingOutcome::Error(_) | ProviderBindingOutcome::Skip(_) => {
                            count += 1;
                        }
                        ProviderBindingOutcome::Resolved(_) => {}
                    }
                }
            }
        }
        count
    }

    fn provider_snapshot_for(&self, workflow_identifier: &str, provider_key: &str) -> Option<&WorkflowProviderSnapshot> {
        let key = format!("{}::{provider_key}", workflow_identifier);
        self.provider_cache.snapshot(&key)
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

fn collect_provider_updates(run_state: &WorkflowRunState) -> Vec<ProviderSnapshotUpdate> {
    run_state
        .workflow
        .inputs
        .iter()
        .filter_map(|(input_name, definition)| {
            provider_identifier(definition).map(|provider_id| {
                let provider_key = format!("{input_name}:{provider_id}");
                let ttl = definition.cache_ttl_sec.map(Duration::from_secs);
                let item_count = run_state
                    .provider_state_for(input_name)
                    .map(|state| {
                        state
                            .argument_outcomes
                            .values()
                            .filter(|outcome| matches!(outcome.outcome, ProviderBindingOutcome::Resolved(_)))
                            .count()
                    })
                    .filter(|count| *count > 0);

                ProviderSnapshotUpdate {
                    key: provider_key,
                    ttl,
                    item_count,
                }
            })
        })
        .collect()
}

struct ProviderSnapshotUpdate {
    key: String,
    ttl: Option<Duration>,
    item_count: Option<usize>,
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

/// Tracks cached provider data associated with workflows for TTL-aware refresh hints.
#[derive(Debug, Default)]
pub struct WorkflowProviderCache {
    entries: HashMap<String, WorkflowProviderSnapshot>,
}

impl WorkflowProviderCache {
    /// Records or updates a provider snapshot keyed by workflow + provider identifier.
    pub fn record_snapshot(&mut self, key: impl Into<String>, item_count: Option<usize>, ttl: Option<Duration>, captured_at: Instant) {
        self.entries.insert(
            key.into(),
            WorkflowProviderSnapshot {
                last_refreshed: captured_at,
                ttl,
                item_count,
            },
        );
    }

    /// Returns a snapshot for the given key if one has been cached.
    pub fn snapshot(&self, key: &str) -> Option<&WorkflowProviderSnapshot> {
        self.entries.get(key)
    }

    /// Removes all cached provider snapshots.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Captures metadata about a provider result, such as last refresh time and cached item count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowProviderSnapshot {
    /// Timestamp of the last provider refresh.
    pub last_refreshed: Instant,
    /// Optional refresh interval advertised by the provider.
    pub ttl: Option<Duration>,
    /// Optional number of items returned on the last fetch.
    pub item_count: Option<usize>,
}

/// Describes a binding target that the collector or field picker is attempting to satisfy.
#[derive(Debug, Clone)]
pub struct WorkflowBindingTarget {
    pub input: String,
    pub argument: String,
    pub source: Option<BindingSource>,
    pub required: bool,
}

fn apply_binding_value_to_state(state: &mut WorkflowRunState, target: &WorkflowBindingTarget, value: JsonValue) -> Result<()> {
    let value_for_state = value.clone();

    match &target.source {
        Some(BindingSource::Input { input_name }) => {
            state.set_input_value(input_name, value_for_state.clone());
        }
        Some(BindingSource::Step { step_id }) => {
            state.run_context_mut().steps.insert(step_id.clone(), value_for_state.clone());
        }
        Some(BindingSource::Multiple { step_id, input_name }) => {
            state.set_input_value(input_name, value_for_state.clone());
            state.run_context_mut().steps.insert(step_id.clone(), value_for_state.clone());
        }
        None => {}
    }

    state.set_input_value(&target.input, value_for_state.clone());
    state.persist_provider_outcome(&target.input, &target.argument, ProviderBindingOutcome::Resolved(value_for_state));
    Ok(())
}
