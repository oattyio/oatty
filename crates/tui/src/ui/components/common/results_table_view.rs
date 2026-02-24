//! Shared results view utilities.
//!
//! This module contains reusable rendering helpers for the results
//! experience. The `ResultsTableView` encapsulates the TUI widgets required to
//! render tabular data, key-value fallback views, and scrolling chrome while
//! leaving ownership of the domain state with the caller.

use oatty_util::format_date_mmddyyyy;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, List, ListItem, Paragraph, Row, Table, Wrap},
};
use serde_json::Value;
use unicode_width::UnicodeWidthStr;

use crate::ui::{
    components::common::render_vertical_scrollbar,
    components::results::state::ResultsTableState,
    theme::{roles::Theme as UiTheme, theme_helpers as th},
    utils::render_value,
};

/// Stateful ratatui widgets used to render results tables.
#[derive(Debug, Default)]
pub struct ResultsTableView {
    fallback_list_area: Option<Rect>,
    split_preview_area: Option<Rect>,
}

impl ResultsTableView {
    /// Returns the latest rendered fallback list area when key/value mode is active.
    pub const fn fallback_list_area(&self) -> Option<Rect> {
        self.fallback_list_area
    }

    /// Returns the latest rendered split preview area when visible.
    pub const fn split_preview_area(&self) -> Option<Rect> {
        self.split_preview_area
    }

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
        self.fallback_list_area = None;
        self.split_preview_area = None;
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
        let is_drill_mode = state.is_in_drill_mode();
        let table_widget = Table::new(rows, widths[truncate_index..].to_vec())
            .header(Row::new(headers.to_owned()).style(th::table_header_row_style(theme)))
            .column_spacing(1)
            .row_highlight_style(if focused {
                if is_drill_mode {
                    Style::default()
                } else {
                    th::table_selected_style(theme)
                }
            } else {
                Style::default()
            })
            .column_highlight_style(Style::default())
            .cell_highlight_style(if focused {
                if is_drill_mode {
                    theme.selection_style().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                } else {
                    Style::default()
                }
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
                let selected_value_overflows = selected_kv_value_overflows(state, area, theme);
                state.set_selected_kv_value_overflows(selected_value_overflows);
                let (list_area, preview_area) = split_key_value_areas(area, state.should_show_split_preview());
                self.fallback_list_area = Some(list_area);
                self.split_preview_area = preview_area;
                let items: Vec<ListItem> = state
                    .kv_entries()
                    .iter()
                    .map(|entry| {
                        let value_spans = render_value(&entry.key, &entry.raw_value, Some(theme)).into_spans();
                        let mut spans = Vec::with_capacity(value_spans.len() + 2);
                        if matches!(entry.raw_value, Value::Object(_) | Value::Array(_)) {
                            spans.push(Span::styled("› ", theme.syntax_type_style().add_modifier(Modifier::BOLD)));
                        } else {
                            spans.push(Span::styled("  ", theme.text_muted_style()));
                        }
                        spans.push(Span::styled(
                            entry.display_key.clone(),
                            theme.syntax_function_style().add_modifier(Modifier::BOLD),
                        ));
                        spans.push(Span::styled(": ", theme.text_muted_style()));
                        spans.extend(value_spans);
                        ListItem::new(Line::from(spans)).style(th::panel_style(theme))
                    })
                    .collect();
                let items = list_items_with_mouse_highlight(items, state.mouse_over_idx, theme);

                let list = List::new(items)
                    .highlight_style(th::table_selected_style(theme))
                    .style(th::panel_style(theme));

                frame.render_stateful_widget(list, list_area, &mut state.list_state);
                let visible_rows = list_area.height as usize;
                let total_rows = state.kv_entries().len();
                self.render_scrollbar(frame, list_area, total_rows, visible_rows, state.list_state.offset(), theme);

                self.render_split_preview(frame, preview_area, state, theme);
            }
            other => {
                state.set_selected_kv_value_overflows(false);
                state.update_split_preview_metrics(0, 0);
                self.fallback_list_area = Some(area);
                self.split_preview_area = None;
                let display = match other {
                    Value::String(text) => format_date_mmddyyyy(text).unwrap_or_else(|| text.clone()),
                    _ => other.to_string(),
                };

                let mut paragraph = Paragraph::new(display).wrap(Wrap { trim: false }).style(theme.text_primary_style());
                let line_count = paragraph.line_count(area.width);
                let visible_rows = area.height as usize;
                let max_offset = line_count.saturating_sub(visible_rows.max(1));
                let offset = state.list_state.offset().min(max_offset);
                state.list_state = state.list_state.with_offset(offset);
                paragraph = paragraph.scroll((offset as u16, 0));
                frame.render_widget(paragraph, area);
                self.render_scrollbar(frame, area, line_count, visible_rows, offset, theme);
            }
        }
    }

    fn render_split_preview(&self, frame: &mut Frame, preview_area: Option<Rect>, state: &mut ResultsTableState, theme: &dyn UiTheme) {
        let Some(preview_area) = preview_area else {
            state.update_split_preview_metrics(0, 0);
            return;
        };
        if preview_area.height == 0 || preview_area.width == 0 {
            state.update_split_preview_metrics(0, 0);
            return;
        }
        let preview_text = selected_kv_preview_text(state);
        let preview_block = th::block(theme, Some("Value Preview"), false);
        let preview_inner = preview_block.inner(preview_area);
        frame.render_widget(preview_block, preview_area);
        if preview_inner.height == 0 || preview_inner.width == 0 {
            state.update_split_preview_metrics(0, 0);
            return;
        }

        let mut paragraph = Paragraph::new(preview_text)
            .style(theme.text_primary_style())
            .wrap(Wrap { trim: false });
        let line_count = paragraph.line_count(preview_inner.width);
        let capped_height = line_count.min(u16::MAX as usize) as u16;
        state.update_split_preview_metrics(capped_height, preview_inner.height);
        paragraph = paragraph.scroll((state.split_preview_scroll_offset(), 0));
        frame.render_widget(paragraph, preview_inner);
        self.render_scrollbar(
            frame,
            preview_inner,
            line_count,
            preview_inner.height as usize,
            state.split_preview_scroll_offset() as usize,
            theme,
        );
    }
}

fn render_empty_placeholder(frame: &mut Frame, area: Rect, theme: &dyn UiTheme) {
    let placeholder = Paragraph::new("No results to display").style(theme.text_muted_style());
    frame.render_widget(placeholder, area);
}

fn split_key_value_areas(area: Rect, show_split_preview: bool) -> (Rect, Option<Rect>) {
    if !show_split_preview || area.height < 8 {
        return (area, None);
    }
    let preview_height = (area.height / 3).max(4).min(area.height.saturating_sub(3));
    let layout = Layout::vertical([Constraint::Min(3), Constraint::Length(preview_height)]).split(area);
    if layout.len() < 2 {
        return (area, None);
    }
    (layout[0], Some(layout[1]))
}

fn selected_kv_preview_text(state: &ResultsTableState) -> String {
    let selected_index = state.list_state.selected().unwrap_or(0);
    let Some(entry) = state.selected_kv_entry(selected_index) else {
        return String::new();
    };
    match &entry.raw_value {
        Value::String(text) => format_date_mmddyyyy(text).unwrap_or_else(|| text.clone()),
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
    }
}

fn selected_kv_value_overflows(state: &ResultsTableState, area: Rect, theme: &dyn UiTheme) -> bool {
    let selected_index = state.list_state.selected().unwrap_or(0);
    let Some(entry) = state.selected_kv_entry(selected_index) else {
        return false;
    };
    let value_plain_text = render_value(&entry.key, &entry.raw_value, Some(theme)).into_plain_text();
    let prefix = if matches!(entry.raw_value, Value::Object(_) | Value::Array(_)) {
        UnicodeWidthStr::width("› ")
    } else {
        UnicodeWidthStr::width("  ")
    };
    let key_width = UnicodeWidthStr::width(entry.display_key.as_str());
    let colon_width = UnicodeWidthStr::width(": ");
    let has_scrollbar = state.kv_entries().len() > area.height as usize;
    let scrollbar_width = if has_scrollbar { 1 } else { 0 };
    let reserved_width = prefix + key_width + colon_width + scrollbar_width;
    let available_width = usize::from(area.width).saturating_sub(reserved_width);
    value_display_width_exceeds_available(value_plain_text.as_str(), available_width)
}

fn value_display_width_exceeds_available(value_text: &str, available_width: usize) -> bool {
    if available_width == 0 {
        return !value_text.is_empty();
    }
    UnicodeWidthStr::width(value_text) > available_width
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

fn list_items_with_mouse_highlight<'a>(
    mut items: Vec<ListItem<'a>>,
    mouse_over_idx: Option<usize>,
    theme: &dyn UiTheme,
) -> Vec<ListItem<'a>> {
    if let Some(highlight_idx) = highlighted_row_index(mouse_over_idx, items.len()) {
        let mut item = items[highlight_idx].clone();
        item = item.style(theme.selection_style().add_modifier(Modifier::BOLD));
        std::mem::swap(&mut items[highlight_idx], &mut item);
    }
    items
}

impl ResultsTableView {
    fn render_scrollbar(&self, frame: &mut Frame, area: Rect, total_rows: usize, visible_rows: usize, offset: usize, theme: &dyn UiTheme) {
        if total_rows <= visible_rows {
            return;
        }

        let viewport_height = scrollbar_viewport_height(visible_rows);
        let max_scroll_offset = total_rows.saturating_sub(viewport_height);
        render_vertical_scrollbar(
            frame,
            area,
            theme,
            max_scroll_offset,
            offset.min(max_scroll_offset),
            viewport_height,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{highlighted_row_index, scrollbar_viewport_height, value_display_width_exceeds_available};
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

    #[test]
    fn value_overflow_detection_uses_display_width() {
        assert!(!value_display_width_exceeds_available("wide", 8));
        assert!(value_display_width_exceeds_available("123456789", 8));
    }
}
