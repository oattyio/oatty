//! Command palette component logic and key event handling.
//!
//! This module contains the event handling logic for the command palette component,
//! including key event processing, suggestion acceptance, and help integration.
//! It provides a clean separation between the palette's UI rendering and its
//! interactive behavior.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app;

/// Handle key events for the command palette when the builder is not open.
///
/// This function processes keyboard input for the command palette, handling
/// text input, navigation, suggestion acceptance, and special commands like
/// help toggling and builder opening.
///
/// # Arguments
///
/// * `app` - The application state to update
/// * `key` - The key event to process
///
/// # Returns
///
/// `Result<bool>` where `true` indicates the key was handled by the palette
///
/// # Key Bindings
///
/// - **Character input**: Adds characters to the palette input
/// - **Backspace**: Removes the character before the cursor
/// - **Arrow keys**: Navigate through suggestions (Up/Down) or move cursor (Left/Right)
/// - **Tab**: Accept the currently selected suggestion
/// - **Ctrl+H**: Open help for the current command or top suggestion
/// - **Ctrl+F**: Open the command builder modal
/// - **Enter**: Execute the current command (if complete)
/// - **Escape**: Clear the palette input and close suggestions
///
/// # Examples
///
/// ```rust
/// use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
/// use heroku_tui::app::App;
///
/// let mut app = App::new(registry);
/// let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty());
/// let handled = handle_key(&mut app, key)?;
/// ```
pub fn handle_key(app: &mut app::App, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            // Handle character input
            app.palette.insert_char(c);
            crate::palette::build_suggestions(&mut app.palette, &app.registry, &app.providers);
            app.palette.popup_open = true;
            app.palette.error = None;
            Ok(true)
        }
        KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Open help for exact command (group sub) or top command suggestion
            // Use the palette's shell-like lexer to respect quoting rules.
            let toks = crate::palette::lex_shell_like(&app.palette.input);
            let mut target: Option<heroku_registry::CommandSpec> = None;

            // Try to find exact command match first
            if toks.len() >= 2 {
                let group = &toks[0];
                let name = &toks[1];
                if let Some(spec) = app
                    .registry
                    .commands
                    .iter()
                    .find(|c| &c.group == group && &c.name == name)
                    .cloned()
                {
                    target = Some(spec);
                }
            }

            // Fall back to top suggestion if no exact match
            if target.is_none() {
                crate::palette::build_suggestions(&mut app.palette, &app.registry, &app.providers);
                if let Some(top) = app.palette.suggestions.first() {
                    if matches!(top.kind, crate::palette::ItemKind::Command) {
                        // Convert "group sub" to registry key
                        let mut parts = top.insert_text.split_whitespace();
                        let group = parts.next().unwrap_or("");
                        let name = parts.next().unwrap_or("");
                        if let Some(spec) = app
                            .registry
                            .commands
                            .iter()
                            .find(|c| c.group == group && c.name == name)
                            .cloned()
                        {
                            target = Some(spec);
                        }
                    }
                }
            }

            // Open help if we found a command
            if let Some(spec) = target {
                app.help_spec = Some(spec);
                let _ = app.update(app::Msg::ToggleHelp);
            }
            Ok(true)
        }
        KeyCode::Backspace => {
            // Handle backspace
            app.palette.backspace();
            crate::palette::build_suggestions(&mut app.palette, &app.registry, &app.providers);
            app.palette.error = None;
            Ok(true)
        }
        KeyCode::Left => {
            // Move cursor left
            app.palette.move_cursor_left();
            Ok(true)
        }
        KeyCode::Right => {
            // Move cursor right
            app.palette.move_cursor_right();
            Ok(true)
        }
        KeyCode::Down => {
            // Navigate down through suggestions
            let len = app.palette.suggestions.len();
            if len > 0 {
                app.palette.selected = (app.palette.selected + 1) % len;
            }
            Ok(true)
        }
        KeyCode::Up | KeyCode::BackTab => {
            // Navigate up through suggestions
            let len = app.palette.suggestions.len();
            if len > 0 {
                app.palette.selected = (app.palette.selected + len - 1) % len;
            }
            Ok(true)
        }
        KeyCode::Tab => {
            // Accept suggestion
            if app.palette.popup_open {
                if let Some(item) = app.palette.suggestions.get(app.palette.selected).cloned() {
                    match item.kind {
                        crate::palette::ItemKind::Command => {
                            // Replace input with command exec
                            crate::palette::accept_command_suggestion(
                                &mut app.palette,
                                &item.insert_text,
                            );
                            app.palette.popup_open = false;
                            app.palette.suggestions.clear();
                        }
                        crate::palette::ItemKind::Positional => {
                            // Accept positional suggestion
                            crate::palette::accept_positional_suggestion(
                                &mut app.palette,
                                &item.insert_text,
                            );
                        }
                        _ => {
                            // Accept flag or value suggestion
                            crate::palette::accept_non_command_suggestion(
                                &mut app.palette,
                                &item.insert_text,
                            );
                        }
                    }

                    // Rebuild suggestions after accepting
                    crate::palette::build_suggestions(
                        &mut app.palette,
                        &app.registry,
                        &app.providers,
                    );
                    app.palette.selected = 0;

                    // Keep popup open unless we accepted a command
                    if !matches!(item.kind, crate::palette::ItemKind::Command) {
                        app.palette.popup_open = !app.palette.suggestions.is_empty();
                    }
                }
            } else {
                // Open suggestions; if only one, accept it
                crate::palette::build_suggestions(&mut app.palette, &app.registry, &app.providers);
                if app.palette.suggestions.len() == 1 {
                    if let Some(item) = app.palette.suggestions.first().cloned() {
                        match item.kind {
                            crate::palette::ItemKind::Command => {
                                crate::palette::accept_command_suggestion(
                                    &mut app.palette,
                                    &item.insert_text,
                                );
                                app.palette.popup_open = false;
                                app.palette.suggestions.clear();
                            }
                            crate::palette::ItemKind::Positional => {
                                crate::palette::accept_positional_suggestion(
                                    &mut app.palette,
                                    &item.insert_text,
                                );
                            }
                            _ => {
                                crate::palette::accept_non_command_suggestion(
                                    &mut app.palette,
                                    &item.insert_text,
                                );
                            }
                        }
                        crate::palette::build_suggestions(
                            &mut app.palette,
                            &app.registry,
                            &app.providers,
                        );
                        app.palette.selected = 0;
                        app.palette.popup_open = !app.palette.suggestions.is_empty();
                    }
                } else {
                    app.palette.popup_open = !app.palette.suggestions.is_empty();
                }
            }
            Ok(true)
        }
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Open command builder
            let _ = app.update(app::Msg::ToggleBuilder);
            Ok(true)
        }
        KeyCode::Enter => {
            // Execute command if complete
            let _ = app.update(app::Msg::Run);
            Ok(true)
        }
        KeyCode::Esc => {
            // Clear input and close suggestions
            app.palette.input.clear();
            app.palette.cursor = 0;
            app.palette.suggestions.clear();
            app.palette.popup_open = false;
            app.palette.error = None;
            Ok(true)
        }
        _ => Ok(false),
    }
}
