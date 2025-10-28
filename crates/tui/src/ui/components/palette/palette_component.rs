//! Command palette component for input and suggestions.
//!
//! This module provides a component for rendering the command palette, which
//! handles text input, command suggestions, and user interactions for
//! building Heroku CLI commands.

use std::hash::{DefaultHasher, Hasher};
use std::vec;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use heroku_types::{Effect, ExecOutcome, ItemKind, Modal, Msg};
use rat_focus::HasFocus;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    prelude::*,
    text::{Line, Span},
    widgets::*,
};

use crate::app::App;
use crate::ui::{
    components::component::Component,
    theme::{Theme, theme_helpers as th},
};

static FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
#[derive(Default, Debug, Clone)]
struct PaletteLayout {
    input_area: Rect,
    error_area: Rect,
    suggestions_area: Rect,
}
/// Command palette component for input and suggestions.
///
/// This component encapsulates the command palette experience, including the
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
/// - **Ctrl+F**: Open command browser
/// - **Enter**: Execute command or insert selected suggestion
/// - **Escape**: Clear input and close suggestions
///
/// # Examples
///
/// ```rust,ignore
/// use heroku_tui::ui::components::PaletteComponent;
///
/// let mut palette = PaletteComponent::default();
/// palette.init()?;
/// ```
#[derive(Debug, Default)]
pub struct PaletteComponent {
    palette_layout: PaletteLayout,
}

impl PaletteComponent {
    pub fn new() -> Self {
        Self::default()
    }
    /// Creates the input paragraph widget with the current state.
    ///
    /// This function creates the input paragraph with throbber, input text, and
    /// ghost text, styled to match the browser's input appearance.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state containing palette data
    /// * `theme` - The current theme for styling
    ///
    /// # Returns
    ///
    /// The input paragraph widget with proper block styling
    fn create_input_paragraph<'a>(&self, app: &'a App, theme: &'a dyn Theme) -> Paragraph<'a> {
        let mut spans: Vec<Span<'a>> = vec![Span::styled(app.palette.input().to_string(), theme.text_primary_style())];

        if let Some(ghost) = app.palette.ghost_text()
            && !ghost.is_empty()
        {
            spans.push(Span::styled(ghost.to_string(), theme.text_muted_style()));
        }

        if app.executing || app.palette.is_provider_loading() {
            let sym = FRAMES[app.throbber_idx % FRAMES.len()];
            spans.push(Span::styled(format!(" {}", sym), theme.accent_emphasis_style()));
        }

        let input_title = self.create_input_title(theme);
        let is_focused = app.palette.f_input.get();
        let mut input_block = th::block(theme, None, is_focused);
        input_block = input_block.title(input_title);

        Paragraph::new(Line::from(spans))
            .style(theme.text_primary_style())
            .block(input_block)
    }

    /// Creates the title for the input panel.
    ///
    /// This method generates the title line for the input panel, matching
    /// the browser's input styling approach.
    ///
    /// # Arguments
    ///
    /// * `theme` - The current theme for styling
    ///
    /// # Returns
    ///
    /// * `Line<'_>` - The formatted title line
    fn create_input_title<'a>(&self, theme: &'a dyn Theme) -> Line<'a> {
        Line::from(Span::styled(
            "Execute Command",
            theme.text_secondary_style().add_modifier(Modifier::BOLD),
        ))
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
    fn create_error_paragraph<'a>(&self, app: &'a App, theme: &'a dyn Theme) -> Option<Paragraph<'a>> {
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

    /// Creates the suggestion list widget.
    ///
    /// This function creates the suggestion list with highlighting and
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
    fn create_suggestions_list<'a>(&'_ self, suggestions: Vec<ListItem<'static>>, theme: &dyn Theme) -> List<'a> {
        List::new(suggestions.to_vec())
            .highlight_style(theme.selection_style().add_modifier(Modifier::BOLD))
            .style(th::panel_style(theme))
            .highlight_symbol("> ")
    }

    /// Positions the cursor in the input line.
    ///
    /// This function calculates the correct cursor position based on the
    /// current cursor position in the palette input, accounting for
    /// character count rather than byte count and the block's inner area.
    /// The cursor is hidden when modals are open.
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to set cursor position on
    /// * `input_area` - The rectangular area of the input line
    /// * `app` - The application state containing palette data
    /// * `theme` - The current theme for styling
    fn position_cursor(&self, frame: &mut Frame, input_area: Rect, app: &App, theme: &dyn Theme) {
        if app.palette.is_focused() {
            // Create the same block structure to get the inner area
            let input_title = self.create_input_title(theme);
            let is_focused = app.palette.is_focused();
            let mut input_block = th::block(theme, None, is_focused);
            input_block = input_block.title(input_title);
            let inner_area = input_block.inner(input_area);

            let col = app
                .palette
                .input()
                .get(..app.palette.selected_cursor_position())
                .map(|s| s.chars().count() as u16)
                .unwrap_or(0);

            let x = inner_area.x.saturating_add(col);
            let y = inner_area.y;
            frame.set_cursor_position((x, y));
        }
    }

    /// Handles character input in the command palette.
    ///
    /// This function processes regular character input (with or without a Shift
    /// modifier) by inserting the character at the current cursor position,
    /// closing the suggestions popup, and clearing any previous error messages.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state to update
    /// * `character` - The character to insert
    fn handle_character_input(&self, app: &mut App, character: char) {
        app.palette.apply_insert_char(character);
        app.palette.set_is_suggestions_open(false);
        app.palette.reduce_clear_error();
        app.palette.apply_ghost_text();
    }

    /// Handles the Ctrl+H key combination to open help for the current command.
    ///
    /// This function ensures suggestions are up to date, retrieves the
    /// currently selected command specification, and opens the help modal
    /// if a valid command is found. The help system provides detailed
    /// information about command usage, flags, and examples.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state to update
    fn handle_help_request(&self, app: &mut App) -> Vec<Effect> {
        let mut effects = app.rebuild_palette_suggestions();
        let spec = app.palette.selected_command();
        if spec.is_some() {
            app.help.set_spec(spec);
            effects.push(Effect::ShowModal(Modal::Help));
        }
        effects
    }

    /// Handles backspace key press in the command palette.
    ///
    /// This function removes the character before the current cursor position,
    /// closes the suggestions popup, and clears any previous error messages.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state to update
    fn handle_backspace(&self, app: &mut App) {
        app.palette.reduce_backspace();
        app.palette.reduce_clear_error();
        app.palette.apply_suggestions(vec![]);
    }

    /// Handles left arrow key press to move the cursor left.
    ///
    /// This function moves the cursor one position to the left within the input
    /// text, allowing users to navigate and edit their command input.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state to update
    fn handle_cursor_left(&self, app: &mut App) {
        app.palette.reduce_move_cursor_left();
    }

    /// Handles right arrow key press to move the cursor right.
    ///
    /// This function moves the cursor one position to the right within the
    /// input text, allowing users to navigate and edit their command input.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state to update
    fn handle_cursor_right(&self, app: &mut App) {
        app.palette.reduce_move_cursor_right();
    }

    /// Handles up/down arrow key presses to navigate through suggestions.
    ///
    /// This function allows users to navigate through the suggestion list using
    /// arrow keys. The selection wraps around at the top and bottom of the list
    /// for a seamless navigation experience. When a suggestion is selected,
    /// ghost text is applied to show what the completed command would look
    /// like.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state to update
    /// * `direction` - The direction to navigate (Up or Down)
    fn handle_suggestion_navigation(&self, app: &mut App, direction: KeyCode) {
        let len = app.palette.suggestions().len();
        if len > 0 {
            if direction == KeyCode::Down {
                app.palette.list_state.select_next()
            } else {
                app.palette.list_state.select_previous()
            };
            app.palette.apply_ghost_text();
        }
    }

    /// Handles tab keypress to trigger or refresh the suggestion list.
    ///
    /// This function triggers building the suggestion list and opens the popup
    /// if suggestions are available.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state to update
    fn handle_tab_press(&self, app: &mut App) -> Vec<Effect> {
        if app.palette.is_input_empty() {
            return Vec::new();
        }
        let effects = app.rebuild_palette_suggestions();
        // Open the popup if we have suggestions or if provider-backed suggestions are loading
        let open = app.palette.suggestions_len() > 0 || app.palette.is_provider_loading();
        app.palette.set_is_suggestions_open(open);
        effects
    }

    /// Handles the Enter keypress.
    fn handle_enter(&self, app: &mut App) -> Vec<Effect> {
        let cmd = app.palette.input().to_string();
        let selected_index = app.palette.list_state.selected().unwrap_or(0);
        if app.palette.is_suggestions_open()
            && let Some(item) = app.palette.suggestions().get(selected_index)
            && item.kind == ItemKind::Command
            && item.meta.as_deref() == Some("history")
            && item.insert_text.trim() != cmd.trim()
        {
            let mut hasher = DefaultHasher::new();
            hasher.write(cmd.as_bytes());
            let hash = hasher.finish();
            app.palette.set_cmd_exec_hash(hash);
            app.palette.reduce_clear_suggestions();
            return vec![Effect::Run {
                hydrated_command: cmd,
                range_override: None,
                request_hash: hash,
            }];
        }

        // Execute the command
        if !app.palette.is_suggestions_open() {
            let mut hasher = DefaultHasher::new();
            hasher.write(cmd.as_bytes());
            let hash = hasher.finish();
            app.palette.set_cmd_exec_hash(hash);
            return vec![Effect::Run {
                hydrated_command: cmd,
                range_override: None,
                request_hash: hash,
            }];
        } else if let Some(item) = app.palette.suggestions().get(selected_index).cloned() {
            match item.kind {
                ItemKind::Command => {
                    // Replace input with command exec
                    app.palette.apply_accept_command_suggestion(&item.insert_text);
                    app.palette.set_is_suggestions_open(false);
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
            app.palette.list_state.select(None);
            app.palette.apply_ghost_text();
            app.palette.set_is_suggestions_open(false);
        }
        vec![]
    }

    /// Handles the Escape key to clear input and close suggestions.
    ///
    /// This function provides a quick way to reset the command palette by
    /// clearing all input text and closing the suggestions popup. This is
    /// useful when users want to start over with a fresh command input.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state to update
    fn handle_escape(&self, app: &mut App) {
        if app.palette.is_suggestions_open() {
            app.palette.set_is_suggestions_open(false);
            app.palette.apply_ghost_text();
        } else {
            app.palette.reduce_clear_all();
        }
    }

    fn get_palette_layout(&self, app: &App, area: Rect) -> PaletteLayout {
        let rects = self.get_preferred_layout(app, area);
        PaletteLayout {
            input_area: rects[0],
            error_area: rects[1],
            suggestions_area: rects[2],
        }
    }
}

impl Component for PaletteComponent {
    fn handle_message(&mut self, app: &mut App, msg: &Msg) -> Vec<Effect> {
        match msg {
            Msg::Tick => {
                if app.executing || app.palette.is_provider_loading() {
                    app.throbber_idx = (app.throbber_idx + 1) % 10;
                }
                Vec::new()
            }
            Msg::ExecCompleted(outcome) => match outcome.as_ref() {
                ExecOutcome::Log(log_message) if log_message.starts_with("Provider fetch failed:") => {
                    app.palette.handle_provider_fetch_failure(log_message, &*app.ctx.theme);
                    Vec::new()
                }
                _ => app.palette.process_general_execution_result(outcome),
            },
            _ => Vec::new(),
        }
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
    /// `Vec<Effect>` containing any effects that should be processed
    ///
    /// # Key Bindings
    ///
    /// - **Character input**: Adds characters to the palette input
    /// - **Backspace**: Removes the character before the cursor
    /// - **Arrow keys**: Navigate through suggestions (Up/Down) or move cursor
    ///   (Left/Right)
    /// - **Tab**: Trigger the suggestions list
    /// - **Ctrl+H**: Open help for the current command or top suggestion
    /// - **Ctrl+F**: Open the command browser
    /// - **Enter**: Execute the current command (if complete) or insert selected suggestion
    /// - **Escape**: Clear the palette input and close suggestions
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Example requires constructing full App and Registry; ignored in doctests.
    /// ```
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let mut effects: Vec<Effect> = vec![];
        match key.code {
            KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
                // Handle character input
                self.handle_character_input(app, c);
            }
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Handle help request
                effects.extend(self.handle_help_request(app));
            }
            KeyCode::Backspace => {
                // Handle backspace
                self.handle_backspace(app);
            }
            KeyCode::Left => {
                // Handle cursor left
                self.handle_cursor_left(app);
            }
            KeyCode::Right => {
                // Handle cursor right
                self.handle_cursor_right(app);
            }
            KeyCode::Down | KeyCode::Up => {
                if app.palette.is_suggestions_open() {
                    // Navigate suggestions when the popup is open
                    self.handle_suggestion_navigation(app, key.code);
                } else {
                    // Navigate command history when the popup is closed
                    let changed = if key.code == KeyCode::Up {
                        app.palette.history_up()
                    } else {
                        app.palette.history_down()
                    };
                    if changed {
                        // Clear errors/suggestions while browsing history
                        app.palette.reduce_clear_error();
                        app.palette.set_is_suggestions_open(false);
                    }
                }
            }
            KeyCode::BackTab => {
                app.focus.prev();
            }
            KeyCode::Tab => {
                // When input is empty, use Tab/BackTab for focus traversal between palette and logs.
                if app.palette.is_input_empty() {
                    app.focus.next();
                } else {
                    // Otherwise, Tab refreshes/opens suggestions
                    effects.extend(self.handle_tab_press(app));
                }
            }
            KeyCode::Enter => {
                // Handle enters keypress
                effects.extend(self.handle_enter(app));
            }
            KeyCode::Esc => {
                // Handle escape
                self.handle_escape(app);
            }
            _ => {}
        }
        effects
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let PaletteLayout {
            suggestions_area,
            input_area,
            ..
        } = &self.palette_layout;
        let position = Position {
            x: mouse.column,
            y: mouse.row,
        };
        let hit_test_suggestions = app.palette.is_suggestions_open() && suggestions_area.contains(position);
        let list_offset = app.palette.list_state.offset();
        let idx = if hit_test_suggestions {
            Some(position.y as usize - suggestions_area.y as usize + list_offset)
        } else {
            None
        };

        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            if input_area.contains(position) {
                app.focus.focus(&app.palette.f_input);
            }

            if hit_test_suggestions {
                app.palette.list_state.select(idx);
                app.palette.apply_ghost_text();
                return self.handle_enter(app);
            }
        }

        if let MouseEventKind::Moved = mouse.kind {
            app.palette.update_mouse_over_idx(idx);
            app.palette.apply_ghost_text();
        }

        Vec::new()
    }

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
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let palette_layout = self.get_palette_layout(app, rect);
        let PaletteLayout {
            input_area,
            error_area,
            suggestions_area,
        } = &palette_layout;
        let theme = &*app.ctx.theme;
        // Render input line with throbber and ghost text
        let input_para = self.create_input_paragraph(app, theme);
        frame.render_widget(input_para, *input_area);

        // Position cursor in the input line
        self.position_cursor(frame, *input_area, app, theme);

        // Render error message if present
        if let Some(error_para) = self.create_error_paragraph(app, theme) {
            frame.render_widget(error_para, *error_area);
        }

        // Render suggestions popup
        let should_show_suggestions =
            app.palette.error_message().is_none() && app.palette.is_suggestions_open() && !app.palette.suggestions().is_empty();

        if should_show_suggestions {
            app.palette.update_suggestions_view_width(suggestions_area.width, theme);
            let suggestions = app.palette.rendered_suggestions().to_vec();
            let suggestions_list = self.create_suggestions_list(suggestions, theme);
            frame.render_stateful_widget(suggestions_list, *suggestions_area, &mut app.palette.list_state);
        }
        self.palette_layout = palette_layout;
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        th::build_hint_spans(
            theme,
            &[
                ("Tab", " Completions "),
                ("↑/↓", " Cycle  "),
                ("Enter", " Accept  "),
                ("Ctrl+H", " Help  "),
                ("Esc", " Cancel"),
            ],
        )
    }

    fn get_preferred_layout(&self, app: &App, area: Rect) -> Vec<Rect> {
        let has_error = app.palette.error_message().is_some();
        // 3 areas in total, stacked on top of one another
        Layout::vertical([
            Constraint::Length(3),                             // input area
            Constraint::Length(if has_error { 1 } else { 0 }), // error area
            Constraint::Min(1),                                // Suggestion area
        ])
        .split(area)
        .to_vec()
    }
}
