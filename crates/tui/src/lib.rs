//! # Heroku CLI TUI Library
//!
//! This library provides a terminal user interface (TUI) for the Heroku CLI.
//! It implements a modern, interactive command-line interface using the Ratatui
//! framework with support for command execution, real-time logs, and interactive
//! command building.
//!
//! ## Key Features
//!
//! - Interactive command palette with autocomplete
//! - Real-time command execution and log streaming
//! - Command builder with field validation
//! - Tabular data display with pagination
//! - Focus management and keyboard navigation
//! - Asynchronous command execution
//!
//! ## Architecture
//!
//! The TUI follows a component-based architecture where each UI element
//! (palette, logs, builder, table, help) is implemented as a separate
//! component that can handle events and render itself.

mod app;
mod cmd;
mod preview;
mod ui;

// Standard library imports
use std::{collections::HashMap, io, sync::atomic::Ordering, time::Duration};

// Third-party imports
use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use rat_focus::FocusBuilder;
use ratatui::{Terminal, prelude::*};
use serde_json::{Map, Value};
use tokio::{signal, sync::mpsc, task};

// Heroku-specific imports
use heroku_types::{CommandSpec, Field};
use heroku_util::{lex_shell_like, resolve_path};

// Local imports
use crate::{
    cmd::{Cmd, run_cmds},
    ui::{
        components::{
            BuilderComponent, HelpComponent, LogsComponent, TableComponent,
            component::Component,
            palette::{HintBarComponent, PaletteComponent},
        },
        main,
    },
};

/// Events that can be sent to the UI event loop.
///
/// This enum represents the different types of events that the UI can process,
/// including user input events and animation ticks for smooth UI updates.
enum UiEvent {
    /// User input event (keyboard, mouse, etc.)
    Input(Event),
    /// Animation tick for periodic UI updates
    Animate,
}

/// Convenience container for all top-level UI components.
struct UiComponents<'a> {
    palette: PaletteComponent,
    hint_bar: HintBarComponent<'a>,
    logs: LogsComponent,
    builder: BuilderComponent,
    help: HelpComponent,
    table: TableComponent<'a>,
}

impl<'a> UiComponents<'a> {
    fn new_initialized() -> Self {
        let mut palette = PaletteComponent::new();
        let _ = palette.init();

        let mut hint_bar = HintBarComponent::new();
        let _ = hint_bar.init();

        let mut logs = LogsComponent::new();
        let _ = logs.init();

        let mut builder = BuilderComponent::new();
        let _ = builder.init();

        let mut help = HelpComponent::new();
        let _ = help.init();

        let mut table = TableComponent::default();
        let _ = table.init();

        Self {
            palette,
            hint_bar,
            logs,
            builder,
            help,
            table,
        }
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn cleanup_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}

fn initial_render<'a>(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    application: &mut app::App,
    comps: &mut UiComponents<'a>,
) -> Result<()> {
    terminal.draw(|frame| {
        main::draw(
            frame,
            application,
            &mut comps.palette,
            &mut comps.hint_bar,
            &mut comps.logs,
            &mut comps.builder,
            &mut comps.help,
            &mut comps.table,
        )
    })?;
    Ok(())
}

fn render<'a>(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    application: &mut app::App,
    comps: &mut UiComponents<'a>,
) -> Result<()> {
    terminal.draw(|frame| {
        main::draw(
            frame,
            application,
            &mut comps.palette,
            &mut comps.hint_bar,
            &mut comps.logs,
            &mut comps.builder,
            &mut comps.help,
            &mut comps.table,
        )
    })?;
    Ok(())
}

fn spawn_ui_event_producer(
    active_execution_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ui_event_sender: mpsc::UnboundedSender<UiEvent>,
    animation_interval: Duration,
) {
    task::spawn_blocking(move || {
        loop {
            if active_execution_count.load(Ordering::Relaxed) > 0 {
                match crossterm::event::poll(animation_interval) {
                    Ok(true) => match crossterm::event::read() {
                        Ok(input_event) => {
                            if ui_event_sender.send(UiEvent::Input(input_event)).is_err() {
                                break;
                            }
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(10)),
                    },
                    Ok(false) => {
                        if ui_event_sender.send(UiEvent::Animate).is_err() {
                            break;
                        }
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(10)),
                }
            } else {
                match crossterm::event::read() {
                    Ok(input_event) => {
                        if ui_event_sender.send(UiEvent::Input(input_event)).is_err() {
                            break;
                        }
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(10)),
                }
            }
        }
    });
}

/// Runs the main TUI application loop.
///
/// This function initializes the terminal interface, sets up all UI components,
/// and runs the main event loop that handles user input, command execution,
/// and UI rendering.
///
/// # Arguments
///
/// * `registry` - The Heroku command registry containing all available commands
///
/// # Returns
///
/// Returns `Ok(())` if the application exits cleanly, or an error if there's
/// a terminal setup or runtime issue.
///
/// # Errors
///
/// This function can return errors for:
/// - Terminal setup failures (raw mode, alternate screen)
/// - Component initialization failures
/// - Event loop runtime errors
///
/// # Example
///
/// ```no_run
/// use heroku_registry::Registry;
/// use heroku_tui::run;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let registry = Registry::new();
///     run(registry).await
/// }
/// ```
pub async fn run(registry: heroku_registry::Registry) -> Result<()> {
    // Initialize the main application state
    let mut application = app::App::new(registry);
    let mut comps = UiComponents::new_initialized();

    // Terminal setup
    let mut terminal = setup_terminal()?;

    // Set up event handling
    let animation_interval = Duration::from_millis(200);
    let (ui_event_sender, mut ui_event_receiver) = mpsc::unbounded_channel::<UiEvent>();
    let active_execution_count = application.active_exec_count.clone();
    // Spawn a blocking task to handle input events and animation ticks
    spawn_ui_event_producer(active_execution_count, ui_event_sender, animation_interval);

    // Perform initial render so UI is visible before any events
    initial_render(&mut terminal, &mut application, &mut comps)?;

    // Main event loop
    loop {
        let mut needs_rerender = false;

        tokio::select! {
            // Handle UI events (input, animation)
            maybe_ui_event = ui_event_receiver.recv() => {
                if let Some(ui_event) = maybe_ui_event {
                    match ui_event {
                        UiEvent::Input(input_event) => match input_event {
                            Event::Key(key_event) => {
                                // Handle Ctrl+C for graceful shutdown
                                if key_event.code == KeyCode::Char('c') && key_event.modifiers.contains(KeyModifiers::CONTROL) {
                                    break;
                                }
                                // Process the key event and check if we should exit
                                if handle_key_event(
                                    &mut application,
                                    &mut comps.palette,
                                    &mut comps.builder,
                                    &mut comps.table,
                                    key_event,
                                )? {
                                    break;
                                }
                                needs_rerender = true;
                            }
                            Event::Resize(width, height) => {
                                let _ = application.update(app::Msg::Resize(width, height));
                                needs_rerender = true;
                            }
                            _ => {}
                        },
                        UiEvent::Animate => {
                            let _ = application.update(app::Msg::Tick);
                            needs_rerender = true;
                        }
                    }
                }
            }
            // Handle command execution completion
            maybe_execution_output = application.exec_receiver.recv() => {
                if let Some(execution_output) = maybe_execution_output {
                    let _ = application.update(app::Msg::ExecCompleted(execution_output));
                    needs_rerender = true;
                }
            }
            // Handle Ctrl+C signal
            _ = signal::ctrl_c() => {
                break;
            }
        }

        // Render the UI if needed
        if needs_rerender {
            render(&mut terminal, &mut application, &mut comps)?;
        }
    }

    // Cleanup: restore terminal to normal mode
    cleanup_terminal(&mut terminal)?;
    Ok(())
}

/// Handles keyboard input events and routes them to the appropriate UI components.
///
/// This function implements the main keyboard event routing logic for the TUI.
/// It determines which component should handle the key event based on the current
/// application state (visible modals, focus, etc.) and delegates the event
/// accordingly.
///
/// # Arguments
///
/// * `application` - The main application state
/// * `palette_component` - The command palette component
/// * `builder_component` - The command builder component
/// * `table_component` - The data table component
/// * `key_event` - The keyboard event to handle
///
/// # Returns
///
/// Returns `Ok(true)` if the application should exit, `Ok(false)` if it should
/// continue running, or an error if there was a processing issue.
///
/// # Event Routing Logic
///
/// 1. Global keys (Esc, Ctrl+F) are handled first
/// 2. Modal-specific routing (table, builder, logs detail)
/// 3. Focus-based routing between palette and logs
/// 4. Tab/Shift+Tab for focus cycling
fn handle_key_event(
    application: &mut app::App,
    palette_component: &mut PaletteComponent,
    builder_component: &mut BuilderComponent,
    table_component: &mut TableComponent,
    key_event: KeyEvent,
) -> Result<bool> {
    // First, check for global key mappings (Esc, Ctrl+F, etc.)
    if let Some(global_message) = map_key_to_global_message(application, &key_event) {
        let _ = application.update(global_message);
        return Ok(false);
    }

    // Route to table component when table modal is visible
    if application.table.is_visible() {
        let component_effects = table_component.handle_key_events(application, key_event);
        let commands = crate::cmd::from_effects(application, component_effects);
        crate::cmd::run_cmds(application, commands);
        return Ok(false);
    }

    // Handle Enter key in builder modal to close and populate palette
    if application.builder.is_visible() && key_event.code == KeyCode::Enter {
        handle_builder_enter(application);
        return Ok(false);
    }

    // Route to logs component when detail view is open
    if application.logs.detail.is_some() {
        let mut logs_component = LogsComponent::new();
        let component_effects = logs_component.handle_key_events(application, key_event);
        let commands = crate::cmd::from_effects(application, component_effects);
        crate::cmd::run_cmds(application, commands);
        return Ok(false);
    }

    // Handle focus management and component routing when not in builder
    if !application.builder.is_visible() {
        // Handle Tab/Shift+Tab for focus cycling between palette and logs
        if handle_focus_cycle(application, &key_event) {
            return Ok(false);
        }

        // Route to focused component
        if application.logs.focus.get() {
            let mut logs_component = LogsComponent::new();
            let component_effects = logs_component.handle_key_events(application, key_event);
            let commands = crate::cmd::from_effects(application, component_effects);
            crate::cmd::run_cmds(application, commands);
            return Ok(false);
        } else {
            let component_effects = palette_component.handle_key_events(application, key_event);
            let commands = crate::cmd::from_effects(application, component_effects);
            crate::cmd::run_cmds(application, commands);
            return Ok(false);
        }
    }

    // Route to builder component when builder is visible
    let component_effects = builder_component.handle_key_events(application, key_event);
    let commands = crate::cmd::from_effects(application, component_effects);
    crate::cmd::run_cmds(application, commands);
    Ok(false)
}

/// Maps global keyboard shortcuts to application messages.
///
/// This function handles application-wide keyboard shortcuts that should be
/// processed regardless of which component currently has focus. It maintains
/// the TEA (The Elm Architecture) pattern by converting key events to messages.
///
/// # Arguments
///
/// * `application` - The current application state
/// * `key_event` - The keyboard event to process
///
/// # Returns
///
/// Returns `Some(message)` if the key event maps to a global action,
/// or `None` if the event should be handled by focused components.
///
/// # Global Shortcuts
///
/// - `Esc`: Close any visible modal (help, table, builder)
/// - `Ctrl+F`: Toggle the command builder modal
fn map_key_to_global_message(application: &app::App, key_event: &KeyEvent) -> Option<app::Msg> {
    // Close any visible modal when Esc is pressed
    if (application.help.is_visible() || application.table.is_visible() || application.builder.is_visible())
        && key_event.code == KeyCode::Esc
    {
        return Some(app::Msg::CloseModal);
    }

    // Toggle command builder with Ctrl+F
    if key_event.code == KeyCode::Char('f') && key_event.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(app::Msg::ToggleBuilder);
    }

    // No global mapping found - let focused components handle the event
    None
}

fn handle_builder_enter(application: &mut app::App) {
    if let Some(command_spec) = application.builder.selected_command() {
        let command_line = build_palette_line_from_spec(command_spec, application.builder.input_fields());
        application.palette.set_input(command_line);
        application.palette.set_cursor(application.palette.input().len());
        application
            .palette
            .apply_build_suggestions(&application.ctx.registry, &application.ctx.providers);
    }
    application.builder.apply_visibility(false);
}

fn handle_focus_cycle(application: &app::App, key_event: &KeyEvent) -> bool {
    if (key_event.code == KeyCode::Tab || key_event.code == KeyCode::BackTab)
        && !key_event.modifiers.contains(KeyModifiers::CONTROL)
    {
        let palette_has_suggestions =
            application.palette.is_suggestions_open() || !application.palette.input().is_empty();

        if palette_has_suggestions && application.palette.focus.get() && key_event.code == KeyCode::Tab {
            // Let palette handle Tab for suggestion navigation/acceptance
            return false;
        }

        let mut focus_builder = FocusBuilder::new(None);
        focus_builder.widget(&application.palette);
        focus_builder.widget(&application.logs);
        let focus_ring = focus_builder.build();

        let _ = if key_event.code == KeyCode::Tab {
            focus_ring.next()
        } else {
            focus_ring.prev()
        };
        return true;
    }
    false
}

/// Builds a command line string from a command specification and input fields.
///
/// This function constructs a complete command line string that can be executed
/// or displayed in the palette. It combines the command group/name with positional
/// arguments and flags based on the provided field values.
///
/// # Arguments
///
/// * `command_spec` - The command specification containing group, name, and argument definitions
/// * `input_fields` - The user-provided field values for arguments and flags
///
/// # Returns
///
/// Returns a formatted command line string ready for execution.
///
/// # Format Rules
///
/// - Command format: `<group> <subcommand> [positional_args...] [--flags...]`
/// - Empty positional arguments are shown as `<arg_name>`
/// - Boolean flags are included only if set to true
/// - Non-boolean flags include their values
///
/// # Example
///
/// ```
/// // For spec with group="apps", name="info", and fields with app_id="my-app"
/// // Returns: "apps info my-app"
/// ```
fn build_palette_line_from_spec(command_spec: &CommandSpec, input_fields: &[Field]) -> String {
    let mut command_parts: Vec<String> = Vec::new();

    // Add command group and subcommand
    let command_group = &command_spec.group;
    let subcommand_name = &command_spec.name;
    command_parts.push(command_group.to_string());
    if !subcommand_name.is_empty() {
        command_parts.push(subcommand_name.to_string());
    }

    // Add positional arguments in order
    for positional_argument in &command_spec.positional_args {
        if let Some(input_field) = input_fields.iter().find(|field| field.name == positional_argument.name) {
            let field_value = input_field.value.trim();
            if field_value.is_empty() {
                command_parts.push(format!("<{}>", positional_argument.name));
            } else {
                command_parts.push(field_value.to_string());
            }
        } else {
            command_parts.push(format!("<{}>", positional_argument.name));
        }
    }

    // Add flags (non-positional fields)
    for input_field in input_fields.iter().filter(|field| {
        !command_spec
            .positional_args
            .iter()
            .any(|pos_arg| pos_arg.name == field.name)
    }) {
        if input_field.is_bool {
            if !input_field.value.is_empty() {
                command_parts.push(format!("--{}", input_field.name));
            }
        } else if !input_field.value.trim().is_empty() {
            command_parts.push(format!("--{}", input_field.name));
            command_parts.push(input_field.value.trim().to_string());
        }
    }

    command_parts.join(" ")
}

/// Parses command arguments and flags from input tokens.
///
/// This function processes the command line tokens after the group and subcommand,
/// separating positional arguments from flags and validating flag syntax.
///
/// # Arguments
///
/// * `argument_tokens` - The tokens after the group and subcommand
/// * `command_spec` - The command specification for validation
///
/// # Returns
///
/// Returns `Ok((flags, args))` where flags is a map of flag names to values
/// and args is a vector of positional arguments, or an error if parsing fails.
///
/// # Flag Parsing Rules
///
/// - `--flag=value` format is supported
/// - Boolean flags don't require values
/// - Non-boolean flags require values (next token or after =)
/// - Unknown flags are rejected
fn parse_command_arguments(
    argument_tokens: &[String],
    command_spec: &CommandSpec,
) -> Result<(HashMap<String, Option<String>>, Vec<String>), String> {
    let mut user_flags: HashMap<String, Option<String>> = HashMap::new();
    let mut user_args: Vec<String> = Vec::new();
    let mut index = 0;

    while index < argument_tokens.len() {
        let token = &argument_tokens[index];

        if token.starts_with("--") {
            let flag_name = token.trim_start_matches('-');

            // Handle --flag=value format
            if let Some(equals_pos) = flag_name.find('=') {
                let name = &flag_name[..equals_pos];
                let value = &flag_name[equals_pos + 1..];
                user_flags.insert(name.to_string(), Some(value.to_string()));
            } else {
                // Handle --flag or --flag value format
                if let Some(flag_spec) = command_spec.flags.iter().find(|f| f.name == flag_name) {
                    if flag_spec.r#type == "boolean" {
                        user_flags.insert(flag_name.to_string(), None);
                    } else {
                        // Non-boolean flag requires a value
                        if index + 1 < argument_tokens.len() && !argument_tokens[index + 1].starts_with('-') {
                            user_flags.insert(flag_name.to_string(), Some(argument_tokens[index + 1].to_string()));
                            index += 1; // Skip the value token
                        } else {
                            return Err(format!("Flag '--{}' requires a value", flag_name));
                        }
                    }
                } else {
                    return Err(format!("Unknown flag '--{}'", flag_name));
                }
            }
        } else {
            // Positional argument
            user_args.push(token.to_string());
        }

        index += 1;
    }

    Ok((user_flags, user_args))
}

/// Validates command arguments and flags against the command specification.
///
/// This function ensures that all required positional arguments and flags are
/// provided with appropriate values.
///
/// # Arguments
///
/// * `positional_arguments` - The provided positional arguments
/// * `user_flags` - The provided flags and their values
/// * `command_spec` - The command specification to validate against
///
/// # Returns
///
/// Returns `Ok(())` if validation passes, or an error message if validation fails.
///
/// # Validation Rules
///
/// - All required positional arguments must be provided
/// - All required flags must be present
/// - Non-boolean required flags must have non-empty values
fn validate_command_arguments(
    positional_arguments: &[String],
    user_flags: &HashMap<String, Option<String>>,
    command_spec: &CommandSpec,
) -> Result<(), String> {
    // Validate required positional arguments
    if positional_arguments.len() < command_spec.positional_args.len() {
        let missing_arguments: Vec<String> = command_spec.positional_args[positional_arguments.len()..]
            .iter()
            .map(|arg| arg.name.to_string())
            .collect();
        return Err(format!(
            "Missing required argument(s): {}",
            missing_arguments.join(", ")
        ));
    }

    // Validate required flags
    for flag_spec in &command_spec.flags {
        if flag_spec.required {
            if flag_spec.r#type == "boolean" {
                if !user_flags.contains_key(&flag_spec.name) {
                    return Err(format!("Missing required flag: --{}", flag_spec.name));
                }
            } else {
                match user_flags.get(&flag_spec.name) {
                    Some(Some(value)) if !value.is_empty() => {}
                    _ => {
                        return Err(format!("Missing required flag value: --{} <value>", flag_spec.name));
                    }
                }
            }
        }
    }

    Ok(())
}

/// Builds a JSON request body from user-provided flags.
///
/// This function converts the parsed flags into a JSON object that can be sent
/// as the request body for the HTTP command execution.
///
/// # Arguments
///
/// * `user_flags` - The flags provided by the user
/// * `command_spec` - The command specification for type information
///
/// # Returns
///
/// Returns a JSON Map containing the flag values with appropriate types.
///
/// # Type Conversion
///
/// - Boolean flags are converted to `true` if present
/// - String flags are converted to their string values
/// - Flags not in the specification are ignored
fn build_request_body(user_flags: HashMap<String, Option<String>>, command_spec: &CommandSpec) -> Map<String, Value> {
    let mut request_body = Map::new();

    for (flag_name, flag_value) in user_flags.into_iter() {
        if let Some(flag_spec) = command_spec.flags.iter().find(|f| f.name == flag_name) {
            if flag_spec.r#type == "boolean" {
                request_body.insert(flag_name, Value::Bool(true));
            } else if let Some(value) = flag_value {
                request_body.insert(flag_name, Value::String(value));
            }
        }
    }

    request_body
}

/// Executes a command from the palette input.
///
/// This function parses the current palette input, validates the command and its
/// arguments, and initiates the HTTP execution. It handles command parsing,
/// argument validation, and sets up the execution context for the command.
///
/// # Arguments
///
/// * `application` - The application state containing the palette input and registry
///
/// # Returns
///
/// Returns `Ok(command_spec)` if the command is valid and execution is started,
/// or `Err(error_message)` if there are validation errors.
///
/// # Validation
///
/// The function validates:
/// - Command format (group subcommand)
/// - Required positional arguments
/// - Required flags and their values
/// - Flag syntax and types
///
/// # Execution Context
///
/// After validation, the function:
/// - Resolves the command path with positional arguments
/// - Builds the request body with flag values
/// - Stores execution context for pagination and replay
/// - Initiates background HTTP execution
///
/// # Example
///
/// ```
/// // For input "apps info my-app --verbose"
/// // Validates command exists, app_id is provided, starts execution
/// ```
pub fn start_palette_execution(application: &mut app::App) -> Result<CommandSpec, String> {
    // Step 1: Parse and validate the palette input
    let input_owned = application.palette.input().to_string();
    let input = input_owned.trim().to_string();
    if input.is_empty() {
        return Err("Empty command input. Type a command (e.g., apps info)".into());
    }
    // Tokenize the input using shell-like parsing for consistent behavior
    let tokens = lex_shell_like(&input);
    if tokens.len() < 2 {
        return Err(format!(
            "Incomplete command '{}'. Use '<group> <sub>' format (e.g., apps info)",
            input
        ));
    }

    // Step 2: Find the command specification in the registry
    let command_spec = application
        .ctx
        .registry
        .commands
        .iter()
        .find(|command| command.group == tokens[0] && command.name == tokens[1])
        .cloned()
        .ok_or_else(|| {
            format!(
                "Unknown command '{} {}'. Check available commands with help.",
                tokens[0], tokens[1]
            )
        })?;

    // Step 3: Parse command arguments and flags from input tokens
    let (user_flags, user_args) = parse_command_arguments(&tokens[2..], &command_spec)?;

    // Step 4: Validate command arguments and flags
    validate_command_arguments(&user_args, &user_flags, &command_spec)?;

    // Step 5: Build request body from flags
    let request_body = build_request_body(user_flags, &command_spec);

    // Step 6: Persist execution context for pagination UI and replay
    persist_execution_context(application, &command_spec, &request_body, &input);
    // Step 7: Execute the command via background HTTP request
    execute_command(application, &command_spec, &request_body, &user_args);
    Ok(command_spec)
}

fn persist_execution_context(
    application: &mut app::App,
    command_spec: &CommandSpec,
    request_body: &Map<String, Value>,
    input: &str,
) {
    application.last_command_ranges = Some(command_spec.ranges.clone());
    application.last_spec = Some(command_spec.clone());
    application.last_body = Some(request_body.clone());

    let init_field = request_body
        .get("range-field")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let init_start = request_body
        .get("range-start")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let init_end = request_body
        .get("range-end")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let init_order = request_body
        .get("order")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let init_max = request_body
        .get("max")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<usize>().ok());
    let initial_range = init_field.map(|field| {
        let mut h = format!("{} {}..{}", field, init_start, init_end);
        if let Some(ord) = init_order {
            h.push_str(&format!("; order={};", ord));
        }
        if let Some(m) = init_max {
            h.push_str(&format!("; max={};", m));
        }
        h
    });

    application.initial_range = initial_range.clone();
    application.pagination_history.clear();
    application.pagination_history.push(initial_range);
    application.palette.push_history_if_needed(input);
}

fn execute_command(
    application: &mut app::App,
    command_spec: &CommandSpec,
    request_body: &Map<String, Value>,
    user_args: &[String],
) {
    let mut command_spec_to_run = command_spec.clone();
    let mut positional_argument_map: HashMap<String, String> = HashMap::new();
    for (index, positional_argument) in command_spec.positional_args.iter().enumerate() {
        positional_argument_map.insert(
            positional_argument.name.clone(),
            user_args.get(index).cloned().unwrap_or_default(),
        );
    }

    command_spec_to_run.path = resolve_path(&command_spec.path, &positional_argument_map);
    run_cmds(
        application,
        vec![Cmd::ExecuteHttp(Box::new(command_spec_to_run), request_body.clone())],
    );
}
