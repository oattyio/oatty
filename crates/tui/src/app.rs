//! Application state and logic for the Heroku TUI.
//!
//! This module contains the main application state, data structures, and
//! business logic for the TUI interface. It manages the application lifecycle,
//! user interactions, and coordinates between different UI components.

use std::{
    sync::{Arc, Mutex, atomic::AtomicUsize},
    time::Duration,
};

use crate::ui::components::nav_bar::VerticalNavBarState;
use crate::ui::components::workflows::collector::SelectorStatus;
use crate::ui::components::workflows::run::RunViewState;
use crate::ui::{
    components::{
        browser::BrowserState, help::HelpState, logs::LogsState, palette::PaletteState, plugins::PluginsState, table::ResultsTableState,
        theme_picker::ThemePickerState, workflows::WorkflowState,
    },
    theme,
};
use heroku_engine::ValueProvider;
use heroku_engine::provider::{CacheLookupOutcome, PendingProviderFetch, ProviderRegistry};
use heroku_mcp::PluginEngine;
use heroku_registry::CommandRegistry;
use heroku_types::{Effect, Modal, Msg, Route, WorkflowRunEvent, WorkflowRunRequest, WorkflowRunStatus, validate_candidate_value};
use heroku_util::{
    DEFAULT_HISTORY_PROFILE, HistoryKey, HistoryStore, InMemoryHistoryStore, JsonHistoryStore, UserPreferences, has_meaningful_value,
    value_contains_secret, workflow_input_uses_history,
};
use rat_focus::{Focus, FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::warn;

/// Cross-cutting shared context owned by the App.
///
/// Holds runtime-wide objects like the command registry and configuration
/// flags. This avoids threading multiple references through components and
/// helps reduce borrow complexity.
pub struct SharedCtx {
    /// Global Heroku command registry
    pub command_registry: Arc<Mutex<CommandRegistry>>,
    /// Value providers for suggestions
    pub providers: Vec<Arc<dyn ValueProvider>>,
    /// Typed ProviderRegistry for provider-backed workflow selectors
    pub provider_registry: Arc<ProviderRegistry>,
    /// Active UI theme (Dracula by default) loaded from env
    pub theme: Box<dyn theme::Theme>,
    /// MCP plugin engine (None until initialized in main_component)
    pub plugin_engine: Arc<PluginEngine>,
    /// On-disk history store for workflow inputs and palette commands.
    pub history_store: Arc<dyn HistoryStore>,
    /// Identifier representing the active history profile.
    pub history_profile_id: String,
    /// Persisted user preferences (theme picker, appearance decisions, etc.).
    pub preferences: Arc<UserPreferences>,
    /// Canonical identifier for the currently loaded theme.
    pub active_theme_id: String,
    /// Whether the runtime can show the theme picker (truecolor terminals only).
    pub theme_picker_available: bool,
}

impl SharedCtx {
    pub fn new(command_registry: Arc<Mutex<CommandRegistry>>, plugin_engine: Arc<PluginEngine>) -> Self {
        let provider_registry = Arc::new(
            ProviderRegistry::with_default_http(Arc::clone(&command_registry), Duration::from_secs(30)).expect("provider registry"),
        );
        let providers: Vec<Arc<dyn ValueProvider>> = vec![provider_registry.clone()];
        let history_store: Arc<dyn HistoryStore> = match JsonHistoryStore::with_defaults() {
            Ok(store) => Arc::new(store),
            Err(error) => {
                warn!(
                    error = %error,
                    "Failed to initialize history store at default path; falling back to in-memory history."
                );
                Arc::new(InMemoryHistoryStore::new())
            }
        };
        let preferences = Arc::new(UserPreferences::with_defaults().unwrap_or_else(|error| {
            warn!(
                error = %error,
                "Failed to load preferences from disk; falling back to ephemeral in-memory store."
            );
            UserPreferences::ephemeral()
        }));
        let preferred_theme = preferences.preferred_theme();
        let loaded_theme = theme::load(preferred_theme.as_deref());
        let theme_picker_available = theme::supports_theme_picker();

        Self {
            command_registry,
            providers,
            provider_registry: provider_registry.clone(),
            theme: loaded_theme.theme,
            plugin_engine,
            history_store,
            history_profile_id: DEFAULT_HISTORY_PROFILE.to_string(),
            preferences,
            active_theme_id: loaded_theme.definition.id.to_string(),
            theme_picker_available,
        }
    }
}

/// Wraps the event receiver for an in-flight workflow run.
#[derive(Debug)]
pub struct WorkflowRunEventReceiver {
    /// Active workflow run identifier associated with these events.
    pub run_id: String,
    /// Stream of workflow events emitted by the background runner.
    pub receiver: UnboundedReceiver<WorkflowRunEvent>,
}

impl WorkflowRunEventReceiver {
    pub fn new(run_id: String, receiver: UnboundedReceiver<WorkflowRunEvent>) -> Self {
        Self { run_id, receiver }
    }
}

pub struct App<'a> {
    /// Container focus flag for the top-level app focus scope
    app_container_focus: FocusFlag,
    /// Currently open modal kind (when Some, modal owns focus)
    pub open_modal_kind: Option<Modal>,
    /// Pending workflow run event receiver awaiting runtime registration.
    workflow_event_rx: Option<WorkflowRunEventReceiver>,
    /// Sequence counter for generating unique workflow run identifiers.
    workflow_run_sequence: u64,
    /// Shared, cross-cutting context (registry, config)
    pub ctx: SharedCtx,
    /// State for the command palette input
    pub palette: PaletteState,
    /// Command browser state
    pub browser: BrowserState,
    /// Table modal state
    pub table: ResultsTableState<'a>,
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
    /// Theme picker / appearance state
    pub theme_picker: ThemePickerState,
    /// Whether a command is currently executing
    pub executing: bool,
    /// Animation frame for the execution throbber
    pub throbber_idx: usize,
    /// Active execution count used by the event pump to decide whether to
    /// animate
    pub active_exec_count: Arc<AtomicUsize>,
    /// Global focus tree for keyboard/mouse traversal
    pub focus: Focus,
    /// Currently active main route for dynamic focus ring building
    pub current_route: Route,
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
        let ctx = SharedCtx::new(Arc::clone(&registry), engine);
        let palette = PaletteState::new(
            Arc::clone(&registry),
            Arc::clone(&ctx.history_store),
            ctx.history_profile_id.clone(),
        );
        let theme_picker_available = ctx.theme_picker_available;
        let mut app = Self {
            ctx,
            browser: BrowserState::new(Arc::clone(&registry)),
            logs: LogsState::default(),
            help: HelpState::default(),
            plugins: PluginsState::new(),
            workflows: WorkflowState::new(),
            table: ResultsTableState::default(),
            palette,
            nav_bar: VerticalNavBarState::defaults_for_views(theme_picker_available),
            theme_picker: ThemePickerState::default(),
            executing: false,
            throbber_idx: 0,
            active_exec_count: Arc::new(AtomicUsize::new(0)),
            focus: Focus::default(),
            app_container_focus: FocusFlag::new().with_name("app.container"),
            current_route: Route::Palette,
            open_modal_kind: None,
            workflow_event_rx: None,
            workflow_run_sequence: 0,
        };
        app.browser.update_browser_filtered();

        // Initialize rat-focus and set a sensible starting focus inside the palette
        app.focus = FocusBuilder::build_for(&app);
        app.focus.focus(&app.palette);
        app.theme_picker.set_active_theme(&app.ctx.active_theme_id);

        app
    }

    fn next_run_identifier(&mut self, workflow_identifier: &str) -> String {
        self.workflow_run_sequence = self.workflow_run_sequence.wrapping_add(1);
        format!("{}-{}", workflow_identifier, self.workflow_run_sequence)
    }

    /// Applies the theme selected inside the picker, rebuilds UI focus state, and persists the choice.
    pub fn apply_theme_selection(&mut self, theme_id: &str) {
        let Some(definition) = theme::catalog::find_by_id(theme_id) else {
            warn!(theme_id, "Unknown theme id requested; ignoring.");
            return;
        };

        self.ctx.theme = definition.build();
        self.ctx.active_theme_id = definition.id.to_string();
        self.theme_picker.set_active_theme(definition.id);
        if let Err(error) = self.ctx.preferences.set_preferred_theme(Some(definition.id.to_string())) {
            warn!(%error, "Failed to persist preferred theme selection");
        }
    }

    /// Registers a new workflow run event stream. Replaces any pending receiver.
    pub fn register_workflow_run_stream(&mut self, run_id: String, receiver: UnboundedReceiver<WorkflowRunEvent>) {
        self.workflow_event_rx = Some(WorkflowRunEventReceiver::new(run_id, receiver));
    }

    /// Extracts a pending workflow run event receiver for runtime registration.
    pub fn take_pending_workflow_events(&mut self) -> Option<WorkflowRunEventReceiver> {
        self.workflow_event_rx.take()
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
    pub fn update(&mut self, message: &Msg) -> Vec<Effect> {
        match message {
            Msg::Tick => self.handle_tick_message(),
            Msg::Resize(..) => vec![],
            Msg::CopyToClipboard(text) => vec![Effect::CopyToClipboardRequested(text.clone())],
            Msg::ProviderValuesReady { provider_id, cache_key } => {
                self.handle_provider_values_ready(provider_id.clone(), cache_key.clone())
            }
            Msg::WorkflowRunEvent { run_id, event } => self.process_workflow_run_event(run_id, event),
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
            return self.rebuild_palette_suggestions();
        }

        vec![]
    }

    fn effects_for_pending_fetches(&self, fetches: Vec<PendingProviderFetch>) -> Vec<Effect> {
        fetches
            .into_iter()
            .filter(|pending| pending.should_dispatch)
            .map(|pending| Effect::ProviderFetchRequested {
                provider_id: pending.plan.provider_id.clone(),
                cache_key: pending.plan.cache_key.clone(),
                args: pending.plan.args.clone(),
            })
            .collect()
    }

    pub fn rebuild_palette_suggestions(&mut self) -> Vec<Effect> {
        let fetches = self.palette.apply_build_suggestions(&self.ctx.providers, &*self.ctx.theme);
        self.effects_for_pending_fetches(fetches)
    }

    pub fn prepare_selector_fetch(&mut self) -> Vec<Effect> {
        let Some(selector) = self.workflows.collector_state_mut() else {
            return Vec::new();
        };

        match self
            .ctx
            .provider_registry
            .cached_values_or_plan(&selector.provider_id, selector.resolved_args.clone())
        {
            CacheLookupOutcome::Hit(items) => {
                selector.set_items(items);
                selector.refresh_table(&*self.ctx.theme);
                Vec::new()
            }
            CacheLookupOutcome::Pending(pending) => {
                selector.pending_cache_key = Some(pending.plan.cache_key.clone());
                selector.status = SelectorStatus::Loading;
                self.effects_for_pending_fetches(vec![pending])
            }
        }
    }

    fn handle_provider_values_ready(&mut self, provider_id: String, cache_key: String) -> Vec<Effect> {
        let mut effects = Vec::new();
        if self.palette.is_provider_loading() {
            effects.extend(self.rebuild_palette_suggestions());
        }

        if let Some(selector) = self.workflows.collector_state_mut() {
            let matches_identifier = selector.provider_id == provider_id;
            let matches_cache_key = selector.pending_cache_key.as_deref().is_none_or(|key| key == cache_key.as_str());
            if matches_identifier && matches_cache_key {
                match self
                    .ctx
                    .provider_registry
                    .cached_values_or_plan(&selector.provider_id, selector.resolved_args.clone())
                {
                    CacheLookupOutcome::Hit(items) => {
                        selector.set_items(items);
                        selector.refresh_table(&*self.ctx.theme);
                    }
                    CacheLookupOutcome::Pending(pending) => {
                        selector.pending_cache_key = Some(pending.plan.cache_key.clone());
                        effects.extend(self.effects_for_pending_fetches(vec![pending]));
                    }
                }
            }
        }

        effects
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

    /// Appends a plain-text message to the logs collections.
    ///
    /// This helper ensures both the flat string list and the rich log entries
    /// remain in sync so detail views can resolve JSON payloads accurately. It
    /// also enforces the maximum log retention window.
    ///
    /// # Arguments
    ///
    /// * `message` - The human-readable message to append to the logs.
    pub fn append_log_message(&mut self, message: impl Into<String>) {
        self.append_log_message_with_level(None, message);
    }

    /// Appends a plain-text message with an optional severity level.
    ///
    /// This variant is useful when callers want to preserve the originating log
    /// level for detail presentation.
    ///
    /// # Arguments
    ///
    /// * `level` - Optional severity level (for example, `"warn"`).
    /// * `message` - The human-readable message to append to the logs.
    pub fn append_log_message_with_level(&mut self, level: Option<String>, message: impl Into<String>) {
        let text = message.into();
        self.logs.append_text_entry_with_level(level, text);
        self.trim_logs_if_needed();
    }

    fn process_workflow_run_event(&mut self, run_id: &str, event: &WorkflowRunEvent) -> Vec<Effect> {
        let mut effects = Vec::new();
        let persist_history = matches!(
            event,
            WorkflowRunEvent::RunCompleted {
                status: WorkflowRunStatus::Succeeded,
                ..
            }
        );

        let log_messages = self.workflows.apply_run_event(run_id, event.clone(), &*self.ctx.theme);
        for message in log_messages {
            effects.push(Effect::Log(message));
        }

        if persist_history {
            self.persist_successful_workflow_run_history();
        }

        effects
    }

    fn persist_successful_workflow_run_history(&mut self) {
        let Some(run_state_rc) = &self.workflows.active_run_state else {
            return;
        };
        let run_state = run_state_rc.borrow();

        for (input_name, definition) in &run_state.workflow.inputs {
            if !workflow_input_uses_history(definition) {
                continue;
            }

            let Some(value) = run_state.run_context.inputs.get(input_name) else {
                continue;
            };

            if !has_meaningful_value(value) || value_contains_secret(value) {
                continue;
            }

            if let Some(validation) = &definition.validate
                && let Err(error) = validate_candidate_value(value, validation)
            {
                warn!(
                    input = %input_name,
                    workflow = %run_state.workflow.identifier,
                    error = %error,
                    "skipping history persistence for value failing validation"
                );
                continue;
            }

            let key = HistoryKey::workflow_input(
                self.ctx.history_profile_id.clone(),
                run_state.workflow.identifier.clone(),
                input_name.clone(),
            );

            if let Err(error) = self.ctx.history_store.insert_value(key, value.clone()) {
                warn!(
                    input = %input_name,
                    workflow = %run_state.workflow.identifier,
                    error = %error,
                    "failed to persist workflow history value"
                );
            }
        }
    }

    /// Requests execution of the currently active workflow run.
    ///
    pub fn run_active_workflow(&mut self) -> Vec<Effect> {
        if self.workflows.unresolved_item_count() > 0 {
            self.append_log_message("Cannot run workflow yet: resolve remaining inputs before running.");
            return Vec::new();
        }
        if self.workflows.active_run_state.is_none() {
            self.append_log_message("No active workflow run is available.");
            return Vec::new();
        }

        let run_state_rc = self.workflows.active_run_state.clone().unwrap();
        let run_state = run_state_rc.borrow();

        let display_name = run_state
            .workflow
            .title
            .as_deref()
            .filter(|title| !title.is_empty())
            .unwrap_or(&run_state.workflow.identifier)
            .to_string();

        let run_id = self.next_run_identifier(run_state.workflow.identifier.clone().as_str());
        let request = WorkflowRunRequest {
            run_id: run_id.clone(),
            workflow: run_state.workflow.clone(),
            inputs: run_state.run_context.inputs.clone(),
            environment: run_state.run_context.environment_variables.clone(),
            step_outputs: run_state.run_context.steps.clone(),
        };

        let mut run_view = RunViewState::new(
            run_id.clone(),
            run_state.workflow.identifier.clone(),
            run_state.workflow.title.clone(),
        );
        run_view.initialize_steps(&run_state.workflow.steps, &*self.ctx.theme);

        self.workflows.close_input_view();
        self.workflows.begin_run_session(run_id.clone(), run_state.clone(), run_view);

        self.append_log_message(format!("Workflow '{}' run started.", display_name));

        vec![
            Effect::WorkflowRunRequested {
                request: Box::new(request.clone()),
            },
            Effect::SwitchTo(Route::WorkflowRun),
        ]
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
                    if let Some(state) = self.workflows.collector.as_ref() {
                        builder.widget(state);
                    } else if let Some(state) = self.workflows.manual_entry.as_ref() {
                        builder.widget(state);
                    }
                }
                Modal::PluginDetails | Modal::Help | Modal::ThemePicker => {
                    // focusable fields TBD; leave the ring empty
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
            }
            Route::Browser => {
                builder.widget(&self.browser);
            }
            Route::Plugins => {
                builder.widget(&self.plugins);
            }
            Route::Workflows | Route::WorkflowInputs | Route::WorkflowRun => {
                builder.widget(&self.workflows);
            }
        }
        if self.logs.is_visible {
            builder.widget(&self.logs);
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
