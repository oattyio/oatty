mod app;
mod component;
mod palette;
mod palette_comp;
mod preview;
mod tables;
mod theme;
mod ui;

use crate::component::Component;
use anyhow::Result;
use arboard::Clipboard;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*, Terminal};
use std::io;
use std::time::{Duration, Instant};

pub fn run(registry: heroku_registry::Registry) -> Result<()> {
    let mut app = app::App::new(registry);
    let mut palette_component = ui::components::PaletteComponent::new();
    let _ = palette_component.init();
    let mut hint_bar_component = ui::components::HintBarComponent::new();
    let _ = hint_bar_component.init();
    let mut steps_component = ui::components::StepsComponent::new();
    let _ = steps_component.init();
    let mut logs_component = ui::components::LogsComponent::new();
    let _ = logs_component.init();
    let mut builder_component = ui::components::BuilderComponent::new();
    let _ = builder_component.init();
    let mut help_component = ui::components::HelpComponent::new();
    let _ = help_component.init();
    let mut table_component = ui::components::TableComponent::new();
    let _ = table_component.init();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();

    loop {
        // Check for async execution completion and route through TEA message
        if let Some(rx) = app.exec_rx.as_ref() {
            if let Ok(out) = rx.try_recv() {
                let _ = app.update(app::Msg::ExecCompleted(out));
            }
        }
        terminal.draw(|f| {
            ui::draw(
                f,
                &mut app,
                &mut palette_component,
                &mut hint_bar_component,
                &mut steps_component,
                &mut logs_component,
                &mut builder_component,
                &mut help_component,
                &mut table_component,
            )
        })?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        break;
                    }
                    if handle_key(
                        &mut app,
                        &mut palette_component,
                        &mut builder_component,
                        key,
                    )? {
                        break;
                    }
                }
                Event::Resize(w, h) => {
                    let _ = app.update(app::Msg::Resize(w, h));
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
            let _ = app.update(app::Msg::Tick);
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn handle_key(
    app: &mut app::App,
    palette: &mut ui::components::PaletteComponent,
    builder: &mut ui::components::BuilderComponent,
    key: KeyEvent,
) -> Result<bool> {
    // First, map common/global keys to messages and handle them uniformly
    if let Some(msg) = map_key_to_msg(app, &key) {
        let _ = app.update(msg);
        return Ok(false);
    }
    // If builder modal open, Enter should close builder and populate palette with constructed command
    if app.show_builder && key.code == KeyCode::Enter {
        if let Some(spec) = &app.picked {
            let line = palette_line_from_spec(spec, &app.fields);
            app.palette.input = line;
            app.palette.cursor = app.palette.input.len();
            crate::palette::build_suggestions(&mut app.palette, &app.registry, &app.providers);
        }
        app.show_builder = false;
        return Ok(false);
    }
    // Default palette interaction when not in builder
    if !app.show_builder {
        if palette.handle_key(app, key)? {
            return Ok(false);
        }
        return Ok(false);
    }

    let effects = builder.handle_key(app, key)?;
    for eff in effects {
        match eff {
            app::Effect::CopyCommandRequested => {
                if let Some(spec) = &app.picked {
                    let cmd = crate::preview::cli_preview(spec, &app.fields);
                    match Clipboard::new().and_then(|mut cb| cb.set_text(cmd.clone())) {
                        Ok(()) => app.logs.push(format!("Copied: {}", cmd)),
                        Err(e) => app.logs.push(format!("Clipboard error: {}", e)),
                    }
                    if app.logs.len() > 500 {
                        let _ = app.logs.drain(0..app.logs.len() - 500);
                    }
                }
            }
        }
    }
    Ok(false)
}

// Map common/global keys to simple messages so the main loop stays TEA-friendly.
fn map_key_to_msg(app: &app::App, key: &KeyEvent) -> Option<app::Msg> {
    // Close any modal on Esc
    if (app.show_help || app.show_table || app.show_builder) && key.code == KeyCode::Esc {
        return Some(app::Msg::CloseModal);
    }
    // Toggle builder with Ctrl+F
    if key.code == KeyCode::Char('f') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(app::Msg::ToggleBuilder);
    }
    // When table modal is open, support scrolling and toggles
    if app.show_table {
        return match key.code {
            KeyCode::Up => Some(app::Msg::TableScroll(-1)),
            KeyCode::Down => Some(app::Msg::TableScroll(1)),
            KeyCode::PageUp => Some(app::Msg::TableScroll(-10)),
            KeyCode::PageDown => Some(app::Msg::TableScroll(10)),
            KeyCode::Home => Some(app::Msg::TableHome),
            KeyCode::End => Some(app::Msg::TableEnd),
            KeyCode::Char('t') => Some(app::Msg::ToggleTable),
            _ => None,
        };
    }
    None
}

// Accept a non-command suggestion (flag/value) without clobbering the resolved command (group sub).
// Rules:
// - If cursor is at a new token position (ends with space), insert suggestion + trailing space.
// - If current token starts with '-' or previous token is a flag expecting a value → replace token.
// - Otherwise (we're on the command tokens or a positional token) → append suggestion separated by space.
// moved palette suggestion helpers to crate::palette
fn palette_line_from_spec(
    spec: &heroku_registry::CommandSpec,
    fields: &[crate::app::Field],
) -> String {
    let mut parts: Vec<String> = Vec::new();
    // Convert spec.name (group:rest) to execution form: "group rest"
    let mut split = spec.name.splitn(2, ':');
    let group = split.next().unwrap_or("");
    let rest = split.next().unwrap_or("");
    parts.push(group.to_string());
    if !rest.is_empty() {
        parts.push(rest.to_string());
    }
    // positionals in order
    for p in &spec.positional_args {
        if let Some(f) = fields.iter().find(|f| &f.name == p) {
            let v = f.value.trim();
            if v.is_empty() {
                parts.push(format!("<{}>", p));
            } else {
                parts.push(v.to_string());
            }
        } else {
            parts.push(format!("<{}>", p));
        }
    }
    // flags
    for f in fields
        .iter()
        .filter(|f| !spec.positional_args.iter().any(|p| p == &f.name))
    {
        if f.is_bool {
            if !f.value.is_empty() {
                parts.push(format!("--{}", f.name));
            }
        } else if !f.value.trim().is_empty() {
            parts.push(format!("--{}", f.name));
            parts.push(f.value.trim().to_string());
        }
    }
    parts.join(" ")
}

pub fn start_palette_execution(app: &mut app::App) -> Result<(), String> {
    // Parse input into tokens: expect "group sub [args...]"
    let input = app.palette.input.trim();
    if input.is_empty() {
        return Err("Type a command (e.g., apps info)".into());
    }
    // Use palette tokenizer to keep quoting behavior consistent across modules
    let tokens = crate::palette::lex_shell_like(input);
    if tokens.len() < 2 {
        return Err("Incomplete command. Use '<group> <sub>' (e.g., apps info)".into());
    }

    let spec = app
        .registry
        .commands
        .iter()
        .find(|c| c.group == tokens[0] && c.name == tokens[1])
        .cloned()
        .ok_or_else(|| format!("Unknown command '{} {}'", tokens[0], tokens[1]))?;

    // Parse flags/args from tokens after first two
    let parts = &tokens[2..];
    let mut user_flags: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
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
                if let Some(fspec) = spec.flags.iter().find(|f| f.name == long) {
                    if fspec.r#type == "boolean" {
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
            .map(|s| s.to_string())
            .collect();
        return Err(format!(
            "Missing required argument(s): {}",
            missing.join(", ")
        ));
    }
    // Validate required flags
    for f in &spec.flags {
        if f.required {
            if f.r#type == "boolean" {
                if !user_flags.contains_key(&f.name) {
                    return Err(format!("Missing required flag: --{}", f.name));
                }
            } else {
                match user_flags.get(&f.name) {
                    Some(Some(v)) if !v.is_empty() => {}
                    _ => return Err(format!("Missing required flag value: --{} <VALUE>", f.name)),
                }
            }
        }
    }

    // Build positional map and body
    let mut pos_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for (i, name) in spec.positional_args.iter().enumerate() {
        pos_map.insert(name.clone(), user_args.get(i).cloned().unwrap_or_default());
    }
    let mut body = serde_json::Map::new();
    for (name, maybe_val) in user_flags.into_iter() {
        if let Some(fspec) = spec.flags.iter().find(|f| f.name == name) {
            if fspec.r#type == "boolean" {
                body.insert(name, serde_json::Value::Bool(true));
            } else if let Some(v) = maybe_val {
                body.insert(name, serde_json::Value::String(v));
            }
        }
    }

    let path = crate::preview::resolve_path(&spec.path, &pos_map);
    let cli_line = format!("heroku {}", app.palette.input.trim());

    let should_dry_run = app.debug_enabled && app.dry_run;
    if should_dry_run {
        let req = crate::preview::request_preview(&spec, &path, &body);
        app.logs.push(format!("Dry-run:\n{}\n{}", cli_line, req));
        if app.logs.len() > 500 {
            let _ = app.logs.drain(0..app.logs.len() - 500);
        }
        // Show demo table for GET collections in debug to visualize
        if spec.method == "GET" && !spec.path.ends_with('}') {
            app.result_json = Some(crate::tables::sample_apps());
            app.show_table = true;
            app.table_offset = 0;
        }
        // Clear input for next command
        app.palette.input.clear();
        app.palette.cursor = 0;
        app.palette.suggestions.clear();
        app.palette.popup_open = false;
        return Ok(());
    }

    // Live request: spawn background task and show throbber
    let (tx, rx) = std::sync::mpsc::channel::<app::ExecOutcome>();
    app.exec_rx = Some(rx);
    app.executing = true;
    app.throbber_idx = 0;

    let spec_clone = spec.clone();
    let path_s = path.clone();
    let body_map = body.clone();
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(app::ExecOutcome {
                    log: format!("Error: failed to start runtime: {}", e),
                    result_json: None,
                    open_table: false,
                });
                return;
            }
        };
        let outcome = rt.block_on(async move {
            let client = heroku_api::HerokuClient::new_from_env().map_err(|e| format!("Auth setup failed: {}. Hint: set HEROKU_API_KEY or configure ~/.netrc", e))?;
            let method = match spec_clone.method.as_str() {
                "GET" => reqwest::Method::GET,
                "POST" => reqwest::Method::POST,
                "DELETE" => reqwest::Method::DELETE,
                "PATCH" => reqwest::Method::PATCH,
                other => return Err(format!("unsupported method: {}", other)),
            };
            let mut builder = client.request(method, &path_s);
            if !body_map.is_empty() { builder = builder.json(&serde_json::Value::Object(body_map.clone())); }
            let resp = builder.send().await.map_err(|e| format!("Network error: {}. Hint: check connection/proxy; ensure HEROKU_API_KEY or ~/.netrc is set", e))?;
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            if status.as_u16() == 401 { return Err("Unauthorized (401). Hint: set HEROKU_API_KEY=... or configure ~/.netrc with machine api.heroku.com".into()); }
            if status.as_u16() == 403 { return Err("Forbidden (403). Hint: check team/app access, permissions, and role membership".into()); }
            let log = format!("{}\n{}", status, text);
            let mut result_json = None;
            let mut open_table = false;
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                open_table = matches!(json, serde_json::Value::Array(_));
                result_json = Some(json);
            }
            Ok::<app::ExecOutcome, String>(app::ExecOutcome { log, result_json, open_table })
        });

        match outcome {
            Ok(out) => {
                let _ = tx.send(out);
            }
            Err(err) => {
                let _ = tx.send(app::ExecOutcome {
                    log: format!("Error: {}", err),
                    result_json: None,
                    open_table: false,
                });
            }
        }
    });

    Ok(())
}
