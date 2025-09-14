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
use heroku_types::{ExecOutcome, Screen};
use rat_focus::FocusBuilder;
use serde_json::{Map as JsonMap, Value as JsonValue};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    start_palette_execution,
    ui::{
        components::{
            browser::BrowserState,
            help::HelpState,
            logs::{LogsState, state::LogEntry},
            palette::{PaletteState, providers::RegistryBackedProvider, state::ValueProvider},
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
    /// Current primary route
    pub route: Screen,
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
    /// Whether Plugins view is in full-screen mode (replaces palette UI)
    pub plugins_fullscreen: bool,
    // moved to ctx: dry_run, debug_enabled, providers
    /// Whether a command is currently executing
    pub executing: bool,
    /// Animation frame for the execution throbber
    pub throbber_idx: usize,
    /// Sender for async execution results
    pub exec_sender: UnboundedSender<ExecOutcome>,
    /// Receiver for async execution results
    pub exec_receiver: UnboundedReceiver<ExecOutcome>,
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
    /// Internal dirty flag to indicate UI should re-render
    dirty: bool,
}

impl<'a> App<'a> {
    /// Gets the available range fields for the currently selected command
    pub fn available_ranges(&self) -> Vec<String> {
        if let Some(r) = &self.last_command_ranges
            && !r.is_empty()
        {
            return r.clone();
        }
        self.browser.available_ranges()
    }
}

impl App<'_> {
    /// Marks the application state as changed, signaling a redraw is needed.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Returns whether the application is currently marked dirty.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Returns the dirty state and resets it to clean in one call.
    pub fn take_dirty(&mut self) -> bool {
        let was_dirty = self.dirty;
        self.dirty = false;
        was_dirty
    }
}

/// Messages that can be sent to update the application state.
///
/// This enum defines all the possible user actions and system events
/// that can trigger state changes in the application.
#[derive(Debug, Clone)]
pub enum Msg {
    /// Toggle the help modal visibility
    ToggleHelp,
    /// Toggle the table modal visibility
    ToggleTable,
    /// Toggle the plugins overlay visibility
    TogglePlugins,
    /// Toggle the command browser visibility
    ToggleBuilder,
    /// Close any currently open modal
    CloseModal,
    /// Execute the current command
    Run,
    /// Copy the current command to clipboard
    CopyCommand,
    /// Periodic UI tick (e.g., throbbers)
    Tick,
    /// Terminal resized
    Resize(u16, u16),
    /// Background execution completed with outcome
    ExecCompleted(ExecOutcome),
    // Logs interactions
    /// Move log selection cursor up
    LogsUp,
    /// Move log selection cursor down
    LogsDown,
    /// Extend selection upwards (Shift+Up)
    LogsExtendUp,
    /// Extend selection downwards (Shift+Down)
    LogsExtendDown,
    /// Open details for the current selection
    LogsOpenDetail,
    /// Close details view and return to list
    LogsCloseDetail,
    /// Copy current selection (redacted)
    LogsCopy,
    /// Toggle pretty/raw for single API response
    LogsTogglePretty,
}

/// Side effects that can be triggered by state changes.
///
/// This enum defines actions that should be performed as a result
/// of state changes, such as copying to clipboard or showing notifications.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum Effect {
    /// Request to copy the current command to clipboard
    CopyCommandRequested,
    /// Request to copy the current logs selection (already rendered/redacted)
    CopyLogsRequested(String),
    /// Request the next page using the Raw Next-Range header
    NextPageRequested(String),
    /// Request the previous page using the prior Range header, if any
    PrevPageRequested,
    /// Request the first page using the initial Range header (or none)
    FirstPageRequested,
    /// Load MCP plugins from config into PluginsState
    PluginsLoadRequested,
    /// Refresh plugin statuses/health
    PluginsRefresh,
    /// Start the selected plugin
    PluginsStart(String),
    /// Stop the selected plugin
    PluginsStop(String),
    /// Restart the selected plugin
    PluginsRestart(String),
    /// Open logs drawer for a plugin
    PluginsOpenLogs(String),
    /// Refresh logs for open logs drawer (follow mode)
    PluginsRefreshLogs(String),
    /// Export logs for a plugin to a default location (redacted)
    PluginsExportLogsDefault(String),
    /// Open environment editor for a plugin
    PluginsOpenSecrets(String),
    /// Save environment changes for a plugin (key/value pairs)
    PluginsSaveEnv { name: String, rows: Vec<(String, String)> },
    /// Open add plugin view
    PluginsOpenAdd,
    /// Open the secrets view
    PluginsValidateAdd,
    /// Apply add plugin patch
    PluginsApplyAdd,
    // Cancel adding a new plugin
    PluginsCancel,
}

// Legacy MainFocus removed; focus is handled via ui::focus::FocusStore

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
        let (exec_sender, exec_receiver) = unbounded_channel();
        let mut app = Self {
            route: Screen::default(),
            ctx: SharedCtx::new(registry),
            browser: BrowserState::default(),
            logs: LogsState::default(),
            plugins_fullscreen: false,
            help: HelpState::default(),
            plugins: PluginsState::new(),
            table: TableState::default(),
            palette: PaletteState::default(),
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
            dirty: false,
        };
        app.browser.set_all_commands(app.ctx.registry.commands.clone());
        app.palette.set_all_commands(app.ctx.registry.commands.clone());
        app.browser.update_browser_filtered();
        // Initialize rat-focus: start with the palette focused at root
        {
            let mut focus_builder = FocusBuilder::new(None);
            focus_builder.widget(&app.palette);
            focus_builder.widget(&app.logs);
            // Plugins overlay will manage its own focus when opened.
            let f = focus_builder.build();
            f.focus(&app.palette);
        }
        app
    }

    /// Updates the application state based on a message.
    ///
    /// This method processes messages and updates the application state
    /// accordingly. It handles user interactions, navigation, and state
    /// changes.
    ///
    /// # Arguments
    ///
    /// * `msg` - The message to process
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
    pub fn update(&mut self, msg: Msg) -> Vec<Effect> {
        let mut effects = Vec::new();
        match msg {
            Msg::Tick => {
                // Animate spinner while executing or while provider-backed suggestions are loading
                if self.executing || self.palette.is_provider_loading() {
                    let before = self.throbber_idx;
                    self.throbber_idx = (self.throbber_idx + 1) % 10;
                    if self.throbber_idx != before {
                        self.mark_dirty();
                    }
                }
                // Periodically refresh plugin statuses when overlay is visible.
                if self.plugins.is_visible() && self.plugins.should_refresh() {
                    effects.push(Effect::PluginsRefresh);
                }
                // Refresh logs in follow mode
                if let Some(logs) = &self.plugins.logs {
                    if logs.follow {
                        effects.push(Effect::PluginsRefreshLogs(logs.name.clone()));
                    }
                }
                // If provider-backed suggestions are loading and the popup is open,
                // rebuild suggestions to pick up newly cached results without requiring
                // another keypress.
                if self.palette.is_suggestions_open() && self.palette.is_provider_loading() {
                    let SharedCtx {
                        registry, providers, ..
                    } = &self.ctx;
                    self.palette
                        .apply_build_suggestions(registry, providers, &*self.ctx.theme);
                    // Suggestions UI likely changed (new results); request redraw
                    self.mark_dirty();
                }
            }
            Msg::Resize(..) => {
                // No-op for now; placeholder to enable TEA-style event
                self.mark_dirty();
            }
            Msg::ToggleHelp => {
                let spec = if self.browser.is_visible() {
                    self.browser.selected_command()
                } else {
                    self.palette.selected_command()
                };
                self.help.toggle_visibility(spec.cloned());
                self.mark_dirty();
            }
            Msg::ToggleTable => {
                self.table.toggle_show();
                self.mark_dirty();
            }
            Msg::TogglePlugins => {
                let now_visible = !self.plugins.is_visible();
                self.plugins.set_visible(now_visible);
                // Focus normalization: when opened, default to search
                if now_visible {
                    let mut focus_builder = FocusBuilder::new(None);
                    focus_builder.widget(&self.plugins);
                    let f = focus_builder.build();
                    f.focus(&self.plugins.search_flag);
                }
                self.mark_dirty();
            }
            Msg::ToggleBuilder => {
                self.browser.toggle_visibility();
                if self.browser.is_visible() {
                    self.browser.normalize_focus();
                }
                self.mark_dirty();
            }
            Msg::CloseModal => {
                self.help.set_visibility(false);
                self.table.apply_visible(false);
                self.browser.apply_visibility(false);
                self.plugins.set_visible(false);
                self.mark_dirty();
            }
            Msg::Run => {
                // always execute from palette
                if !self.palette.is_input_empty() {
                    match start_palette_execution(self) {
                        // Execution started successfully
                        Ok(_) => {
                            let input = &self.palette.input();
                            self.logs.entries.push(format!("Running: {}", input));
                            self.logs.rich_entries.push(LogEntry::Text {
                                level: Some("info".into()),
                                msg: format!("Running: {}", input),
                            });
                            self.mark_dirty();
                        }
                        Err(e) => {
                            self.palette.apply_error(e);
                            self.mark_dirty();
                        }
                    }
                }
            }
            Msg::CopyCommand => {
                effects.push(Effect::CopyCommandRequested);
            }
            Msg::ExecCompleted(out) => {
                let raw = out.log;
                // Keep executing=true if other executions are still active
                let still_active = self.active_exec_count.load(Ordering::Relaxed) > 0;
                self.executing = still_active;
                // If this is a plugins refresh payload, apply it and short-circuit other UI updates
                if let Some(ref json) = out.result_json {
                    if let Some(refresh) = json.get("plugins_refresh").and_then(|v| v.as_array()) {
                        let mut updates = Vec::new();
                        for v in refresh {
                            if let (Some(name), Some(status)) = (
                                v.get("name").and_then(|s| s.as_str()),
                                v.get("status").and_then(|s| s.as_str()),
                            ) {
                                let latency = v.get("latency_ms").and_then(|l| l.as_u64());
                                let last_error = v.get("last_error").and_then(|e| e.as_str()).map(|s| s.to_string());
                                updates.push((name.to_string(), status.to_string(), latency, last_error));
                            }
                        }
                        self.plugins.apply_refresh_updates(updates);
                        self.mark_dirty();
                        // Also log, redacted
                        self.logs.entries.push(heroku_util::redact_sensitive(&raw));
                        return effects;
                    }
                    if let Some(plogs) = json.get("plugins_logs").and_then(|v| v.as_object()) {
                        if let (Some(name), Some(lines)) = (
                            plogs.get("name").and_then(|s| s.as_str()),
                            plogs.get("lines").and_then(|l| l.as_array()),
                        ) {
                            if let Some(logs_state) = &mut self.plugins.logs {
                                if logs_state.name == name {
                                    let mut out_lines = Vec::with_capacity(lines.len());
                                    for l in lines {
                                        if let Some(s) = l.as_str() {
                                            out_lines.push(s.to_string());
                                        }
                                    }
                                    logs_state.set_lines(out_lines);
                                    self.mark_dirty();
                                    self.logs.entries.push(heroku_util::redact_sensitive(&raw));
                                    return effects;
                                }
                            }
                        }
                    }
                    if let Some(penv) = json.get("plugins_env").and_then(|v| v.as_object()) {
                        if let (Some(name), Some(rows)) = (
                            penv.get("name").and_then(|s| s.as_str()),
                            penv.get("rows").and_then(|l| l.as_array()),
                        ) {
                            if let Some(env_state) = &mut self.plugins.env {
                                if env_state.name == name {
                                    let mut out_rows = Vec::with_capacity(rows.len());
                                    for r in rows {
                                        if let (Some(k), Some(v), Some(is_secret)) = (
                                            r.get("key").and_then(|s| s.as_str()),
                                            r.get("value").and_then(|s| s.as_str()),
                                            r.get("is_secret").and_then(|b| b.as_bool()),
                                        ) {
                                            out_rows.push(crate::ui::components::plugins::EnvRow {
                                                key: k.to_string(),
                                                value: v.to_string(),
                                                is_secret,
                                            });
                                        }
                                    }
                                    env_state.set_rows(out_rows);
                                    self.mark_dirty();
                                    self.logs.entries.push(heroku_util::redact_sensitive(&raw));
                                    return effects;
                                }
                            } else {
                                // If env not open yet, open and set rows
                                self.plugins.open_secrets(name.to_string());
                                if let Some(env_state) = &mut self.plugins.env {
                                    let mut out_rows = Vec::new();
                                    for r in rows {
                                        if let (Some(k), Some(v), Some(is_secret)) = (
                                            r.get("key").and_then(|s| s.as_str()),
                                            r.get("value").and_then(|s| s.as_str()),
                                            r.get("is_secret").and_then(|b| b.as_bool()),
                                        ) {
                                            out_rows.push(crate::ui::components::plugins::EnvRow {
                                                key: k.to_string(),
                                                value: v.to_string(),
                                                is_secret,
                                            });
                                        }
                                    }
                                    env_state.set_rows(out_rows);
                                    self.mark_dirty();
                                    self.logs.entries.push(heroku_util::redact_sensitive(&raw));
                                    return effects;
                                }
                            }
                        }
                    }
                    if let Some(preview) = json.get("plugins_add_preview").and_then(|v| v.as_object()) {
                        if let Some(wiz) = &mut self.plugins.add {
                            wiz.validation = preview.get("message").and_then(|s| s.as_str()).map(|s| s.to_string());
                            wiz.preview = preview.get("patch").and_then(|s| s.as_str()).map(|s| s.to_string());
                            self.mark_dirty();
                            self.logs.entries.push(heroku_util::redact_sensitive(&raw));
                            return effects;
                        }
                    }
                }
                // Pre-redact for list display to avoid per-frame redaction
                self.logs.entries.push(heroku_util::redact_sensitive(&raw));
                self.logs.rich_entries.push(LogEntry::Api {
                    status: 0,
                    raw,
                    json: out.result_json.clone(),
                });
                let log_len = self.logs.entries.len();
                if log_len > 500 {
                    let _ = self.logs.entries.drain(0..log_len - 500);
                }
                let rich_len = self.logs.rich_entries.len();
                if rich_len > 500 {
                    let _ = self.logs.rich_entries.drain(0..rich_len - 500);
                }
                self.table.apply_result_json(out.result_json, &*self.ctx.theme);
                self.table.apply_visible(out.open_table);
                // Update last seen pagination info for the table/pagination component
                self.last_pagination = out.pagination;
                // Clear palette input and suggestion state
                self.palette.reduce_clear_all();
                self.mark_dirty();
            }
            // Placeholder handlers for upcoming logs features
            Msg::LogsUp | Msg::LogsDown | Msg::LogsExtendUp | Msg::LogsExtendDown => {}
            Msg::LogsOpenDetail | Msg::LogsCloseDetail | Msg::LogsCopy | Msg::LogsTogglePretty => {}
        }
        effects
    }
}
