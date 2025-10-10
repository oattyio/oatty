//! Application state and logic for the Heroku TUI.
//!
//! This module contains the main application state, data structures, and
//! business logic for the TUI interface. It manages the application lifecycle,
//! user interactions, and coordinates between different UI components.

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use anyhow::{Result, anyhow};
use heroku_api::HerokuClient;
use heroku_engine::{
    ProviderBindingOutcome, ProviderResolutionSource, RegistryCommandRunner, RuntimeWorkflow, StepResult, StepStatus, WorkflowRunState,
};
use heroku_mcp::{PluginDetail, PluginEngine};
use heroku_registry::Registry;
use heroku_types::service::ServiceId;
use heroku_types::{Effect, ExecOutcome, Modal, Msg, Route};
use rat_focus::{Focus, FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use serde_json::{Map as JsonMap, Value as JsonValue};

use crate::ui::components::nav_bar::VerticalNavBarComponent;
use crate::ui::components::workflows::WorkflowInputsComponent;
use crate::ui::components::workflows::collector::WorkflowCollectorComponent;
use crate::ui::components::{
    BrowserComponent, HelpComponent, PluginsComponent, TableComponent, WorkflowsComponent, logs::LogDetailsComponent,
    nav_bar::VerticalNavBarState, plugins::PluginsDetailsComponent,
};
use crate::ui::{
    components::{
        browser::BrowserState,
        component::Component,
        help::HelpState,
        logs::{LogsState, state::LogEntry},
        palette::{PaletteComponent, PaletteState, providers::RegistryBackedProvider, state::ValueProvider},
        plugins::PluginsState,
        table::TableState,
        workflows::WorkflowState,
    },
    theme,
};

/// Cross-cutting shared context owned by the App.
///
/// Holds runtime-wide objects like the command registry and configuration
/// flags. This avoids threading multiple references through components and
/// helps reduce borrow complexity.
#[derive(Debug)]
pub struct SharedCtx {
    /// Global Heroku command registry
    pub registry: Arc<Mutex<Registry>>,
    /// Global debug flag (from env)
    pub debug_enabled: bool,
    /// Value providers for suggestions
    pub providers: Vec<Box<dyn ValueProvider>>,
    /// Active UI theme (Dracula by default) loaded from env
    pub theme: Box<dyn theme::Theme>,
    /// MCP plugin engine (None until initialized in main.rs)
    pub plugin_engine: Arc<PluginEngine>,
}

impl SharedCtx {
    pub fn new(registry: Arc<Mutex<Registry>>, plugin_engine: Arc<PluginEngine>) -> Self {
        let debug_enabled = std::env::var("DEBUG")
            .map(|v| !v.is_empty() && v != "0" && v.to_lowercase() != "false")
            .unwrap_or(false);
        // Add a registry-backed provider with a small TTL cache
        let providers: Vec<Box<dyn ValueProvider>> = vec![Box::new(RegistryBackedProvider::new(Duration::from_secs(45)))];
        Self {
            registry,
            debug_enabled,
            providers,
            theme: theme::load_from_env(),
            plugin_engine,
        }
    }
}

pub struct App<'a> {
    /// Shared, cross-cutting context (registry, config)
    pub ctx: SharedCtx,
    /// State for the command palette input
    pub palette: PaletteState,
    /// Command browser state
    pub browser: BrowserState,
    /// Table modal state
    pub table: TableState<'a>,
    /// Help modal state
    pub help: HelpState,
    /// Plugins state (MCP management)
    pub plugins: PluginsState,
    /// Workflow UI and execution state
    pub workflows: WorkflowState,
    /// Application logs and status messages
    pub logs: LogsState,
    /// Vertical navigation bar state (left rail)
    pub nav_bar: VerticalNavBarState,
    // moved to ctx: dry_run, debug_enabled, providers
    /// Whether a command is currently executing
    pub executing: bool,
    /// Animation frame for the execution throbber
    pub throbber_idx: usize,
    /// Active execution count used by the event pump to decide whether to
    /// animate
    pub active_exec_count: Arc<AtomicUsize>,
    /// Last pagination info returned by an execution (if any)
    pub last_pagination: Option<heroku_types::Pagination>,
    /// Ranges supported by the last executed command (for pagination UI)
    pub last_command_ranges: Option<Vec<String>>,
    /// Last executed CommandSpec (for pagination replays)
    pub last_spec: Option<heroku_registry::CommandSpec>,
    /// Last request body used for the executed command
    pub last_body: Option<JsonMap<String, JsonValue>>,
    /// History of Range headers used per page request (None means no Range header)
    pub pagination_history: Vec<Option<String>>,
    /// Initial Range header used (if any)
    pub initial_range: Option<String>,
    /// Current main view component
    pub main_view: Option<Box<dyn Component>>,
    /// Main view for the nav bar
    pub nav_bar_view: Option<Box<dyn Component>>,
    /// Currently open modal component
    pub open_modal: Option<Box<dyn Component>>,
    /// Global focus tree for keyboard/mouse traversal
    pub focus: Focus,
    // the widget_id of the focus just before a modal is opened
    transient_focus_id: Option<usize>,
    /// Container focus flag for the top-level app focus scope
    app_container_focus: FocusFlag,
    /// Currently active main route for dynamic focus ring building
    current_route: Route,
    /// Currently open modal kind (when Some, modal owns focus)
    open_modal_kind: Option<Modal>,
}

impl App<'_> {
    /// Creates a new application instance with the given registry.
    ///
    /// This constructor initializes the application state with default values
    /// and loads all commands from the provided registry.
    ///
    /// # Arguments
    ///
    /// * `registry` - The Heroku command registry containing all available
    ///   commands
    ///
    /// # Returns
    ///
    /// A new App instance with an initialized state.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Requires constructing a full Registry and App; ignored in doctests.
    /// ```
    pub fn new(registry: Arc<Mutex<Registry>>, engine: Arc<PluginEngine>) -> Self {
        let mut app = Self {
            ctx: SharedCtx::new(Arc::clone(&registry), engine),
            browser: BrowserState::new(Arc::clone(&registry)),
            logs: LogsState::default(),
            help: HelpState::default(),
            plugins: PluginsState::new(),
            workflows: WorkflowState::new(),
            table: TableState::default(),
            palette: PaletteState::new(Arc::clone(&registry)),
            nav_bar: VerticalNavBarState::defaults_for_views(),
            executing: false,
            throbber_idx: 0,
            active_exec_count: Arc::new(AtomicUsize::new(0)),
            last_pagination: None,
            last_command_ranges: None,
            last_spec: None,
            last_body: None,
            pagination_history: Vec::new(),
            initial_range: None,
            main_view: Some(Box::new(PaletteComponent::default())),
            open_modal: None,
            nav_bar_view: Some(Box::new(VerticalNavBarComponent::new())),
            focus: Focus::default(),
            transient_focus_id: None,
            app_container_focus: FocusFlag::named("app.container"),
            current_route: Route::Palette,
            open_modal_kind: None,
        };
        app.browser.update_browser_filtered();

        // Initialize rat-focus and set a sensible starting focus inside the palette
        app.focus = FocusBuilder::build_for(&app);
        app.focus.focus(&app.palette);

        app
    }

    /// Updates the application state based on a message.
    ///
    /// This method processes messages and updates the application state
    /// accordingly. It handles user interactions, navigation, and state
    /// changes. The method delegates to specialized handlers for different
    /// types of messages to keep the logic organized and maintainable.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to process
    ///
    /// # Returns
    ///
    /// Vector of side effects that should be performed.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Example requires real App/Msg types; ignored to avoid compile in doctests.
    /// ```
    pub fn update(&mut self, message: Msg) -> Vec<Effect> {
        match message {
            Msg::Tick => self.handle_tick_message(),
            Msg::Resize(..) => vec![],
            Msg::CopyToClipboard(text) => vec![Effect::CopyToClipboardRequested(text)],
            Msg::ExecCompleted(execution_outcome) => self.handle_execution_completion(execution_outcome),
            // Placeholder handlers for upcoming logs features
            Msg::LogsUp | Msg::LogsDown | Msg::LogsExtendUp | Msg::LogsExtendDown => vec![],
            Msg::LogsOpenDetail | Msg::LogsCloseDetail | Msg::LogsCopy | Msg::LogsTogglePretty => vec![],
        }
    }

    /// Handles tick messages for periodic updates and animations.
    ///
    /// This method manages periodic tasks such as animating the execution
    /// throbber, refreshing plugin statuses, updating logs in follow mode,
    /// and rebuilding suggestions when provider-backed results are available.
    ///
    /// # Arguments
    ///
    fn handle_tick_message(&mut self) -> Vec<Effect> {
        // Animate spinner while executing or while provider-backed suggestions are loading
        if self.executing || self.palette.is_provider_loading() {
            let previous_throbber_index = self.throbber_idx;
            self.throbber_idx = (self.throbber_idx + 1) % 10;
            if self.throbber_idx != previous_throbber_index {}
        }

        // Periodically refresh plugin statuses when overlay is visible
        if self.plugins.table.should_refresh() {
            return vec![Effect::PluginsRefresh];
        }

        // If provider-backed suggestions are loading and the popup is open,
        // rebuild suggestions to pick up newly cached results without requiring
        // another keypress
        if self.palette.is_suggestions_open() && self.palette.is_provider_loading() {
            let SharedCtx { providers, .. } = &self.ctx;
            self.palette.apply_build_suggestions(providers, &*self.ctx.theme);
        }
        vec![]
    }

    /// Handles execution completion messages and processes the results.
    ///
    /// This method processes the results of command execution, including
    /// plugin-specific responses, logs updates, and general command results.
    /// It handles special plugin responses and falls back to general result
    /// processing for regular commands.
    ///
    /// # Arguments
    ///
    /// * `execution_outcome` - The result of the command execution
    ///
    /// # Returns
    ///
    /// Returns `true` if the execution was handled as a special case (plugin response)
    /// and the caller should return early, `false` if normal processing should continue.
    fn handle_execution_completion(&mut self, execution_outcome: Box<ExecOutcome>) -> Vec<Effect> {
        let execution_outcome = *execution_outcome;
        // Keep executing=true if other executions are still active
        let still_executing = self.active_exec_count.load(Ordering::Relaxed) > 0;
        self.executing = still_executing;
        match execution_outcome {
            ExecOutcome::Http(..) => self.process_general_execution_result(execution_outcome),
            ExecOutcome::Mcp(log, value) => self.process_mcp_execution_result(log, value),
            ExecOutcome::PluginDetailLoad(name, result) => self.handle_plugin_detail_load(name, result),
            ExecOutcome::PluginDetail(log, maybe_detail) => self.handle_plugin_detail(log, maybe_detail),
            ExecOutcome::PluginsRefresh(log, maybe_plugins) => self.handle_plugin_refresh_response(log, maybe_plugins),
            ExecOutcome::Log(log) => {
                self.logs.entries.push(log);
                vec![]
            }
            _ => vec![],
        }
    }

    /// Handles plugin details responses from command execution.
    ///
    /// # Arguments
    ///
    /// * `log` - The raw log output for redaction
    /// * `maybe_detail` - The plugin detail to apply
    ///
    /// # Returns
    ///
    /// Returns `Vec<Effect>` if follow up effects are needed
    fn handle_plugin_detail(&mut self, log: String, maybe_detail: Option<PluginDetail>) -> Vec<Effect> {
        self.logs.entries.push(log);
        let Some(detail) = maybe_detail else { return vec![] };
        if let Some(state) = self.plugins.details.as_mut()
            && state.selected_plugin().is_some_and(|selected| selected == detail.name)
        {
            state.apply_detail(detail.clone());
        }

        self.plugins.table.update_item(detail);

        vec![]
    }

    fn handle_plugin_detail_load(&mut self, name: String, result: Result<PluginDetail, String>) -> Vec<Effect> {
        match result {
            Ok(detail) => {
                self.logs.entries.push(format!("Plugins: loaded details for '{name}'"));
                if let Some(state) = self.plugins.details.as_mut()
                    && state.selected_plugin().is_some_and(|selected| selected == name)
                {
                    state.apply_detail(detail.clone());
                }
                self.plugins.table.update_item(detail);
            }
            Err(error) => {
                self.logs
                    .entries
                    .push(format!("Plugins: failed to load details for '{name}': {error}"));
                if let Some(state) = self.plugins.details.as_mut()
                    && state.selected_plugin().is_some_and(|selected| selected == name)
                {
                    state.mark_error(error);
                }
            }
        }

        vec![]
    }

    /// Handles plugin refresh responses from command execution.
    ///
    /// # Arguments
    ///
    /// * `log` - The raw log output for redaction
    /// * `plugin_updates` - The updates to apply
    ///
    /// # Returns
    ///
    /// Returns `Vec<Effect>` if follow up effects are needed
    fn handle_plugin_refresh_response(&mut self, log: String, plugin_updates: Option<Vec<PluginDetail>>) -> Vec<Effect> {
        self.logs.entries.push(log);
        let Some(updated_plugins) = plugin_updates else {
            return vec![];
        };
        self.plugins.table.replace_items(updated_plugins);
        vec![]
    }

    /// Processes general command execution results (non-plugin specific).
    ///
    /// This method handles the standard processing of command results including
    /// logging, table updates, and pagination information.
    ///
    /// # Arguments
    ///
    /// * `execution_outcome` - The result of the command execution
    fn process_general_execution_result(&mut self, execution_outcome: ExecOutcome) -> Vec<Effect> {
        let ExecOutcome::Http(log, value, maybe_pagination, open_table) = execution_outcome else {
            return vec![];
        };

        // nothing to do
        if !open_table || (log.is_empty() && value.is_null()) {
            return vec![];
        }
        let (summary, status_code) = summarize_execution_outcome(self.last_spec.as_ref(), &log);

        let normalized_value = Self::normalize_result_payload(value);

        self.logs.entries.push(summary);
        self.logs.rich_entries.push(LogEntry::Api {
            status: status_code.unwrap_or(0),
            raw: log,
            json: Some(normalized_value.clone()),
        });

        self.trim_logs_if_needed();

        if open_table {
            self.table.apply_result_json(Some(normalized_value), &*self.ctx.theme);
            self.table.normalize();
            self.last_pagination = maybe_pagination;
            self.palette.reduce_clear_all();

            return vec![Effect::ShowModal(Modal::Results)];
        }

        vec![]
    }

    fn process_mcp_execution_result(&mut self, log: String, value: JsonValue) -> Vec<Effect> {
        let label = command_label(self.last_spec.as_ref());
        let success = if log.contains("failed") { "failed" } else { "succeeded" };
        let summary = format!("{} - {}", label, success);

        let normalized_value = Self::normalize_result_payload(value);
        let raw_payload = Self::stringify_result_payload(&normalized_value);

        self.logs.entries.push(summary);
        self.logs.rich_entries.push(LogEntry::MCP {
            raw: raw_payload,
            json: Some(normalized_value.clone()),
        });
        self.trim_logs_if_needed();

        if normalized_value.is_object() || normalized_value.is_array() {
            self.table.apply_result_json(Some(normalized_value), &*self.ctx.theme);
            self.table.normalize();
            self.palette.reduce_clear_all();
            return vec![Effect::ShowModal(Modal::Results)];
        }

        vec![]
    }

    /// Normalize execution payloads to ensure single-key collections render in the results table.
    ///
    /// Some APIs return objects shaped as `{ "items": [ ... ] }`. The table expects an array at
    /// the root level, so this helper unwraps objects that meet this pattern. All other payloads
    /// are returned unchanged.
    fn normalize_result_payload(value: JsonValue) -> JsonValue {
        if let JsonValue::Object(map) = &value {
            if map.len() == 1 {
                if let Some(inner_value) = map.values().next() {
                    if inner_value.is_array() {
                        return inner_value.clone();
                    }
                }
            }
        }
        value
    }

    /// Produce a human-readable string representation of a JSON payload for logging.
    fn stringify_result_payload(value: &JsonValue) -> String {
        match value {
            JsonValue::String(text) => text.clone(),
            _ => value.to_string(),
        }
    }

    /// Trims log entries if they exceed the maximum allowed size.
    ///
    /// This method maintains reasonable memory usage by limiting the number
    /// of log entries stored in memory.
    fn trim_logs_if_needed(&mut self) {
        const MAX_LOG_ENTRIES: usize = 500;

        let log_length = self.logs.entries.len();
        if log_length > MAX_LOG_ENTRIES {
            let _ = self.logs.entries.drain(0..log_length - MAX_LOG_ENTRIES);
        }

        let rich_log_length = self.logs.rich_entries.len();
        if rich_log_length > MAX_LOG_ENTRIES {
            let _ = self.logs.rich_entries.drain(0..rich_log_length - MAX_LOG_ENTRIES);
        }
    }

    pub fn run_workflow(&mut self, workflow: &RuntimeWorkflow) -> Result<Vec<Effect>> {
        let run_state = WorkflowRunState::new(workflow.clone());
        self.process_run_state(run_state, false)
    }

    /// Execute the workflow using the run state accumulated via the Guided Input Collector.
    pub fn execute_workflow_from_collector(&mut self) -> Result<Vec<Effect>> {
        let run_state = match self.workflows.take_run_state() {
            Some(state) => state,
            None => {
                self.logs.entries.push("No workflow run state available to execute".to_string());
                return Ok(vec![Effect::CloseModal]);
            }
        };
        self.workflows.set_collector_visible(false);
        let mut effects = vec![Effect::CloseModal];
        effects.extend(self.process_run_state(run_state, true)?);
        Ok(effects)
    }

    /// Open the interactive input view for the selected workflow.
    pub fn open_workflow_inputs(&mut self, workflow: &RuntimeWorkflow) -> Result<()> {
        let mut run_state = WorkflowRunState::new(workflow.clone());
        run_state.evaluate_input_providers()?;
        self.workflows.observe_provider_refresh(&run_state);
        self.workflows.begin_inputs_session(run_state);
        self.set_current_route(Route::Workflows);
        Ok(())
    }

    /// Close the workflow input view, discarding any unsubmitted run state.
    pub fn close_workflow_inputs(&mut self) {
        self.workflows.end_inputs_session();
        self.set_current_route(Route::Workflows);
    }

    /// Execute a workflow using the inputs prepared in the input view.
    pub fn execute_workflow_from_inputs(&mut self) -> Result<Vec<Effect>> {
        let run_state = match self.workflows.take_run_state() {
            Some(state) => state,
            None => {
                self.logs.entries.push("No workflow run state available to execute".to_string());
                return Ok(Vec::new());
            }
        };

        self.workflows.end_inputs_session();
        let effects = self.process_run_state(run_state, true)?;
        self.set_current_route(Route::Workflows);
        Ok(effects)
    }

    fn process_run_state(&mut self, mut run_state: WorkflowRunState, already_evaluated: bool) -> Result<Vec<Effect>> {
        if !already_evaluated {
            run_state.evaluate_input_providers()?;
        }

        self.workflows.observe_provider_refresh(&run_state);

        if let Some(blocked) = run_state
            .telemetry()
            .provider_resolution_events()
            .iter()
            .find(|event| matches!(event.outcome, ProviderBindingOutcome::Prompt(_) | ProviderBindingOutcome::Error(_)))
        {
            let input = blocked.input.clone();
            let argument = blocked.argument.clone();
            let outcome_desc = describe_provider_outcome(&blocked.outcome);
            self.workflows.set_active_run_state(run_state);
            self.workflows.set_collector_visible(true);
            self.logs
                .entries
                .push(format!("Collector: {}.{} requires attention: {}", input, argument, outcome_desc));
            return Ok(vec![Effect::ShowModal(Modal::WorkflowCollector)]);
        }

        let registry_snapshot = self
            .ctx
            .registry
            .lock()
            .map_err(|_| anyhow!("could not obtain command registry lock"))?
            .clone();

        let client = HerokuClient::new_from_service_id(ServiceId::CoreApi)?;
        let runner = RegistryCommandRunner::new(registry_snapshot, client);
        let results = run_state.execute_with_runner(&runner);

        self.log_workflow_execution(&run_state.workflow, &run_state, &results);

        let mut effects = Vec::new();
        if let Some(last) = results.last() {
            if !last.output.is_null() {
                self.table.apply_result_json(Some(last.output.clone()), &*self.ctx.theme);
                self.table.normalize();
                effects.push(Effect::ShowModal(Modal::Results));
            }
        }

        Ok(effects)
    }
    /// Logs the execution details of a workflow, including telemetry events and step results. The
    /// method appends log entries to the internal `logs` structure for both concise and rich logging.
    ///
    /// # Arguments
    ///
    /// * `workflow` - A reference to a `RuntimeWorkflow` that contains information about the executed workflow.
    /// * `run_state` - A reference to a `WorkflowRunState` that provides access to telemetry events from the execution.
    /// * `results` - A slice of `StepResult` containing the outcome of each step executed in the workflow.
    ///
    /// # Behavior
    ///
    /// This method performs the following:
    /// - Logs a summary entry for the workflow execution, including the workflow identifier and the number of steps executed.
    /// - Logs a richer `LogEntry::Text` with an "info" level indicating workflow execution.
    /// - Iterates through the provider resolution events from the workflow run's telemetry and logs detailed entries for each event,
    ///   describing the source (automatic or manual) and the outcome of the provider resolution.
    /// - Iterates through the step results and logs a detailed entry for each step, including its identifier and status.
    /// - Ensures the logs are trimmed if needed to conform to any size constraints.
    ///
    /// # Internal Structures
    ///
    /// - The `logs.entries` collection stores brief, human-readable log entries as plain text.
    /// - The `logs.rich_entries` collection stores more structured log entries (like `LogEntry::Text`).
    ///
    /// # Dependencies
    ///
    /// - Relies on helper functions like `describe_provider_outcome` to translate provider outcomes into human-readable strings.
    /// - Relies on `format_step_status` to convert step statuses into readable formats.
    ///
    /// Note: The method assumes the `self` type has a `trim_logs_if_needed` method to manage the size of log storage.
    fn log_workflow_execution(&mut self, workflow: &RuntimeWorkflow, run_state: &WorkflowRunState, results: &[StepResult]) {
        self.logs
            .entries
            .push(format!("Workflow '{}' executed ({} steps)", workflow.identifier, results.len()));
        self.logs.rich_entries.push(LogEntry::Text {
            level: Some("info".into()),
            msg: format!("Workflow '{}' executed", workflow.identifier),
        });

        for event in run_state.telemetry().provider_resolution_events() {
            self.logs.entries.push(format!(
                "  provider {}.{} [{}] {}",
                event.input,
                event.argument,
                match event.source {
                    ProviderResolutionSource::Automatic => "auto",
                    ProviderResolutionSource::Manual => "manual",
                },
                describe_provider_outcome(&event.outcome)
            ));
        }

        for step in results {
            self.logs
                .entries
                .push(format!("  step {} {}", step.id, format_step_status(step.status)));
        }

        self.trim_logs_if_needed();
    }

    /// Update the current main route for focus building.
    pub fn set_current_route(&mut self, route: Route) {
        if matches!(route, Route::Workflows) {
            if let Err(error) = self.workflows.ensure_loaded(&self.ctx.registry) {
                self.logs.entries.push(format!("Failed to load workflows: {error}"));
            }
        }

        let (view, state): (Box<dyn Component>, Box<&dyn HasFocus>) = match route {
            Route::Browser => (Box::new(BrowserComponent::default()), Box::new(&self.browser)),
            Route::Palette => (Box::new(PaletteComponent::default()), Box::new(&self.palette)),
            Route::Plugins => (Box::new(PluginsComponent::default()), Box::new(&self.plugins)),
            Route::Workflows => {
                if self.workflows.inputs_view_active() {
                    (Box::new(WorkflowInputsComponent::default()), Box::new(&self.workflows))
                } else {
                    (Box::new(WorkflowsComponent::default()), Box::new(&self.workflows))
                }
            }
        };

        self.current_route = self.nav_bar.set_route(route);
        self.main_view = Some(view);
        self.focus = FocusBuilder::build_for(self);
        self.focus.focus(*state);
    }

    /// Update the open modal kind (use None to clear).
    pub fn set_open_modal_kind(&mut self, modal: Option<Modal>) {
        let previous = self.open_modal_kind.clone();
        if let Some(modal_kind) = modal.clone() {
            let modal_view: Box<dyn Component> = match modal_kind {
                Modal::Help => Box::new(HelpComponent::default()),
                Modal::Results => Box::new(TableComponent::default()),
                Modal::LogDetails => Box::new(LogDetailsComponent::default()),
                Modal::PluginDetails => Box::new(PluginsDetailsComponent::default()),
                Modal::WorkflowCollector => {
                    self.workflows.set_collector_visible(true);
                    Box::new(WorkflowCollectorComponent::default())
                }
            };
            self.open_modal = Some(modal_view);
            // save the current focus to restore when the modal is closed
            self.transient_focus_id = self.focus.focused().and_then(|f| Some(f.widget_id()));
        } else {
            self.open_modal = None;
        }
        self.open_modal_kind = modal;

        if matches!(self.open_modal_kind, Some(Modal::PluginDetails)) {
            self.plugins.ensure_details_state();
        }

        if matches!(previous, Some(Modal::PluginDetails)) && !matches!(self.open_modal_kind, Some(Modal::PluginDetails)) {
            self.plugins.clear_details_state();
        }
        // If we are closing the collector, drop the retained run state
        if matches!(previous, Some(Modal::WorkflowCollector)) && !matches!(self.open_modal_kind, Some(Modal::WorkflowCollector)) {
            self.workflows.set_collector_visible(false);
            let _ = self.workflows.take_run_state();
        }
    }

    pub fn restore_focus(&mut self) {
        if let Some(id) = self.transient_focus_id
            && self.open_modal.is_none()
        {
            self.focus.by_widget_id(id);
            self.transient_focus_id = None;
        } else {
            self.focus.first();
        }
    }
}

const EXECUTION_SUMMARY_LIMIT: usize = 160;

fn describe_provider_outcome(outcome: &ProviderBindingOutcome) -> String {
    match outcome {
        ProviderBindingOutcome::Resolved(value) => {
            if let Some(s) = value.as_str() {
                format!("resolved to '{s}'")
            } else {
                format!("resolved to {}", value)
            }
        }
        ProviderBindingOutcome::Prompt(prompt) => format!("prompted (required: {}, reason: {})", prompt.required, prompt.reason.message),
        ProviderBindingOutcome::Skip(decision) => {
            format!("skipped ({})", decision.reason.message)
        }
        ProviderBindingOutcome::Error(error) => format!("error: {}", error.message),
    }
}

fn format_step_status(status: StepStatus) -> &'static str {
    match status {
        StepStatus::Succeeded => "succeeded",
        StepStatus::Failed => "failed",
        StepStatus::Skipped => "skipped",
    }
}

fn summarize_execution_outcome(command_spec: Option<&heroku_registry::CommandSpec>, raw_log: &str) -> (String, Option<u16>) {
    let label = command_label(command_spec);
    let trimmed_log = raw_log.trim();

    if trimmed_log.starts_with("Plugins:") {
        let sanitized = heroku_util::redact_sensitive(trimmed_log);
        return (sanitized, None);
    }

    if let Some(error_message) = trimmed_log.strip_prefix("Error:") {
        let redacted = heroku_util::redact_sensitive(error_message.trim());
        let truncated = truncate_for_summary(&redacted, EXECUTION_SUMMARY_LIMIT);
        let summary = format!("{} - failed: {}", label, truncated);
        return (summary, None);
    }

    let status_line = trimmed_log.lines().next().unwrap_or_default().trim();
    let status_code = status_line.split_whitespace().next().and_then(|code| code.parse::<u16>().ok());

    let success = if status_code.is_some_and(|c| c.clamp(200, 399) == c) {
        "success"
    } else {
        "failed"
    };
    let summary = if status_line.is_empty() {
        format!("{} - {}", label, success)
    } else {
        let sanitized_status = heroku_util::redact_sensitive(status_line);
        format!("{} - {} ({})", label, success, sanitized_status)
    };

    (summary, status_code)
}

fn command_label(command_spec: Option<&heroku_registry::CommandSpec>) -> String {
    match command_spec {
        Some(spec) if spec.name.is_empty() => spec.group.clone(),
        Some(spec) => format!("{} {}", spec.group, spec.name),
        None => "Command".to_string(),
    }
}

fn truncate_for_summary(text: &str, max_len: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_len {
        return trimmed.to_string();
    }

    // Reserve space for the trailing ellipsis ("...").
    let target_len = max_len.saturating_sub(3);
    let mut truncated = String::new();
    for (idx, ch) in trimmed.chars().enumerate() {
        if idx >= target_len {
            break;
        }
        truncated.push(ch);
    }
    let trimmed_truncated = truncated.trim_end();
    format!("{}...", trimmed_truncated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use heroku_registry::CommandSpec;
    use heroku_types::{ServiceId, command::HttpCommandSpec};

    fn sample_spec() -> CommandSpec {
        CommandSpec::new_http(
            "apps".to_string(),
            "info".to_string(),
            String::new(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/apps", ServiceId::CoreApi, Vec::new(), None),
        )
    }

    #[test]
    fn summarize_success_includes_status_code() {
        let spec = sample_spec();
        let (summary, status) = summarize_execution_outcome(Some(&spec), "200 OK\n{\"foo\":\"bar\"}");

        assert_eq!(summary, "apps info - success (200 OK)");
        assert_eq!(status, Some(200));
    }

    #[test]
    fn summarize_error_marks_failure_and_truncates() {
        let spec = sample_spec();
        let long_error = format!("Error: {}", "SensitiveToken123".repeat((EXECUTION_SUMMARY_LIMIT / 5) + 10));

        let (summary, status) = summarize_execution_outcome(Some(&spec), &long_error);

        assert!(summary.starts_with("apps info - failed: "));
        assert!(summary.ends_with("..."));
        assert_eq!(status, None);
    }

    #[test]
    fn summarize_without_spec_uses_generic_label() {
        let (summary, status) = summarize_execution_outcome(None, "200 OK\n{}");

        assert_eq!(summary, "Command - success (200 OK)");
        assert_eq!(status, Some(200));
    }
}

impl HasFocus for App<'_> {
    /// Build the top-level focus container for the application.
    ///
    /// Order matters: traversal follows the order widgets are added here.
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        // If a modal is open, it is the sole focus scope.
        if let Some(kind) = &self.open_modal_kind {
            match kind {
                Modal::Results => {
                    builder.widget(&self.table);
                }
                Modal::LogDetails => {
                    builder.widget(&self.logs);
                }
                Modal::WorkflowCollector => {
                    // no focusable fields; leave ring empty (collector stub)
                }
                Modal::PluginDetails | Modal::Help => {
                    // no focusable fields; leave ring empty
                }
            }
            builder.end(tag);
            return;
        }

        // Otherwise, include the nav bar, active main view, and sibling logs for Tab
        builder.widget(&self.nav_bar);

        match self.current_route {
            Route::Palette => {
                builder.widget(&self.palette);
                builder.widget(&self.logs);
            }
            Route::Browser => {
                builder.widget(&self.browser);
            }
            Route::Plugins => {
                builder.widget(&self.plugins);
            }
            Route::Workflows => {
                builder.widget(&self.workflows);
                builder.widget(&self.logs);
            }
        }

        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.app_container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
