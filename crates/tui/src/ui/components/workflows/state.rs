use crate::ui::components::common::TextInputState;
use crate::ui::components::common::manual_entry_modal::state::ManualEntryState;
use crate::ui::components::table::ResultsTableState;
use crate::ui::components::workflows::collector::{CollectorViewState, SelectorStatus, WorkflowSelectorFieldMetadata};
use crate::ui::components::workflows::input::WorkflowInputViewState;
use crate::ui::components::workflows::list::WorkflowListState;
use crate::ui::components::workflows::run::{RunViewState, StepFinishedData, WorkflowRunControlHandle};
use crate::ui::theme::Theme;
use anyhow::Result;
use indexmap::IndexMap;
use oatty_engine::{ProviderBindingOutcome, WorkflowRunState};
use oatty_registry::CommandRegistry;
use oatty_types::{
    command::SchemaProperty,
    workflow::{
        RuntimeWorkflow, WorkflowInputDefinition, WorkflowRunControl, WorkflowRunEvent, WorkflowRunStatus, WorkflowRunStepStatus,
        WorkflowValueProvider,
    },
};
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use ratatui::widgets::ListState;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;

/// Aggregates workflow list state with execution metadata, modal visibility, and provider cache snapshots.
#[derive(Debug, Default)]
pub struct WorkflowState {
    pub list: WorkflowListState,
    /// Manual entry modal state (when open); None when not editing.
    pub manual_entry: Option<ManualEntryState>,
    /// Provider-backed selector state (when open); None when not selecting.
    pub collector: Option<CollectorViewState<'static>>,
    /// The focus flags for the workflow view.
    pub container_focus: FocusFlag,
    pub f_search: FocusFlag,
    pub active_run_state: Option<Rc<RefCell<WorkflowRunState>>>,
    input_view: Option<WorkflowInputViewState>,
    run_view: Option<RunViewState>,
    active_run_id: Option<String>,
    run_control: Option<WorkflowRunControlHandle>,
}

impl WorkflowState {
    /// Creates a new workflow view state with the default list configuration.
    pub fn new() -> Self {
        Self {
            list: WorkflowListState::new(),
            active_run_state: None,
            input_view: None,
            run_view: None,
            manual_entry: None,
            collector: None,
            container_focus: FocusFlag::new().with_name("workflow.container"),
            f_search: FocusFlag::new().with_name("workflow.search"),
            active_run_id: None,
            run_control: None,
        }
    }

    /// Lazily loads workflows from the registry and refreshes derived metadata.
    pub fn ensure_loaded(&mut self, registry: &Arc<Mutex<CommandRegistry>>) -> Result<()> {
        self.list.ensure_loaded(registry)?;
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

    /// Provides the search cursor position in display columns (character count).
    pub fn search_cursor_columns(&self) -> usize {
        self.list.search_cursor_columns()
    }

    /// Sets the search cursor based on a display column within the search input.
    pub fn set_search_cursor_from_column(&mut self, column: u16) {
        self.list.set_search_cursor_from_column(column);
    }

    /// Move the search cursor one character to the left (UTF‑8 safe).
    pub fn move_search_left(&mut self) {
        self.list.move_search_left();
    }

    /// Move the search cursor one character to the right (UTF‑8 safe).
    pub fn move_search_right(&mut self) {
        self.list.move_search_right();
    }

    /// Updates the search query and recalculates the filtered list.
    pub fn append_search_char(&mut self, character: char) {
        self.list.append_search_char(character);
    }

    /// Removes the trailing character from the search query.
    pub fn pop_search_char(&mut self) {
        self.list.pop_search_char();
    }

    /// Clears any search filters currently applied to the workflow catalogue.
    pub fn clear_search(&mut self) {
        self.list.clear_search();
    }

    /// Advances the selection to the next workflow and updates metadata.
    pub fn select_next(&mut self) {
        self.list.select_next();
    }

    /// Moves the selection to the previous workflow and updates metadata.
    pub fn select_prev(&mut self) {
        self.list.select_prev();
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
        self.list.filtered_title_width() + 1 // for the trailing space
    }

    /// Returns a mutable reference to the input view state when active.
    pub fn input_view_state_mut(&mut self) -> Option<&mut WorkflowInputViewState> {
        self.input_view.as_mut()
    }

    /// Returns an immutable reference to the input view state when active.
    pub fn input_view_state(&self) -> Option<&WorkflowInputViewState> {
        self.input_view.as_ref()
    }

    pub fn is_running(&self) -> bool {
        self.run_control.is_some()
    }

    /// Begins an input session by storing the prepared run state and initializing view state.
    pub fn begin_inputs_session(&mut self, run_state: WorkflowRunState) {
        let run_state_arc = Rc::new(RefCell::new(run_state));
        self.input_view = Some(WorkflowInputViewState::new(run_state_arc.clone()));
        self.active_run_state = Some(run_state_arc);
        self.run_view = None;
        self.active_run_id = None;
        self.run_control = None;
    }

    /// Begins a run session by persisting the run identifier, engine state, and view state.
    pub fn begin_run_session(&mut self, run_id: String, run_state: WorkflowRunState, run_view: RunViewState) {
        let run_state_arc = Rc::new(RefCell::new(run_state));
        self.active_run_state = Some(run_state_arc);
        self.run_view = Some(run_view);
        self.active_run_id = Some(run_id);
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
        self.collector = None;
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
        let run_state = self.active_run_state.as_ref()?.borrow();
        let idx = self.input_view_state()?.input_list_state.selected()?;
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
        run_state.borrow().unresolved_item_count()
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

fn provider_identifier(definition: &WorkflowInputDefinition) -> Option<String> {
    definition.provider.as_ref().map(|provider| match provider {
        WorkflowValueProvider::Id(id) => id.clone(),
        WorkflowValueProvider::Detailed(detail) => detail.id.clone(),
    })
}

impl WorkflowState {
    /// Opens the Manual Entry modal for the currently selected input.
    pub fn open_manual_for_active_input(&mut self) -> Option<bool> {
        let run_state = self.active_run_state.as_ref()?.borrow();
        let view = self.input_view_state()?;
        let idx = view.input_list_state.selected()?;
        let (name, def) = run_state.workflow.inputs.get_index(idx)?;

        let existing_value = run_state.run_context.inputs.get(name);
        let label = def.display_name(name).into_owned();
        let state = ManualEntryState::from_definition(def, &label, existing_value);
        self.manual_entry = Some(state);
        Some(true)
    }

    /// Returns an immutable reference to the manual entry state when present.
    pub fn manual_entry_state(&self) -> Option<&ManualEntryState> {
        self.manual_entry.as_ref()
    }

    /// Returns a mutable reference to the manual entry state when present.
    pub fn manual_entry_state_mut(&mut self) -> Option<&mut ManualEntryState> {
        self.manual_entry.as_mut()
    }

    /// Returns the active run view state, if present.
    pub fn run_view_state(&self) -> Option<&RunViewState> {
        self.run_view.as_ref()
    }

    /// Returns the active run view state mutably, if present.
    pub fn run_view_state_mut(&mut self) -> Option<&mut RunViewState> {
        self.run_view.as_mut()
    }

    /// Registers the control channel for the currently active run.
    pub fn register_run_control(&mut self, run_id: &str, sender: UnboundedSender<WorkflowRunControl>) {
        if self.active_run_id.as_deref() == Some(run_id) {
            self.run_control = Some(WorkflowRunControlHandle { sender });
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
            WorkflowRunEvent::StepStarted { index, step_id, .. } => {
                run_view.mark_step_running(index, &step_id, theme);
            }
            WorkflowRunEvent::StepAttempt {
                step_id,
                attempt,
                max_attempts,
            } => {
                run_view.update_repeat_attempt(&step_id, attempt, max_attempts, theme);
            }
            WorkflowRunEvent::StepFinished {
                step_id,
                status,
                output,
                logs,
                attempts,
                duration_ms,
            } => {
                if let Some(state) = self.active_run_state.as_ref() {
                    state.borrow_mut().run_context.steps.insert(step_id.clone(), output.clone());
                }
                run_view.mark_step_finished(
                    &step_id,
                    StepFinishedData {
                        status,
                        attempts,
                        duration_ms,
                        output,
                        logs: logs.clone(),
                    },
                    theme,
                );
                log_messages.push(format!("Step '{}' {}", step_id, describe_step_status(status)));
                if status == WorkflowRunStepStatus::Failed {
                    self.run_control = None;
                }
            }
            WorkflowRunEvent::RunOutputAccumulated { key, value } => {
                run_view.append_output(&key, value);
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
        let run_state = self.active_run_state.as_ref()?.borrow();
        let idx = self.input_view_state()?.input_list_state.selected()?;
        run_state.workflow.inputs.get_index(idx).map(|(k, _)| k.clone())
    }

    /// Initializes the provider-backed selector for the currently active input.
    pub fn open_selector_for_active_input(&mut self, registry: &Arc<Mutex<CommandRegistry>>) -> Option<bool> {
        let run_state = self.active_run_state.as_ref()?.borrow();
        let view = self.input_view_state()?;
        let idx = view.input_list_state.selected()?;
        let (name, def) = run_state.workflow.inputs.get_index(idx)?;

        let provider_id = provider_identifier(def)?;
        let field_metadata = resolve_selector_field_metadata(registry, &provider_id);

        // Collect resolved provider args from the binding outcomes.
        let mut args = serde_json::Map::new();
        if let Some(provider_state) = run_state.provider_state_for(name) {
            for (arg, outcome) in &provider_state.argument_outcomes {
                if let ProviderBindingOutcome::Resolved(value) = &outcome.outcome {
                    args.insert(arg.clone(), value.clone());
                }
            }
        }

        let value_field = def.select.as_ref().and_then(|s| s.value_field.clone());

        let table: ResultsTableState<'static> = Default::default();

        self.collector = Some(CollectorViewState {
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
            field_metadata,
            staged_selection: None,
            ..Default::default()
        });

        Some(true)
    }

    /// Returns the provider selector modal state when it is active.
    pub fn collector_state(&self) -> Option<&CollectorViewState<'static>> {
        self.collector.as_ref()
    }

    /// Returns a mutable reference to the provider selector modal state when it is active.
    pub fn collector_state_mut(&mut self) -> Option<&mut CollectorViewState<'static>> {
        self.collector.as_mut()
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
        let Ok(lock) = registry.lock() else {
            return IndexMap::new();
        };
        lock.find_by_group_and_cmd(&group, &name).ok()
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

#[cfg(test)]
mod workflow_run_tests {
    use super::*;
    use crate::ui::components::workflows::run::RunViewState;
    use crate::ui::components::workflows::run::state::RunExecutionStatus;
    use crate::ui::theme::dracula::DraculaTheme;
    use chrono::Utc;
    use indexmap::IndexMap;
    use oatty_types::workflow::{
        RuntimeWorkflow, WorkflowDefaultSource, WorkflowInputDefault, WorkflowInputDefinition, WorkflowStepDefinition,
    };
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

        let region = WorkflowInputDefinition {
            default: Some(WorkflowInputDefault {
                from: WorkflowDefaultSource::Literal,
                value: Some(json!("us")),
            }),
            ..Default::default()
        };
        inputs.insert("region".into(), region.clone());

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
        let stored_state = state.active_run_state.as_ref().expect("active run state");
        assert_eq!(stored_state.borrow().run_context.inputs.get("region"), Some(&json!("us")));
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
        let row = state.run_view_state().unwrap().steps_table.selected_data(0).cloned().expect("row");
        assert_eq!(row["Status"], Value::String("succeeded".into()));
    }
}
