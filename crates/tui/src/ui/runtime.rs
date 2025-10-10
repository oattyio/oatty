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
//! - Smart ticking: fast interval (125 ms) only while animating; long interval
//!   (5 s) when idle. `App::update(Msg::Tick)` marks dirty only on visible
//!   changes.
//!
//! Entry Point
//! - `run_app(registry)` is called from `lib::run` and performs setup,
//!   event processing, and teardown.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Result;
use crossterm::event::MouseEventKind;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::{StreamExt, stream::FuturesUnordered};
use heroku_mcp::PluginEngine;
use heroku_types::{Effect, ExecOutcome, Msg};
use ratatui::{Terminal, prelude::*};
use tokio::task::JoinHandle;
use tokio::{
    signal,
    sync::mpsc,
    time::{self, MissedTickBehavior},
};

use crate::app::App;
use crate::ui::components::component::Component;
use crate::{cmd, ui::main};
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
    let (sender, receiver) = mpsc::channel(500);
    tokio::spawn(async move {
        loop {
            if event::poll(Duration::from_millis(10)).expect("poll failed") {
                match event::read() {
                    Ok(event) => {
                        let send = if let Some(mouse_event) = event.as_mouse_event() {
                            match mouse_event.kind {
                                MouseEventKind::Down(_) => true,
                                _ => false,
                            }
                        } else {
                            true
                        };
                        if send && let Err(e) = sender.send(event).await {
                            tracing::warn!("Failed to send event: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read event: {}", e);
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
/// Returns a ratatui `Terminal` backed by Crossterm for later drawing.
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
fn render<'a>(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>, application: &mut App) -> Result<()> {
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
async fn handle_input_event<'a>(
    app: &mut App<'_>,
    input_event: Event,
    pending_execs: &mut FuturesUnordered<JoinHandle<ExecOutcome>>,
) -> Result<LoopAction> {
    match input_event {
        Event::Key(key_event) => {
            if key_event.code == KeyCode::Char('c') && key_event.modifiers.contains(KeyModifiers::CONTROL) {
                return Ok(LoopAction::Exit);
            }
            handle_delegate_event(app, Event::Key(key_event), pending_execs).await?;
        }
        Event::Mouse(mouse_event) => {
            handle_delegate_event(app, Event::Mouse(mouse_event), pending_execs).await?;
        }
        Event::Resize(width, height) => {
            let _ = app.update(Msg::Resize(width, height));
        }
        // Avoid marking dirty for ignored events
        Event::FocusGained | Event::FocusLost | Event::Paste(_) => {}
    }
    Ok(LoopAction::Continue)
}
///
async fn handle_delegate_event(
    app: &mut App<'_>,
    event: Event,
    pending_execs: &mut FuturesUnordered<JoinHandle<ExecOutcome>>,
) -> Result<()> {
    // Temporarily take components to avoid borrow checker issues
    let mut open_modal = std::mem::take(&mut app.open_modal);
    let mut main_view = std::mem::take(&mut app.main_view);
    let mut nav_bar = std::mem::take(&mut app.nav_bar_view);

    let mut effects = Vec::new();
    if event.is_key() {
        let Event::Key(key_event) = event else { return Ok(()) };
        let Some(view) = get_target_view(app, main_view.as_mut(), open_modal.as_mut(), nav_bar.as_mut()) else {
            return Ok(());
        };
        effects.extend(view.handle_key_events(app, key_event));
    }
    if event.is_mouse() {
        let Event::Mouse(mouse_event) = event else { return Ok(()) };
        if let Some(nav_bar) = nav_bar.as_mut() {
            effects.extend(nav_bar.handle_mouse_events(app, mouse_event))
        }
        if let Some(main) = main_view.as_mut() {
            effects.extend(main.handle_mouse_events(app, mouse_event))
        }
        if let Some(modal) = open_modal.as_mut() {
            effects.extend(modal.handle_mouse_events(app, mouse_event))
        }
    }

    // Move components back
    app.main_view = main_view;
    app.open_modal = open_modal;
    app.nav_bar_view = nav_bar;

    // Run the effects
    process_effects(app, effects, pending_execs).await;
    Ok(())
}

fn get_target_view<'a>(
    app: &mut App,
    maybe_view: Option<&'a mut Box<dyn Component>>,
    maybe_modal: Option<&'a mut Box<dyn Component>>,
    nav_bar: Option<&'a mut Box<dyn Component>>,
) -> Option<&'a mut Box<dyn Component>> {
    if maybe_modal.is_some() {
        return maybe_modal;
    }
    let nav_has_focus = app.nav_bar.container_focus.get() || app.nav_bar.item_focus_flags.iter().any(|f| f.get());
    if nav_has_focus {
        return nav_bar;
    }
    maybe_view
}

/// Entry point for the TUI runtime: sets up the terminal, spawns the event
/// producer, runs the async event loop, and performs cleanup on exit.
pub async fn run_app(registry: Arc<Mutex<heroku_registry::Registry>>, plugin_engine: Arc<PluginEngine>) -> Result<()> {
    let mut app = App::new(registry, plugin_engine);
    let mut terminal = setup_terminal()?;

    // Input comes from a dedicated blocking thread to ensure reliability.
    let mut input_receiver = spawn_input_thread().await;
    let mut pending_execs: FuturesUnordered<JoinHandle<ExecOutcome>> = FuturesUnordered::new();

    // Ticking strategy: fast while animating, very slow when idle.
    let fast_interval = Duration::from_millis(125);
    let idle_interval = Duration::from_millis(5000);
    let mut current_interval = idle_interval;
    let mut ticker = time::interval(current_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    render(&mut terminal, &mut app)?;
    // run initialization effects
    process_effects(&mut app, vec![Effect::PluginsLoadRequested], &mut pending_execs).await;
    // Track the last known terminal size to synthesize Resize messages when
    // some terminals fail to emit them reliably (e.g., certain iTerm2 setups).
    let mut last_size: Option<(u16, u16)> = crossterm::terminal::size().ok();

    loop {
        // Determine if we need animation ticks and adjust the ticker dynamically.
        let needs_animation = app.executing || app.palette.is_provider_loading();
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
                    match handle_input_event(&mut app, event, &mut pending_execs).await? {
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
                    let effects = app.update(Msg::Tick);
                    process_effects(&mut app, effects, &mut pending_execs).await;
                    needs_render = true;
                }
            }

            Some(joined) = pending_execs.next(), if !pending_execs.is_empty() => {
                let outcome = joined.unwrap_or_else(|error| ExecOutcome::Log(format!("Execution task failed: {error}")));
                let follow_up = app.update(Msg::ExecCompleted(Box::new(outcome)));
                process_effects(&mut app, follow_up, &mut pending_execs).await;
                needs_render = true;
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
                let _ = app.update(Msg::Resize(w, h));
            }
        }

        // Render if dirty
        if needs_render {
            render(&mut terminal, &mut app)?;
        }
    }

    cleanup_terminal(&mut terminal)?;
    Ok(())
}

async fn process_effects(app: &mut App<'_>, effects: Vec<Effect>, pending_execs: &mut FuturesUnordered<JoinHandle<ExecOutcome>>) {
    if effects.is_empty() {
        return;
    }

    let command_batch = cmd::run_from_effects(app, effects).await;
    if !command_batch.pending.is_empty() {
        pending_execs.extend(command_batch.pending);
    }

    if command_batch.immediate.is_empty() {
        return;
    }

    let mut follow_up_effects: Vec<Effect> = Vec::new();
    for outcome in command_batch.immediate {
        follow_up_effects.extend(app.update(Msg::ExecCompleted(Box::new(outcome))));
    }
    if follow_up_effects.is_empty() {
        return;
    }
    Box::pin(process_effects(app, follow_up_effects, pending_execs)).await;
}
