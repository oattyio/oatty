mod app;
mod cmd;
mod preview;
mod ui;

use std::{collections::HashMap, io, sync::atomic::Ordering, time::Duration};

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use heroku_types::{CommandSpec, Field};
use heroku_util::lex_shell_like;
use rat_focus::FocusBuilder;
use ratatui::{Terminal, prelude::*};
use serde_json::{Map, Value};
use tokio::{signal, sync::mpsc, task};

use crate::{
    cmd::{Cmd, run_cmds},
    preview::resolve_path,
    ui::{
        components::{
            BuilderComponent, HelpComponent, LogsComponent, TableComponent,
            component::Component,
            palette::{HintBarComponent, PaletteComponent},
        },
        main,
    },
};

enum UiEvent {
    Input(Event),
    Animate,
}

pub async fn run(registry: heroku_registry::Registry) -> Result<()> {
    let mut app = app::App::new(registry);
    let mut palette_component = PaletteComponent::new();
    let _ = palette_component.init();
    let mut hint_bar_component = HintBarComponent::new();
    let _ = hint_bar_component.init();

    let mut logs_component = LogsComponent::new();
    let _ = logs_component.init();
    let mut builder_component = BuilderComponent::new();
    let _ = builder_component.init();
    let mut help_component = HelpComponent::new();
    let _ = help_component.init();
    let mut table_component = TableComponent::default();
    let _ = table_component.init();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let animate_rate = Duration::from_millis(200);
    let (ui_tx, mut ui_rx) = mpsc::unbounded_channel::<UiEvent>();
    let active = app.active_exec_count.clone();
    task::spawn_blocking(move || {
        loop {
            if active.load(Ordering::Relaxed) > 0 {
                match crossterm::event::poll(animate_rate) {
                    Ok(true) => match crossterm::event::read() {
                        Ok(ev) => {
                            if ui_tx.send(UiEvent::Input(ev)).is_err() {
                                break;
                            }
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(10)),
                    },
                    Ok(false) => {
                        if ui_tx.send(UiEvent::Animate).is_err() {
                            break;
                        }
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(10)),
                }
            } else {
                match crossterm::event::read() {
                    Ok(ev) => {
                        if ui_tx.send(UiEvent::Input(ev)).is_err() {
                            break;
                        }
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(10)),
                }
            }
        }
    });

    // Initial render so UI is visible before any events
    terminal.draw(|frame| {
        main::draw(
            frame,
            &mut app,
            &mut palette_component,
            &mut hint_bar_component,
            &mut logs_component,
            &mut builder_component,
            &mut help_component,
            &mut table_component,
        )
    })?;

    loop {
        let mut should_render = false;

        tokio::select! {
            maybe_event = ui_rx.recv() => {
                if let Some(event) = maybe_event {
                    match event {
                        UiEvent::Input(event) => match event {
                        Event::Key(key) => {
                            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                                break;
                            }
                            if handle_key(
                                &mut app,
                                &mut palette_component,
                                &mut builder_component,
                                &mut table_component,
                                key,
                            )? {
                                break;
                            }
                            should_render = true;
                        }
                        Event::Resize(w, h) => {
                            let _ = app.update(app::Msg::Resize(w, h));
                            should_render = true;
                        }
                        _ => {}
                        },
                        UiEvent::Animate => {
                            let _ = app.update(app::Msg::Tick);
                            should_render = true;
                        }
                    }
                }
            }
            maybe_out = app.exec_receiver.recv() => {
                if let Some(out) = maybe_out {
                    let _ = app.update(app::Msg::ExecCompleted(out));
                    should_render = true;
                }
            }
            _ = signal::ctrl_c() => {
                break;
            }
        }

        if should_render {
            terminal.draw(|frame| {
                main::draw(
                    frame,
                    &mut app,
                    &mut palette_component,
                    &mut hint_bar_component,
                    &mut logs_component,
                    &mut builder_component,
                    &mut help_component,
                    &mut table_component,
                )
            })?;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}

fn handle_key(
    app: &mut app::App,
    palette: &mut PaletteComponent,
    builder: &mut BuilderComponent,
    table: &mut TableComponent,
    key: KeyEvent,
) -> Result<bool> {
    // First, map common/global keys to messages and handle them uniformly
    if let Some(msg) = map_key_to_msg(app, &key) {
        let _ = app.update(msg);
        return Ok(false);
    }
    // When table modal is visible, route keys to the table component (local-first
    // handling)
    if app.table.is_visible() {
        let effects = table.handle_key_events(app, key);
        let cmds = crate::cmd::from_effects(app, effects);
        crate::cmd::run_cmds(app, cmds);
        return Ok(false);
    }
    // If builder modal open, Enter should close builder and populate palette with
    // constructed command
    if app.builder.is_visible() && key.code == KeyCode::Enter {
        if let Some(spec) = app.builder.selected_command() {
            let line = palette_line_from_spec(spec, app.builder.input_fields());
            app.palette.set_input(line);
            app.palette.set_cursor(app.palette.input().len());
            app.palette
                .apply_build_suggestions(&app.ctx.registry, &app.ctx.providers);
        }
        app.builder.apply_visibility(false);
        return Ok(false);
    }
    // Logs detail takes precedence for navigation/copy while open
    if app.logs.detail.is_some() {
        let mut logs = LogsComponent::new();
        let effects = logs.handle_key_events(app, key);
        let cmds = crate::cmd::from_effects(app, effects);
        crate::cmd::run_cmds(app, cmds);
        return Ok(false);
    }

    // Default palette/logs interaction when not in builder
    if !app.builder.is_visible() {
        // Top-level focus toggle with Tab / Shift+Tab using rat-focus for root
        if (key.code == KeyCode::Tab || key.code == KeyCode::BackTab) && !key.modifiers.contains(KeyModifiers::CONTROL)
        {
            let palette_busy = app.palette.is_suggestions_open() || !app.palette.input().is_empty();
            if palette_busy && app.palette.focus.get() && key.code == KeyCode::Tab {
                // Let palette handle Tab for suggestions/accept
            } else {
                let mut b = FocusBuilder::new(None);
                b.widget(&app.palette);
                b.widget(&app.logs);
                let focus = b.build();
                let _ = if key.code == KeyCode::Tab {
                    focus.next()
                } else {
                    focus.prev()
                };
                return Ok(false);
            }
        }

        if app.logs.focus.get() {
            let mut logs = LogsComponent::new();
            let effects = logs.handle_key_events(app, key);
            let cmds = crate::cmd::from_effects(app, effects);
            crate::cmd::run_cmds(app, cmds);
            return Ok(false);
        } else {
            let effects = palette.handle_key_events(app, key);
            let cmds = crate::cmd::from_effects(app, effects);
            crate::cmd::run_cmds(app, cmds);
            return Ok(false);
        }
    }

    let effects = builder.handle_key_events(app, key);
    let cmds = crate::cmd::from_effects(app, effects);
    crate::cmd::run_cmds(app, cmds);
    Ok(false)
}

// Map common/global keys to simple messages so the main loop stays
// TEA-friendly.
fn map_key_to_msg(app: &app::App, key: &KeyEvent) -> Option<app::Msg> {
    // Close any modal on Esc
    if (app.help.is_visible() || app.table.is_visible() || app.builder.is_visible()) && key.code == KeyCode::Esc {
        return Some(app::Msg::CloseModal);
    }
    // Toggle builder with Ctrl+F
    if key.code == KeyCode::Char('f') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(app::Msg::ToggleBuilder);
    }
    // Table modal keys are handled in TableComponent::handle_key
    None
}

// Accept a non-command suggestion (flag/value) without clobbering the resolved
// command (group sub). Rules:
// - If cursor is at a new token position (ends with space), insert suggestion +
//   trailing space.
// - If current token starts with '-' or previous token is a flag expecting a
//   value → replace token.
// - Otherwise (we're on the command tokens or a positional token) → append
//   suggestion separated by space.
// moved palette suggestion helpers to crate::palette
fn palette_line_from_spec(spec: &CommandSpec, fields: &[Field]) -> String {
    let mut parts: Vec<String> = Vec::new();
    // Convert spec.name (group:rest) to execution form: "group rest"
    let group = &spec.group;
    let rest = &spec.name;
    parts.push(group.to_string());
    if !rest.is_empty() {
        parts.push(rest.to_string());
    }
    // positionals in order
    for positional_arg in &spec.positional_args {
        if let Some(field) = fields.iter().find(|f| f.name == positional_arg.name) {
            let v = field.value.trim();
            if v.is_empty() {
                parts.push(format!("<{}>", positional_arg.name));
            } else {
                parts.push(v.to_string());
            }
        } else {
            parts.push(format!("<{}>", positional_arg.name));
        }
    }
    // flags
    for field in fields
        .iter()
        .filter(|f| !spec.positional_args.iter().any(|p| p.name == f.name))
    {
        if field.is_bool {
            if !field.value.is_empty() {
                parts.push(format!("--{}", field.name));
            }
        } else if !field.value.trim().is_empty() {
            parts.push(format!("--{}", field.name));
            parts.push(field.value.trim().to_string());
        }
    }
    parts.join(" ")
}

pub fn start_palette_execution(app: &mut app::App) -> Result<CommandSpec, String> {
    // Parse input into tokens: expect "group sub [args...]"
    let input_owned = app.palette.input().to_string();
    let input = input_owned.trim().to_string();
    if input.is_empty() {
        return Err("Type a command (e.g., apps info)".into());
    }
    // Use palette tokenizer to keep quoting behavior consistent across modules
    let tokens = lex_shell_like(&input);
    if tokens.len() < 2 {
        return Err("Incomplete command. Use '<group> <sub>' (e.g., apps info)".into());
    }

    let spec = app
        .ctx
        .registry
        .commands
        .iter()
        .find(|c| c.group == tokens[0] && c.name == tokens[1])
        .cloned()
        .ok_or_else(|| format!("Unknown command '{} {}'", tokens[0], tokens[1]))?;

    // Parse flags/args from tokens after first two
    let parts = &tokens[2..];
    let mut user_flags: HashMap<String, Option<String>> = HashMap::new();
    let mut user_args: Vec<String> = Vec::new();
    let mut i = 0;
    while i < parts.len() {
        let t = &parts[i];
        if t.starts_with("--") {
            let long = t.trim_start_matches('-');
            // Equals form
            if let Some(eq) = long.find('=') {
                let name = &long[..eq];
                let val = &long[eq + 1..];
                user_flags.insert(name.to_string(), Some(val.to_string()));
            } else {
                // Boolean or expects a value
                if let Some(flag) = spec.flags.iter().find(|f| f.name == long) {
                    if flag.r#type == "boolean" {
                        user_flags.insert(long.to_string(), None);
                    } else {
                        // Next token is value if present and not another flag
                        if i + 1 < parts.len() && !parts[i + 1].starts_with('-') {
                            user_flags.insert(long.to_string(), Some(parts[i + 1].to_string()));
                            i += 1;
                        } else {
                            return Err(format!("Flag '--{}' requires a value", long));
                        }
                    }
                } else {
                    return Err(format!("Unknown flag '--{}'", long));
                }
            }
        } else {
            user_args.push(t.to_string());
        }
        i += 1;
    }

    // Validate required positionals
    if user_args.len() < spec.positional_args.len() {
        let missing: Vec<String> = spec.positional_args[user_args.len()..]
            .iter()
            .map(|p| p.name.to_string())
            .collect();
        return Err(format!("Missing required argument(s): {}", missing.join(", ")));
    }
    // Validate required flags
    for flag in &spec.flags {
        if flag.required {
            if flag.r#type == "boolean" {
                if !user_flags.contains_key(&flag.name) {
                    return Err(format!("Missing required flag: --{}", flag.name));
                }
            } else {
                match user_flags.get(&flag.name) {
                    Some(Some(v)) if !v.is_empty() => {}
                    _ => {
                        return Err(format!("Missing required flag value: --{} <value>", flag.name));
                    }
                }
            }
        }
    }

    // Build positional map and body
    let mut pos_map: HashMap<String, String> = HashMap::new();
    for (i, pa) in spec.positional_args.iter().enumerate() {
        pos_map.insert(pa.name.clone(), user_args.get(i).cloned().unwrap_or_default());
    }
    let mut body = Map::new();
    for (name, maybe_val) in user_flags.into_iter() {
        if let Some(flag) = spec.flags.iter().find(|f| f.name == name) {
            if flag.r#type == "boolean" {
                body.insert(name, Value::Bool(true));
            } else if let Some(v) = maybe_val {
                body.insert(name, Value::String(v));
            }
        }
    }

    let path = resolve_path(&spec.path, &pos_map);
    // Persist ranges and last-execution context for pagination UI and replay
    app.last_command_ranges = Some(spec.ranges.clone());
    app.last_spec = Some(spec.clone());
    app.last_path = Some(path.clone());
    app.last_body = Some(body.clone());
    // Compute and persist initial Range header used (if any)
    let init_field = body
        .get("range-field")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let init_start = body.get("range-start").and_then(|v| v.as_str()).unwrap_or("").trim();
    let init_end = body.get("range-end").and_then(|v| v.as_str()).unwrap_or("").trim();
    let init_order = body
        .get("order")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let init_max = body
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
    app.initial_range = initial_range.clone();
    app.pagination_history.clear();
    app.pagination_history.push(initial_range);
    // Add to palette history before executing
    app.palette.push_history_if_needed(&input);
    // Live request: enqueue background HTTP execution via Cmd system
    run_cmds(app, vec![Cmd::ExecuteHttp(Box::new(spec.clone()), path, body)]);
    Ok(spec)
}
