//! Shared results table view utilities.
//!
//! This module contains reusable rendering helpers for the results table
//! experience. The `ResultsTableView` encapsulates the TUI widgets required to
//! render tabular data, key-value fallback views, and scrolling chrome while
//! leaving ownership of the domain state with the caller.

use heroku_util::format_date_mmddyyyy;
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, List, ListItem, ListState, Paragraph, Row, Scrollbar, ScrollbarState, Table, Wrap},
};
use serde_json::Value;

use crate::ui::{
    components::table::state::{KeyValueEntry, TableState},
    theme::{roles::Theme as UiTheme, theme_helpers as th},
};

/// Stateful ratatui widgets used to render results tables.
#[derive(Debug, Default)]
pub struct ResultsTableView<'a> {
    table_widget: Table<'a>,
    widget_state: ratatui::widgets::TableState,
    scrollbar_widget: Scrollbar<'a>,
    scrollbar_state: ScrollbarState,
}

impl<'a> ResultsTableView<'a> {
    /// Renders the primary results region.
    ///
    /// When the provided table state contains tabular rows, this renders the
    /// table view along with the scrollbar. When there are no rows, the method
    /// falls back to rendering key-value entries or a simple paragraph
    /// representation of the JSON payload.
    ///
    /// Returns `true` when a tabular view was rendered which allows controllers
    /// to decide whether supporting UI (such as pagination) should be shown.
    pub fn render_results(&mut self, frame: &mut Frame, area: Rect, state: &TableState<'_>, focused: bool, theme: &dyn UiTheme) -> bool {
        let Some(json) = state.selected_result_json() else {
            let placeholder = Paragraph::new("No results to display").style(theme.text_muted_style());
            frame.render_widget(placeholder, area);
            return false;
        };

        if state.rows().map(|rows| !rows.is_empty()).unwrap_or_default() {
            self.render_json_table(frame, area, state, focused, theme);
            return true;
        }

        Self::render_kv_or_text(
            frame,
            area,
            state.kv_entries(),
            select_entry_index(state),
            kv_offset(state),
            json,
            theme,
        );
        false
    }

    /// Renders a JSON array as a table with pagination-aware selection.
    fn render_json_table(&mut self, frame: &mut Frame, area: Rect, state: &TableState<'_>, focused: bool, theme: &dyn UiTheme) {
        let rows = state.rows().unwrap();
        let widths: &[Constraint] = state.column_constraints().map_or(&[][..], |constraints| constraints.as_slice());
        let headers: &[Cell<'_>] = state.headers().map_or(&[][..], |header_cells| header_cells.as_slice());
        let offset = state.count_offset();
        let selected = state.selected_index();

        let visible_rows = area.height.saturating_sub(1) as usize;
        if visible_rows == 0 || rows.is_empty() {
            let placeholder = Paragraph::new("No results to display").style(theme.text_muted_style());
            frame.render_widget(placeholder, area);
            return;
        }

        let max_start = rows.len().saturating_sub(visible_rows.max(1));
        let start = offset.min(max_start);
        let end = (start + visible_rows).min(rows.len());

        let table = self
            .table_widget
            .clone()
            .rows(rows[start..end].to_owned())
            .widths(widths)
            .header(Row::new(headers.to_owned()).style(th::table_header_row_style(theme)))
            .column_spacing(1)
            .row_highlight_style(if focused {
                th::table_selected_style(theme)
            } else {
                Style::default()
            })
            .style(th::panel_style(theme));

        let selected_in_view = selected.saturating_sub(start);
        self.widget_state.select(Some(selected_in_view));
        frame.render_stateful_widget(table, area, &mut self.widget_state);

        let mut scrollbar_state = self.scrollbar_state.content_length(max_start).position(start);
        let scrollbar = self
            .scrollbar_widget
            .clone()
            .thumb_style(Style::default().fg(theme.roles().scrollbar_thumb))
            .track_style(Style::default().fg(theme.roles().scrollbar_track));
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
        self.scrollbar_state = scrollbar_state;
    }

    /// Renders JSON payloads as key-value entries or plain text.
    pub fn render_kv_or_text(
        frame: &mut Frame,
        area: Rect,
        entries: &[KeyValueEntry],
        selection: Option<usize>,
        offset: usize,
        json: &Value,
        theme: &dyn UiTheme,
    ) {
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

                let mut list_state = ListState::default();
                if let Some(selected_index) = selection {
                    list_state.select(Some(selected_index.min(entries.len().saturating_sub(1))));
                }
                if !entries.is_empty() {
                    let capped_offset = offset.min(entries.len().saturating_sub(1));
                    *list_state.offset_mut() = capped_offset;
                }

                let list = List::new(items)
                    .highlight_style(th::table_selected_style(theme))
                    .style(th::panel_style(theme));

                frame.render_stateful_widget(list, area, &mut list_state);
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

fn select_entry_index(state: &TableState<'_>) -> Option<usize> {
    let entries = state.kv_entries();
    if entries.is_empty() {
        None
    } else {
        Some(state.selected_index().min(entries.len().saturating_sub(1)))
    }
}

fn kv_offset(state: &TableState<'_>) -> usize {
    let entries = state.kv_entries();
    if entries.is_empty() {
        0
    } else {
        state.count_offset().min(entries.len().saturating_sub(1))
    }
}
