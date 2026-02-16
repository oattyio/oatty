//! Shared results view utilities.
//!
//! This module contains reusable rendering helpers for the results
//! experience. The `ResultsTableView` encapsulates the TUI widgets required to
//! render tabular data, key-value fallback views, and scrolling chrome while
//! leaving ownership of the domain state with the caller.

use oatty_util::format_date_mmddyyyy;
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, List, ListItem, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, Wrap},
};
use serde_json::Value;

use crate::ui::{
    components::results::state::ResultsTableState,
    theme::{roles::Theme as UiTheme, theme_helpers as th},
};

/// Stateful ratatui widgets used to render results tables.
#[derive(Debug, Default)]
pub struct ResultsTableView;

impl ResultsTableView {
    /// Renders the primary results' region.
    ///
    /// When the provided results state contains tabular rows, this renders the
    /// results view along with the scrollbar. When there are no rows, the method
    /// falls back to rendering key-value entries or a simple paragraph
    /// representation of the JSON payload.
    ///
    /// Returns `true` when a tabular view was rendered which allows controllers
    /// to decide whether supporting UI (such as pagination) should be shown.
    pub fn render_results(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        state: &mut ResultsTableState,
        focused: bool,
        theme: &dyn UiTheme,
    ) -> bool {
        if state.selected_result_json().is_none() {
            render_empty_placeholder(frame, area, theme);
            return false;
        }
        let mouse_over_index = state.mouse_over_idx;
        let table_layout = TableLayout::new(area);
        let rows = state.create_rows(table_layout.table_area.width, theme);
        if let Some((truncate_index, rows)) = rows
            && !rows.is_empty()
        {
            let rows = rows_with_mouse_highlight(rows, mouse_over_index, theme);
            let render_input = JsonTableRenderInput {
                table_layout,
                rows,
                truncate_index,
                focused,
            };
            self.render_json_table(frame, state, theme, render_input);
            return true;
        }
        let json = state.selected_result_json().cloned().unwrap();
        self.render_key_value_or_text(frame, area, state, &json, theme);
        false
    }

    fn render_json_table(
        &mut self,
        frame: &mut Frame,
        state: &mut ResultsTableState<'_>,
        theme: &dyn UiTheme,
        render_input: JsonTableRenderInput<'_>,
    ) {
        let JsonTableRenderInput {
            table_layout,
            rows,
            truncate_index,
            focused,
        } = render_input;
        let total_rows = rows.len();

        let widths: &[Constraint] = state.column_constraints().map_or(&[][..], |constraints| constraints.as_slice());
        let headers: &[Cell<'_>] = state.headers().map_or(&[][..], |header_cells| &header_cells[truncate_index..]);

        let row_offset = state.table_state.offset();
        let visible_rows = table_layout.visible_rows;
        if visible_rows == 0 || rows.is_empty() {
            render_empty_placeholder(frame, table_layout.table_area, theme);
            return;
        }
        let table_widget = Table::new(rows, widths[truncate_index..].to_vec())
            .header(Row::new(headers.to_owned()).style(th::table_header_row_style(theme)))
            .column_spacing(1)
            .row_highlight_style(if focused {
                th::table_selected_style(theme)
            } else {
                Style::default()
            })
            .column_highlight_style(if focused {
                theme.selection_style().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            })
            .cell_highlight_style(if focused {
                theme.selection_style().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default()
            })
            .style(th::panel_style(theme));

        let mut cloned_state = state.table_state;
        frame.render_stateful_widget(&table_widget, table_layout.table_area, &mut cloned_state);

        self.render_scrollbar(frame, table_layout.scrollbar_area, total_rows, visible_rows, row_offset, theme);

        state.table_state = cloned_state;
    }

    /// Renders JSON payloads as key-value entries or plain text.
    pub fn render_key_value_or_text(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        state: &mut ResultsTableState,
        json: &Value,
        theme: &dyn UiTheme,
    ) {
        match json {
            Value::Object(_) => {
                let items: Vec<ListItem> = state
                    .kv_entries()
                    .iter()
                    .map(|entry| {
                        ListItem::new(Line::from(vec![
                            Span::styled(entry.display_key.clone(), theme.text_secondary_style().add_modifier(Modifier::BOLD)),
                            Span::raw(": "),
                            Span::styled(entry.display_value.clone(), theme.text_primary_style()),
                        ]))
                    })
                    .collect();

                let list = List::new(items)
                    .highlight_style(th::table_selected_style(theme))
                    .style(th::panel_style(theme));

                frame.render_stateful_widget(list, area, &mut state.list_state);
            }
            other => {
                let display = match other {
                    Value::String(text) => format_date_mmddyyyy(text).unwrap_or_else(|| text.clone()),
                    _ => other.to_string(),
                };

                let paragraph = Paragraph::new(display).wrap(Wrap { trim: false }).style(theme.text_primary_style());
                frame.render_widget(paragraph, area);
            }
        }
    }
}

fn render_empty_placeholder(frame: &mut Frame, area: Rect, theme: &dyn UiTheme) {
    let placeholder = Paragraph::new("No results to display").style(theme.text_muted_style());
    frame.render_widget(placeholder, area);
}

#[derive(Clone, Copy, Debug)]
struct TableLayout {
    table_area: Rect,
    scrollbar_area: Rect,
    visible_rows: usize,
}

struct JsonTableRenderInput<'a> {
    table_layout: TableLayout,
    rows: Vec<Row<'a>>,
    truncate_index: usize,
    focused: bool,
}

impl TableLayout {
    fn new(area: Rect) -> Self {
        let (table_area, scrollbar_area) = split_table_and_scrollbar_area(area);
        let visible_rows = visible_row_count(table_area);
        Self {
            table_area,
            scrollbar_area,
            visible_rows,
        }
    }
}

fn split_table_and_scrollbar_area(area: Rect) -> (Rect, Rect) {
    if area.width <= 1 {
        return (area, area);
    }

    let table_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width.saturating_sub(1),
        height: area.height,
    };
    let scrollbar_area = Rect {
        x: area.x + area.width.saturating_sub(1),
        y: area.y,
        width: 1,
        height: area.height,
    };
    (table_area, scrollbar_area)
}

fn visible_row_count(area: Rect) -> usize {
    area.height.saturating_sub(1) as usize
}
fn scrollbar_viewport_height(visible_rows: usize) -> usize {
    visible_rows.max(1)
}

fn rows_with_mouse_highlight<'a>(mut rows: Vec<Row<'a>>, mouse_over_idx: Option<usize>, theme: &dyn UiTheme) -> Vec<Row<'a>> {
    if let Some(highlight_idx) = highlighted_row_index(mouse_over_idx, rows.len()) {
        let mut row = rows[highlight_idx].clone();
        // Highlight the row if the mouse is over it.
        row = row.style(theme.selection_style().add_modifier(Modifier::BOLD));
        std::mem::swap(&mut rows[highlight_idx], &mut row);
    }
    rows
}

fn highlighted_row_index(mouse_over_idx: Option<usize>, rows_len: usize) -> Option<usize> {
    match mouse_over_idx {
        Some(idx) if idx < rows_len => Some(idx),
        _ => None,
    }
}

impl ResultsTableView {
    fn render_scrollbar(&self, frame: &mut Frame, area: Rect, total_rows: usize, visible_rows: usize, offset: usize, theme: &dyn UiTheme) {
        if total_rows <= visible_rows {
            return;
        }

        let viewport_height = scrollbar_viewport_height(visible_rows);
        let max_scroll_offset = total_rows.saturating_sub(viewport_height);
        let mut scrollbar_state = ScrollbarState::new(max_scroll_offset)
            .position(offset.min(max_scroll_offset))
            .viewport_content_length(viewport_height);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .thumb_style(Style::default().fg(theme.roles().scrollbar_thumb))
            .track_style(Style::default().fg(theme.roles().scrollbar_track));
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

#[cfg(test)]
mod tests {
    use super::{highlighted_row_index, scrollbar_viewport_height};
    #[test]
    fn scrollbar_viewport_height_never_zero() {
        assert_eq!(scrollbar_viewport_height(0), 1);
        assert_eq!(scrollbar_viewport_height(5), 5);
    }

    #[test]
    fn highlighted_row_index_validates_bounds() {
        assert_eq!(highlighted_row_index(Some(2), 3), Some(2));
        assert_eq!(highlighted_row_index(Some(3), 3), None);
        assert_eq!(highlighted_row_index(None, 3), None);
    }
}
