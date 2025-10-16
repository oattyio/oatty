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

use crate::ui::components::nav_bar::VerticalNavBarComponent;
use crate::ui::components::workflows::{WorkflowCollectorComponent, WorkflowInputsComponent};
use crate::ui::components::{
    BrowserComponent, HelpComponent, PluginsComponent, TableComponent, WorkflowsComponent, logs::LogDetailsComponent,
    nav_bar::VerticalNavBarState, plugins::PluginsDetailsComponent,
};
use crate::ui::utils::centered_rect;
use crate::ui::{
    components::{
        browser::BrowserState,
        component::Component,
        help::HelpState,
        logs::{LogsState, state::LogEntry},
        palette::{PaletteComponent, PaletteState},
        plugins::PluginsState,
        table::TableState,
        workflows::WorkflowState,
    },
    theme,
};
use anyhow::{Result, anyhow};
use heroku_api::HerokuClient;
use heroku_engine::provider::ProviderRegistry;
use heroku_engine::{
    ProviderBindingOutcome, ProviderResolutionSource, RegistryCommandRunner, StepResult, StepStatus, ValueProvider, WorkflowRunState,
};
use heroku_mcp::{PluginDetail, PluginEngine};
use heroku_registry::CommandRegistry;
use heroku_types::service::ServiceId;
use heroku_types::workflow::RuntimeWorkflow;
use heroku_types::{Effect, ExecOutcome, Modal, Msg, Pagination, Route};
use rat_focus::{Focus, FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use serde_json::{Map as JsonMap, Value as JsonValue};

/// Cross-cutting shared context owned by the App.
///
/// Holds runtime-wide objects like the command registry and configuration
/// flags. This avoids threading multiple references through components and
/// helps reduce borrow complexity.
#[derive(Debug)]
pub struct SharedCtx {
    /// Global Heroku command registry
    pub command_registry: Arc<Mutex<CommandRegistry>>,
    /// Value providers for suggestions
    pub providers: Vec<Arc<dyn ValueProvider>>,
    /// Typed ProviderRegistry for provider-backed workflow selectors
    pub provider_registry: Arc<ProviderRegistry>,
    /// Active UI theme (Dracula by default) loaded from env
    pub theme: Box<dyn theme::Theme>,
    /// MCP plugin engine (None until initialized in main.rs)
    pub plugin_engine: Arc<PluginEngine>,
}

impl SharedCtx {
    pub fn new(command_registry: Arc<Mutex<CommandRegistry>>, plugin_engine: Arc<PluginEngine>) -> Self {
        let provider_registry = Arc::new(
            ProviderRegistry::with_default_http(Arc::clone(&command_registry), Duration::from_secs(30)).expect("provider registry"),
        );
        let providers: Vec<Arc<dyn ValueProvider>> = vec![provider_registry.clone()];

        Self {
            command_registry,
            providers,
            provider_registry: provider_registry.clone(),
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
    /// Whether a command is currently executing
    pub executing: bool,
    /// Animation frame for the execution throbber
    pub throbber_idx: usize,
    /// Active execution count used by the event pump to decide whether to
    /// animate
    pub active_exec_count: Arc<AtomicUsize>,
    /// Last pagination info returned by an execution (if any)
    pub last_pagination: Option<Pagination>,
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
    pub open_modal: Option<(Box<dyn Component>, Box<dyn Fn(Rect) -> Rect>)>,
    /// Currently open logs view component
    pub logs_view: Option<Box<dyn Component>>,
    /// Global focus tree for keyboard/mouse traversal
    pub focus: Focus,
    /// the widget_id of the focus just before a modal is opened
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
    pub fn new(registry: Arc<Mutex<CommandRegistry>>, engine: Arc<PluginEngine>) -> Self {
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
            logs_view: None,
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
            _ => Vec::new(),
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
    fn handle_execution_completion(&mut self, execution_outcome: Box<ExecOutcome>) {
        let execution_outcome = *execution_outcome;
        // Keep executing=true if other executions are still active
        match execution_outcome {
            ExecOutcome::PluginDetailLoad(name, result) => self.handle_plugin_detail_load(name, result),
            ExecOutcome::PluginDetail(log, maybe_detail) => self.handle_plugin_detail(log, maybe_detail),
            ExecOutcome::PluginsRefresh(log, maybe_plugins) => self.handle_plugin_refresh_response(log, maybe_plugins),
            ExecOutcome::Log(log) => { self.logs.entries.push(log) },
            _ => {},
        }
    }

    /// Handles plugin details responses from command execution.
    ///
    /// # Arguments
    ///
    /// * `log` - The raw log output for redaction
    /// * `maybe_detail` - The plugin detail to apply
    fn handle_plugin_detail(&mut self, log: String, maybe_detail: Option<PluginDetail>) {
        self.logs.entries.push(log);
        let Some(detail) = maybe_detail else { return; };
        if let Some(state) = self.plugins.details.as_mut()
            && state.selected_plugin().is_some_and(|selected| selected == detail.name)
        {
            state.apply_detail(detail.clone());
        }

        self.plugins.table.update_item(detail);
    }

    fn handle_plugin_detail_load(&mut self, name: String, result: Result<PluginDetail, String>) {
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
    }

    /// Handles plugin refresh responses from command execution.
    ///
    /// # Arguments
    ///
    /// * `log` - The raw log output for redaction
    /// * `plugin_updates` - The updates to apply
    ///
    fn handle_plugin_refresh_response(&mut self, log: String, plugin_updates: Option<Vec<PluginDetail>>) {
        self.logs.entries.push(log);
        let Some(updated_plugins) = plugin_updates else {
            return;
        };
        self.plugins.table.replace_items(updated_plugins);
    }

    // fn route_exec_outcome(&mut self, destination: ExecOutcomeDestination, normalized_value: JsonValue) -> Vec<Effect> {
    //     let mut effects = Vec::new();
    //     match destination {
    //         ExecOutcomeDestination::WorkflowCollector => {
    //             let Some(collector_state) = self.workflows.selector_state_mut() else {return Vec::new()};
    //             collector_state.table.apply_result_json(Some(normalized_value), &*self.ctx.theme);
    //             collector_state.table.normalize();
    //             self.focus.focus(&collector_state.table.grid_f);
    //             collector_state.status = SelectorStatus::Loaded;
    //             collector_state.error_message = None;
    //         },
    //     }
    //     effects
    // }

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

    /// Open the interactive input view for the selected workflow.
    fn open_workflow_inputs(&mut self) -> Result<()> {
        let Some(workflow) = self.workflows.selected_workflow() else {
            self.logs.entries.push("No workflow selected".to_string());
            return Ok(());
        };
        let mut run_state = WorkflowRunState::new(workflow.clone());
        run_state.evaluate_input_providers()?;
        self.workflows.begin_inputs_session(run_state);
        Ok(())
    }

    /// Close the workflow input view, discarding any unsubmitted run state.
    pub fn close_workflow_inputs(&mut self) {
        self.workflows.end_inputs_session();
    }

    fn process_run_state(&mut self, mut run_state: WorkflowRunState, already_evaluated: bool) -> Result<Vec<Effect>> {
        if !already_evaluated {
            run_state.evaluate_input_providers()?;
        }

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
            self.logs
                .entries
                .push(format!("Collector: {}.{} requires attention: {}", input, argument, outcome_desc));
            return Ok(vec![Effect::ShowModal(Modal::WorkflowCollector)]);
        }

        let registry_snapshot = self
            .ctx
            .command_registry
            .lock()
            .map_err(|_| anyhow!("could not obtain command registry lock"))?
            .clone();

        let client = HerokuClient::new_from_service_id(ServiceId::CoreApi)?;
        let runner = RegistryCommandRunner::new(registry_snapshot, client);
        let results = run_state.execute_with_runner(&runner);

        self.log_workflow_execution(&run_state.workflow, &run_state, &results);

        let mut effects = Vec::new();
        if let Some(last) = results.last()
            && !last.output.is_null()
        {
            self.table.apply_result_json(Some(last.output.clone()), &*self.ctx.theme);
            self.table.normalize();
            // effects.push(Effect::ShowModal(Modal::Results));
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

    /// Updates the current route of the application and performs necessary state transitions.
    /// Note that this method is not intended to be called directly. Instead, use Effect::SwitchTo.
    ///
    /// # Arguments
    /// * `route` - A `Route` enum variant representing the new route to be set.
    ///
    /// # Behavior
    /// 1. Based on the provided `Route`, determines the corresponding components and their states.
    /// 2. For specific routes:
    ///     * **`Route::WorkflowInputs`**: Attempts to open workflow inputs and logs any errors encountered.
    ///     * **`Route::Workflows`**: Ensures workflows are loaded via the registry and logs any errors encountered.
    /// 3. Updates the navigation bar to reflect the new route.
    /// 4. Changes the main view to the component corresponding to the new route.
    /// 5. Updates the focus behavior using a `FocusBuilder` and sets the focus to the respective state.
    ///
    /// # Errors
    /// - Logs errors related to loading workflows or opening workflow inputs if the operations fail.
    ///
    /// # Side Effects
    /// - Updates internal state fields:
    ///   * `current_route` - Tracks the currently active route.
    ///   * `main_view` - Holds the new route's component as a boxed trait object.
    ///   * `focus` - Responsible for managing the focus and is updated dynamically based on the route.
    ///
    /// # Example
    /// ```rust
    /// let mut app = MyApp::new();
    /// app.set_current_route(Route::Palette);
    /// ```
    pub fn set_current_route(&mut self, route: Route) {
        let (view, state): (Box<dyn Component>, Box<&dyn HasFocus>) = match route {
            Route::Browser => (Box::new(BrowserComponent), Box::new(&self.browser)),
            Route::Palette => (Box::new(PaletteComponent::default()), Box::new(&self.palette)),
            Route::Plugins => (Box::new(PluginsComponent::default()), Box::new(&self.plugins)),
            Route::WorkflowInputs => {
                if let Err(error) = self.open_workflow_inputs() {
                    self.logs.entries.push(format!("Failed to open workflow inputs: {error}"));
                }

                (Box::new(WorkflowInputsComponent), Box::new(&self.workflows))
            }
            Route::Workflows => {
                if let Err(error) = self.workflows.ensure_loaded(&self.ctx.command_registry) {
                    self.logs.entries.push(format!("Failed to load workflows: {error}"));
                }
                (Box::new(WorkflowsComponent), Box::new(&self.workflows))
            }
        };

        self.current_route = self.nav_bar.set_route(route);
        self.main_view = Some(view);
        self.focus = FocusBuilder::build_for(self);
        self.focus.focus(*state);
    }

    /// Update the open modal kind (use None to clear).
    pub fn set_open_modal_kind(&mut self, modal: Option<Modal>) {
        if let Some(modal_kind) = modal.clone() {
            let modal_view: (Box<dyn Component>, Box<dyn Fn(Rect) -> Rect>) = match modal_kind {
                Modal::Help => (Box::new(HelpComponent::default()), Box::new(|rect| centered_rect(80, 70, rect))),
                Modal::Results(exec_outcome) => {
                    self.table.process_general_execution_result(&exec_outcome, &*self.ctx.theme);
                    (Box::new(TableComponent::default()), Box::new(|rect| centered_rect(96, 90, rect))) 
                },
                Modal::LogDetails => (
                    Box::new(LogDetailsComponent::default()),
                    Box::new(|rect| centered_rect(80, 70, rect)),
                ),
                Modal::PluginDetails => (
                    Box::new(PluginsDetailsComponent::default()),
                    Box::new(|rect| centered_rect(90, 80, rect)),
                ),
                Modal::WorkflowCollector => (
                    Box::new(WorkflowCollectorComponent::default()),
                    Box::new(|rect| centered_rect(96, 90, rect)),
                ),
            };
            self.open_modal = Some(modal_view);
            // save the current focus to restore when the modal is closed
            self.transient_focus_id = self.focus.focused().map(|focus| focus.widget_id());
        } else {
            self.open_modal = None;
        }
        self.open_modal_kind = modal;
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

impl HasFocus for App<'_> {
    /// Build the top-level focus container for the application.
    ///
    /// Order matters: traversal follows the order widgets are added here.
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        // If a modal is open, it is the sole focus scope.
        if let Some(kind) = &self.open_modal_kind {
            match kind {
                Modal::Results(_) => {
                    builder.widget(&self.table);
                }
                Modal::LogDetails => {
                    builder.widget(&self.logs);
                }
                Modal::WorkflowCollector => {
                    // no focusable fields; leave ring empty (collector stub)
                }
                Modal::PluginDetails | Modal::Help => {
                    // no focusable fields; leave the ring empty
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
            Route::Workflows | Route::WorkflowInputs => {
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
