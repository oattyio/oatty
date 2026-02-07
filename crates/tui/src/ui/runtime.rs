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
use anyhow::Result;
use crossterm::event::MouseEventKind;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::{StreamExt, stream::FuturesUnordered};
use oatty_mcp::PluginEngine;
use oatty_registry::CommandRegistry;
use oatty_types::{Effect, ExecOutcome, Msg};
use ratatui::{Terminal, prelude::*};
use std::rc::Rc;
use std::time::Instant;
use std::{
    sync::atomic::Ordering,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::task::JoinHandle;
use tokio::{
    signal,
    sync::mpsc,
    time::{self, MissedTickBehavior},
};

use crate::app::{App, WorkflowRunEventReceiver};
use crate::cmd;
use crate::ui::components::LibraryComponent;
use crate::ui::components::component::Component;
use crate::ui::main_component::MainView;
use rat_focus::FocusBuilder;

/// Spawn a dedicated input thread that blocks on terminal input and forwards
/// `crossterm` events over a Tokio channel.
///
/// Keeping `poll()` and `read()` on the same OS thread avoids lost or delayed
/// events in some terminals. We call `read()` directly and never use `poll()`
/// here — the blocking behavior is isolated to this thread.
async fn spawn_input_thread() -> mpsc::Receiver<Event> {
    let (sender, receiver) = mpsc::channel(500);
    let mut las_mouse_event: Option<Instant> = Some(Instant::now());

    tokio::spawn(async move {
        let sixteen_ms = Duration::from_millis(16);
        loop {
            if event::poll(sixteen_ms).is_ok() {
                match event::read() {
                    Ok(event) => {
                        // Throttle mouse moves events to once per 16 ms.
                        let is_mouse_move = event.as_mouse_event().is_some_and(|e| e.kind == MouseEventKind::Moved);
                        let should_send = !is_mouse_move || las_mouse_event.is_some_and(|last| last.elapsed() >= sixteen_ms);
                        if is_mouse_move && should_send {
                            las_mouse_event = Some(Instant::now());
                        }

                        if should_send && let Err(e) = sender.send(event).await {
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
fn render(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>, app: &mut App, main_view: &mut MainView) -> Result<()> {
    // Rebuild focus just before rendering so structure changes are reflected
    let old_focus = std::mem::take(&mut app.focus);
    app.focus = Rc::new(FocusBuilder::rebuild_for(app, Some(Rc::unwrap_or_clone(old_focus))));
    if app.focus.focused().is_none() {
        main_view.restore_focus(app);
    }
    terminal.draw(|frame| main_view.render(frame, frame.area(), app))?;
    Ok(())
}

/// Handle raw crossterm input events and update `App`/components.
/// Returns `Exit` for Ctrl+C, otherwise `Continue`.
fn handle_input_event(app: &mut App<'_>, main_view: &mut MainView, input_event: Event) -> Vec<Effect> {
    match input_event {
        Event::Key(key_event) => main_view.handle_key_events(app, key_event),
        Event::Mouse(mouse_event) => main_view.handle_mouse_events(app, mouse_event),
        Event::Resize(width, height) => main_view.handle_message(app, Msg::Resize(width, height)),

        Event::FocusGained | Event::FocusLost | Event::Paste(_) => Vec::new(),
    }
}

/// Entry point for the TUI runtime: sets up the terminal, spawns the event
/// producer, runs the async event loop, and performs cleanup on exit.
pub async fn run_app(registry: Arc<Mutex<CommandRegistry>>, plugin_engine: Arc<PluginEngine>) -> Result<()> {
    // Input comes from a dedicated blocking thread to ensure reliability.
    let mut input_receiver = spawn_input_thread().await;
    let mut main_view = MainView::new(Some(Box::new(LibraryComponent::default())));

    let mut app = App::new(registry, plugin_engine);
    let mut terminal = setup_terminal()?;

    let mut pending_execs: FuturesUnordered<JoinHandle<ExecOutcome>> = FuturesUnordered::new();
    let mut effects: Vec<Effect> = Vec::with_capacity(5);
    // Defer plugin loading until the main loop runs, so secret interpolation
    // prompts cannot stall startup before the event loop is active.
    effects.push(Effect::PluginsLoadRequested);

    let mcp_config = app.ctx.plugin_engine.config_snapshot().await;
    if let Some(bind_address) = mcp_config.http_server.bind_address.as_ref() {
        app.mcp_http_server.set_configured_bind_address(bind_address.clone());
    }
    app.mcp_http_server.auto_start = mcp_config.http_server.auto_start;
    if app.mcp_http_server.auto_start {
        app.mcp_http_server.mark_starting();
        effects.push(Effect::McpHttpServerStart);
    }

    let mut workflow_events: Option<WorkflowRunEventReceiver> = None;
    let mut mcp_http_logs = app.take_pending_mcp_http_logs();

    // Ticking strategy: fast while animating, very slow when idle.
    let fast_interval = Duration::from_millis(100);
    let idle_interval = Duration::from_millis(5000);
    let mut current_interval = idle_interval;
    let mut ticker = time::interval(current_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    render(&mut terminal, &mut app, &mut main_view)?;

    // Track the last known terminal size to synthesize Resize messages when
    // some terminals fail to emit them reliably (e.g., certain iTerm2 setups).
    let mut last_size: Option<(u16, u16)> = crossterm::terminal::size().ok();

    loop {
        // Determine if we need animation ticks and adjust the ticker dynamically.
        // note this is a candidate for optimization; it does not scale well.
        let needs_animation = app.executing || !effects.is_empty() || app.palette.is_provider_loading() || app.workflows.is_running();
        let target_interval = if needs_animation { fast_interval } else { idle_interval };
        if target_interval != current_interval {
            current_interval = target_interval;
            ticker = time::interval(current_interval);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        }
        let mut needs_render = false;
        tokio::select! {
            maybe_log = async {
                match mcp_http_logs.as_mut() {
                    Some(receiver) => receiver.recv().await,
                    None => None,
                }
            }, if mcp_http_logs.is_some() => {
                match maybe_log {
                    Some(entry) => {
                        app.append_mcp_http_log_entry(entry);
                        needs_render = true;
                    }
                    None => {
                        mcp_http_logs = None;
                    }
                }
            }
            // Terminal input events
            maybe_event = input_receiver.recv() => {
                if let Some(event) = maybe_event {
                    if let Event::Key(key_event) = event
                        && key_event.code == KeyCode::Char('c') && key_event.modifiers.contains(KeyModifiers::CONTROL) {
                            break;
                        }
                    effects.extend(handle_input_event(&mut app, &mut main_view, event));
                } else {
                    // Input channel closed; break out to shut down cleanly.
                    break;
                }
                needs_render = true;
            }

            // Periodic animation tick
            _ = ticker.tick() => {
                effects.extend(main_view.handle_message(&mut app, Msg::Tick));
                needs_render = needs_animation || !effects.is_empty();
                if !effects.is_empty() {
                    // move effects out of their Vec to avoid processing new effects while processing current ones
                    let mut effects_to_process = Vec::with_capacity(effects.len());
                    effects_to_process.append(&mut effects);

                    handle_navigation_effects(&mut app, &mut main_view, &mut effects_to_process, &mut effects);
                    process_effects(&mut app, &mut main_view, effects_to_process, &mut pending_execs, &mut effects).await;
                }
            }

            Some(joined) = pending_execs.next(), if !pending_execs.is_empty() => {
                let outcome = joined.unwrap_or_else(|error| ExecOutcome::Log(format!("Execution task failed: {error}")));
                match outcome {
                    ExecOutcome::ProviderValues { provider_id, cache_key, .. } => {
                        effects.extend(main_view.handle_message(&mut app, Msg::ProviderValuesReady { provider_id, cache_key }));
                    }
                    other => {
                        effects.extend(main_view.handle_message(&mut app, Msg::ExecCompleted(Box::new(other))));
                    }
                }
                let still_running = app.active_exec_count.load(Ordering::Relaxed) > 0 || !pending_execs.is_empty();
                app.executing = still_running;
                if !still_running {
                    app.throbber_idx = 0;
                }
                needs_render = true;
            }

            maybe_run_event = async {
                if let Some(receiver) = workflow_events.as_mut() {
                    receiver.receiver.recv().await.map(|event| (receiver.run_id.clone(), event))
                } else {
                    None
                }
            }, if workflow_events.is_some() => {
                match maybe_run_event {
                    Some((run_id, event)) => {
                        effects.extend(main_view.handle_message(&mut app, Msg::WorkflowRunEvent { run_id, event }));
                        needs_render = true;
                    }
                    None => {
                        workflow_events = None;
                    }
                }
            }
            // Handle Ctrl+C
            _ = signal::ctrl_c() => { break; }
        }

        if let Some(new_receiver) = app.take_pending_workflow_events() {
            workflow_events = Some(new_receiver);
        }
        if let Some(new_receiver) = app.take_pending_mcp_http_logs() {
            mcp_http_logs = Some(new_receiver);
        }

        // Fallback: detect terminal size changes even if no explicit Resize
        // event was received. This handles terminals that miss SIGWINCH or
        // drop resize notifications during interactive operations.
        if let Ok((w, h)) = crossterm::terminal::size()
            && last_size != Some((w, h))
        {
            last_size = Some((w, h));
            let _ = app.update(&Msg::Resize(w, h));
        }

        // Render if dirty
        if needs_render {
            render(&mut terminal, &mut app, &mut main_view)?;
        }
    }

    if let Some(runtime) = app.mcp_http_server_runtime.take()
        && let Err(error) = runtime.stop().await
    {
        tracing::warn!("Failed to stop MCP HTTP server during shutdown: {}", error);
    }
    cleanup_terminal(&mut terminal)?;
    Ok(())
}

fn handle_navigation_effects(app: &mut App<'_>, main_view: &mut MainView, effects: &mut Vec<Effect>, queued_effects: &mut Vec<Effect>) {
    let navigation_effects = effects
        .extract_if(0.., |effect| {
            matches!(effect, Effect::SwitchTo(_) | Effect::ShowModal(_) | Effect::CloseModal)
        })
        .collect::<Vec<Effect>>();

    for effect in navigation_effects {
        match effect {
            Effect::SwitchTo(route) => {
                if let Some(mut view) = main_view.content_view.take() {
                    queued_effects.extend(view.on_route_exit(app));
                }
                main_view.set_current_route(app, route);
                if let Some(view) = main_view.content_view.as_mut() {
                    queued_effects.extend(view.on_route_enter(app));
                }
            }
            Effect::ShowModal(modal) => {
                if let Some((mut view, _)) = main_view.modal_view.take() {
                    queued_effects.extend(view.on_route_enter(app));
                }
                main_view.set_open_modal_kind(app, Some(modal));
                if let Some((view, _)) = main_view.modal_view.as_mut() {
                    queued_effects.extend(view.on_route_enter(app));
                }
            }
            Effect::CloseModal => {
                if let Some((mut view, _)) = main_view.modal_view.take() {
                    queued_effects.extend(view.on_route_enter(app));
                }
                main_view.set_open_modal_kind(app, None);
            }
            _ => {}
        }
    }
}

async fn process_effects(
    app: &mut App<'_>,
    main_view: &mut MainView,
    effects: Vec<Effect>,
    pending_execs: &mut FuturesUnordered<JoinHandle<ExecOutcome>>,
    effects_out: &mut Vec<Effect>,
) {
    if effects.is_empty() {
        return;
    }

    let command_batch = cmd::run_from_effects(app, effects).await;
    if !command_batch.pending.is_empty() {
        let was_executing = app.executing;
        pending_execs.extend(command_batch.pending);
        if !was_executing {
            app.throbber_idx = 0;
        }
        app.executing = true;
    }

    if command_batch.immediate.is_empty() {
        if app.active_exec_count.load(Ordering::Relaxed) == 0 && pending_execs.is_empty() {
            app.executing = false;
        }
        return;
    }

    for outcome in command_batch.immediate {
        let still_executing = app.active_exec_count.load(Ordering::Relaxed) > 0 || !pending_execs.is_empty();
        app.executing = still_executing;
        match outcome {
            ExecOutcome::ProviderValues {
                provider_id, cache_key, ..
            } => {
                effects_out.extend(main_view.handle_message(app, Msg::ProviderValuesReady { provider_id, cache_key }));
            }
            ExecOutcome::Message(msg) => {
                effects_out.extend(main_view.handle_message(app, msg));
            }
            other => {
                effects_out.extend(main_view.handle_message(app, Msg::ExecCompleted(Box::new(other))));
            }
        }
    }
}
