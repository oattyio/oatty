//! Runtime: unified event loop and input routing for the TUI.
//!
//! Responsibilities
//! - Own the terminal lifecycle (enter/leave alternate screen, raw mode).
//! - Drive a single, efficient event loop that handles input and animations.
//! - Route keys to focused components and execute returned `Effect`s.
//! - Render via `ui::main::draw` only when `App` marks itself dirty.
//!
//! Unified Event Loop Strategy
//! - Single loop eliminates dual-loop architecture that caused CPU bleed-off.
//! - Dedicated input thread blocks on `crossterm::event::read()` and forwards
//!   events over a channel, avoiding cross-thread poll/read issues and ensuring
//!   reliable resize delivery across terminals (including iTerm2).
//! - Smart ticking: fast interval (125ms) only while animating; long interval
//!   (5s) when idle. `App::update(Msg::Tick)` marks dirty only on visible
//!   changes.
//!
//! Entry Point
//! - `run_app(registry)` is called from `lib::run` and performs setup,
//!   event processing, and teardown.

use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, read},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use heroku_types::Msg;
use ratatui::{Terminal, prelude::*};
use tokio::{
    signal,
    sync::mpsc,
    task::spawn_blocking,
    time::{self, MissedTickBehavior},
};

use crate::ui::components::component::Component;
use crate::ui::components::nav_bar::VerticalNavBarComponent;
use crate::{app, cmd, ui::main};
use rat_focus::FocusBuilder;

/// Control flow signal for the main loop
enum LoopAction {
    Continue,
    Exit,
}

/// Spawn a dedicated input thread that blocks on terminal input and forwards
/// `crossterm` events over a Tokio channel.
///
/// Keeping `poll()` and `read()` on the same OS thread avoids lost or delayed
/// events in some terminals. We call `read()` directly and never use `poll()`
/// here — the blocking behavior is isolated to this thread.
async fn spawn_input_thread() -> mpsc::Receiver<Event> {
    let (sender, receiver) = mpsc::channel(25);
    tokio::spawn(async move {
        loop {
            if event::poll(Duration::from_millis(10)).expect("poll failed") {
                match event::read() {
                    Ok(event) => {
                        if let Err(e) = sender.send(event).await {
                            eprintln!("Failed to send event: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read event: {}", e);
                        break;
                    }
                }
            }
        }
    });
    receiver
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

/// Renders a frame by delegating to `ui::main::draw`.
fn render<'a>(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>, application: &mut app::App) -> Result<()> {
    // Rebuild focus just before rendering so structure changes are reflected
    let old_focus = std::mem::take(&mut application.focus);
    application.focus = FocusBuilder::rebuild_for(application, Some(old_focus));
    if application.focus.focused().is_none() {
        application.restore_focus();
    }
    terminal.draw(|frame| main::draw(frame, application))?;
    Ok(())
}

/// Handle raw crossterm input events and update `App`/components.
/// Returns `Exit` for Ctrl+C, otherwise `Continue`.
fn handle_input_event<'a>(app: &mut app::App, input_event: Event) -> Result<LoopAction> {
    match input_event {
        Event::Key(key_event) => {
            // Ctrl-C: if plugins fullscreen active, exit that mode; otherwise exit app
            if key_event.code == KeyCode::Char('c') && key_event.modifiers.contains(KeyModifiers::CONTROL) {
                return Ok(LoopAction::Exit);
            }

            // Temporarily take components to avoid borrow checker issues
            let mut open_modal = std::mem::take(&mut app.open_modal);
            let mut main_view = std::mem::take(&mut app.main_view);

            let effects = if let Some(modal) = open_modal.as_mut() {
                modal.handle_key_events(app, key_event)
            } else if let Some(current) = main_view.as_mut() {
                // Route to nav bar when it (or any of its items) has focus; otherwise to current view
                let nav_has_focus =
                    app.nav_bar.container_focus.get() || app.nav_bar.item_focus_flags.iter().any(|f| f.get());
                if nav_has_focus {
                    let mut nav = VerticalNavBarComponent::new();
                    nav.handle_key_events(app, key_event)
                } else {
                    current.handle_key_events(app, key_event)
                }
            } else {
                Vec::new()
            };

            // Move components back if they were't replaced
            if app.main_view.is_none() {
                app.main_view = main_view;
            }
            if app.open_modal.is_none() {
                app.open_modal = open_modal;
            }

            // Run the effects
            cmd::run_from_effects(app, effects);
        }
        Event::Resize(width, height) => {
            let _ = app.update(Msg::Resize(width, height));
        }
        // Avoid marking dirty for mouse movement and other ignored events
        Event::Mouse(_) | Event::FocusGained | Event::FocusLost | Event::Paste(_) => {}
    }
    Ok(LoopAction::Continue)
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

/// When pressing Enter in the browser, populate the palette with the
/// constructed command and close the command browser.
fn handle_browser_enter(application: &mut app::App) {
    if let Some(command_spec) = application.browser.selected_command() {
        let command_line = build_palette_line_from_spec(command_spec, application.browser.input_fields());
        application.palette.set_input(command_line);
        application.palette.set_cursor(application.palette.input().len());
        application.palette.apply_build_suggestions(
            &application.ctx.registry,
            &application.ctx.providers,
            &*application.ctx.theme,
        );
    }
}

/// Entry point for the TUI runtime: sets up terminal, spawns the event
/// producer, runs the async event loop, and performs cleanup on exit.
pub async fn run_app(registry: heroku_registry::Registry) -> Result<()> {
    let mut application = app::App::new(registry);
    let mut terminal = setup_terminal()?;

    // Input comes from a dedicated blocking thread to ensure reliability.
    let mut input_receiver = spawn_input_thread().await;

    // Ticking strategy: fast while animating, very slow when idle.
    let fast_interval = Duration::from_millis(125);
    let idle_interval = Duration::from_millis(5000);
    let mut current_interval = idle_interval;
    let mut ticker = time::interval(current_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    render(&mut terminal, &mut application)?;

    // Track the last known terminal size to synthesize Resize messages when
    // some terminals fail to emit them reliably (e.g., certain iTerm2 setups).
    let mut last_size: Option<(u16, u16)> = crossterm::terminal::size().ok();

    loop {
        // Determine if we need animation ticks and adjust ticker dynamically.
        let needs_animation = application.executing || application.palette.is_provider_loading();
        let target_interval = if needs_animation { fast_interval } else { idle_interval };
        if target_interval != current_interval {
            current_interval = target_interval;
            ticker = time::interval(current_interval);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        }
        let mut needs_render = false;
        tokio::select! {
            // Terminal input events
            maybe_event = input_receiver.recv() => {
                if let Some(event) = maybe_event {
                    match handle_input_event(&mut application, event)? {
                        LoopAction::Continue => {}
                        LoopAction::Exit => return Ok(()),
                    }
                } else {
                    // Input channel closed; break out to shut down cleanly.
                    break;
                }
                needs_render = true;
            }

            // Periodic animation tick
            _ = ticker.tick() => {
                if needs_animation {
                    let effects = application.update(Msg::Tick);
                    cmd::run_from_effects(&mut application, effects);
                }
            }

            // Handle execution results
            maybe_execution_output = application.exec_receiver.recv() => {
                if let Some(execution_output) = maybe_execution_output {
                    let effects = application.update(Msg::ExecCompleted(execution_output));
                    cmd::run_from_effects(&mut application, effects);
                }
            }

            // Handle Ctrl+C
            _ = signal::ctrl_c() => { break; }
        }

        // Fallback: detect terminal size changes even if no explicit Resize
        // event was received. This handles terminals that miss SIGWINCH or
        // drop resize notifications during interactive operations.
        if let Ok((w, h)) = crossterm::terminal::size() {
            if last_size != Some((w, h)) {
                last_size = Some((w, h));
                let _ = application.update(Msg::Resize(w, h));
            }
        }

        // Render if dirty
        if needs_render {
            render(&mut terminal, &mut application)?;
        }
    }

    cleanup_terminal(&mut terminal)?;
    Ok(())
}
