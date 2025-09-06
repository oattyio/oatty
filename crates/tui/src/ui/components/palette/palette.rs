//! Command palette component for input and suggestions.
//!
//! This module provides a component for rendering the command palette, which
//! handles text input, command suggestions, and user interactions for
//! building Heroku CLI commands.

use std::vec;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::*,
    text::{Line, Span},
    widgets::*,
};

use crate::{
    app,
    ui::{
        components::component::Component,
        theme::{Theme, helpers as th},
    },
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
/// - **Tab**: Trigger suggestions list
/// - **Ctrl+H**: Open help for current command
/// - **Ctrl+F**: Open command builder modal
/// - **Enter**: Execute command or insert selected suggestion
/// - **Escape**: Clear input and close suggestions
///
/// # Examples
///
/// ```rust,ignore
/// use heroku_tui::ui::components::PaletteComponent;
///
/// let mut palette = PaletteComponent::new();
/// palette.init()?;
/// ```
#[derive(Default)]
pub struct PaletteComponent {
    // Throbber animation frames
    throbber_frames: [&'static str; 10],
}

impl PaletteComponent {
    /// Creates a new palette component instance.
    ///
    /// # Returns
    ///
    /// A new PaletteComponent with default state
    pub fn new() -> Self {
        Self {
            throbber_frames: ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
        }
    }

    /// Creates the input paragraph widget with current state.
    ///
    /// This function creates the input paragraph with throbber, input text, and
    /// ghost text.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state containing palette data
    /// * `theme` - The current theme for styling
    ///
    /// # Returns
    ///
    /// The input paragraph widget
    fn create_input_paragraph(&'_ self, app: &app::App, theme: &dyn Theme) -> Paragraph<'_> {
        let dimmed = app.builder.is_visible() || app.help.is_visible();
        let base_style = if dimmed {
            theme.text_muted_style()
        } else {
            theme.text_primary_style()
        };

        let mut spans: Vec<Span> = Vec::new();

        // Add main input text
        spans.push(Span::styled(app.palette.input().to_string(), base_style));

        // Add ghost text if available
        if let Some(ghost) = app.palette.ghost_text()
            && !ghost.is_empty()
        {
            spans.push(Span::styled(ghost.to_string(), theme.text_muted_style()));
        }

        // Add throbber at end if executing or provider-loading
        if app.executing || app.palette.is_provider_loading() {
            let sym = self.throbber_frames[app.throbber_idx % self.throbber_frames.len()];
            spans.push(Span::styled(format!(" {}", sym), theme.accent_emphasis_style()));
        }

        Paragraph::new(Line::from(spans)).block(Block::default())
    }

    /// Creates the error paragraph widget if an error exists.
    ///
    /// This function creates the error paragraph with appropriate styling.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state containing palette data
    /// * `theme` - The current theme for styling
    ///
    /// # Returns
    ///
    /// The error paragraph widget, or None if no error
    fn create_error_paragraph(&'_ self, app: &app::App, theme: &dyn Theme) -> Option<Paragraph<'_>> {
        if let Some(err) = app.palette.error_message() {
            let line = Line::from(vec![
                Span::styled("✖ ".to_string(), Style::default().fg(theme.roles().error)),
                Span::styled(err.to_string(), theme.text_primary_style()),
            ]);
            Some(Paragraph::new(line))
        } else {
            None
        }
    }

    /// Creates the suggestions list widget.
    ///
    /// This function creates the suggestions list with highlighting and
    /// styling.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state containing palette data
    /// * `theme` - The current theme for styling
    ///
    /// # Returns
    ///
    /// The suggestions list widget
    fn create_suggestions_list<'a>(&'_ self, app: &'a app::App, theme: &dyn Theme) -> List<'a> {
        // Create popup with border
        let popup_block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(theme.roles().focus))
            .border_type(BorderType::Plain);

        List::new(app.palette.rendered_suggestions().to_vec())
            .block(popup_block)
            .highlight_style(theme.selection_style().add_modifier(Modifier::BOLD))
            .style(th::panel_style(theme))
            .highlight_symbol("► ")
    }

    /// Renders the main palette border and returns the inner layout areas.
    ///
    /// This function creates the visual border around the palette and sets up
    /// the internal layout constraints for the input line, content area, and
    /// footer area.
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to render to
    /// * `rect` - The rectangular area to render in
    /// * `theme` - The current theme for styling
    ///
    /// # Returns
    ///
    /// The split layout areas
    fn render_palette_border(&mut self, frame: &mut Frame, rect: Rect, theme: &dyn Theme) -> Vec<Rect> {
        let block = Block::default()
            .borders(Borders::LEFT)
            .border_style(theme.border_style(true))
            .border_type(BorderType::Thick)
            .style(th::panel_style(theme));

        frame.render_widget(block.clone(), rect);

        let inner = block.inner(rect);
        let splits = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Input line
                Constraint::Min(1),    // Content area (error messages, suggestions)
                Constraint::Length(1), // Footer area
            ])
            .split(inner);

        splits.to_vec()
    }

    /// Positions the cursor in the input line.
    ///
    /// This function calculates the correct cursor position based on the
    /// current cursor position in the palette input, accounting for
    /// character count rather than byte count. The cursor is hidden when
    /// modals are open.
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to set cursor position on
    /// * `input_area` - The rectangular area of the input line
    /// * `app` - The application state containing palette data
    fn position_cursor(frame: &mut Frame, input_area: Rect, app: &app::App) {
        let dimmed = app.builder.is_visible() || app.help.is_visible();
        if dimmed {
            return;
        }

        let col = app
            .palette
            .input()
            .get(..app.palette.selected_cursor_position())
            .map(|s| s.chars().count() as u16)
            .unwrap_or(0);

        let x = input_area.x.saturating_add(col);
        let y = input_area.y;
        frame.set_cursor_position((x, y));
    }


}

impl Component for PaletteComponent {
    /// Renders the command palette with input and suggestions.
    ///
    /// This method orchestrates the rendering of all palette components:
    /// - Main border and layout
    /// - Input line with throbber and ghost text
    /// - Cursor positioning
    /// - Error message display
    /// - Suggestions popup with highlighting
    ///
    /// The rendering is optimized to handle different states (executing,
    /// error, suggestions open) and provides a smooth user experience
    /// with appropriate visual feedback.
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to render to
    /// * `rect` - The rectangular area to render in
    /// * `app` - The application state containing palette data
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut app::App) {
        let theme = &*app.ctx.theme;

        // Render main border and get layout areas
        let splits = self.render_palette_border(frame, rect, theme);

        // Render input line with throbber and ghost text
        let input_para = self.create_input_paragraph(app, theme);
        frame.render_widget(input_para, splits[0]);

        // Position cursor in input line
        Self::position_cursor(frame, splits[0], app);

        // Render error message if present
        if let Some(error_para) = self.create_error_paragraph(app, theme) {
            frame.render_widget(error_para, splits[1]);
        }

        // Render suggestions popup
        let should_show_suggestions = app.palette.error_message().is_none()
            && app.palette.is_suggestions_open()
            && !app.builder.is_visible()
            && !app.help.is_visible()
            && !app.palette.suggestions().is_empty();

        if should_show_suggestions {
            let suggestions_list = self.create_suggestions_list(app, theme);

            // Calculate popup dimensions
            let max_rows = 10usize;
            let rows = app.palette.suggestions().len().min(max_rows);
            let popup_height = rows as u16 + 3;
            let popup_area = Rect::new(rect.x, rect.y + 1, rect.width, popup_height);

            // Update list state
            let sel = if app.palette.suggestions().is_empty() {
                None
            } else {
                Some(app.palette.suggestion_index().min(app.palette.suggestions().len() - 1))
            };
            let mut list_state = ListState::default();
            list_state.select(sel);

            frame.render_stateful_widget(suggestions_list, popup_area, &mut list_state);
        }
    }

    fn handle_key_events(&mut self, app: &mut app::App, key: KeyEvent) -> Option<app::Msg> {
        match key.code {
            KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
                Some(app::Msg::PaletteInput(c))
            }
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // This one is tricky because it needs to build suggestions first.
                // For now, let's keep it as a special case in the main handler.
                // A better solution would be a multi-step message.
                None
            }
            KeyCode::Backspace => Some(app::Msg::PaletteBackspace),
            KeyCode::Left => Some(app::Msg::PaletteCursorLeft),
            KeyCode::Right => Some(app::Msg::PaletteCursorRight),
            KeyCode::Down => {
                if app.palette.is_suggestions_open() {
                    Some(app::Msg::PaletteNavigateSuggestions(app::Direction::Down))
                } else {
                    Some(app::Msg::PaletteNavigateHistory(app::Direction::Down))
                }
            }
            KeyCode::Up => {
                if app.palette.is_suggestions_open() {
                    Some(app::Msg::PaletteNavigateSuggestions(app::Direction::Up))
                } else {
                    Some(app::Msg::PaletteNavigateHistory(app::Direction::Up))
                }
            }
            KeyCode::Tab => Some(app::Msg::PaletteSuggest),
            KeyCode::Enter => {
                if !app.palette.is_suggestions_open() {
                    Some(app::Msg::Run)
                } else {
                    Some(app::Msg::PaletteAcceptSuggestion)
                }
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(app::Msg::ToggleBuilder)
            }
            KeyCode::Esc => Some(app::Msg::PaletteClear),
            _ => None,
        }
    }
}

// rat-focus integration uses PaletteState.focus; component-level HasFocus not
// needed
