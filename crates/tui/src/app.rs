//! Application state and logic for the Heroku TUI.
//!
//! This module contains the main application state, data structures, and business
//! logic for the TUI interface. It manages the application lifecycle, user
//! interactions, and coordinates between different UI components.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use heroku_registry::Registry;
use heroku_types::{ExecOutcome, Screen};

use crate::ui::components::logs::state::LogEntry;
use crate::ui::theme;
use crate::{
    start_palette_execution,
    ui::components::{
        builder::BuilderState,
        help::HelpState,
        logs::LogsState,
        palette::{PaletteState, state::ValueProvider},
        table::TableState,
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
}

impl SharedCtx {
    pub fn new(registry: Registry) -> Self {
        let debug_enabled = std::env::var("DEBUG")
            .map(|v| !v.is_empty() && v != "0" && v.to_lowercase() != "false")
            .unwrap_or(false);
        Self {
            registry,
            debug_enabled,
            providers: vec![],
            theme: theme::load_from_env(),
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
    /// Builder modal state
    pub builder: BuilderState,
    /// Table modal state
    pub table: TableState<'a>,
    /// Help modal state
    pub help: HelpState,
    /// Application logs and status messages
    pub logs: LogsState,
    // moved to ctx: dry_run, debug_enabled, providers
    /// Whether a command is currently executing
    pub executing: bool,
    /// Animation frame for the execution throbber
    pub throbber_idx: usize,
    /// Sender for async execution results
    pub exec_sender: UnboundedSender<ExecOutcome>,
    /// Receiver for async execution results
    pub exec_receiver: UnboundedReceiver<ExecOutcome>,
    /// Top-level focus between palette and logs
    pub main_focus: MainFocus,
    /// Active execution count used by the event pump to decide whether to animate
    pub active_exec_count: Arc<AtomicUsize>,
    /// Last pagination info returned by an execution (if any)
    pub last_pagination: Option<heroku_types::Pagination>,
    /// Ranges supported by the last executed command (for pagination UI)
    pub last_command_ranges: Option<Vec<String>>,
}

impl<'a> App<'a> {
    /// Gets the available range fields for the currently selected command
    pub fn available_ranges(&self) -> Vec<String> {
        if let Some(r) = &self.last_command_ranges {
            if !r.is_empty() {
                return r.clone();
            }
        }
        self.builder.available_ranges()
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
    /// Toggle the builder modal visibility
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
pub enum Effect {
    /// Request to copy the current command to clipboard
    CopyCommandRequested,
    /// Request to copy the current logs selection (already rendered/redacted)
    CopyLogsRequested(String),
}

/// Top-level focus for main screen interactions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainFocus {
    Palette,
    Logs,
}

impl App<'_> {
    /// Creates a new application instance with the given registry.
    ///
    /// This constructor initializes the application state with default values
    /// and loads all commands from the provided registry.
    ///
    /// # Arguments
    ///
    /// * `registry` - The Heroku command registry containing all available commands
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
            builder: BuilderState::default(),
            logs: LogsState::default(),
            help: HelpState::default(),
            table: TableState::default(),
            palette: PaletteState::default(),
            executing: false,
            throbber_idx: 0,
            exec_sender,
            exec_receiver,
            main_focus: MainFocus::Palette,
            active_exec_count: Arc::new(AtomicUsize::new(0)),
            last_pagination: None,
            last_command_ranges: None,
        };
        app.builder.set_all_commands(app.ctx.registry.commands.clone());
        app.palette.set_all_commands(app.ctx.registry.commands.clone());
        app.builder.update_browser_filtered();
        app
    }

    /// Updates the application state based on a message.
    ///
    /// This method processes messages and updates the application state
    /// accordingly. It handles user interactions, navigation, and state changes.
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
                if self.executing {
                    self.throbber_idx = (self.throbber_idx + 1) % 10;
                }
            }
            Msg::Resize(_, _) => {
                // No-op for now; placeholder to enable TEA-style event
            }
            Msg::ToggleHelp => {
                let spec = if self.builder.is_visible() {
                    self.builder.selected_command()
                } else {
                    self.palette.selected_command()
                };
                self.help.toggle_visibility(spec.cloned());
            }
            Msg::ToggleTable => {
                self.table.toggle_show();
            }
            Msg::ToggleBuilder => {
                self.builder.toggle_visibility();
            }
            Msg::CloseModal => {
                self.help.set_visibility(false);
                self.table.apply_visible(false);
                self.builder.apply_visibility(false);
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
                        }
                        Err(e) => {
                            self.palette.apply_error(e);
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
            }
            // Placeholder handlers for upcoming logs features
            Msg::LogsUp | Msg::LogsDown | Msg::LogsExtendUp | Msg::LogsExtendDown => {}
            Msg::LogsOpenDetail | Msg::LogsCloseDetail | Msg::LogsCopy | Msg::LogsTogglePretty => {}
        }
        effects
    }
}
