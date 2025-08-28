//! Command palette component for input and suggestions.
//!
//! This module provides a component for rendering the command palette, which
//! handles text input, command suggestions, and user interactions for
//! building Heroku CLI commands.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::*,
    text::{Line, Span},
    widgets::*,
};

use crate::{
    app, theme,
    ui::components::{component::Component, palette::state::ItemKind},
};

/// Command palette component for input and suggestions.
///
/// This component encapsulates the command palette experience including the
/// input line, suggestions popup, and help integration. It provides a
/// comprehensive interface for building and executing Heroku commands.
///
/// # Features
///
/// - Text input with cursor navigation
/// - Real-time command suggestions
/// - Suggestion acceptance and completion
/// - Help integration (Ctrl+H)
/// - Error display and validation
/// - Ghost text for completion hints
///
/// # Key Bindings
///
/// - **Character input**: Add characters to the input
/// - **Backspace**: Remove character before cursor
/// - **Arrow keys**: Navigate suggestions (Up/Down) or move cursor (Left/Right)
/// - **Tab**: Accept current suggestion
/// - **Ctrl+H**: Open help for current command
/// - **Ctrl+F**: Open command builder modal
/// - **Enter**: Execute command
/// - **Escape**: Clear input and close suggestions
///
/// # Examples
///
/// ```rust
/// use heroku_tui::ui::components::PaletteComponent;
///
/// let mut palette = PaletteComponent::new();
/// palette.init()?;
/// ```
#[derive(Default)]
pub struct PaletteComponent;

impl PaletteComponent {
    /// Creates a new palette component instance.
    ///
    /// # Returns
    ///
    /// A new PaletteComponent with default state
    pub fn new() -> Self {
        Self
    }

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
    pub fn handle_key(&self, app: &mut app::App, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
                // Handle character input
                app.palette.apply_insert_char(c);
                app.palette
                    .apply_build_suggestions(&app.ctx.registry, &app.ctx.providers);
                app.palette.set_popup_open(true);
                app.palette.reduce_clear_error();
                Ok(true)
            }
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ensure suggestions are up to date, then fetch effective command
                app.palette
                    .apply_build_suggestions(&app.ctx.registry, &app.ctx.providers);
                let spec = app.palette.selected_command();
                if spec.is_some() {
                    app.help.set_spec(spec.cloned());
                    let _ = app.update(app::Msg::ToggleHelp);
                }
                Ok(true)
            }
            KeyCode::Backspace => {
                // Handle backspace
                app.palette.reduce_backspace();
                app.palette
                    .apply_build_suggestions(&app.ctx.registry, &app.ctx.providers);
                app.palette.reduce_clear_error();
                Ok(true)
            }
            KeyCode::Left => {
                // Move cursor left
                app.palette.reduce_move_cursor_left();
                Ok(true)
            }
            KeyCode::Right => {
                // Move cursor right
                app.palette.reduce_move_cursor_right();
                Ok(true)
            }
            KeyCode::Down | KeyCode::Up => {
                // Navigate down/up through suggestions
                let len = app.palette.suggestions().len();
                if len > 0 {
                    let selected = app.palette.suggestion_index() as isize;
                    let delta = if key.code == KeyCode::Down { 1isize } else { -1isize };
                    // Wrap around using modulus with length as isize
                    let new_selected = (selected + delta).rem_euclid(len as isize) as usize;
                    app.palette.set_selected(new_selected);
                    app.palette.apply_ghost_text();
                }
                Ok(true)
            }
            KeyCode::Tab => {
                // Accept suggestion
                if app.palette.is_suggestions_open() {
                    if let Some(item) = app.palette.suggestions().get(app.palette.suggestion_index()).cloned() {
                        match item.kind {
                            ItemKind::Command => {
                                // Replace input with command exec
                                app.palette.apply_accept_command_suggestion(&item.insert_text);
                                app.palette.set_popup_open(false);
                                app.palette.reduce_clear_suggestions();
                            }
                            ItemKind::Positional => {
                                // Accept positional suggestion
                                app.palette.apply_accept_positional_suggestion(&item.insert_text);
                            }
                            _ => {
                                // Accept flag or value suggestion
                                app.palette.apply_accept_non_command_suggestion(&item.insert_text);
                            }
                        }

                        // Rebuild suggestions after accepting
                        app.palette
                            .apply_build_suggestions(&app.ctx.registry, &app.ctx.providers);
                        app.palette.set_selected(0);

                        // Keep popup open unless we accepted a command
                        if !matches!(item.kind, ItemKind::Command) {
                            app.palette.set_popup_open(!app.palette.is_suggestions_open());
                        }
                    }
                } else {
                    // Open suggestions; if only one, accept it
                    app.palette
                        .apply_build_suggestions(&app.ctx.registry, &app.ctx.providers);
                    if app.palette.suggestions_len() == 1 {
                        if let Some(item) = app.palette.suggestions().first().cloned() {
                            match item.kind {
                                ItemKind::Command => {
                                    app.palette.apply_accept_command_suggestion(&item.insert_text);
                                    app.palette.set_popup_open(false);
                                    app.palette.reduce_clear_suggestions();
                                }
                                ItemKind::Positional => {
                                    app.palette.apply_accept_positional_suggestion(&item.insert_text);
                                }
                                _ => {
                                    app.palette.apply_accept_non_command_suggestion(&item.insert_text);
                                }
                            }
                            app.palette
                                .apply_build_suggestions(&app.ctx.registry, &app.ctx.providers);
                            app.palette.set_selected(0);
                            app.palette.set_popup_open(!app.palette.is_suggestions_open());
                        }
                    } else {
                        app.palette.set_popup_open(!app.palette.is_suggestions_open());
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
                app.palette.reduce_clear_all();
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

impl Component for PaletteComponent {
    /// Renders the command palette with input and suggestions.
    ///
    /// This method handles the input display, suggestion popup, and cursor positioning.
    ///
    /// # Arguments
    ///
    /// * `f` - The frame to render to
    /// * `rect` - The rectangular area to render in
    /// * `app` - The application state containing palette data
    fn render(&mut self, f: &mut Frame, rect: Rect, app: &mut app::App) {
        let block = Block::default()
            .borders(Borders::LEFT)
            .border_style(theme::border_style(true))
            .border_type(BorderType::Thick);
        f.render_widget(block.clone(), rect);
        let inner = block.inner(rect);
        let splits = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)])
            .split(inner);

        // Input line with ghost text; dim when a modal is open. Show throbber if executing.
        let dimmed = app.builder.is_visible() || app.help.is_visible();
        let base_style = if dimmed {
            theme::text_muted()
        } else {
            theme::text_style()
        };
        let mut spans: Vec<Span> = Vec::new();
        if app.executing {
            let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let sym = frames[app.throbber_idx % frames.len()];
            spans.push(Span::styled(
                format!("{} ", sym),
                theme::title_style().fg(theme::ACCENT),
            ));
        }
        spans.push(Span::styled(app.palette.input(), base_style));
        if let Some(ghost) = app.palette.ghost_text()
            && !ghost.is_empty()
        {
            spans.push(Span::styled(ghost.as_str(), theme::text_muted()));
        }
        let p = Paragraph::new(Line::from(spans)).block(Block::default());
        f.render_widget(p, splits[0]);

        // Cursor placement (count characters, not bytes); hide when a modal is open
        if !dimmed {
            let col = app
                .palette
                .input()
                .get(..app.palette.selected_cursor_position())
                .map(|s| s.chars().count() as u16)
                .unwrap_or(0);
            let x = splits[0].x.saturating_add(col);
            let y = splits[0].y;
            f.set_cursor_position((x, y));
        }

        // Error line below input when present
        if let Some(err) = app.palette.error_message() {
            let line = Line::from(vec![
                Span::styled("✖ ", Style::default().fg(theme::WARN)),
                Span::styled(err.as_str(), theme::text_style()),
            ]);
            f.render_widget(Paragraph::new(line), splits[1]);
        }

        // Popup suggestions (separate popup under the input; no overlap with input text).
        // Hidden if error is present or no suggestions exist.
        if app.palette.error_message().is_none()
            && app.palette.is_suggestions_open()
            && !app.builder.is_visible()
            && !app.help.is_visible()
            && !app.palette.suggestions().is_empty()
        {
            let items_all: Vec<ListItem> = app
                .palette
                .suggestions()
                .iter()
                .map(|s| ListItem::new(s.display.clone()).style(theme::text_style()))
                .collect();
            let max_rows = 10usize;
            let rows = items_all.len().min(max_rows);
            if rows == 0 {
                return;
            }
            let popup_height = rows as u16 + 3;
            let popup_area = Rect::new(rect.x, rect.y + 1, rect.width, popup_height);
            let popup_block = Block::default()
                .borders(Borders::NONE)
                .border_style(theme::border_style(false))
                .border_type(BorderType::Thick);
            let list = List::new(items_all)
                .block(popup_block)
                .highlight_style(theme::list_highlight_style())
                .highlight_symbol("► ");
            let mut list_state = ListState::default();
            let sel = if app.palette.suggestions().is_empty() {
                None
            } else {
                Some(app.palette.suggestion_index().min(app.palette.suggestions().len() - 1))
            };
            list_state.select(sel);
            f.render_stateful_widget(list, popup_area, &mut list_state);
        }
    }
}
