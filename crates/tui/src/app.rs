//! Application state and logic for the Heroku TUI.
//!
//! This module contains the main application state, data structures, and
//! business logic for the TUI interface. It manages the application lifecycle,
//! user interactions, and coordinates between different UI components.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use heroku_registry::Registry;
use heroku_types::{ExecOutcome, Screen};
use rat_focus::FocusBuilder;
use serde_json::{Map as JsonMap, Value as JsonValue};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{
    start_palette_execution,
    ui::{
        components::{
            builder::BuilderState,
            help::HelpState,
            logs::{LogsState, state::LogEntry},
            palette::{
                state::{ItemKind, ValueProvider},
                PaletteState, providers::RegistryBackedProvider,
            },
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
}

impl<'a> App<'a> {
    /// Gets the available range fields for the currently selected command
    pub fn available_ranges(&self) -> Vec<String> {
        if let Some(r) = &self.last_command_ranges
            && !r.is_empty()
        {
            return r.clone();
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

    // Palette interactions
    PaletteInput(char),
    PaletteBackspace,
    PaletteCursorLeft,
    PaletteCursorRight,
    PaletteSuggest,
    PaletteAcceptSuggestion,
    PaletteNavigateSuggestions(Direction),
    PaletteNavigateHistory(Direction),
    PaletteClear,

    // Table interactions
    TableScroll(isize),
    TableHome,
    TableEnd,
    TableFocusNext,
    TableFocusPrev,

    // Pagination interactions
    PaginationFirst,
    PaginationPrev,
    PaginationNext,
    PaginationLast,

    // Builder interactions
    BuilderSearchInput(char),
    BuilderSearchBackspace,
    BuilderSearchClear,
    BuilderMoveSelection(isize),
    BuilderAccept,
    BuilderCycleFocus,
    BuilderCycleFocusBack,
    BuilderCycleField(Direction),
    BuilderFieldInput(char),
    BuilderFieldBackspace,
    BuilderToggleBooleanField,
    BuilderCycleEnum(Direction),

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

#[derive(Debug, Clone)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
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
            builder: BuilderState::default(),
            logs: LogsState::default(),
            help: HelpState::default(),
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
        };
        app.builder.set_all_commands(app.ctx.registry.commands.clone());
        app.palette.set_all_commands(app.ctx.registry.commands.clone());
        app.builder.update_browser_filtered();
        // Initialize rat-focus: start with the palette focused at root
        {
            let mut focus_builder = FocusBuilder::new(None);
            focus_builder.widget(&app.palette);
            focus_builder.widget(&app.logs);
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
    pub fn update(&mut self, msg: Msg) -> (Vec<Effect>, bool) {
        let mut effects = Vec::new();
        let mut needs_rerender = true;

        match msg {
            Msg::Tick => {
                let mut is_animating = false;
                if self.executing || self.palette.is_provider_loading() {
                    self.throbber_idx = (self.throbber_idx + 1) % 10;
                    is_animating = true;
                }
                if self.palette.is_suggestions_open() && self.palette.is_provider_loading() {
                    self.palette.apply_build_suggestions(&self.ctx.registry, &self.ctx.providers, &*self.ctx.theme);
                    is_animating = true;
                }
                needs_rerender = is_animating;
            }
            Msg::Resize(..) => {}
            Msg::ToggleHelp => {
                let spec = if self.builder.is_visible() {
                    self.builder.selected_command()
                } else {
                    self.palette.selected_command()
                };
                self.help.toggle_visibility(spec.cloned());
            }
            Msg::ToggleTable => self.table.toggle_show(),
            Msg::ToggleBuilder => {
                self.builder.toggle_visibility();
                if self.builder.is_visible() {
                    self.builder.normalize_focus();
                }
            }
            Msg::CloseModal => {
                self.help.set_visibility(false);
                self.table.apply_visible(false);
                self.builder.apply_visibility(false);
            }
            Msg::Run => {
                if !self.palette.is_input_empty() {
                    match start_palette_execution(self) {
                        Ok(_) => {
                            let input = &self.palette.input();
                            self.logs.entries.push(format!("Running: {}", input));
                            self.logs.rich_entries.push(LogEntry::Text {
                                level: Some("info".into()),
                                msg: format!("Running: {}", input),
                            });
                        }
                        Err(e) => self.palette.apply_error(e),
                    }
                }
            }
            Msg::CopyCommand => {
                effects.push(Effect::CopyCommandRequested);
            }
            Msg::ExecCompleted(out) => {
                let raw = out.log;
                self.executing = self.active_exec_count.load(Ordering::Relaxed) > 0;
                self.logs.entries.push(heroku_util::redact_sensitive(&raw));
                self.logs.rich_entries.push(LogEntry::Api {
                    status: 0,
                    raw,
                    json: out.result_json.clone(),
                });
                if self.logs.entries.len() > 500 {
                    let _ = self.logs.entries.drain(0..self.logs.entries.len() - 500);
                }
                if self.logs.rich_entries.len() > 500 {
                    let _ = self.logs.rich_entries.drain(0..self.logs.rich_entries.len() - 500);
                }
                self.table.apply_result_json(out.result_json, &*self.ctx.theme);
                self.table.apply_visible(out.open_table);
                self.last_pagination = out.pagination;
                self.palette.reduce_clear_all();
            }
            Msg::PaletteInput(c) => {
                self.palette.apply_insert_char(c);
                self.palette.set_is_suggestions_open(false);
                self.palette.reduce_clear_error();
            }
            Msg::PaletteBackspace => {
                self.palette.reduce_backspace();
                self.palette.reduce_clear_error();
                self.palette.apply_suggestions(vec![]);
            }
            Msg::PaletteCursorLeft => self.palette.reduce_move_cursor_left(),
            Msg::PaletteCursorRight => self.palette.reduce_move_cursor_right(),
            Msg::PaletteSuggest => {
                self.palette.apply_build_suggestions(&self.ctx.registry, &self.ctx.providers, &*self.ctx.theme);
                self.palette.set_is_suggestions_open(self.palette.suggestions_len() > 0);
            }
            Msg::PaletteAcceptSuggestion => {
                if let Some(item) = self.palette.selected_suggestion().cloned() {
                    match item.kind {
                        ItemKind::Command => {
                            self.palette.apply_accept_command_suggestion(&item.insert_text);
                            self.palette.set_is_suggestions_open(false);
                            self.palette.reduce_clear_suggestions();
                        }
                        ItemKind::Positional => self.palette.apply_accept_positional_suggestion(&item.insert_text),
                        _ => self.palette.apply_accept_non_command_suggestion(&item.insert_text),
                    }
                    self.palette.apply_build_suggestions(&self.ctx.registry, &self.ctx.providers, &*self.ctx.theme);
                    self.palette.set_selected(0);
                    self.palette.set_is_suggestions_open(false);
                }
            }
            Msg::PaletteNavigateSuggestions(direction) => {
                let len = self.palette.suggestions().len();
                if len > 0 {
                    let selected = self.palette.suggestion_index() as isize;
                    let delta = if matches!(direction, Direction::Down) { 1isize } else { -1isize };
                    let new_selected = (selected + delta).rem_euclid(len as isize) as usize;
                    self.palette.set_selected(new_selected);
                }
            }
            Msg::PaletteNavigateHistory(direction) => {
                let changed = if matches!(direction, Direction::Up) {
                    self.palette.history_up()
                } else {
                    self.palette.history_down()
                };
                if changed {
                    self.palette.reduce_clear_error();
                    self.palette.set_is_suggestions_open(false);
                }
            }
            Msg::PaletteClear => {
                if self.palette.is_suggestions_open() {
                    self.palette.set_is_suggestions_open(false);
                } else {
                    self.palette.reduce_clear_all();
                }
            }
            Msg::TableScroll(delta) => self.table.reduce_scroll(delta),
            Msg::TableHome => self.table.reduce_home(),
            Msg::TableEnd => self.table.reduce_end(),
            Msg::TableFocusNext => { /* TODO */ needs_rerender = false; }
            Msg::TableFocusPrev => { /* TODO */ needs_rerender = false; }
            Msg::PaginationFirst => {
                self.pagination_history.truncate(1);
                effects.push(Effect::FirstPageRequested);
            }
            Msg::PaginationPrev => {
                if self.pagination_history.len() > 1 {
                    self.pagination_history.pop();
                    effects.push(Effect::PrevPageRequested);
                }
            }
            Msg::PaginationNext | Msg::PaginationLast => {
                if let Some(next_range) = self.last_pagination.as_ref().and_then(|p| p.next_range.clone()) {
                    self.pagination_history.push(Some(next_range.clone()));
                    effects.push(Effect::NextPageRequested(next_range));
                }
            }
            Msg::LogsUp => { /* TODO */ needs_rerender = false; }
            Msg::LogsDown => { /* TODO */ needs_rerender = false; }
            Msg::LogsExtendUp => { /* TODO */ needs_rerender = false; }
            Msg::LogsExtendDown => { /* TODO */ needs_rerender = false; }
            Msg::LogsOpenDetail => { /* TODO */ needs_rerender = false; }
            Msg::LogsCloseDetail => {
                self.logs.detail = None;
            }
            Msg::LogsCopy => { /* TODO */ needs_rerender = false; }
            Msg::LogsTogglePretty => { /* TODO */ needs_rerender = false; }

            // Builder handlers
            Msg::BuilderSearchInput(c) => self.builder.search_input_push(c),
            Msg::BuilderSearchBackspace => self.builder.search_input_pop(),
            Msg::BuilderSearchClear => self.builder.search_input_clear(),
            Msg::BuilderMoveSelection(delta) => self.builder.move_selection(delta),
            Msg::BuilderAccept => {
                self.builder.apply_enter();
                self.builder.inputs_flag.set(true);
                self.builder.search_flag.set(false);
                self.builder.commands_flag.set(false);
            }
            Msg::BuilderCycleFocus => { self.builder.focus_ring().next(); }
            Msg::BuilderCycleFocusBack => { self.builder.focus_ring().prev(); }
            Msg::BuilderCycleField(direction) => {
                if matches!(direction, Direction::Up) {
                    self.builder.reduce_move_field_up(self.ctx.debug_enabled);
                } else {
                    self.builder.reduce_move_field_down(self.ctx.debug_enabled);
                }
            }
            Msg::BuilderFieldInput(c) => self.builder.reduce_add_char_to_field(c),
            Msg::BuilderFieldBackspace => self.builder.reduce_remove_char_from_field(),
            Msg::BuilderToggleBooleanField => self.builder.reduce_toggle_boolean_field(),
            Msg::BuilderCycleEnum(direction) => {
                if matches!(direction, Direction::Left) {
                    self.builder.reduce_cycle_enum_left();
                } else {
                    self.builder.reduce_cycle_enum_right();
                }
            }
        }
        (effects, needs_rerender)
    }
}
