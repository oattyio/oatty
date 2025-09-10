//! Runtime: event loop and input routing for the TUI.
//!
//! Responsibilities
//! - Own the terminal lifecycle (enter/leave alternate screen, raw mode).
//! - Drive the async event loop: input + periodic animation ticks.
//! - Route keys to focused components and execute returned `Effect`s.
//! - Render via `ui::main::draw` only when `App` marks itself dirty.
//!
//! Rendering Strategy
//! - The event pump emits a constant tick (default 125ms) to support
//!   smooth animations like throbbers.
//! - `App::update(Msg::Tick)` advances animation state and calls
//!   `mark_dirty()` only when a visible change occurred.
//! - The main loop calls `application.take_dirty()` to decide when to draw,
//!   preventing unnecessary renders when idle.
//!
//! Entry Point
//! - `run_app(registry)` is called from `lib::run` and performs setup,
//!   event processing, and teardown.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use rat_focus::FocusBuilder;
use ratatui::{Terminal, prelude::*};
use tokio::{signal, sync::mpsc, task};

use crate::{
    app, cmd,
    ui::{
        components::{
            BuilderComponent, HelpComponent, LogsComponent, PluginsComponent, TableComponent,
            component::Component,
            palette::{HintBarComponent, PaletteComponent},
        },
        main,
    },
};

/// Events that can be sent to the UI event loop.
/// Events flowing from the event producer into the async UI loop.
///
/// Wraps raw crossterm `Event`s and a periodic `Animate` tick used to
/// advance animations and refresh provider-driven suggestions.
enum UiEvent {
    /// User input event (keyboard, mouse, etc.)
    Input(Event),
    /// Animation tick for periodic UI updates
    Animate,
}

/// Control flow signal for the main loop
/// Control flow signal from handlers to the main loop.
enum LoopAction {
    Continue,
    Exit,
}
/// Convenience container for all top-level UI components.
/// Avoid performance penalty by using concrete types.
/// e.g. do not use Vec<Box<Component>> and iterate.
/// Container of top-level UI components constructed once and reused.
///
/// Using concrete types avoids dynamic dispatch overhead when routing input
/// and drawing.
#[derive(Debug, Default)]
struct UiComponents<'a> {
    palette: PaletteComponent,
    hint_bar: HintBarComponent<'a>,
    logs: LogsComponent,
    builder: BuilderComponent,
    help: HelpComponent,
    table: TableComponent<'a>,
    plugins: PluginsComponent<'a>,
}

/// Put the terminal into raw mode and enter the alternate screen.
///
/// Returns a ratatui `Terminal` backed by Crossterm for subsequent drawing.
fn setup_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore terminal settings and leave the alternate screen.
fn cleanup_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}

/// Performs the very first draw before the event loop runs.
fn initial_render<'a>(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
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
            &mut comps.plugins,
        )
    })?;
    Ok(())
}

/// Renders a frame by delegating to `ui::main::draw`.
fn render<'a>(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
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
            &mut comps.plugins,
        )
    })?;
    Ok(())
}

/// Blocking producer that polls for terminal input and emits periodic ticks.
///
/// The poll interval is read dynamically from `interval_ms`, allowing the
/// runtime to slow down when idle and speed up while animating.
fn spawn_ui_event_producer(
    ui_event_sender: mpsc::UnboundedSender<UiEvent>,
    interval_ms: Arc<std::sync::atomic::AtomicU64>,
) {
    let _ = Arc::clone(&interval_ms); // keep for move semantics clarity
    task::spawn_blocking(move || {
        loop {
            // Read the desired interval dynamically to reduce idle wake-ups
            let ms = interval_ms.load(Ordering::Relaxed);
            let poll_interval = Duration::from_millis(ms.max(1));

            match crossterm::event::poll(poll_interval) {
                Ok(true) => match crossterm::event::read() {
                    Ok(input_event) => {
                        if ui_event_sender.send(UiEvent::Input(input_event)).is_err() {
                            break;
                        }
                    }
                    // Backoff on read errors to avoid busy looping
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                },
                Ok(false) => {
                    if ui_event_sender.send(UiEvent::Animate).is_err() {
                        break;
                    }
                }
                // Backoff on poll errors to avoid frequent wake-ups under error
                Err(_) => std::thread::sleep(Duration::from_millis(100)),
            }
        }
    });
}

/// Adjust the event producer's poll interval based on runtime state.
fn update_animation_interval(
    application: &app::App,
    interval_ms: &std::sync::atomic::AtomicU64,
    fast_interval_ms: u64,
    slow_interval_ms: u64,
) {
    let needs_animation = application.executing || application.palette.is_provider_loading();
    let target = if needs_animation {
        fast_interval_ms
    } else {
        slow_interval_ms
    };
    if interval_ms.load(Ordering::Relaxed) != target {
        interval_ms.store(target, Ordering::Relaxed);
    }
}

/// Handle raw crossterm input events and update `App`/components.
/// Returns `Exit` for Ctrl+C, otherwise `Continue`.
fn handle_input_event<'a>(
    application: &mut app::App,
    comps: &mut UiComponents<'a>,
    input_event: Event,
) -> Result<LoopAction> {
    match input_event {
        Event::Key(key_event) => {
            // Ctrl-C: if plugins fullscreen active, exit that mode; otherwise exit app
            if key_event.code == KeyCode::Char('c') && key_event.modifiers.contains(KeyModifiers::CONTROL) {
                if application.plugins_fullscreen {
                    application.plugins_fullscreen = false;
                    application.mark_dirty();
                    return Ok(LoopAction::Continue);
                } else {
                    return Ok(LoopAction::Exit);
                }
            }
            // Ctrl-P: enter plugins fullscreen
            if key_event.code == KeyCode::Char('p') && key_event.modifiers.contains(KeyModifiers::CONTROL) {
                application.plugins_fullscreen = true;
                // Load plugins if empty
                if application.plugins.items.is_empty() {
                    run_component_effects(application, vec![app::Effect::PluginsLoadRequested]);
                }
                // Focus search
                let ring = application.plugins.focus_ring();
                ring.focus(&application.plugins.search_flag);
                application.mark_dirty();
                return Ok(LoopAction::Continue);
            }
            let _ = handle_key_event(
                application,
                &mut comps.palette,
                &mut comps.builder,
                &mut comps.table,
                &mut comps.logs,
                &mut comps.plugins,
                key_event,
            )?;
            application.mark_dirty();
        }
        Event::Resize(width, height) => {
            let _ = application.update(app::Msg::Resize(width, height));
        }
        // Avoid marking dirty for mouse movement and other ignored events
        Event::Mouse(_) | Event::FocusGained | Event::FocusLost | Event::Paste(_) => {}
    }
    Ok(LoopAction::Continue)
}

/// Handle a high-level UI event (`Input` or `Animate`).
/// Converts to app state changes and returns loop control.
fn handle_ui_event<'a>(
    application: &mut app::App,
    comps: &mut UiComponents<'a>,
    ui_event: UiEvent,
) -> Result<LoopAction> {
    match ui_event {
        UiEvent::Input(input_event) => handle_input_event(application, comps, input_event),
        UiEvent::Animate => {
            let effects = application.update(app::Msg::Tick);
            run_component_effects(application, effects);
            Ok(LoopAction::Continue)
        }
    }
}

/// Translate component `Effect`s into runnable `Cmd`s and execute them.
/// Translate component `Effect`s into `Cmd`s and run them, mutating `App`.
fn run_component_effects(application: &mut app::App, effects: Vec<app::Effect>) {
    let commands = cmd::from_effects(application, effects);
    cmd::run_cmds(application, commands);
}

/// Key routing for the table modal.
/// Route table-related keystrokes to the table component and run effects.
fn handle_table_keys(application: &mut app::App, table: &mut TableComponent, key: KeyEvent) {
    let effects = table.handle_key_events(application, key);
    run_component_effects(application, effects);
}

/// Key routing for the logs view and details overlay.
/// Route logs-related keystrokes (including detail view) and run effects.
fn handle_logs_keys(application: &mut app::App, logs: &mut LogsComponent, key: KeyEvent) {
    let effects = logs.handle_key_events(application, key);
    run_component_effects(application, effects);
}

/// Key routing for the command palette.
/// Route palette keystrokes and run effects.
fn handle_palette_keys(application: &mut app::App, palette: &mut PaletteComponent, key: KeyEvent) {
    let effects = palette.handle_key_events(application, key);
    run_component_effects(application, effects);
}

/// Key routing for the command builder modal.
/// Route builder keystrokes and run effects.
fn handle_builder_keys(application: &mut app::App, builder: &mut BuilderComponent, key: KeyEvent) {
    let effects = builder.handle_key_events(application, key);
    run_component_effects(application, effects);
}

/// Handles Tab/BackTab focus cycling between focusable root widgets.
/// Returns true if focus changed and event is consumed.
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

/// Maps global shortcuts (Esc, Ctrl+F) to high-level application messages.
fn map_key_to_global_message(application: &app::App, key_event: &KeyEvent) -> Option<app::Msg> {
    if (application.help.is_visible()
        || application.table.is_visible()
        || application.builder.is_visible()
        || application.plugins.is_visible())
        && key_event.code == KeyCode::Esc
    {
        return Some(app::Msg::CloseModal);
    }
    if key_event.code == KeyCode::Char('f') && key_event.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(app::Msg::ToggleBuilder);
    }
    if key_event.code == KeyCode::F(9) {
        return Some(app::Msg::TogglePlugins);
    }
    None
}

/// Builds a command line string from a spec and current builder inputs.
fn build_palette_line_from_spec(
    command_spec: &heroku_types::CommandSpec,
    input_fields: &[heroku_types::Field],
) -> String {
    let mut command_parts: Vec<String> = Vec::new();
    let command_group = &command_spec.group;
    let subcommand_name = &command_spec.name;
    command_parts.push(command_group.to_string());
    if !subcommand_name.is_empty() {
        command_parts.push(subcommand_name.to_string());
    }
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

/// When pressing Enter in the builder, populate the palette with the
/// constructed command and close the builder modal.
fn handle_builder_enter(application: &mut app::App) {
    if let Some(command_spec) = application.builder.selected_command() {
        let command_line = build_palette_line_from_spec(command_spec, application.builder.input_fields());
        application.palette.set_input(command_line);
        application.palette.set_cursor(application.palette.input().len());
        application.palette.apply_build_suggestions(
            &application.ctx.registry,
            &application.ctx.providers,
            &*application.ctx.theme,
        );
    }
    application.builder.apply_visibility(false);
}

/// Central key routing based on current UI state and focus.
/// Returns Ok(true) when the app should exit, otherwise Ok(false).
fn handle_key_event(
    application: &mut app::App,
    palette_component: &mut PaletteComponent,
    builder_component: &mut BuilderComponent,
    table_component: &mut TableComponent,
    logs_component: &mut LogsComponent,
    plugins_component: &mut PluginsComponent,
    key_event: KeyEvent,
) -> Result<bool> {
    if let Some(global_message) = map_key_to_global_message(application, &key_event) {
        let _ = application.update(global_message);
        // After toggling Plugins overlay via F9, trigger initial load if needed
        if matches!(key_event.code, KeyCode::F(9)) && application.plugins.is_visible() {
            // Only load when list is empty to avoid re-reading repeatedly
            if application.plugins.items.is_empty() {
                run_component_effects(application, vec![app::Effect::PluginsLoadRequested]);
            }
            // Normalize focus to search when opened for parity with Builder
            let ring = application.plugins.focus_ring();
            ring.focus(&application.plugins.search_flag);
        }
        return Ok(false);
    }
    if application.plugins_fullscreen || application.plugins.is_visible() {
        let effects = plugins_component.handle_key_events(application, key_event);
        run_component_effects(application, effects);
        return Ok(false);
    }
    if application.table.is_visible() {
        handle_table_keys(application, table_component, key_event);
        return Ok(false);
    }
    if application.builder.is_visible() && key_event.code == KeyCode::Enter {
        handle_builder_enter(application);
        return Ok(false);
    }
    if application.logs.detail.is_some() {
        handle_logs_keys(application, logs_component, key_event);
        return Ok(false);
    }
    if !application.builder.is_visible() {
        if handle_focus_cycle(application, &key_event) {
            return Ok(false);
        }
        if application.logs.focus.get() {
            handle_logs_keys(application, logs_component, key_event);
        } else {
            handle_palette_keys(application, palette_component, key_event);
        }
        return Ok(false);
    }
    handle_builder_keys(application, builder_component, key_event);
    Ok(false)
}

/// Entry point for the TUI runtime: sets up terminal, spawns the event
/// producer, runs the async event loop, and performs cleanup on exit.
pub async fn run_app(registry: heroku_registry::Registry) -> Result<()> {
    let mut application = app::App::new(registry);
    let mut comps = UiComponents::default();

    let mut terminal = setup_terminal()?;

    // Initialize MCP supervisor (non-blocking if it fails; UI can still run)
    if application.ctx.mcp.is_none() {
        match McpSupervisor::new().await {
            Ok(sup) => {
                if let Err(e) = sup.start().await {
                    application
                        .logs
                        .entries
                        .push(format!("MCP supervisor start failed: {}", e));
                }
                application.ctx.mcp = Some(Arc::new(sup));
            }
            Err(e) => {
                // Best-effort log: show a line in the logs area
                application
                    .logs
                    .entries
                    .push(format!("MCP supervisor init failed: {}", e));
            }
        }
    }

    // Target ~8 FPS for spinners without wasting CPU. Rendering is
    // skipped if no frame advanced, so this remains efficient.
    let fast_interval_ms: u64 = 125;
    let slow_interval_ms: u64 = 1000;
    // Shared knob for the producer to dynamically adjust poll interval
    let interval_ms = Arc::new(std::sync::atomic::AtomicU64::new(fast_interval_ms));
    let (ui_event_sender, mut ui_event_receiver) = mpsc::unbounded_channel::<UiEvent>();
    spawn_ui_event_producer(ui_event_sender, Arc::clone(&interval_ms));

    initial_render(&mut terminal, &mut application, &mut comps)?;

    loop {
        tokio::select! {
            maybe_ui_event = ui_event_receiver.recv() => {
                if let Some(ui_event) = maybe_ui_event {
                    match handle_ui_event(&mut application, &mut comps, ui_event)? {
                        LoopAction::Continue => {}
                        LoopAction::Exit => break,
                    }
                }
            }
            maybe_execution_output = application.exec_receiver.recv() => {
                if let Some(execution_output) = maybe_execution_output {
                    let _ = application.update(app::Msg::ExecCompleted(execution_output));
                }
            }
            _ = signal::ctrl_c() => { break; }
        }

        // Dynamically adapt the animation interval based on runtime state
        update_animation_interval(&application, &interval_ms, fast_interval_ms, slow_interval_ms);

        if application.take_dirty() {
            render(&mut terminal, &mut application, &mut comps)?;
        }
    }

    cleanup_terminal(&mut terminal)?;
    Ok(())
}
