//! Application state and logic for the Heroku TUI.
//!
//! This module contains the main application state, data structures, and business
//! logic for the TUI interface. It manages the application lifecycle, user
//! interactions, and coordinates between different UI components.

use ratatui::widgets::ListState;

use crate::start_palette_execution;

/// Represents a single input field for a command parameter.
///
/// This struct contains all the metadata and state for a command parameter
/// including its type, validation rules, current value, and UI state.
#[derive(Debug, Clone)]
pub struct Field {
    /// The name of the field (e.g., "app", "region", "stack")
    pub name: String,
    /// Whether this field is required for the command to execute
    pub required: bool,
    /// Whether this field represents a boolean value (checkbox)
    pub is_bool: bool,
    /// The current value entered by the user
    pub value: String,
    /// Valid enum values for this field (empty if not an enum)
    pub enum_values: Vec<String>,
    /// Current selection index for enum fields
    pub enum_idx: Option<usize>,
}

/// Represents the current focus area in the UI.
///
/// This enum tracks which part of the interface currently has focus,
/// allowing for proper keyboard navigation and input handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// Search input field in the command palette
    Search,
    /// Command list in the builder modal
    Commands,
    /// Input fields form in the builder modal
    Inputs,
}

/// The main application state containing all UI data and business logic.
///
/// This struct serves as the central state container for the entire TUI
/// application, managing user interactions, data flow, and UI state.
pub struct App {
    /// The registry containing all available Heroku commands
    pub registry: heroku_registry::Registry,
    /// Current focus area in the UI
    pub focus: Focus,
    /// Search query for filtering commands
    pub search: String,
    /// All available commands from the registry
    pub all_commands: Vec<heroku_registry::CommandSpec>,
    /// Indices of commands that match the current search filter
    pub filtered: Vec<usize>,
    /// Index of the currently selected command in the filtered list
    pub selected: usize,

    /// The currently selected command specification
    pub picked: Option<heroku_registry::CommandSpec>,
    /// Input fields for the selected command
    pub fields: Vec<Field>,
    /// Index of the currently focused field
    pub field_idx: usize,

    /// Application logs and status messages
    pub logs: Vec<String>,
    /// Whether the help modal is currently shown
    pub show_help: bool,
    /// Command specification to show help for (may differ from picked)
    pub help_spec: Option<heroku_registry::CommandSpec>,
    /// State for the command list widget
    pub list_state: ListState,
    /// Whether to run commands in dry-run mode
    pub dry_run: bool,
    /// Whether debug mode is enabled
    pub debug_enabled: bool,
    /// JSON result from the last executed command
    pub result_json: Option<serde_json::Value>,
    /// Whether the table modal is currently shown
    pub show_table: bool,
    /// Scroll offset for table display
    pub table_offset: usize,
    /// Whether the builder modal is currently shown
    pub show_builder: bool,
    /// State for the command palette input
    pub palette: crate::palette::PaletteState,
    /// Value providers for command suggestions
    pub providers: Vec<Box<dyn crate::palette::ValueProvider>>,
    /// Whether a command is currently executing
    pub executing: bool,
    /// Animation frame for the execution throbber
    pub throbber_idx: usize,
    /// Receiver for async execution results
    pub exec_rx: Option<std::sync::mpsc::Receiver<ExecOutcome>>,
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

/// Result of an asynchronous command execution.
///
/// This struct contains the outcome of a command execution including
/// logs, results, and any UI state changes that should occur.
#[derive(Debug, Clone)]
pub struct ExecOutcome {
    /// Log message from the command execution
    pub log: String,
    /// JSON result from the command (if any)
    pub result_json: Option<serde_json::Value>,
    /// Whether to automatically open the table modal
    pub open_table: bool,
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
        let all = registry.commands.clone();
        let mut app = Self {
            registry,
            focus: Focus::Search,
            search: String::new(),
            filtered: (0..all.len()).collect(),
            selected: 0,
            picked: None,
            fields: Vec::new(),
            field_idx: 0,
            logs: vec!["Welcome to Heroku TUI".into()],
            show_help: false,
            help_spec: None,
            all_commands: all,
            list_state: ListState::default(),
            dry_run: false,
            debug_enabled: std::env::var("DEBUG")
                .map(|v| !v.is_empty() && v != "0" && v.to_lowercase() != "false")
                .unwrap_or(false),
            result_json: None,
            show_table: false,
            table_offset: 0,
            show_builder: false,
            palette: crate::palette::PaletteState::default(),
            providers: vec![],
            executing: false,
            throbber_idx: 0,
            exec_rx: None,
        };
        app.update_filtered();
        app
    }

    /// Updates the filtered command list based on the current search query.
    ///
    /// This method filters the available commands using fuzzy matching
    /// and updates the filtered indices and selection state.
    pub fn update_filtered(&mut self) {
        if self.search.is_empty() {
            self.filtered = (0..self.all_commands.len()).collect();
        } else {
            self.filtered = self
                .all_commands
                .iter()
                .enumerate()
                .filter_map(|(i, cmd)| {
                    if cmd
                        .name
                        .to_lowercase()
                        .contains(&self.search.to_lowercase())
                    {
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect();
        }
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
        self.list_state.select(Some(self.selected));
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
        self.fields
            .iter()
            .filter(|f| f.required && f.value.is_empty())
            .map(|f| f.name.clone())
            .collect()
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
                self.show_help = !self.show_help;
                if self.show_help {
                    self.help_spec = self.picked.clone();
                }
            }
            Msg::ToggleTable => {
                self.show_table = !self.show_table;
                if self.show_table {
                    self.table_offset = 0;
                }
            }
            Msg::ToggleBuilder => {
                self.show_builder = !self.show_builder;
            }
            Msg::CloseModal => {
                self.show_help = false;
                self.show_table = false;
                self.show_builder = false;
            }
            Msg::FocusNext => {
                self.focus = match self.focus {
                    Focus::Search => Focus::Commands,
                    Focus::Commands => Focus::Inputs,
                    Focus::Inputs => Focus::Search,
                };
            }
            Msg::FocusPrev => {
                self.focus = match self.focus {
                    Focus::Search => Focus::Inputs,
                    Focus::Commands => Focus::Search,
                    Focus::Inputs => Focus::Commands,
                };
            }
            Msg::MoveSelection(delta) => {
                if !self.filtered.is_empty() {
                    let new_selected = if delta > 0 {
                        self.selected.saturating_add(delta as usize)
                    } else {
                        self.selected.saturating_sub((-delta) as usize)
                    };
                    self.selected = new_selected.min(self.filtered.len().saturating_sub(1));
                    self.list_state.select(Some(self.selected));
                }
            }
            Msg::Enter => {
                if !self.filtered.is_empty() {
                    let idx = self.filtered[self.selected];
                    self.picked = Some(self.all_commands[idx].clone());
                    self.fields = self
                        .picked
                        .as_ref()
                        .unwrap()
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
                    self.field_idx = 0;
                    self.focus = Focus::Inputs;
                }
            }
            Msg::SearchChar(c) => {
                self.search.push(c);
                self.update_filtered();
            }
            Msg::SearchBackspace => {
                self.search.pop();
                self.update_filtered();
            }
            Msg::SearchClear => {
                self.search.clear();
                self.update_filtered();
            }
            Msg::InputsUp => {
                if self.field_idx > 0 {
                    self.field_idx -= 1;
                } else if self.debug_enabled {
                    self.field_idx = self.fields.len();
                }
            }
            Msg::InputsDown => {
                if self.debug_enabled && self.field_idx == self.fields.len() {
                    self.field_idx = 0;
                } else if self.field_idx < self.fields.len().saturating_sub(1) {
                    self.field_idx += 1;
                }
            }
            Msg::InputsChar(c) => {
                if let Some(field) = self.fields.get_mut(self.field_idx) {
                    if field.is_bool {
                        if c == ' ' {
                            field.value = if field.value.is_empty() {
                                "true".into()
                            } else {
                                String::new()
                            };
                        }
                    } else {
                        field.value.push(c);
                    }
                }
            }
            Msg::InputsBackspace => {
                if let Some(field) = self.fields.get_mut(self.field_idx) {
                    if !field.is_bool {
                        field.value.pop();
                    }
                }
            }
            Msg::InputsToggleSpace => {
                if let Some(field) = self.fields.get_mut(self.field_idx) {
                    if field.is_bool {
                        field.value = if field.value.is_empty() {
                            "true".into()
                        } else {
                            String::new()
                        };
                    }
                }
            }
            Msg::InputsCycleLeft => {
                if let Some(field) = self.fields.get_mut(self.field_idx) {
                    if !field.enum_values.is_empty() {
                        let current = field.enum_idx.unwrap_or(0);
                        let new_idx = if current == 0 {
                            field.enum_values.len() - 1
                        } else {
                            current - 1
                        };
                        field.enum_idx = Some(new_idx);
                        field.value = field.enum_values[new_idx].clone();
                    }
                }
            }
            Msg::InputsCycleRight => {
                if let Some(field) = self.fields.get_mut(self.field_idx) {
                    if !field.enum_values.is_empty() {
                        let current = field.enum_idx.unwrap_or(0);
                        let new_idx = (current + 1) % field.enum_values.len();
                        field.enum_idx = Some(new_idx);
                        field.value = field.enum_values[new_idx].clone();
                    }
                }
            }
            Msg::Run => {
                if let Some(spec) = &self.picked {
                    if self.missing_required().is_empty() {
                        self.executing = true;
                        self.throbber_idx = 0;
                        // Start async execution here
                        // For now, just simulate
                        self.logs.push(format!("Executing: {}", spec.name));
                        self.executing = false;
                    }
                } else if !self.palette.input.trim().is_empty() {
                    match start_palette_execution(self) {
                        Ok(()) => {
                            // Execution started successfully
                        }
                        Err(e) => {
                            self.palette.error = Some(e);
                        }
                    }
                }
            }
            Msg::CopyCommand => {
                effects.push(Effect::CopyCommandRequested);
            }
            Msg::TableScroll(delta) => {
                let new_offset = if delta > 0 {
                    self.table_offset.saturating_add(delta as usize)
                } else {
                    self.table_offset.saturating_sub((-delta) as usize)
                };
                self.table_offset = new_offset;
            }
            Msg::TableHome => {
                self.table_offset = 0;
            }
            Msg::TableEnd => {
                // Set to a large value to scroll to end
                self.table_offset = usize::MAX;
            }
            Msg::ExecCompleted(out) => {
                self.exec_rx = None;
                self.executing = false;
                self.logs.push(out.log);
                if self.logs.len() > 500 {
                    let _ = self.logs.drain(0..self.logs.len() - 500);
                }
                self.result_json = out.result_json;
                self.show_table = out.open_table;
                if out.open_table {
                    self.table_offset = 0;
                }
                // Clear palette input and suggestion state
                self.palette.input.clear();
                self.palette.cursor = 0;
                self.palette.suggestions.clear();
                self.palette.popup_open = false;
                self.palette.error = None;
            }
        }
        effects
    }
}
