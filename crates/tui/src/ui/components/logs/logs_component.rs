//! Logs component for displaying and interacting with application logs.
//!
//! This component provides a comprehensive interface for viewing, navigating,
//! and interacting with log entries. It supports:
//!
//! - **Detail views**: JSON logs can be viewed in formatted detail modals
//! - **Table integration**: Array JSON data is routed to the global results
//!   component
//! - **Copy functionality**: The selected log can be copied to the clipboard
//! - **Syntax highlighting**: Timestamps, UUIDs, and hex IDs are styled
//! - **Security**: Sensitive data is automatically redacted
//!
//! The component follows the TEA (The Elm Architecture) pattern and integrates
//! with the application's focus management system.

use super::state::LogEntry;
use crate::app::App;
use crate::ui::theme::theme_helpers::{create_list_with_highlight, highlight_segments};
use crate::ui::{components::component::Component, theme::theme_helpers as th, utils::build_copy_text};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use oatty_types::{Effect, Modal};
use oatty_util::truncate_with_ellipsis;
use ratatui::prelude::Position;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::*,
};

/// Component for displaying and interacting with application logs.
///
/// The LogsComponent provides a rich interface for viewing log entries with
/// support for selection, detail views, and various interaction modes. It
/// automatically handles data redaction for security and provides visual
/// enhancements for better readability.
#[derive(Debug, Default)]
pub struct LogsComponent {
    layout: LogsLayout,
    mouse_hover_filtered_index: Option<usize>,
}

#[derive(Debug, Default, Clone, Copy)]
struct LogsLayout {
    search_area: Rect,
    search_inner_area: Rect,
    list_area: Rect,
}

impl LogsComponent {
    // ============================================================================
    // Selection and Navigation Methods
    // ============================================================================

    fn selected_entry<'a>(&self, app: &'a App) -> Option<&'a LogEntry> {
        let selected_index = app.logs.selected_rich_index()?;
        app.logs.rich_entries.get(selected_index)
    }

    fn selected_json_entry(&self, app: &App) -> Option<serde_json::Value> {
        match self.selected_entry(app) {
            Some(LogEntry::Api { json: Some(value), .. }) | Some(LogEntry::Mcp { json: Some(value), .. }) => Some(value.clone()),
            _ => None,
        }
    }

    fn selection_supports_pretty_toggle(&self, app: &App) -> bool {
        matches!(
            self.selected_entry(app),
            Some(LogEntry::Api { json: Some(_), .. }) | Some(LogEntry::Mcp { json: Some(_), .. })
        )
    }

    fn apply_results_table_for_selected_entry(&self, app: &mut App) {
        if let Some(json) = self.selected_json_entry(app) {
            app.logs.results_table.apply_result_json(Some(json), &*app.ctx.theme, true);
        }
    }

    fn hover_index_for_position(&self, app: &App, position: Position) -> Option<usize> {
        hover_index_for_position(
            self.layout.list_area,
            position,
            app.logs.list_state.offset(),
            app.logs.filtered_indices().len(),
        )
    }

    fn handle_search_keys(&self, app: &mut App, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                app.logs.clear_search_query();
                app.logs.activate_search();
            }
            KeyCode::Char(character) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
                if !character.is_control() {
                    app.logs.append_search_character(character);
                }
            }
            KeyCode::Backspace => app.logs.remove_search_character(),
            KeyCode::Left => app.logs.move_search_cursor_left(),
            KeyCode::Right => app.logs.move_search_cursor_right(),
            KeyCode::Tab | KeyCode::BackTab => {
                app.logs.deactivate_search();
                if key.code == KeyCode::Tab {
                    app.focus.next();
                } else {
                    app.focus.prev();
                }
            }
            _ => {}
        }
    }

    fn render_search_panel(&self, frame: &mut Frame, app: &mut App, area: Rect) -> Rect {
        let search_title = Line::from(Span::styled(
            "Filter Logs",
            app.ctx.theme.text_secondary_style().add_modifier(Modifier::BOLD),
        ));
        let mut search_block = th::block::<String>(&*app.ctx.theme, None, app.logs.is_search_active());
        search_block = search_block.title(search_title);
        let inner_area = search_block.inner(area);
        let query = app.logs.search_query();
        let content_line = if app.logs.is_search_active() || !query.is_empty() {
            Line::from(Span::styled(query.to_string(), app.ctx.theme.text_primary_style()))
        } else {
            Line::from(Span::raw(""))
        };
        let paragraph = Paragraph::new(content_line)
            .style(app.ctx.theme.text_primary_style())
            .block(search_block);
        frame.render_widget(paragraph, area);
        if app.logs.is_search_active() {
            let cursor_columns = app.logs.search_cursor_columns() as u16;
            let cursor_x = inner_area.x.saturating_add(cursor_columns);
            frame.set_cursor_position((cursor_x, inner_area.y));
        }
        inner_area
    }
}

impl Component for LogsComponent {
    /// Handles keyboard input events for the log component.
    ///
    /// This method processes various key combinations to provide navigation,
    /// selection, and interaction functionality:
    ///
    /// ## Navigation Keys
    /// - **↑/↓**: Move cursor up/down
    /// ## Action Keys
    /// - **Enter**: Open detail view for selected entry
    /// - **c**: Copy selected content to clipboard
    /// - **v**: Toggle JSON pretty-printing (API entries only)
    ///
    /// # Arguments
    ///
    /// * `app` - Mutable reference to application state
    /// * `key` - The key event to process
    ///
    /// # Returns
    ///
    /// A vector of effects to be processed by the application
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let mut effects = Vec::new();

        if app.logs.is_search_active() {
            self.handle_search_keys(app, key);
            return effects;
        }

        // Handle main navigation and action keys
        match key.code {
            // tab navigation
            KeyCode::BackTab => {
                app.focus.prev();
            }
            KeyCode::Tab => {
                app.focus.next();
            }
            KeyCode::Esc => {
                if app.logs.has_search_query() {
                    app.logs.clear_search_query();
                    app.logs.activate_search();
                }
            }
            KeyCode::Char('/') => {
                app.logs.activate_search();
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.logs.activate_search();
            }
            KeyCode::Up => {
                app.logs.list_state.select_previous();
            }
            KeyCode::Down => {
                app.logs.list_state.select_next();
            }
            KeyCode::PageUp => {
                app.logs.list_state.scroll_up_by(10);
            }
            KeyCode::PageDown => {
                app.logs.list_state.scroll_down_by(10);
            }
            KeyCode::Home => {
                app.logs.list_state.scroll_up_by(u16::MAX);
            }
            KeyCode::End => {
                app.logs.list_state.scroll_down_by(u16::MAX);
            }
            KeyCode::Enter => {
                if self.selected_entry(app).is_some() {
                    self.apply_results_table_for_selected_entry(app);
                    effects.push(Effect::ShowModal(Modal::LogDetails));
                }
            }
            KeyCode::Char('c') => {
                let text = build_copy_text(app);
                effects.push(Effect::CopyLogsRequested(text));
            }
            KeyCode::Char('v') => {
                if self.selection_supports_pretty_toggle(app) {
                    app.logs.pretty_json = !app.logs.pretty_json;
                }
                return effects;
            }
            _ => {}
        }
        effects
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let pos = Position::new(mouse.column, mouse.row);
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) && self.layout.search_area.contains(pos) {
            app.focus.focus(&app.logs.f_list);
            app.logs.activate_search();
            let relative_column = mouse.column.saturating_sub(self.layout.search_inner_area.x);
            app.logs.set_search_cursor_from_column(relative_column);
            return Vec::new();
        }

        let hover_index = self.hover_index_for_position(app, pos);

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) if hover_index.is_some() => {
                app.logs.deactivate_search();
                app.focus.focus(&app.logs.f_list);
                if app.logs.list_state.selected() == hover_index {
                    self.apply_results_table_for_selected_entry(app);
                    return vec![Effect::ShowModal(Modal::LogDetails)];
                }
                app.logs.list_state.select(hover_index);
            }

            MouseEventKind::Moved | MouseEventKind::Up(MouseButton::Left) => {
                self.mouse_hover_filtered_index = hover_index;
            }
            MouseEventKind::ScrollDown if self.layout.list_area.contains(pos) => {
                app.logs.list_state.scroll_down_by(1);
            }
            MouseEventKind::ScrollUp if self.layout.list_area.contains(pos) => {
                app.logs.list_state.scroll_up_by(1);
            }
            _ => {}
        }

        Vec::new()
    }

    /// Renders the logs component to the terminal frame.
    ///
    /// This method handles the complete rendering of the logs interface,
    /// including:
    ///
    /// - **Main log list**: Displays all log entries with syntax highlighting
    /// - **Selection highlighting**: Shows current selection with visual
    ///   indicators
    /// - **Scrollbar**: Indicates position within the log list when focused
    /// - **Hint bar**: Shows available keyboard shortcuts when focused
    /// - **Detail modal**: Overlays detailed view for selected entries
    ///
    /// The component adapts its appearance based on focus state and provides
    /// visual feedback for user interactions.
    ///
    /// # Arguments
    ///
    /// * `frame` - The terminal frame to render to
    /// * `rect` - The rectangular area allocated for this component
    /// * `app` - The application state containing logs and UI state
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let focused = app.logs.container_focus.get();
        let filtered_count = app.logs.filtered_indices().len();
        let total_count = app.logs.rich_entries.len();
        let title = if app.logs.has_search_query() {
            format!("Logs ({filtered_count}/{total_count})")
        } else {
            format!("Logs ({total_count})")
        };
        let block = th::block(&*app.ctx.theme, Some(&title), focused);
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        let panels = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(inner);
        let search_inner_area = self.render_search_panel(frame, app, panels[0]);
        let list_area = panels[1];
        let search_query = app.logs.search_query();
        let filtered_indices = app.logs.filtered_indices().to_vec();

        let items: Vec<ListItem> = filtered_indices
            .iter()
            .enumerate()
            .map(|(filtered_index, rich_index)| {
                let entry = &app.logs.rich_entries[*rich_index];
                let entry_text = entry.to_string();
                let first_line = entry_text.lines().next().unwrap_or_default();
                let display_text = truncate_with_ellipsis(first_line, list_area.width.saturating_sub(2) as usize);
                let line = if search_query.trim().is_empty() {
                    th::styled_line(&*app.ctx.theme, &display_text)
                } else {
                    Line::from(highlight_segments(
                        search_query,
                        &display_text,
                        app.ctx.theme.text_primary_style(),
                        app.ctx.theme.search_highlight_style(),
                    ))
                };
                let mut item = ListItem::new(line);
                if self.mouse_hover_filtered_index == Some(filtered_index) && app.logs.list_state.selected() != Some(filtered_index) {
                    item = item.style(app.ctx.theme.selection_style());
                }
                item
            })
            .collect();
        let list = create_list_with_highlight(items, &*app.ctx.theme, focused, None);
        frame.render_stateful_widget(list, list_area, &mut app.logs.list_state);

        let content_len = filtered_count;
        if focused && content_len > 0 {
            let visible = list_area.height as usize;
            if visible > 0 && content_len > visible {
                let viewport_height = scrollbar_viewport_height(visible);
                let max_scroll_offset = content_len.saturating_sub(viewport_height);
                let top = app.logs.list_state.offset().min(max_scroll_offset);
                let mut sb_state = ScrollbarState::new(max_scroll_offset)
                    .position(top)
                    .viewport_content_length(viewport_height);
                let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_style(Style::default().fg(app.ctx.theme.roles().scrollbar_thumb))
                    .track_style(Style::default().fg(app.ctx.theme.roles().scrollbar_track));
                frame.render_stateful_widget(sb, list_area, &mut sb_state);
            }
        }
        self.layout = LogsLayout {
            search_area: panels[0],
            search_inner_area,
            list_area,
        };
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        // Only render when logs are focused (rat-focus)
        if !app.logs.container_focus.get() {
            return vec![];
        }

        // Decide if we should show the pretty/raw toggle hint
        let mut show_pretty_toggle = false;
        if let Some(selected_index) = app.logs.selected_rich_index()
            && matches!(
                app.logs.rich_entries.get(selected_index),
                Some(LogEntry::Api { json: Some(_), .. }) | Some(LogEntry::Mcp { json: Some(_), .. })
            )
        {
            show_pretty_toggle = true;
        }

        let theme = &*app.ctx.theme;
        let mut spans = th::build_hint_spans(
            theme,
            &[
                ("/", " Search  "),
                ("Ctrl+F", " Focus Search  "),
                ("↑/↓", " Move  "),
                ("PgUp/PgDn", " Page  "),
                ("Home/End", " Jump  "),
                ("Enter", " Open  "),
                ("C", " Copy  "),
            ],
        );
        if app.logs.is_search_active() || app.logs.has_search_query() {
            spans.extend(th::build_hint_spans(theme, &[("Esc", " Clear Search  ")]));
        }
        if show_pretty_toggle {
            spans.push(Span::styled("V ", theme.accent_emphasis_style()));
            // Show current mode with green highlight
            if app.logs.pretty_json {
                spans.push(Span::styled("pretty", Style::default().fg(theme.roles().success)));
                spans.push(Span::styled("/raw  ", theme.text_muted_style()));
            } else {
                spans.push(Span::styled("pretty/", theme.text_muted_style()));
                spans.push(Span::styled("raw  ", Style::default().fg(theme.roles().success)));
            }
        }

        spans
    }
}

fn hover_index_for_position(list_area: Rect, position: Position, offset: usize, entry_len: usize) -> Option<usize> {
    if !list_area.contains(position) {
        return None;
    }
    let relative_row = position.y.saturating_sub(list_area.y) as usize;
    let index = relative_row + offset;
    if index < entry_len { Some(index) } else { None }
}

fn scrollbar_viewport_height(visible_rows: usize) -> usize {
    visible_rows.max(1)
}

#[cfg(test)]
mod tests {
    use super::{hover_index_for_position, scrollbar_viewport_height};
    use ratatui::layout::Rect;
    use ratatui::prelude::Position;

    #[test]
    fn hover_index_returns_none_outside_list_area() {
        let area = Rect::new(0, 0, 10, 5);
        let position = Position::new(20, 2);
        assert_eq!(hover_index_for_position(area, position, 0, 10), None);
    }

    #[test]
    fn hover_index_accounts_for_scroll_offset() {
        let area = Rect::new(0, 0, 10, 5);
        let position = Position::new(1, 3);
        assert_eq!(hover_index_for_position(area, position, 4, 10), Some(7));
    }

    #[test]
    fn hover_index_bounds_checks_entries() {
        let area = Rect::new(0, 0, 10, 5);
        let position = Position::new(1, 4);
        assert_eq!(hover_index_for_position(area, position, 4, 5), None);
    }

    #[test]
    fn scrollbar_viewport_height_never_zero() {
        assert_eq!(scrollbar_viewport_height(0), 1);
        assert_eq!(scrollbar_viewport_height(5), 5);
    }
}
