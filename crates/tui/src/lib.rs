mod app;
mod palette;
mod preview;
mod tables;
mod theme;
mod ui;

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

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();

    loop {
        // Check for async execution completion
        if let Some(rx) = app.exec_rx.as_ref() {
            if let Ok(out) = rx.try_recv() {
                app.exec_rx = None;
                app.executing = false;
                app.logs.push(out.log);
                if app.logs.len() > 500 {
                    let _ = app.logs.drain(0..app.logs.len() - 500);
                }
                app.result_json = out.result_json;
                app.show_table = out.open_table;
                if out.open_table {
                    app.table_offset = 0;
                }
                // Clear input for next command
                app.palette.input.clear();
                app.palette.cursor = 0;
                app.palette.suggestions.clear();
                app.palette.popup_open = false;
                app.palette.error = None;
            }
        }
        terminal.draw(|f| ui::draw(f, &mut app))?;

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
                    if handle_key(&mut app, key)? {
                        break;
                    }
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
            if app.executing {
                app.throbber_idx = (app.throbber_idx + 1) % 10;
            }
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

fn handle_key(app: &mut app::App, key: KeyEvent) -> Result<bool> {
    // Global: close modal on Esc
    if (app.show_help || app.show_table || app.show_builder) && key.code == KeyCode::Esc {
        app::update(app, app::Msg::CloseModal);
        return Ok(false);
    }
    // Toggle builder modal (Ctrl+F)
    if key.code == KeyCode::Char('f') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app::update(app, app::Msg::ToggleBuilder);
        return Ok(false);
    }
    // While table modal open, handle scrolling keys
    if app.show_table {
        match key.code {
            KeyCode::Up => {
                app::update(app, app::Msg::TableScroll(-1));
                return Ok(false);
            }
            KeyCode::Down => {
                app::update(app, app::Msg::TableScroll(1));
                return Ok(false);
            }
            KeyCode::PageUp => {
                app::update(app, app::Msg::TableScroll(-10));
                return Ok(false);
            }
            KeyCode::PageDown => {
                app::update(app, app::Msg::TableScroll(10));
                return Ok(false);
            }
            KeyCode::Home => {
                app::update(app, app::Msg::TableHome);
                return Ok(false);
            }
            KeyCode::End => {
                app::update(app, app::Msg::TableEnd);
                return Ok(false);
            }
            KeyCode::Char('t') => {
                app::update(app, app::Msg::ToggleTable);
                return Ok(false);
            }
            _ => {}
        }
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
        match key.code {
            KeyCode::Char(c)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                app.palette.insert_char(c);
                crate::palette::build_suggestions(&mut app.palette, &app.registry, &app.providers);
                app.palette.popup_open = true;
                app.palette.error = None;
            }
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Open help for exact command (group sub) or top command suggestion
                let toks: Vec<&str> = app.palette.input.split_whitespace().collect();
                let mut target: Option<heroku_registry::CommandSpec> = None;
                if toks.len() >= 2 {
                    let key = format!("{}:{}", toks[0], toks[1]);
                    if let Some(spec) = app
                        .registry
                        .commands
                        .iter()
                        .find(|c| c.name == key)
                        .cloned()
                    {
                        target = Some(spec);
                    }
                }
                if target.is_none() {
                    crate::palette::build_suggestions(
                        &mut app.palette,
                        &app.registry,
                        &app.providers,
                    );
                    if let Some(top) = app.palette.suggestions.get(0) {
                        if matches!(top.kind, crate::palette::ItemKind::Command) {
                            // Convert "group sub" to registry key
                            let mut parts = top.insert_text.split_whitespace();
                            let group = parts.next().unwrap_or("");
                            let sub = parts.next().unwrap_or("");
                            let key = format!("{}:{}", group, sub);
                            if let Some(spec) = app
                                .registry
                                .commands
                                .iter()
                                .find(|c| c.name == key)
                                .cloned()
                            {
                                target = Some(spec);
                            }
                        }
                    }
                }
                if let Some(spec) = target {
                    app.help_spec = Some(spec);
                    app.toggle_help();
                }
            }
            KeyCode::Backspace => {
                app.palette.backspace();
                crate::palette::build_suggestions(&mut app.palette, &app.registry, &app.providers);
                app.palette.error = None;
            }
            KeyCode::Left => app.palette.move_cursor_left(),
            KeyCode::Right => app.palette.move_cursor_right(),
            KeyCode::Down => {
                let len = app.palette.suggestions.len();
                if len > 0 {
                    app.palette.selected = (app.palette.selected + 1) % len;
                }
            }
            KeyCode::Up | KeyCode::BackTab => {
                let len = app.palette.suggestions.len();
                if len > 0 {
                    app.palette.selected = (app.palette.selected + len - 1) % len;
                }
            }
            KeyCode::Tab => {
                if app.palette.popup_open {
                    if let Some(item) = app.palette.suggestions.get(app.palette.selected).cloned() {
                        if matches!(item.kind, crate::palette::ItemKind::Command) {
                            app.palette.input = format!("{} ", item.insert_text);
                            app.palette.cursor = app.palette.input.len();
                        } else if matches!(item.kind, crate::palette::ItemKind::Positional) {
                            accept_positional_suggestion(&mut app.palette, &item.insert_text);
                        } else {
                            accept_non_command_suggestion(&mut app.palette, &item.insert_text);
                        }
                        crate::palette::build_suggestions(
                            &mut app.palette,
                            &app.registry,
                            &app.providers,
                        );
                        app.palette.selected = 0;
                        if matches!(item.kind, crate::palette::ItemKind::Command) {
                            app.palette.popup_open = false;
                            app.palette.suggestions.clear();
                        } else {
                            app.palette.popup_open = !app.palette.suggestions.is_empty();
                        }
                    }
                } else {
                    // Open suggestions; if only one, accept it
                    crate::palette::build_suggestions(
                        &mut app.palette,
                        &app.registry,
                        &app.providers,
                    );
                    if app.palette.suggestions.len() == 1 {
                        if let Some(item) = app.palette.suggestions.get(0).cloned() {
                            if matches!(item.kind, crate::palette::ItemKind::Command) {
                                app.palette.input = format!("{} ", item.insert_text);
                                app.palette.cursor = app.palette.input.len();
                            } else if matches!(item.kind, crate::palette::ItemKind::Positional) {
                                accept_positional_suggestion(&mut app.palette, &item.insert_text);
                            } else {
                                accept_non_command_suggestion(&mut app.palette, &item.insert_text);
                            }
                            crate::palette::build_suggestions(
                                &mut app.palette,
                                &app.registry,
                                &app.providers,
                            );
                            app.palette.selected = 0;
                            if matches!(item.kind, crate::palette::ItemKind::Command) {
                                app.palette.popup_open = false;
                                app.palette.suggestions.clear();
                            } else {
                                app.palette.popup_open = !app.palette.suggestions.is_empty();
                            }
                        }
                    } else {
                        app.palette.popup_open = !app.palette.suggestions.is_empty();
                    }
                }
            }
            KeyCode::Enter => {
                if let Err(msg) = start_palette_execution(app) {
                    app.palette.error = Some(msg);
                } else {
                    app.palette.error = None;
                }
            }
            KeyCode::Esc => {
                app.palette.popup_open = false;
            }
            _ => {}
        }
        return Ok(false);
    }

    let mut effect: Option<app::Effect> = None;
    match app.focus {
        app::Focus::Search => match key.code {
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                effect = app::update(app, app::Msg::ToggleTable)
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                effect = app::update(app, app::Msg::ToggleBuilder)
            }
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                effect = app::update(app, app::Msg::ToggleHelp)
            }
            KeyCode::Char(c)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                effect = app::update(app, app::Msg::SearchChar(c))
            }
            KeyCode::Backspace => effect = app::update(app, app::Msg::SearchBackspace),
            KeyCode::Esc => effect = app::update(app, app::Msg::SearchClear),
            KeyCode::Tab => effect = app::update(app, app::Msg::FocusNext),
            KeyCode::BackTab => effect = app::update(app, app::Msg::FocusPrev),
            KeyCode::Down => effect = app::update(app, app::Msg::MoveSelection(1)),
            KeyCode::Up => effect = app::update(app, app::Msg::MoveSelection(-1)),
            KeyCode::Enter => effect = app::update(app, app::Msg::Enter),
            KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                effect = app::update(app, app::Msg::CopyCommand)
            }
            _ => {}
        },
        app::Focus::Commands => match key.code {
            KeyCode::Char('t') => effect = app::update(app, app::Msg::ToggleTable),
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                effect = app::update(app, app::Msg::ToggleBuilder)
            }
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                effect = app::update(app, app::Msg::ToggleHelp)
            }
            KeyCode::Down => effect = app::update(app, app::Msg::MoveSelection(1)),
            KeyCode::Up => effect = app::update(app, app::Msg::MoveSelection(-1)),
            KeyCode::Enter => effect = app::update(app, app::Msg::Enter),
            KeyCode::Tab => effect = app::update(app, app::Msg::FocusNext),
            KeyCode::BackTab => effect = app::update(app, app::Msg::FocusPrev),
            KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                effect = app::update(app, app::Msg::CopyCommand)
            }
            _ => {}
        },
        app::Focus::Inputs => match key.code {
            KeyCode::Char('t') => effect = app::update(app, app::Msg::ToggleTable),
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                effect = app::update(app, app::Msg::ToggleBuilder)
            }
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                effect = app::update(app, app::Msg::ToggleHelp)
            }
            KeyCode::Tab => effect = app::update(app, app::Msg::FocusNext),
            KeyCode::BackTab => effect = app::update(app, app::Msg::FocusPrev),
            KeyCode::Up => effect = app::update(app, app::Msg::InputsUp),
            KeyCode::Down => effect = app::update(app, app::Msg::InputsDown),
            KeyCode::Enter => effect = app::update(app, app::Msg::Run),
            KeyCode::Left => effect = app::update(app, app::Msg::InputsCycleLeft),
            KeyCode::Right => effect = app::update(app, app::Msg::InputsCycleRight),
            KeyCode::Backspace => effect = app::update(app, app::Msg::InputsBackspace),
            KeyCode::Char(' ') => effect = app::update(app, app::Msg::InputsToggleSpace),
            KeyCode::Char(c)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                effect = app::update(app, app::Msg::InputsChar(c))
            }
            KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                effect = app::update(app, app::Msg::CopyCommand)
            }
            _ => {}
        },
    }
    if let Some(app::Effect::CopyCommandRequested) = effect {
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
    Ok(false)
}

// Accept a non-command suggestion (flag/value) without clobbering the resolved command (group sub).
// Rules:
// - If cursor is at a new token position (ends with space), insert suggestion + trailing space.
// - If current token starts with '-' or previous token is a flag expecting a value → replace token.
// - Otherwise (we're on the command tokens or a positional token) → append suggestion separated by space.
fn accept_non_command_suggestion(p: &mut crate::palette::PaletteState, text: &str) {
    let input = &p.input;
    let bytes = input.as_bytes();
    let at_new_token = input.ends_with(' ');
    // Tokenize into (start,end)
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let start = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let end = i;
        spans.push((start, end));
    }
    // Helper: safe insertion with space separation
    let insert_with_space = |p: &mut crate::palette::PaletteState, s: &str| {
        if !p.input.ends_with(' ') && !p.input.is_empty() {
            p.input.push(' ');
        }
        p.input.push_str(s);
        p.input.push(' ');
        p.cursor = p.input.len();
    };
    if at_new_token || spans.is_empty() {
        // If the last token was a lone '-' or '--', replace it with the suggestion
        if !spans.is_empty() {
            let (ls, le) = spans[spans.len() - 1];
            let last_tok = &p.input[ls..le];
            if last_tok == "-" || last_tok == "--" {
                p.input.replace_range(ls..p.input.len(), "");
                p.cursor = p.input.len();
            }
        }
        insert_with_space(p, text);
        return;
    }
    // Find token under cursor; assume end token if cursor at end
    let mut idx = None;
    let cur = p.cursor.min(p.input.len());
    for (ti, (start, end)) in spans.iter().enumerate() {
        if *start <= cur && cur <= *end {
            idx = Some(ti);
            break;
        }
    }
    let token_index = idx.unwrap_or(spans.len().saturating_sub(1));
    // Extract current token and previous token text
    let (start, end) = spans[token_index];
    let current_token = &p.input[start..end];
    let prev_token = if token_index > 0 {
        Some(&p.input[spans[token_index - 1].0..spans[token_index - 1].1])
    } else {
        None
    };

    let prev_is_flag = prev_token.map(|t| t.starts_with("--")).unwrap_or(false);
    let inserting_is_flag = text.starts_with("--");
    // If the cursor is on a flag value (previous token is a flag and current token isn't a flag)
    // and the chosen suggestion is another flag, we should append the new flag instead of
    // replacing the value the user has already typed.
    if prev_is_flag && !current_token.starts_with("-") && inserting_is_flag {
        p.cursor = p.input.len();
        insert_with_space(p, text);
    } else if current_token.starts_with("--") || prev_is_flag {
        // Replace current token (flag token itself, or a value while inserting a value)
        p.input.replace_range(start..end, text);
        p.cursor = start + text.len();
        if !p.input.ends_with(' ') {
            p.input.push(' ');
            p.cursor += 1;
        }
    } else {
        // We are on command tokens or positional-looking token → append
        // Move cursor to end and insert suggestion
        p.cursor = p.input.len();
        insert_with_space(p, text);
    }
}

// Accept a positional suggestion/value: fill the next positional slot after "group sub".
// If the last existing positional is a placeholder like "<app>", replace it; otherwise append before any flags.
fn accept_positional_suggestion(p: &mut crate::palette::PaletteState, value: &str) {
    let tokens: Vec<&str> = p.input.split_whitespace().collect();
    if tokens.len() < 2 {
        // No command yet; just append
        if !p.input.ends_with(' ') && !p.input.is_empty() {
            p.input.push(' ');
        }
        p.input.push_str(value);
        p.input.push(' ');
        p.cursor = p.input.len();
        return;
    }
    // Identify first flag position after command tokens
    let mut first_flag_idx = tokens.len();
    for (i, t) in tokens.iter().enumerate().skip(2) {
        if t.starts_with("--") {
            first_flag_idx = i;
            break;
        }
    }
    // Existing positionals are tokens[2..first_flag_idx]
    let mut out: Vec<String> = Vec::new();
    out.push(tokens[0].to_string());
    out.push(tokens[1].to_string());
    // Copy existing positionals as-is, then append new positional value
    for t in tokens[2..first_flag_idx].iter() {
        out.push((*t).to_string());
    }
    out.push(value.to_string());
    // Append the rest (flags and any trailing tokens) in original order
    for t in tokens.iter().skip(first_flag_idx) {
        out.push((*t).to_string());
    }
    p.input = out.join(" ") + " ";
    p.cursor = p.input.len();
}

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

fn start_palette_execution(app: &mut app::App) -> Result<(), String> {
    // Parse input into tokens: expect "group sub [args...]"
    let input = app.palette.input.trim();
    if input.is_empty() {
        return Err("Type a command (e.g., apps info)".into());
    }
    let tokens: Vec<&str> = input.split_whitespace().collect();
    if tokens.len() < 2 {
        return Err("Incomplete command. Use '<group> <sub>' (e.g., apps info)".into());
    }
    let key = format!("{}:{}", tokens[0], tokens[1]);
    let spec = app
        .registry
        .commands
        .iter()
        .find(|c| c.name == key)
        .cloned()
        .ok_or_else(|| format!("Unknown command '{} {}'", tokens[0], tokens[1]))?;

    // Parse flags/args from tokens after first two
    let parts = &tokens[2..];
    let mut user_flags: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
    let mut user_args: Vec<String> = Vec::new();
    let mut i = 0;
    while i < parts.len() {
        let t = parts[i];
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
