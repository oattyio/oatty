//! Command browser component for interactive command discovery and selection.
//!
//! This module provides a modal interface for browsing and selecting Oatty commands.
//! The browser features:
//! - A search bar for filtering commands by name or group
//! - A scrollable list of filtered commands with keyboard navigation
//! - An inline help panel that displays detailed command information
//! - Focus management between search, command list, and help panels
//! - Keyboard shortcuts for common actions (Enter to send it to palette, Esc to close)
//!
//! The component follows the TUI architecture pattern where it implements the `Component`
//! trait and manages its rendering and event handling through focused helper methods.
//! State is managed through the `BrowserState` struct in the app context.
//!
//! # Usage
//! The browser is typically opened via a global shortcut (Ctrl+F) and provides
//! an interactive way to discover and select commands without needing to remember
//! exact command names or syntax.

use crate::app::App;
use crate::ui::components::HelpComponent;
use crate::ui::components::browser::state::CursorDirection;
use crate::ui::theme::theme_helpers::{create_list_with_highlight, highlight_segments};
use crate::ui::{components::component::Component, theme::theme_helpers as th};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use oatty_types::{Effect, Route};
use ratatui::layout::Position;
use ratatui::style::Modifier;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::*,
};

/// A modal component for browsing and selecting Oatty commands interactively.
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
/// The component operates on the `BrowserState,` which is owned by the app context.
/// This allows other parts of the UI to coordinate with the browser's state.
#[derive(Debug, Default, Clone, Copy)]
struct BrowserLayout {
    search_area: Rect,
    search_inner_area: Rect,
    list_area: Rect,
}

#[derive(Debug, Default)]
pub struct BrowserComponent {
    layout: BrowserLayout,
    mouse_over_idx: Option<usize>,
    help_component: HelpComponent,
}

impl Component for BrowserComponent {
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

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let effects = self.help_component.handle_mouse_events(app, mouse);
        let pos = Position {
            x: mouse.column,
            y: mouse.row,
        };

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if self.layout.search_area.contains(pos) {
                    app.focus.focus(&app.browser.f_search);
                    let relative_column = mouse.column.saturating_sub(self.layout.search_inner_area.x);
                    app.browser.set_search_cursor_from_column(relative_column);
                }
                if self.layout.list_area.contains(pos) {
                    app.focus.focus(&app.browser.f_commands);
                    if let Some(idx) = self.hit_test_list(app, &pos) {
                        app.browser.list_state.select(Some(idx));
                        app.browser.commit_selection();
                    }
                }
            }
            MouseEventKind::Moved | MouseEventKind::Up(MouseButton::Left) => {
                self.mouse_over_idx = if self.layout.list_area.contains(pos) {
                    self.hit_test_list(app, &pos)
                } else {
                    None
                };
            }

            MouseEventKind::ScrollDown => {
                if self.layout.list_area.contains(pos) {
                    app.browser.list_state.scroll_down_by(1);
                }
            }
            MouseEventKind::ScrollUp => {
                if self.layout.list_area.contains(pos) {
                    app.browser.list_state.scroll_up_by(1);
                }
            }
            _ => {}
        }

        effects
    }

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
        let layout_chunks = self.get_preferred_layout(app, rect);

        let search_inner_area = self.render_search_panel(frame, app, layout_chunks[0]);

        let main_layout = self.create_main_layout(layout_chunks[1]);
        let list_area = self.render_commands_panel(frame, app, main_layout[0]);
        self.render_inline_help_panel(frame, app, main_layout[1]);
        self.layout = BrowserLayout {
            search_area: layout_chunks[0],
            search_inner_area,
            list_area,
        };
    }

    /// Renders the footer with keyboard shortcut hints.
    ///
    /// This method displays helpful keyboard shortcuts at the bottom of the
    /// browser modal to guide user interaction.
    ///
    /// # Arguments
    /// * `app` - The application state containing theme information
    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        th::build_hint_spans(theme, &[("Esc", " Clear "), ("Enter", " Send to palette  ")])
    }

    fn get_preferred_layout(&self, _app: &App, area: Rect) -> Vec<Rect> {
        Layout::vertical([
            Constraint::Length(3), // Search panel
            Constraint::Min(10),   // Main content
        ])
        .split(area)
        .to_vec()
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
            KeyCode::Esc => {
                app.browser.clear_search_query();
                app.focus.focus(&app.browser.f_search);
            }
            KeyCode::Char(character) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
                if !character.is_control() {
                    app.browser.append_search_character(character);
                }
            }
            KeyCode::Backspace => app.browser.remove_search_character(),
            KeyCode::Left => app.browser.move_search_cursor_left(),
            KeyCode::Right => app.browser.move_search_cursor_right(),
            KeyCode::Tab | KeyCode::BackTab => {
                if key.code == KeyCode::Tab {
                    app.focus.next();
                } else {
                    app.focus.prev();
                };
            }
            _ => {}
        }
    }

    fn handle_hot_keys(&self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Enter => self.apply_enter(app),
            KeyCode::Esc => self.handle_escape(app),
            _ => vec![],
        }
    }

    fn handle_escape(&self, app: &mut App) -> Vec<Effect> {
        let search_focused = app.browser.f_search.get();
        let has_query = !app.browser.search_query().trim().is_empty();
        if has_query || search_focused {
            app.browser.clear_search_query();
            app.focus.focus(&app.browser.f_search);
        }
        Vec::new()
    }

    /// Applies the 'enter' keypress action by switching to the palette
    /// and sending the selected command to the input
    fn apply_enter(&self, app: &App) -> Vec<Effect> {
        if let Some(spec) = app.browser.selected_command().cloned() {
            return vec![Effect::SwitchTo(Route::Palette), Effect::SendToPalette(Box::new(spec))];
        }
        vec![]
    }

    /// Handles keyboard input when the command list panel has focus.
    ///
    /// This method processes keyboard events specific to the command list,
    /// including up/down navigation, Enter to select, and Tab/BackTab for focus
    /// switching between panels.
    ///
    /// # Arguments
    /// * `app` - The application state to modify
    /// * `key` - The key event to process
    fn handle_commands_keys(&self, app: &mut App, key: KeyEvent) {
        match key.code {
            KeyCode::Down => app.browser.move_selection(CursorDirection::Down),
            KeyCode::Up => app.browser.move_selection(CursorDirection::Up),
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

    /// Creates the main horizontal layout for commands and help panels.
    ///
    /// This method splits the available area into two sections: 30% for the
    /// command list and 70% for the inline help panel.
    ///
    /// # Arguments
    /// * `area` - The area to split into panels
    ///
    /// # Returns
    /// * `Vec<Rect>` - Vector containing the commands and help panel areas
    fn create_main_layout(&self, area: Rect) -> Vec<Rect> {
        Layout::horizontal([
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
    fn render_search_panel(&self, frame: &mut Frame, app: &mut App, area: Rect) -> Rect {
        let search_title = self.create_search_title(app);
        let is_focused = app.browser.f_search.get();
        let mut search_block = th::block::<String>(&*app.ctx.theme, None, is_focused);
        search_block = search_block.title(search_title);
        let inner_area = search_block.inner(area);
        let theme = &*app.ctx.theme;
        let query = app.browser.search_query();
        let content_line = if is_focused || !query.is_empty() {
            Line::from(Span::styled(query.to_string(), theme.text_primary_style()))
        } else {
            Line::from(Span::from(""))
        };
        let search_paragraph = Paragraph::new(content_line).style(theme.text_primary_style()).block(search_block);
        frame.render_widget(search_paragraph, area);
        self.set_search_cursor(frame, app, inner_area);
        inner_area
    }

    /// Creates the title for the search panel with an optional debug indicator.
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
            "Filter Commands",
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
            let cursor_columns = app.browser.search_cursor_columns() as u16;
            let cursor_x = inner_area.x.saturating_add(cursor_columns);
            let cursor_y = inner_area.y;
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }

    fn hit_test_list(&mut self, app: &mut App, pos: &Position) -> Option<usize> {
        let list_offset = app.browser.list_state.offset();
        let idx = pos.y.saturating_sub(self.layout.list_area.y) as usize + list_offset;
        if idx >= app.browser.filtered().len() {
            return None;
        }

        Some(idx)
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
    fn render_commands_panel(&mut self, frame: &mut Frame, app: &mut App, area: Rect) -> Rect {
        let browser = &mut app.browser;
        let commands_title = format!("Commands ({})", browser.filtered().len());
        let is_focused = browser.f_commands.get();
        let commands_block = th::block(&*app.ctx.theme, Some(&commands_title), is_focused);
        let inner_height = commands_block.inner(area).height as usize;
        browser.set_viewport_rows(inner_height);
        let selection_style = app.ctx.theme.selection_style().add_modifier(Modifier::BOLD);
        // Create command items and get list state separately to avoid borrowing conflicts
        let command_items: Vec<ListItem<'_>> = {
            let Some(lock) = browser.registry.lock().ok() else {
                return Rect::default();
            };
            let all_commands = &lock.commands;
            let search_query = browser.search_query();
            let theme = &*app.ctx.theme;
            browser
                .filtered()
                .iter()
                .enumerate()
                .map(|(idx, command_index)| {
                    let group = &all_commands[*command_index].group;
                    let name = &all_commands[*command_index].name;
                    let mut spans: Vec<Span<'_>> = Vec::new();
                    if !group.is_empty() {
                        spans.extend(highlight_segments(
                            search_query,
                            group,
                            theme.syntax_type_style(),
                            theme.search_highlight_style(),
                        ));
                        if !name.is_empty() {
                            spans.push(Span::raw(" "));
                        }
                    }
                    if !name.is_empty() {
                        spans.extend(highlight_segments(
                            search_query,
                            name,
                            theme.syntax_function_style(),
                            theme.search_highlight_style(),
                        ));
                    }
                    let mut list_item = ListItem::new(Line::from(spans)).style(app.ctx.theme.text_primary_style());
                    if self.mouse_over_idx.is_some_and(|hover| hover == idx) {
                        list_item = list_item.style(selection_style);
                    }

                    list_item
                })
                .collect()
        };
        let is_focused = browser.f_commands.get();
        let commands_list = create_list_with_highlight(command_items, &*app.ctx.theme, is_focused, Some(commands_block));
        let list_state = &mut browser.list_state;
        frame.render_stateful_widget(commands_list, area, list_state);

        Rect {
            x: area.x,
            y: area.y + 1, // 1 for border-top
            width: area.width,
            height: area.height.saturating_sub(2), // 1 for border-top and 1 for border-bottom
        }
    }

    /// Renders the inline help panel with command documentation.
    ///
    /// This method displays detailed help information for the currently selected
    ///  command or a placeholder message if no command is selected.
    ///
    /// # Arguments
    /// * `frame` - The Ratatui frame to render to
    /// * `app` - The application state containing selected command and theme information
    /// * `area` - The area to render the help panel in
    fn render_inline_help_panel(&mut self, frame: &mut Frame, app: &mut App, area: Rect) {
        self.help_component.set_focused(app.browser.f_help.get());
        self.help_component.render(frame, area, app);
    }
}
