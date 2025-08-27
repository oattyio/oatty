//! Application state and logic for the Heroku TUI.
//!
//! This module contains the main application state, data structures, and business
//! logic for the TUI interface. It manages the application lifecycle, user
//! interactions, and coordinates between different UI components.

use std::sync::mpsc::Receiver;

use heroku_registry::Registry;
use heroku_types::{ExecOutcome, Screen};

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
#[derive(Debug)]
pub struct SharedCtx {
    /// Global Heroku command registry
    pub registry: Registry,
    /// Global debug flag (from env)
    pub debug_enabled: bool,
    /// Value providers for suggestions
    pub providers: Vec<Box<dyn ValueProvider>>,
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
        }
    }
}

pub struct App {
    /// Current primary route
    pub route: Screen,
    /// Shared, cross-cutting context (registry, config)
    pub ctx: SharedCtx,
    /// State for the command palette input
    pub palette: PaletteState,
    /// Builder modal state
    pub builder: BuilderState,
    /// Table modal state
    pub table: TableState,
    /// Help modal state
    pub help: HelpState,
    /// Application logs and status messages
    pub logs: LogsState,
    // moved to ctx: dry_run, debug_enabled, providers
    /// Whether a command is currently executing
    pub executing: bool,
    /// Animation frame for the execution throbber
    pub throbber_idx: usize,
    /// Receiver for async execution results
    pub exec_receiver: Option<Receiver<ExecOutcome>>,
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
    /// Move focus to the next UI element
    FocusNext,
    /// Move focus to the previous UI element
    FocusPrev,
    /// Move selection in a list by the given offset
    MoveSelection(isize),
    /// Execute the currently selected action
    Enter,
    /// Add a character to the search input
    SearchChar(char),
    /// Remove a character from the search input
    SearchBackspace,
    /// Clear the search input
    SearchClear,
    /// Move up in the inputs form
    InputsUp,
    /// Move down in the inputs form
    InputsDown,
    /// Add a character to the current input field
    InputsChar(char),
    /// Remove a character from the current input field
    InputsBackspace,
    /// Toggle a boolean field value
    InputsToggleSpace,
    /// Cycle through enum values to the left
    InputsCycleLeft,
    /// Cycle through enum values to the right
    InputsCycleRight,
    /// Execute the current command
    Run,
    /// Copy the current command to clipboard
    CopyCommand,
    /// Scroll the table by the given offset
    TableScroll(isize),
    /// Jump to the beginning of the table
    TableHome,
    /// Jump to the end of the table
    TableEnd,
    /// Periodic UI tick (e.g., throbbers)
    Tick,
    /// Terminal resized
    Resize(u16, u16),
    /// Background execution completed with outcome
    ExecCompleted(ExecOutcome),
}

/// Side effects that can be triggered by state changes.
///
/// This enum defines actions that should be performed as a result
/// of state changes, such as copying to clipboard or showing notifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Request to copy the current command to clipboard
    CopyCommandRequested,
}

impl App {
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
    /// ```rust
    /// use heroku_registry::Registry;
    ///
    /// let registry = Registry::from_embedded_schema()?;
    /// let app = App::new(registry);
    /// ```
    pub fn new(registry: heroku_registry::Registry) -> Self {
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
            exec_receiver: None,
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
    /// ```rust
    /// let effects = app.update(Msg::ToggleHelp);
    /// for effect in effects {
    ///     match effect {
    ///         Effect::CopyCommandRequested => {
    ///             // Handle clipboard copy
    ///         }
    ///     }
    /// }
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
                self.table.apply_show(false);
                self.builder.apply_visibility(false);
            }
            Msg::FocusNext => {
                self.builder.apply_next_focus();
            }
            Msg::FocusPrev => {
                self.builder.apply_previous_focus();
            }
            Msg::MoveSelection(delta) => {
                self.builder.move_selection(delta);
            }
            Msg::Enter => {
                self.builder.apply_enter();
            }
            Msg::SearchChar(ch) => {
                self.builder.search_input_push(ch);
            }
            Msg::SearchBackspace => {
                self.builder.search_input_pop();
            }
            Msg::SearchClear => {
                self.builder.search_input_clear();
            }
            Msg::InputsUp => {
                self.builder.reduce_move_field_up(self.ctx.debug_enabled);
            }
            Msg::InputsDown => {
                self.builder.reduce_move_field_down(self.ctx.debug_enabled);
            }
            Msg::InputsChar(c) => {
                self.builder.reduce_add_char_to_field(c);
            }
            Msg::InputsBackspace => {
                self.builder.reduce_remove_char_from_field();
            }
            Msg::InputsToggleSpace => {
                self.builder.reduce_toggle_boolean_field();
            }
            Msg::InputsCycleLeft => {
                self.builder.reduce_cycle_enum_left();
            }
            Msg::InputsCycleRight => {
                self.builder.reduce_cycle_enum_right();
            }
            Msg::Run => {
                // always execute from palette
                if !self.palette.is_input_empty() {
                    match start_palette_execution(self) {
                        Ok(_) => {
                            let input = &self.palette.input();
                            self.logs.entries.push(format!("Running: {}", input));
                            // Execution started successfully
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
            Msg::TableScroll(delta) => {
                self.table.reduce_scroll(delta);
            }
            Msg::TableHome => {
                self.table.reduce_home();
            }
            Msg::TableEnd => {
                // Set to a large value to scroll to end
                self.table.reduce_end();
            }
            Msg::ExecCompleted(out) => {
                self.exec_receiver = None;
                self.executing = false;
                self.logs.entries.push(out.log);
                let log_len = self.logs.entries.len();
                if log_len > 500 {
                    let _ = self.logs.entries.drain(0..log_len - 500);
                }
                self.table.apply_result_json(out.result_json);
                self.table.apply_show(out.open_table);
                // Clear palette input and suggestion state
                self.palette.reduce_clear_all();
            }
        }
        effects
    }
}
