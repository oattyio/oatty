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
use crate::ui::theme::theme_helpers::create_list_with_highlight;
use crate::ui::{components::component::Component, theme::theme_helpers as th, utils::build_copy_text};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use oatty_types::{Effect, Modal};
use oatty_util::truncate_with_ellipsis;
use ratatui::prelude::Position;
use ratatui::{Frame, layout::Rect, style::Style, text::Span, widgets::*};

/// Component for displaying and interacting with application logs.
///
/// The LogsComponent provides a rich interface for viewing log entries with
/// support for selection, detail views, and various interaction modes. It
/// automatically handles data redaction for security and provides visual
/// enhancements for better readability.
#[derive(Debug, Default)]
pub struct LogsComponent {
    list_area: Rect,
    mouse_hover_index: Option<usize>,
}

impl LogsComponent {
    // ============================================================================
    // Selection and Navigation Methods
    // ============================================================================

    fn selected_entry<'a>(&self, app: &'a App) -> Option<&'a LogEntry> {
        let selected_index = app.logs.list_state.selected()?;
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
        hover_index_for_position(self.list_area, position, app.logs.list_state.offset(), app.logs.rich_entries.len())
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

        // Handle main navigation and action keys
        match key.code {
            // tab navigation
            KeyCode::BackTab => {
                app.focus.prev();
            }
            KeyCode::Tab => {
                app.focus.next();
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
        let hover_index = self.hover_index_for_position(app, pos);

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) if hover_index.is_some() => {
                app.logs.list_state.select(hover_index);
                self.apply_results_table_for_selected_entry(app);
                return vec![Effect::ShowModal(Modal::LogDetails)];
            }

            MouseEventKind::Moved | MouseEventKind::Up(MouseButton::Left) => {
                self.mouse_hover_index = hover_index;
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
        let title = format!("Logs ({})", app.logs.rich_entries.len());
        let block = th::block(&*app.ctx.theme, Some(&title), focused);
        let inner = block.inner(rect);
        // Note: Entries are pre redacted when appended
        let items: Vec<ListItem> = app
            .logs
            .rich_entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let entry_text = entry.to_string();
                let first_line = entry_text.lines().next().unwrap_or_default();
                let display_text = truncate_with_ellipsis(first_line, inner.width as usize - 2);
                let line = th::styled_line(&*app.ctx.theme, &display_text);
                let mut item = ListItem::new(line);
                if self.mouse_hover_index == Some(index) && app.logs.list_state.selected() != Some(index) {
                    item = item.style(app.ctx.theme.selection_style());
                }
                item
            })
            .collect();
        let list = create_list_with_highlight(items, &*app.ctx.theme, focused, Some(block));
        frame.render_stateful_widget(list, rect, &mut app.logs.list_state);

        let content_len = app.logs.rich_entries.len();
        if focused && content_len > 0 {
            let visible = rect.height.saturating_sub(2) as usize; // Account for borders
            if visible > 0 && content_len > visible {
                let top = app.logs.list_state.offset().min(content_len.saturating_sub(visible));
                let mut sb_state = ScrollbarState::new(content_len).position(top).viewport_content_length(visible);
                let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_style(Style::default().fg(app.ctx.theme.roles().scrollbar_thumb))
                    .track_style(Style::default().fg(app.ctx.theme.roles().scrollbar_track));
                frame.render_stateful_widget(sb, rect, &mut sb_state);
            }
        }
        self.list_area = inner;
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        // Only render when logs are focused (rat-focus)
        if !app.logs.container_focus.get() {
            return vec![];
        }

        // Decide if we should show the pretty/raw toggle hint
        let mut show_pretty_toggle = false;
        if let Some(selected_index) = app.logs.list_state.selected()
            && matches!(
                app.logs.rich_entries.get(selected_index),
                Some(LogEntry::Api { json: Some(_), .. }) | Some(LogEntry::Mcp { json: Some(_), .. })
            )
        {
            show_pretty_toggle = true;
        }

        let theme = &*app.ctx.theme;
        let mut spans = th::build_hint_spans(theme, &[("↑/↓", " Move  "), ("Enter", " Open  "), ("C", " Copy  ")]);
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

#[cfg(test)]
mod tests {
    use super::hover_index_for_position;
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
}
