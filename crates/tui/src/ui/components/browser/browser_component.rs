//! Command browser component for interactive command discovery and selection.
//!
//! This module provides a modal interface for browsing and selecting Heroku commands.
//! The browser features:
//! - A search bar for filtering commands by name or group
//! - A scrollable list of filtered commands with keyboard navigation
//! - An inline help panel that displays detailed command information
//! - Focus management between search, commands list, and help panels
//! - Keyboard shortcuts for common actions (Enter to send to palette, Esc to close)
//!
//! The component follows the TUI architecture pattern where it implements the `Component`
//! trait and manages its rendering and event handling through focused helper methods.
//! State is managed through the `BrowserState` struct in the app context.
//!
//! # Usage
//! The browser is typically opened via a global shortcut (Ctrl+F) and provides
//! an interactive way to discover and select commands without needing to remember
//! exact command names or syntax.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use heroku_types::{Effect, Route};
use ratatui::style::Modifier;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::*,
};

use crate::app::App;
use crate::ui::components::help::content::build_command_help_text;
use crate::ui::{
    components::{browser::layout::BrowserLayout, component::Component},
    theme::theme_helpers as th,
};

/// A modal component for browsing and selecting Heroku commands interactively.
///
/// The `BrowserComponent` provides a comprehensive command discovery interface that
/// allows users to search, browse, and select commands through a modal overlay.
/// It implements the `Component` trait and integrates with the app's focus management
/// system to provide keyboard navigation between different panels.
///
/// # Features
/// - **Search functionality**: Real-time filtering of commands based on fuzzy matching
/// - **Command list**: Scrollable list of filtered commands with selection highlighting
/// - **Inline help**: Dynamic help panel that updates based on the selected command
/// - **Focus management**: Tab/BackTab navigation between search, commands, and help panels
/// - **Keyboard shortcuts**: Global shortcuts for common actions like closing or copying
///
/// # State Management
/// The component operates on the `BrowserState` which is owned by the app context.
/// This allows other parts of the UI to coordinate with the browser's state.
#[derive(Debug, Default)]
pub struct BrowserComponent;

impl Component for BrowserComponent {
    /// Renders the browser modal with all its panels and components.
    ///
    /// This method creates a centered modal overlay that contains the search panel,
    /// commands list, inline help panel, and footer. It uses the browser layout
    /// system to properly arrange the components within the available space.
    ///
    /// # Arguments
    /// * `frame` - The Ratatui frame to render to
    /// * `rect` - The available rendering area
    /// * `app` - The application state containing browser data and theme
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let layout_chunks = BrowserLayout::vertical_layout(rect);

        self.render_search_panel(frame, app, layout_chunks[0]);

        let main_layout = self.create_main_layout(layout_chunks[1]);
        self.render_commands_panel(frame, app, main_layout[0]);
        self.render_inline_help_panel(frame, app, main_layout[1]);

        let hint_spans = self.get_hint_spans(app, true);

        let paragraph = Paragraph::new(Line::from(hint_spans));
        frame.render_widget(paragraph, layout_chunks[2]);
    }

    /// Renders the footer with keyboard shortcut hints.
    ///
    /// This method displays helpful keyboard shortcuts at the bottom of the
    /// browser modal to guide user interaction.
    ///
    /// # Arguments
    /// * `frame` - The Ratatui frame to render to
    /// * `app` - The application state containing theme information
    /// * `area` - The area to render the footer in
    fn get_hint_spans(&self, app: &App, is_root: bool) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        let mut spans = vec![];
        if is_root {
            spans.push(Span::styled("Hint: ", theme.text_muted_style()));
        }
        spans.extend([
            Span::styled("Esc", theme.accent_emphasis_style()),
            Span::styled(" Clear ", theme.text_muted_style()),
            Span::styled("Enter", theme.accent_emphasis_style()),
            Span::styled(" Send to palette  ", theme.text_muted_style()),
        ]);
        spans
    }

    /// Handles keyboard events for the browser component.
    ///
    /// This method routes keyboard events to the appropriate handler based on
    /// which panel currently has focus. It first checks for global shortcuts,
    /// then delegates to either the search or commands panel handlers.
    ///
    /// # Arguments
    /// * `app` - The application state to modify
    /// * `key` - The key event to process
    ///
    /// # Returns
    /// * `Vec<Effect>` - Effects to be processed by the runtime
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let effects = self.handle_hot_keys(app, key);

        if app.browser.f_search.get() {
            self.handle_search_keys(app, key);
        } else if app.browser.f_commands.get() {
            self.handle_commands_keys(app, key);
        } else {
            match key.code {
                KeyCode::BackTab => app.focus.prev(),
                KeyCode::Tab => app.focus.next(),
                _ => false,
            };
        }
        effects
    }
}

impl BrowserComponent {
    /// Handles keyboard input when the search panel has focus.
    ///
    /// This method processes keyboard events specific to the search input field,
    /// including character input, backspace, escape, navigation keys, and focus
    /// switching via Tab/BackTab.
    ///
    /// # Arguments
    /// * `app` - The application state to modify
    /// * `key` - The key event to process
    fn handle_search_keys(&self, app: &mut App, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
                app.browser.search_input_push(c);
            }
            KeyCode::Backspace => app.browser.search_input_pop(),

            KeyCode::Tab | KeyCode::BackTab => {
                if key.code == KeyCode::Tab {
                    app.focus.next();
                } else {
                    app.focus.prev();
                };
            }
            KeyCode::Down => app.browser.move_selection(1),
            KeyCode::Up => app.browser.move_selection(-1),
            _ => {}
        }
    }

    fn handle_hot_keys(&self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Enter => {
                return self.apply_enter(app);
            }
            KeyCode::Esc => {
                app.browser.search_input_clear();
                return vec![];
            }
            _ => return vec![],
        }
    }

    /// Handle Enter within the browser context (noop for now).
    pub fn apply_enter(&self, app: &App) -> Vec<Effect> {
        if let Some(spec) = app.browser.selected_command().cloned() {
            return vec![Effect::SwitchTo(Route::Palette), Effect::SendToPalette(spec)];
        }
        vec![]
    }

    /// Handles keyboard input when the commands list panel has focus.
    ///
    /// This method processes keyboard events specific to the commands list,
    /// including up/down navigation, Enter to select, and Tab/BackTab for focus
    /// switching between panels.
    ///
    /// # Arguments
    /// * `app` - The application state to modify
    /// * `key` - The key event to process
    fn handle_commands_keys(&self, app: &mut App, key: KeyEvent) {
        match key.code {
            KeyCode::Down => app.browser.move_selection(1),
            KeyCode::Up => app.browser.move_selection(-1),
            KeyCode::Tab | KeyCode::BackTab => {
                if key.code == KeyCode::Tab {
                    app.focus.next();
                } else {
                    app.focus.prev();
                }
            }
            _ => {}
        }
    }

    /// Creates the help content (title and text) based on the selected command.
    fn create_help_content<'a>(&self, app: &'a App) -> (String, ratatui::text::Text<'a>) {
        if let Some(selected_command_spec) = app.browser.selected_command() {
            let command_display_name = self.format_command_display_name(selected_command_spec);
            let help_title = format!("Help — {}", command_display_name);
            let help_text = build_command_help_text(&*app.ctx.theme, selected_command_spec);
            (help_title, help_text)
        } else {
            let default_title = "Help".to_string();
            let default_text = ratatui::text::Text::from(Line::from(Span::styled(
                "Select a command to view detailed help.",
                app.ctx.theme.text_secondary_style().add_modifier(Modifier::BOLD),
            )));
            (default_title, default_text)
        }
    }

    /// Formats the command name for display in the help panel.
    fn format_command_display_name(&self, command_spec: &heroku_types::CommandSpec) -> String {
        if command_spec.name.is_empty() {
            return command_spec.group.clone();
        }

        let mut name_parts = command_spec.name.splitn(2, ':');
        let group_name = name_parts.next().unwrap_or("");
        let remaining_name = name_parts.next().unwrap_or("");

        if remaining_name.is_empty() {
            group_name.to_string()
        } else {
            format!("{} {}", group_name, remaining_name)
        }
    }

    /// Creates the main horizontal layout for commands and help panels.
    ///
    /// This method splits the available area into two sections: 30% for the
    /// commands list and 70% for the inline help panel.
    ///
    /// # Arguments
    /// * `area` - The area to split into panels
    ///
    /// # Returns
    /// * `Vec<Rect>` - Vector containing the commands and help panel areas
    fn create_main_layout(&self, area: Rect) -> Vec<Rect> {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30), // Commands
                Constraint::Percentage(70), // Inline Help
            ])
            .split(area)
            .to_vec()
    }

    /// Renders the search input panel with cursor positioning.
    ///
    /// This method creates the search input field with appropriate styling
    /// and focus indication. It also positions the cursor correctly within
    /// the input field.
    ///
    /// # Arguments
    /// * `frame` - The Ratatui frame to render to
    /// * `app` - The application state containing search input and theme
    /// * `area` - The area to render the search panel in
    fn render_search_panel(&self, frame: &mut Frame, app: &mut App, area: Rect) {
        let search_title = self.create_search_title(app);
        let is_focused = app.browser.f_search.get();
        let mut search_block = th::block(&*app.ctx.theme, None, is_focused);
        search_block = search_block.title(search_title);
        let inner_area = search_block.inner(area);
        let search_paragraph = Paragraph::new(app.browser.search_input().as_str())
            .style(app.ctx.theme.text_primary_style())
            .block(search_block);
        frame.render_widget(search_paragraph, area);
        self.set_search_cursor(frame, app, inner_area);
    }

    /// Creates the title for the search panel with optional debug indicator.
    ///
    /// This method generates the title line for the search panel, including
    /// a debug indicator when debug mode is enabled.
    ///
    /// # Arguments
    /// * `app` - The application state containing debug and theme information
    ///
    /// # Returns
    /// * `Line<'_>` - The formatted title line
    fn create_search_title(&self, app: &App) -> Line<'_> {
        let theme = &*app.ctx.theme;
        Line::from(Span::styled(
            "Browse Commands",
            theme.text_secondary_style().add_modifier(Modifier::BOLD),
        ))
    }

    /// Sets the cursor position within the search input field.
    ///
    /// This method positions the cursor at the end of the current search input
    /// when the search panel has focus.
    ///
    /// # Arguments
    /// * `frame` - The Ratatui frame to set cursor position on
    /// * `app` - The application state containing search input and focus information
    /// * `inner_area` - The inner area of the search panel
    fn set_search_cursor(&self, frame: &mut Frame, app: &App, inner_area: Rect) {
        if app.browser.f_search.get() {
            let cursor_x = inner_area.x.saturating_add(app.browser.search_input().chars().count() as u16);
            let cursor_y = inner_area.y;
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }

    /// Renders the commands list panel with selection highlighting.
    ///
    /// This method creates a scrollable list of filtered commands with proper
    /// selection highlighting and focus indication.
    ///
    /// # Arguments
    /// * `frame` - The Ratatui frame to render to
    /// * `app` - The application state containing commands and theme information
    /// * `area` - The area to render the commands panel in
    fn render_commands_panel(&self, frame: &mut Frame, app: &mut App, area: Rect) {
        let commands_title = format!("Commands ({})", app.browser.filtered().len());
        let is_focused = app.browser.f_commands.get();
        let commands_block = th::block(&*app.ctx.theme, Some(&commands_title), is_focused);
        let inner_height = commands_block.inner(area).height as usize;
        app.browser.set_viewport_rows(inner_height);

        // Create command items and get list state separately to avoid borrowing conflicts
        let command_items = {
            let browser = &app.browser;
            browser
                .filtered()
                .iter()
                .map(|command_index| {
                    let all_commands = browser.all_commands();
                    let command_group = &all_commands[*command_index].group;
                    let command_name = &all_commands[*command_index].name;
                    let display_text = if command_name.is_empty() {
                        command_group.to_string()
                    } else {
                        format!("{} {}", command_group, command_name)
                    };
                    ListItem::new(display_text).style(app.ctx.theme.text_primary_style())
                })
                .collect::<Vec<_>>()
        };

        let commands_list = List::new(command_items)
            .block(commands_block)
            .highlight_style(app.ctx.theme.selection_style().add_modifier(Modifier::BOLD))
            .highlight_symbol("> ");
        let list_state = app.browser.list_state();
        frame.render_stateful_widget(commands_list, area, list_state);
    }

    /// Renders the inline help panel with command documentation.
    ///
    /// This method displays detailed help information for the currently selected
    /// command, or a placeholder message if no command is selected.
    ///
    /// # Arguments
    /// * `frame` - The Ratatui frame to render to
    /// * `app` - The application state containing selected command and theme information
    /// * `area` - The area to render the help panel in
    fn render_inline_help_panel(&self, frame: &mut Frame, app: &mut App, area: Rect) {
        let (help_title, help_text) = self.create_help_content(app);
        let help_block = th::block(&*app.ctx.theme, Some(&help_title), false);
        let inner_area = help_block.inner(area);
        frame.render_widget(help_block, area);
        let help_paragraph = Paragraph::new(help_text)
            .style(app.ctx.theme.text_primary_style())
            .wrap(Wrap { trim: false });
        frame.render_widget(help_paragraph, inner_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::roles::ThemeRoles;
    use heroku_types::{CommandSpec, ServiceId};
    use ratatui::style::Style;

    /// Mock theme for testing purposes
    struct MockTheme;
    impl crate::ui::theme::Theme for MockTheme {
        fn roles(&self) -> &ThemeRoles {
            &ThemeRoles {
                modal_bg: ratatui::style::Color::Black,
                background: ratatui::style::Color::Black,
                surface: ratatui::style::Color::DarkGray,
                surface_muted: ratatui::style::Color::Gray,
                border: ratatui::style::Color::Gray,
                divider: ratatui::style::Color::DarkGray,
                text: ratatui::style::Color::White,
                text_secondary: ratatui::style::Color::Gray,
                text_muted: ratatui::style::Color::DarkGray,
                accent_primary: ratatui::style::Color::Blue,
                accent_secondary: ratatui::style::Color::Cyan,
                accent_subtle: ratatui::style::Color::DarkGray,
                info: ratatui::style::Color::Blue,
                success: ratatui::style::Color::Green,
                warning: ratatui::style::Color::Yellow,
                error: ratatui::style::Color::Red,
                selection_bg: ratatui::style::Color::Blue,
                selection_fg: ratatui::style::Color::White,
                focus: ratatui::style::Color::Blue,
                scrollbar_track: ratatui::style::Color::DarkGray,
                scrollbar_thumb: ratatui::style::Color::Gray,
            }
        }
        fn text_primary_style(&self) -> Style {
            Style::default()
        }
        fn text_secondary_style(&self) -> Style {
            Style::default()
        }
        fn text_muted_style(&self) -> Style {
            Style::default()
        }
        fn accent_emphasis_style(&self) -> Style {
            Style::default()
        }
        fn selection_style(&self) -> Style {
            Style::default()
        }
        fn border_style(&self, _focused: bool) -> Style {
            Style::default()
        }
    }

    #[test]
    fn test_format_command_display_name_with_colon() {
        let component = BrowserComponent;
        let command_spec = CommandSpec {
            name: "apps:create".to_string(),
            group: "apps".to_string(),
            summary: "Create a new app".to_string(),
            method: "POST".to_string(),
            path: "/apps".to_string(),
            service_id: ServiceId::CoreApi,
            flags: vec![],
            positional_args: vec![],
            ranges: vec![],
        };

        let result = component.format_command_display_name(&command_spec);
        assert_eq!(result, "apps create");
    }

    #[test]
    fn test_format_command_display_name_without_colon() {
        let component = BrowserComponent;
        let command_spec = CommandSpec {
            name: "apps".to_string(),
            group: "apps".to_string(),
            summary: "List apps".to_string(),
            method: "GET".to_string(),
            path: "/apps".to_string(),
            service_id: ServiceId::CoreApi,
            flags: vec![],
            positional_args: vec![],
            ranges: vec![],
        };

        let result = component.format_command_display_name(&command_spec);
        assert_eq!(result, "apps");
    }

    #[test]
    fn test_format_command_display_name_empty_name() {
        let component = BrowserComponent;
        let command_spec = CommandSpec {
            name: "".to_string(),
            group: "apps".to_string(),
            summary: "Apps command".to_string(),
            method: "GET".to_string(),
            path: "/apps".to_string(),
            service_id: ServiceId::CoreApi,
            flags: vec![],
            positional_args: vec![],
            ranges: vec![],
        };

        let result = component.format_command_display_name(&command_spec);
        assert_eq!(result, "apps");
    }
}
