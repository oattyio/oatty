//! Command palette component for input and suggestions.
//!
//! This module provides a component for rendering the command palette, which
//! handles text input, command suggestions, and user interactions for
//! building Heroku CLI commands.

use std::vec;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use heroku_types::{Effect, ItemKind, Modal, Msg};
use rat_focus::HasFocus;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::*,
    text::{Line, Span},
    widgets::*,
};

use crate::{
    app::{self, SharedCtx},
    ui::{
        components::{LogsComponent, component::Component},
        layout::MainLayout,
        theme::{Theme, theme_helpers as th},
    },
};
static FRAMES: [&'static str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
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
    logs: LogsComponent,
}

impl PaletteComponent {
    /// Creates the input paragraph widget with current state.
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
    fn create_input_paragraph<'a>(&self, app: &'a app::App, theme: &'a dyn Theme) -> Paragraph<'a> {
        let mut spans: Vec<Span<'a>> = Vec::new();

        // Add main input text
        spans.push(Span::styled(
            app.palette.input().to_string(),
            theme.text_primary_style(),
        ));

        // Add ghost text if available
        if let Some(ghost) = app.palette.ghost_text()
            && !ghost.is_empty()
        {
            spans.push(Span::styled(ghost.to_string(), theme.text_muted_style()));
        }

        // Add throbber at end if executing or provider-loading
        if app.executing || app.palette.is_provider_loading() {
            let sym = FRAMES[app.throbber_idx % FRAMES.len()];
            spans.push(Span::styled(format!(" {}", sym), theme.accent_emphasis_style()));
        }

        // Create block with title and focus styling, matching browser input
        let input_title = self.create_input_title(theme);
        let is_focused = app.palette.is_focused();
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
    fn create_error_paragraph<'a>(&self, app: &'a app::App, theme: &'a dyn Theme) -> Option<Paragraph<'a>> {
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
        List::new(app.palette.rendered_suggestions().to_vec())
            .highlight_style(theme.selection_style().add_modifier(Modifier::BOLD))
            .style(th::panel_style(theme))
            .highlight_symbol("► ")
    }

    /// Calculates the inner layout areas for the palette input,
    /// error message, suggestions and hint bar.
    ///
    /// # Arguments
    ///
    /// * `rect` - The rectangular area to render in
    ///
    /// # Returns
    ///
    /// The split layout areas
    fn layout_palette_input(&mut self, rect: Rect) -> Vec<Rect> {
        let splits = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Input line - accommodates block with title and borders
                Constraint::Min(1),    // Content area - error messages, suggestions
                Constraint::Length(1), // Hints bar
            ])
            .split(rect);

        splits.to_vec()
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
    fn position_cursor(&self, frame: &mut Frame, input_area: Rect, app: &app::App, theme: &dyn Theme) {
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
    /// This function processes regular character input (with or without Shift
    /// modifier) by inserting the character at the current cursor position,
    /// closing the suggestions popup, and clearing any previous error messages.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state to update
    /// * `character` - The character to insert
    fn handle_character_input(&self, app: &mut app::App, character: char) {
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
    fn handle_help_request(&self, app: &mut app::App) -> Vec<Effect> {
        // Ensure suggestions are up to date, then fetch effective command
        let SharedCtx {
            registry, providers, ..
        } = &app.ctx;
        app.palette
            .apply_build_suggestions(registry, providers, &*app.ctx.theme);
        let spec = app.palette.selected_command();
        if spec.is_some() {
            app.help.set_spec(spec.cloned());
            return vec![Effect::ShowModal(Modal::Help)];
        }
        vec![]
    }

    /// Handles backspace key press in the command palette.
    ///
    /// This function removes the character before the current cursor position,
    /// closes the suggestions popup, and clears any previous error messages.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state to update
    fn handle_backspace(&self, app: &mut app::App) {
        app.palette.reduce_backspace();
        app.palette.reduce_clear_error();
        app.palette.apply_suggestions(vec![]);
    }

    /// Handles left arrow key press to move cursor left.
    ///
    /// This function moves the cursor one position to the left within the input
    /// text, allowing users to navigate and edit their command input.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state to update
    fn handle_cursor_left(&self, app: &mut app::App) {
        app.palette.reduce_move_cursor_left();
    }

    /// Handles right arrow key press to move cursor right.
    ///
    /// This function moves the cursor one position to the right within the
    /// input text, allowing users to navigate and edit their command input.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state to update
    fn handle_cursor_right(&self, app: &mut app::App) {
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
    fn handle_suggestion_navigation(&self, app: &mut app::App, direction: KeyCode) {
        let len = app.palette.suggestions().len();
        if len > 0 {
            let selected = app.palette.suggestion_index() as isize;
            let delta = if direction == KeyCode::Down { 1isize } else { -1isize };
            // Wrap around using modulus with length as isize
            let new_selected = (selected + delta).rem_euclid(len as isize) as usize;
            app.palette.set_selected(new_selected);
            app.palette.apply_ghost_text();
        }
    }

    /// Handles tab keypress to trigger or refresh the suggestions list.
    ///
    /// This function triggers building the suggestions list and opens the popup
    /// if suggestions are available.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state to update
    fn handle_tab_press(&self, app: &mut app::App) {
        if app.palette.is_input_empty() {
            return;
        }
        let SharedCtx {
            registry, providers, ..
        } = &app.ctx;

        app.palette
            .apply_build_suggestions(registry, providers, &*app.ctx.theme);
        // Open popup if we have suggestions or if provider-backed suggestions are loading
        let open = app.palette.suggestions_len() > 0 || app.palette.is_provider_loading();
        app.palette.set_is_suggestions_open(open);
    }

    /// Handles the Enter keypress.
    fn handle_enter(&self, app: &mut app::App) -> Vec<Effect> {
        // Execute the command
        if !app.palette.is_suggestions_open() {
            return vec![Effect::Run];
        } else {
            // otherwise, select from the list
            if let Some(item) = app.palette.suggestions().get(app.palette.suggestion_index()).cloned() {
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
                app.palette.set_selected(0);
                app.palette.set_is_suggestions_open(false);
            }
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
    fn handle_escape(&self, app: &mut app::App) {
        if app.palette.is_suggestions_open() {
            app.palette.set_is_suggestions_open(false);
            app.palette.apply_ghost_text();
        } else {
            app.palette.reduce_clear_all();
        }
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
        let main_regions = MainLayout::responsive_layout(rect, app);
        let theme = &*app.ctx.theme;
        // Render main border and get layout areas
        let splits = self.layout_palette_input(main_regions[0]);

        // Render input line with throbber and ghost text
        let input_para = self.create_input_paragraph(app, theme);
        frame.render_widget(input_para, splits[0]);

        // Position cursor in input line
        self.position_cursor(frame, splits[0], app, theme);

        // Render error message if present
        if let Some(error_para) = self.create_error_paragraph(app, theme) {
            frame.render_widget(error_para, splits[1]);
        }

        // Render suggestions popup
        let should_show_suggestions = app.palette.error_message().is_none()
            && app.palette.is_suggestions_open()
            && !app.palette.suggestions().is_empty();

        if should_show_suggestions {
            let suggestions_list = self.create_suggestions_list(app, theme);

            // Calculate popup dimensions
            let max_rows = 10usize;
            let rows = app.palette.suggestions().len().min(max_rows);
            let popup_height = rows as u16 + 2;
            let popup_area = Rect::new(rect.x, rect.y + 3, rect.width, popup_height);

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

        let hint_spans = self.get_hint_spans(app, true);
        let hints_widget = Paragraph::new(Line::from(hint_spans)).style(theme.text_muted_style());
        frame.render_widget(hints_widget, main_regions[2]);

        self.logs.render(frame, main_regions[1], app);
    }

    fn get_hint_spans(&self, app: &app::App, is_root: bool) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        let mut spans = Vec::new();
        if is_root {
            spans.push(Span::styled("Hints: ", theme.text_muted_style()));
        }

        if app.logs.focus.get() {
            spans.extend(self.logs.get_hint_spans(app, false));
        } else {
            spans.extend([
                Span::styled("Tab", theme.accent_emphasis_style()),
                Span::styled(" Completions ", theme.text_muted_style()),
                Span::styled("↑/↓", theme.accent_emphasis_style()),
                Span::styled(" Cycle  ", theme.text_muted_style()),
                Span::styled("Enter", theme.accent_emphasis_style()),
                Span::styled(" Accept  ", theme.text_muted_style()),
                Span::styled("Ctrl+H", theme.accent_emphasis_style()),
                Span::styled(" Help  ", theme.text_muted_style()),
                Span::styled("Esc", theme.accent_emphasis_style()),
                Span::styled(" Cancel", theme.text_muted_style()),
            ]);
        }
        spans
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
    fn handle_key_events(&mut self, app: &mut app::App, key: KeyEvent) -> Vec<Effect> {
        let mut effects: Vec<Effect> = vec![];
        // Delegate to logs if focused.
        if app.logs.focus.get() {
            return self.logs.handle_key_events(app, key);
        }

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
                    // Navigate suggestions when popup is open
                    self.handle_suggestion_navigation(app, key.code);
                } else {
                    // Navigate command history when popup is closed
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
                    self.handle_tab_press(app);
                }
            }
            KeyCode::Enter => {
                // Handle enter keypress
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
}
