//! Shared results table view utilities.
//!
//! This module contains reusable rendering helpers for the result table
//! experience. The `ResultsTableView` encapsulates the TUI widgets required to
//! render tabular data, key-value fallback views, and scrolling chrome while
//! leaving ownership of the domain state with the caller.

use heroku_util::format_date_mmddyyyy;
use ratatui::widgets::TableState;
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, List, ListItem, ListState, Paragraph, Row, Scrollbar, ScrollbarState, Table, Wrap},
};
use serde_json::Value;

use crate::ui::{
    components::table::state::{KeyValueEntry, ResultsTableState},
    theme::{roles::Theme as UiTheme, theme_helpers as th},
};

/// Stateful ratatui widgets used to render results tables.
#[derive(Debug, Default)]
pub struct ResultsTableView {
    pub table_state: TableState,
    pub list_state: ListState,
}

impl ResultsTableView {
    /// Renders the primary results' region.
    ///
    /// When the provided table state contains tabular rows, this renders the
    /// table view along with the scrollbar. When there are no rows, the method
    /// falls back to rendering key-value entries or a simple paragraph
    /// representation of the JSON payload.
    ///
    /// Returns `true` when a tabular view was rendered which allows controllers
    /// to decide whether supporting UI (such as pagination) should be shown.
    pub fn render_results(&mut self, frame: &mut Frame, area: Rect, state: &ResultsTableState, focused: bool, theme: &dyn UiTheme) -> bool {
        let Some(json) = state.selected_result_json() else {
            let placeholder = Paragraph::new("No results to display").style(theme.text_muted_style());
            frame.render_widget(placeholder, area);
            return false;
        };

        if state.rows().map(|rows| !rows.is_empty()).unwrap_or_default() {
            self.render_json_table(frame, area, state, focused, theme);
            return true;
        }

        self.render_kv_or_text(frame, area, state.kv_entries(), json, theme);
        false
    }

    /// Renders a JSON array as a table with pagination-aware selection.
    fn render_json_table(&mut self, frame: &mut Frame, area: Rect, state: &ResultsTableState<'_>, focused: bool, theme: &dyn UiTheme) {
        let mut rows = state.rows().unwrap().iter().cloned().collect::<Vec<_>>();
        let should_highlight_row = state.mouse_over_idx.is_some();
        let highlight_idx = state.mouse_over_idx.unwrap_or(0);
        let rows_len = rows.len();
        if should_highlight_row {
            let mut row = rows[highlight_idx].clone();
            // Highlight the row if the mouse is over it.
            row = row.style(theme.selection_style().add_modifier(Modifier::BOLD));
            std::mem::swap(&mut rows[highlight_idx], &mut row);
        }

        let widths: &[Constraint] = state.column_constraints().map_or(&[][..], |constraints| constraints.as_slice());
        let headers: &[Cell<'_>] = state.headers().map_or(&[][..], |header_cells| header_cells.as_slice());

        let offset = self.table_state.offset();
        let visible_rows = area.height.saturating_sub(1) as usize;
        if visible_rows == 0 || rows.is_empty() {
            let placeholder = Paragraph::new("No results to display").style(theme.text_muted_style());
            frame.render_widget(placeholder, area);
            return;
        }

        let table_widget = Table::new(rows, widths)
            .header(Row::new(headers.to_owned()).style(th::table_header_row_style(theme)))
            .column_spacing(1)
            .row_highlight_style(if focused {
                th::table_selected_style(theme)
            } else {
                Style::default()
            })
            .style(th::panel_style(theme));

        frame.render_stateful_widget(&table_widget, area, &mut self.table_state);

        let max_start = rows_len.saturating_sub(visible_rows.max(1));
        let start = offset.min(max_start);
        let mut scrollbar_state = ScrollbarState::new(max_start).position(start);
        let scrollbar = Scrollbar::default()
            .thumb_style(Style::default().fg(theme.roles().scrollbar_thumb))
            .track_style(Style::default().fg(theme.roles().scrollbar_track));
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }

    /// Renders JSON payloads as key-value entries or plain text.
    pub fn render_kv_or_text(&mut self, frame: &mut Frame, area: Rect, entries: &[KeyValueEntry], json: &Value, theme: &dyn UiTheme) {
        match json {
            Value::Object(_) => {
                let items: Vec<ListItem> = entries
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

                frame.render_stateful_widget(list, area, &mut self.list_state);
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
