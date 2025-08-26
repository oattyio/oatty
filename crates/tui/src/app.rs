//! Application state and logic for the Heroku TUI.
//!
//! This module contains the main application state, data structures, and business
//! logic for the TUI interface. It manages the application lifecycle, user
//! interactions, and coordinates between different UI components.

use std::sync::{Arc, mpsc::Receiver};

use heroku_registry::Registry;
use heroku_types::{CommandSpec, Field, Focus, Screen, ExecOutcome};
use heroku_util::fuzzy_score;
use ratatui::widgets::ListState;

use crate::{
    start_palette_execution,
    ui::components::{builder::BuilderState, palette::{state::ValueProvider, PaletteState}, table::TableState},
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

/// The main application state containing all UI data and business logic.
///
/// This struct serves as the central state container for the entire TUI
/// application, managing user interactions, data flow, and UI state.
#[derive(Debug)]
pub struct LogsState {
    pub entries: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct HelpState {
    pub show: bool,
    pub spec: Option<heroku_registry::CommandSpec>,
}

#[derive(Debug)]
pub struct CommandBrowserState {
    pub search: String,
    pub all_commands: Arc<[CommandSpec]>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub list_state: ListState,
}

pub struct App {
    /// Current primary route
    pub route: Screen,
    /// Shared, cross-cutting context (registry, config)
    pub ctx: SharedCtx,
    /// State for the command palette input
    pub palette: PaletteState,
    /// Browser state for searching/selecting commands
    pub browser: CommandBrowserState,
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
        let ctx = SharedCtx::new(registry);
        let all = ctx.registry.commands.clone();
        let mut app = Self {
            route: Screen::default(),
            ctx,
            browser: CommandBrowserState {
                search: String::new(),
                all_commands: all,
                filtered: Vec::new(),
                selected: 0,
                list_state: ListState::default(),
            },
            builder: BuilderState::default(),
            logs: LogsState {
                entries: vec!["Welcome to Heroku TUI".into()],
            },
            help: HelpState::default(),
            table: TableState {
                show: false,
                offset: 0,
                result_json: None,
            },
            palette: PaletteState::default(),
            executing: false,
            throbber_idx: 0,
            exec_receiver: None,
        };
        app.update_browser_filtered();
        app
    }

    /// Updates the filtered command list based on the current search query.
    ///
    /// This method filters the available commands using fuzzy matching
    /// and updates the filtered indices and selection state.
    pub fn update_browser_filtered(&mut self) {
        if self.browser.search.is_empty() {
            self.browser.filtered = (0..self.browser.all_commands.len()).collect();
        } else {
            let mut items: Vec<(i64, usize)> = self
                .browser
                .all_commands
                .iter()
                .enumerate()
                .filter_map(|(i, command)| {
                    let group = &command.group;
                    let name = &command.name;
                    let exec = if name.is_empty() {
                        group.to_string()
                    } else {
                        format!("{} {}", group, name)
                    };
                    if let Some(score) = fuzzy_score(&exec, &self.browser.search) {
                        return Some((score, i));
                    }
                    None
                })
                .into_iter()
                .collect();
            items.sort_by(|a, b| b.0.cmp(&a.0));

            self.browser.filtered = items.iter().map(|x| x.1).collect();
        }
        self.browser.selected = self
            .browser
            .selected
            .min(self.browser.filtered.len().saturating_sub(1));
        self.browser.list_state.select(Some(self.browser.selected));
    }

    /// Returns a list of required field names that are currently empty.
    ///
    /// This method checks all required fields and returns the names
    /// of those that don't have values, for validation purposes.
    ///
    /// # Returns
    ///
    /// Vector of field names that are required but empty.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let missing = app.missing_required();
    /// if !missing.is_empty() {
    ///     println!("Missing required fields: {:?}", missing);
    /// }
    /// ```
    pub fn missing_required(&self) -> Vec<String> {
        self.builder.missing_required_fields()
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
                self.help.show = !self.help.show;
                if self.help.show {
                    self.help.spec = self.builder.selected_command().cloned();
                }
            }
            Msg::ToggleTable => {
                self.table.show = !self.table.show;
                if self.table.show {
                    self.table.offset = 0;
                }
            }
            Msg::ToggleBuilder => {
                self.builder.toggle_visibility();
            }
            Msg::CloseModal => {
                self.help.show = false;
                self.table.show = false;
                self.builder.apply_visibility(false);
            }
            Msg::FocusNext => {
                self.builder.apply_next_focus();
            }
            Msg::FocusPrev => {
                self.builder.apply_previous_focus();
            }
            Msg::MoveSelection(delta) => {
                if !self.browser.filtered.is_empty() {
                    let new_selected = if delta > 0 {
                        self.browser.selected.saturating_add(delta as usize)
                    } else {
                        self.browser.selected.saturating_sub((-delta) as usize)
                    };
                    self.browser.selected =
                        new_selected.min(self.browser.filtered.len().saturating_sub(1));
                    self.browser.list_state.select(Some(self.browser.selected));
                    let idx = self.browser.filtered[self.browser.selected];
                    self.builder.apply_command_selection(self.browser.all_commands[idx].clone());
                }
            }
            Msg::Enter => {
                if !self.browser.filtered.is_empty() {
                    let idx = self.browser.filtered[self.browser.selected];
                    let command = self.browser.all_commands[idx].clone();
                    self.builder.apply_command_selection(command.clone());
                    let fields = command
                        .flags
                        .iter()
                        .map(|f| Field {
                            name: f.name.clone(),
                            required: f.required,
                            is_bool: f.r#type == "boolean",
                            value: f.default_value.clone().unwrap_or_default(),
                            enum_values: f.enum_values.clone(),
                            enum_idx: None,
                        })
                        .collect();
                    self.builder.apply_fields(fields);
                    self.builder.apply_field_idx(0);
                    self.builder.apply_focus(Focus::Inputs);
                }
            }
            Msg::SearchChar(c) => {
                self.browser.search.push(c);
                self.update_browser_filtered();
            }
            Msg::SearchBackspace => {
                self.browser.search.pop();
                self.update_browser_filtered();
            }
            Msg::SearchClear => {
                self.browser.search.clear();
                self.update_browser_filtered();
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
                if let Some(spec) = self.builder.selected_command() {
                    if self.missing_required().is_empty() {
                        self.executing = true;
                        self.throbber_idx = 0;
                        // Start async execution here
                        // For now, just simulate
                        self.logs.entries.push(format!("Executing: {}", spec.name));
                        self.executing = false;
                    }
                } else if !self.palette.is_input_empty() {
                    match start_palette_execution(self) {
                        Ok(()) => {
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
                let new_offset = if delta > 0 {
                    self.table.offset.saturating_add(delta as usize)
                } else {
                    self.table.offset.saturating_sub((-delta) as usize)
                };
                self.table.offset = new_offset;
            }
            Msg::TableHome => {
                self.table.offset = 0;
            }
            Msg::TableEnd => {
                // Set to a large value to scroll to end
                self.table.offset = usize::MAX;
            }
            Msg::ExecCompleted(out) => {
                self.exec_receiver = None;
                self.executing = false;
                self.logs.entries.push(out.log);
                let log_len = self.logs.entries.len();
                if log_len > 500 {
                    let _ = self.logs.entries.drain(0..log_len - 500);
                }
                self.table.result_json = out.result_json;
                self.table.show = out.open_table;
                if out.open_table {
                    self.table.offset = 0;
                }
                // Clear palette input and suggestion state
                self.palette.reduce_clear_all();
            }
        }
        effects
    }
}
