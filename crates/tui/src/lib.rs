mod app;
mod preview;
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

pub fn run() -> Result<()> {
    let registry = heroku_registry::Registry::from_embedded_schema()?;
    let mut app = app::App::new(registry);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();

    loop {
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
    if app.show_help && key.code == KeyCode::Esc {
        app::update(app, app::Msg::CloseModal);
        return Ok(false);
    }
    let mut effect: Option<app::Effect> = None;
    match app.focus {
        app::Focus::Search => match key.code {
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
