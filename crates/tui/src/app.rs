//! Application state and logic for the Heroku TUI.
//!
//! This module contains the main application state, data structures, and
//! business logic for the TUI interface. It manages the application lifecycle,
//! user interactions, and coordinates between different UI components.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use heroku_mcp::client::McpClientManager;
use heroku_registry::Registry;
use heroku_types::{Effect, ExecOutcome, Modal, Msg, Route};
use rat_focus::{Focus, FocusBuilder, FocusFlag, HasFocus};
use serde_json::{Map as JsonMap, Value as JsonValue};
use tokio::sync::mpsc::{Receiver, Sender, channel};

use crate::ui::components::{
    BrowserComponent, HelpComponent, PluginsComponent, TableComponent, logs::LogDetailsComponent,
    nav_bar::VerticalNavBarState, plugins::PluginsSecretsComponent,
};
use crate::{
    start_palette_execution,
    ui::{
        components::{
            browser::BrowserState,
            component::Component,
            help::HelpState,
            logs::{LogsState, state::LogEntry},
            palette::{PaletteComponent, PaletteState, providers::RegistryBackedProvider, state::ValueProvider},
            plugins::PluginsState,
            table::TableState,
        },
        theme,
    },
};

/// Cross-cutting shared context owned by the App.
///
/// Holds runtime-wide objects like the command registry and configuration
/// flags. This avoids threading multiple references through components and
/// helps reduce borrow complexity.
pub struct SharedCtx {
    /// Global Heroku command registry
    pub registry: Registry,
    /// Global debug flag (from env)
    pub debug_enabled: bool,
    /// Value providers for suggestions
    pub providers: Vec<Box<dyn ValueProvider>>,
    /// Active UI theme (Dracula by default) loaded from env
    pub theme: Box<dyn theme::Theme>,
    /// MCP supervisor for plugins (optional until initialized)
    pub mcp: Option<Arc<McpClientManager>>,
}

impl SharedCtx {
    pub fn new(registry: Registry) -> Self {
        let debug_enabled = std::env::var("DEBUG")
            .map(|v| !v.is_empty() && v != "0" && v.to_lowercase() != "false")
            .unwrap_or(false);
        // Add registry-backed provider with a small TTL cache
        let providers: Vec<Box<dyn ValueProvider>> = vec![Box::new(RegistryBackedProvider::new(
            std::sync::Arc::new(registry.clone()),
            std::time::Duration::from_secs(45),
        ))];
        Self {
            registry,
            debug_enabled,
            providers,
            theme: theme::load_from_env(),
            mcp: None,
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
    /// Plugins overlay state (MCP management)
    pub plugins: PluginsState,
    /// Application logs and status messages
    pub logs: LogsState,
    /// Vertical navigation bar state (left rail)
    pub nav_bar: VerticalNavBarState,
    // moved to ctx: dry_run, debug_enabled, providers
    /// Whether a command is currently executing
    pub executing: bool,
    /// Animation frame for the execution throbber
    pub throbber_idx: usize,
    /// Sender for async execution results
    pub exec_sender: Sender<ExecOutcome>,
    /// Receiver for async execution results
    pub exec_receiver: Receiver<ExecOutcome>,
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
    /// A new App instance with initialized state.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Requires constructing a full Registry and App; ignored in doctests.
    /// ```
    pub fn new(registry: heroku_registry::Registry) -> Self {
        let (exec_sender, exec_receiver) = channel(25);
        let mut application = Self {
            ctx: SharedCtx::new(registry),
            browser: BrowserState::default(),
            logs: LogsState::default(),
            help: HelpState::default(),
            plugins: PluginsState::new(),
            table: TableState::default(),
            palette: PaletteState::default(),
            nav_bar: VerticalNavBarState::defaults_for_views(),
            executing: false,
            throbber_idx: 0,
            exec_sender,
            exec_receiver,
            active_exec_count: Arc::new(AtomicUsize::new(0)),
            last_pagination: None,
            last_command_ranges: None,
            last_spec: None,
            last_body: None,
            pagination_history: Vec::new(),
            initial_range: None,
            main_view: Some(Box::new(PaletteComponent::default())),
            open_modal: None,
            focus: Focus::default(),
            transient_focus_id: None,
            app_container_focus: FocusFlag::named("app.container"),
            current_route: Route::Palette,
            open_modal_kind: None,
        };

        // Initialize command browser and palette with all available commands
        application
            .browser
            .set_all_commands(application.ctx.registry.commands.clone());
        application
            .palette
            .set_all_commands(application.ctx.registry.commands.clone());
        application.browser.update_browser_filtered();

        // Initialize rat-focus and set a sensible starting focus inside palette
        application.focus = FocusBuilder::build_for(&application);
        application.focus.first_in(&application.palette);

        application
    }

    /// Gets the available range fields for the currently selected command.
    ///
    /// This method returns the range fields that are available for the current
    /// command, either from the last executed command or from the browser's
    /// current selection.
    ///
    /// # Returns
    ///
    /// A vector of available range field names.
    pub fn available_ranges(&self) -> Vec<String> {
        if let Some(last_ranges) = &self.last_command_ranges
            && !last_ranges.is_empty()
        {
            return last_ranges.clone();
        }
        self.browser.available_ranges()
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
        let mut effects = Vec::new();
        match message {
            Msg::Tick => {
                self.handle_tick_message(&mut effects);
            }
            Msg::Resize(..) => {
                // No-op for now; placeholder to enable TEA-style event
            }
            Msg::Run => {
                // Always execute from palette
                if !self.palette.is_input_empty() {
                    match start_palette_execution(self) {
                        // Execution started successfully
                        Ok(_) => {
                            let command_input = &self.palette.input();
                            self.logs.entries.push(format!("Running: {}", command_input));
                            self.logs.rich_entries.push(LogEntry::Text {
                                level: Some("info".into()),
                                msg: format!("Running: {}", command_input),
                            });
                        }
                        Err(execution_error) => {
                            self.palette.apply_error(execution_error);
                        }
                    }
                }
            }
            Msg::CopyToClipboard(text) => {
                effects.push(Effect::CopyToClipboardRequested(text));
            }
            Msg::ExecCompleted(execution_outcome) => {
                if self.handle_execution_completion(execution_outcome, &mut effects) {
                    return effects;
                }
            }
            // Placeholder handlers for upcoming logs features
            Msg::LogsUp | Msg::LogsDown | Msg::LogsExtendUp | Msg::LogsExtendDown => {}
            Msg::LogsOpenDetail | Msg::LogsCloseDetail | Msg::LogsCopy | Msg::LogsTogglePretty => {}
        }
        effects
    }

    /// Handles tick messages for periodic updates and animations.
    ///
    /// This method manages periodic tasks such as animating the execution
    /// throbber, refreshing plugin statuses, updating logs in follow mode,
    /// and rebuilding suggestions when provider-backed results are available.
    ///
    /// # Arguments
    ///
    /// * `effects` - Mutable reference to the effects vector to add new effects
    fn handle_tick_message(&mut self, effects: &mut Vec<Effect>) {
        // Animate spinner while executing or while provider-backed suggestions are loading
        if self.executing || self.palette.is_provider_loading() {
            let previous_throbber_index = self.throbber_idx;
            self.throbber_idx = (self.throbber_idx + 1) % 10;
            if self.throbber_idx != previous_throbber_index {}
        }

        // Periodically refresh plugin statuses when overlay is visible
        if self.plugins.should_refresh() {
            effects.push(Effect::PluginsRefresh);
        }

        // Refresh logs in follow mode
        if let Some(logs_state) = &self.plugins.logs {
            if logs_state.follow {
                effects.push(Effect::PluginsRefreshLogs(logs_state.name.clone()));
            }
        }

        // If provider-backed suggestions are loading and the popup is open,
        // rebuild suggestions to pick up newly cached results without requiring
        // another keypress
        if self.palette.is_suggestions_open() && self.palette.is_provider_loading() {
            let SharedCtx {
                registry, providers, ..
            } = &self.ctx;
            self.palette
                .apply_build_suggestions(registry, providers, &*self.ctx.theme);
            // Suggestions UI likely changed (new results); request redraw
        }
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
    /// * `effects` - Mutable reference to the effects vector to add new effects
    ///
    /// # Returns
    ///
    /// Returns `true` if the execution was handled as a special case (plugin response)
    /// and the caller should return early, `false` if normal processing should continue.
    fn handle_execution_completion(&mut self, execution_outcome: ExecOutcome, effects: &mut Vec<Effect>) -> bool {
        // Keep executing=true if other executions are still active
        let still_executing = self.active_exec_count.load(Ordering::Relaxed) > 0;
        self.executing = still_executing;

        // Handle special plugin responses first
        if let Some(ref result_json) = execution_outcome.result_json {
            if self.handle_plugin_refresh_response(result_json, &execution_outcome.log) {
                return true;
            }
            if self.handle_plugin_logs_response(result_json, &execution_outcome.log) {
                return true;
            }
            if self.handle_plugin_secrets_response(result_json, &execution_outcome.log) {
                effects.push(Effect::ShowModal(Modal::Secrets));
                return true;
            }
            if self.handle_plugin_add_preview_response(result_json, &execution_outcome.log) {
                return true;
            }
        }

        // Handle general command results
        self.process_general_execution_result(execution_outcome);
        effects.push(Effect::ShowModal(Modal::Results)); // Show the data from the execution outcome

        false
    }

    /// Handles plugin refresh responses from command execution.
    ///
    /// # Arguments
    ///
    /// * `result_json` - The JSON result from the execution
    /// * `raw_log` - The raw log output for redaction
    ///
    /// # Returns
    ///
    /// Returns `true` if this was a plugin refresh response, `false` otherwise.
    fn handle_plugin_refresh_response(&mut self, result_json: &JsonValue, raw_log: &str) -> bool {
        if let Some(refresh_array) = result_json.get("plugins_refresh").and_then(|v| v.as_array()) {
            let mut plugin_updates = Vec::new();
            for plugin_data in refresh_array {
                if let (Some(name), Some(status)) = (
                    plugin_data.get("name").and_then(|s| s.as_str()),
                    plugin_data.get("status").and_then(|s| s.as_str()),
                ) {
                    let latency = plugin_data.get("latency_ms").and_then(|l| l.as_u64());
                    let last_error = plugin_data
                        .get("last_error")
                        .and_then(|e| e.as_str())
                        .map(|s| s.to_string());
                    plugin_updates.push((name.to_string(), status.to_string(), latency, last_error));
                }
            }
            self.plugins.apply_refresh_updates(plugin_updates);

            // Also log, redacted
            let (summary, _) = summarize_execution_outcome(self.last_spec.as_ref(), raw_log);
            self.logs.entries.push(summary);
            return true;
        }
        false
    }

    /// Handles plugin logs responses from command execution.
    ///
    /// # Arguments
    ///
    /// * `result_json` - The JSON result from the execution
    /// * `raw_log` - The raw log output for redaction
    ///
    /// # Returns
    ///
    /// Returns `true` if this was a plugin logs response, `false` otherwise.
    fn handle_plugin_logs_response(&mut self, result_json: &JsonValue, raw_log: &str) -> bool {
        if let Some(plugin_logs) = result_json.get("plugins_logs").and_then(|v| v.as_object()) {
            if let (Some(plugin_name), Some(log_lines)) = (
                plugin_logs.get("name").and_then(|s| s.as_str()),
                plugin_logs.get("lines").and_then(|l| l.as_array()),
            ) {
                if let Some(logs_state) = &mut self.plugins.logs {
                    if logs_state.name == plugin_name {
                        let mut output_lines = Vec::with_capacity(log_lines.len());
                        for line in log_lines {
                            if let Some(line_str) = line.as_str() {
                                output_lines.push(line_str.to_string());
                            }
                        }
                        logs_state.set_lines(output_lines);

                        let (summary, _) = summarize_execution_outcome(self.last_spec.as_ref(), raw_log);
                        self.logs.entries.push(summary);
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Handles plugin environment responses from command execution.
    ///
    /// # Arguments
    ///
    /// * `result_json` - The JSON result from the execution
    /// * `raw_log` - The raw log output for redaction
    ///
    /// # Returns
    ///
    /// Returns `true` if this was a plugin environment response, `false` otherwise.
    fn handle_plugin_secrets_response(&mut self, result_json: &JsonValue, raw_log: &str) -> bool {
        if let Some(plugin_env) = result_json.get("plugins_env").and_then(|v| v.as_object()) {
            if let (Some(plugin_name), Some(env_rows)) = (
                plugin_env.get("name").and_then(|s| s.as_str()),
                plugin_env.get("rows").and_then(|l| l.as_array()),
            ) {
                if let Some(environment_state) = &mut self.plugins.secrets {
                    if environment_state.name == plugin_name {
                        let mut output_rows = Vec::with_capacity(env_rows.len());
                        for row in env_rows {
                            if let (Some(key), Some(value), Some(is_secret)) = (
                                row.get("key").and_then(|s| s.as_str()),
                                row.get("value").and_then(|s| s.as_str()),
                                row.get("is_secret").and_then(|b| b.as_bool()),
                            ) {
                                output_rows.push(crate::ui::components::plugins::EnvRow {
                                    key: key.to_string(),
                                    value: value.to_string(),
                                    is_secret,
                                });
                            }
                        }
                        environment_state.set_rows(output_rows);

                        let (summary, _) = summarize_execution_outcome(self.last_spec.as_ref(), raw_log);
                        self.logs.entries.push(summary);
                        return true;
                    }
                } else {
                    // If environment editor not open yet, open and set rows
                    self.plugins.open_secrets(plugin_name.to_string());
                    if let Some(environment_state) = &mut self.plugins.secrets {
                        let mut output_rows = Vec::new();
                        for row in env_rows {
                            if let (Some(key), Some(value), Some(is_secret)) = (
                                row.get("key").and_then(|s| s.as_str()),
                                row.get("value").and_then(|s| s.as_str()),
                                row.get("is_secret").and_then(|b| b.as_bool()),
                            ) {
                                output_rows.push(crate::ui::components::plugins::EnvRow {
                                    key: key.to_string(),
                                    value: value.to_string(),
                                    is_secret,
                                });
                            }
                        }
                        environment_state.set_rows(output_rows);

                        let (summary, _) = summarize_execution_outcome(self.last_spec.as_ref(), raw_log);
                        self.logs.entries.push(summary);
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Handles plugin add preview responses from command execution.
    ///
    /// # Arguments
    ///
    /// * `result_json` - The JSON result from the execution
    /// * `raw_log` - The raw log output for redaction
    ///
    /// # Returns
    ///
    /// Returns `true` if this was a plugin add preview response, `false` otherwise.
    fn handle_plugin_add_preview_response(&mut self, result_json: &JsonValue, raw_log: &str) -> bool {
        if let Some(preview_data) = result_json.get("plugins_add_preview").and_then(|v| v.as_object()) {
            if let Some(add_plugin) = &mut self.plugins.add {
                add_plugin.validation = preview_data
                    .get("message")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string());
                add_plugin.preview = preview_data
                    .get("patch")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string());

                let (summary, _) = summarize_execution_outcome(self.last_spec.as_ref(), raw_log);
                self.logs.entries.push(summary);
                return true;
            }
        }
        false
    }

    /// Processes general command execution results (non-plugin specific).
    ///
    /// This method handles the standard processing of command results including
    /// logging, table updates, and pagination information.
    ///
    /// # Arguments
    ///
    /// * `execution_outcome` - The result of the command execution
    fn process_general_execution_result(&mut self, execution_outcome: ExecOutcome) {
        let ExecOutcome {
            log: raw_log,
            result_json,
            open_table: _,
            pagination,
        } = execution_outcome;

        let (summary, status_code) = summarize_execution_outcome(self.last_spec.as_ref(), &raw_log);

        self.logs.entries.push(summary);
        self.logs.rich_entries.push(LogEntry::Api {
            status: status_code.unwrap_or(0),
            raw: raw_log,
            json: result_json.clone(),
        });

        self.trim_logs_if_needed();

        self.table.apply_result_json(result_json, &*self.ctx.theme);
        self.table.normalize();

        self.last_pagination = pagination;

        self.palette.reduce_clear_all();
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

    /// Update the current main route for focus building.
    pub fn set_current_route(&mut self, route: Route) {
        let (view, state): (Box<dyn Component>, Box<&dyn HasFocus>) = match route {
            Route::Browser => (Box::new(BrowserComponent::default()), Box::new(&self.browser)),
            Route::Palette => (Box::new(PaletteComponent::default()), Box::new(&self.palette)),
            Route::Plugins => (Box::new(PluginsComponent::default()), Box::new(&self.plugins)),
        };
        self.current_route = route;
        self.main_view = Some(view);
        self.focus = rat_focus::FocusBuilder::build_for(self);
        self.focus.first_in(*state);
    }

    /// Update the open modal kind (use None to clear).
    pub fn set_open_modal_kind(&mut self, modal: Option<Modal>) {
        if let Some(modal_kind) = modal.clone() {
            let modal_view: Box<dyn Component> = match modal_kind {
                Modal::Help => Box::new(HelpComponent::default()),
                Modal::Results => Box::new(TableComponent::default()),
                Modal::Secrets => Box::new(PluginsSecretsComponent::default()),
                Modal::LogDetails => Box::new(LogDetailsComponent::default()),
            };
            self.open_modal = Some(modal_view);
            // save the current focus to restore when the modal is closed
            self.transient_focus_id = self.focus.focused().and_then(|f| Some(f.widget_id()));
        } else {
            self.open_modal = None;
        }
        self.open_modal_kind = modal;
    }

    /// Enqueue an [`ExecOutcome`] for asynchronous processing by the runtime loop.
    ///
    /// This helper wraps the Tokio channel used to ferry execution results
    /// back into the application. When the channel is full or closed we record
    /// a log entry so the user can diagnose why results might be missing.
    pub fn enqueue_exec_outcome(&mut self, outcome: ExecOutcome) {
        if let Err(error) = self.exec_sender.try_send(outcome) {
            self.logs
                .entries
                .push(format!("Failed to enqueue execution outcome: {error}"));
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

fn summarize_execution_outcome(
    command_spec: Option<&heroku_registry::CommandSpec>,
    raw_log: &str,
) -> (String, Option<u16>) {
    let label = command_label(command_spec);
    let trimmed_log = raw_log.trim();

    if let Some(error_message) = trimmed_log.strip_prefix("Error:") {
        let redacted = heroku_util::redact_sensitive(error_message.trim());
        let truncated = truncate_for_summary(&redacted, EXECUTION_SUMMARY_LIMIT);
        let summary = format!("{} - failed: {}", label, truncated);
        return (summary, None);
    }

    let status_line = trimmed_log.lines().next().unwrap_or_default().trim();
    let status_code = status_line
        .split_whitespace()
        .next()
        .and_then(|code| code.parse::<u16>().ok());

    let summary = if status_line.is_empty() {
        format!("{} - succeeded", label)
    } else {
        let sanitized_status = heroku_util::redact_sensitive(status_line);
        format!("{} - succeeded ({})", label, sanitized_status)
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
    use heroku_types::ServiceId;

    fn sample_spec() -> CommandSpec {
        CommandSpec {
            group: "apps".to_string(),
            name: "info".to_string(),
            summary: String::new(),
            positional_args: Vec::new(),
            flags: Vec::new(),
            method: "GET".to_string(),
            path: "/apps".to_string(),
            ranges: Vec::new(),
            service_id: ServiceId::CoreApi,
        }
    }

    #[test]
    fn summarize_success_includes_status_code() {
        let spec = sample_spec();
        let (summary, status) = summarize_execution_outcome(Some(&spec), "200 OK\n{\"foo\":\"bar\"}");

        assert_eq!(summary, "apps info - succeeded (200 OK)");
        assert_eq!(status, Some(200));
    }

    #[test]
    fn summarize_error_marks_failure_and_truncates() {
        let spec = sample_spec();
        let long_error = format!(
            "Error: {}",
            "SensitiveToken123".repeat((EXECUTION_SUMMARY_LIMIT / 5) + 10)
        );

        let (summary, status) = summarize_execution_outcome(Some(&spec), &long_error);

        assert!(summary.starts_with("apps info - failed: "));
        assert!(summary.ends_with("..."));
        assert_eq!(status, None);
    }

    #[test]
    fn summarize_without_spec_uses_generic_label() {
        let (summary, status) = summarize_execution_outcome(None, "200 OK\n{}");

        assert_eq!(summary, "Command - succeeded (200 OK)");
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
                Modal::Secrets => {
                    builder.widget(&self.plugins);
                }
                Modal::LogDetails => {
                    builder.widget(&self.logs);
                }
                Modal::Help => {
                    // Help has no focusable fields; leave ring empty
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
        }

        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.app_container_focus.clone()
    }

    fn area(&self) -> ratatui::layout::Rect {
        ratatui::layout::Rect::default()
    }
}
