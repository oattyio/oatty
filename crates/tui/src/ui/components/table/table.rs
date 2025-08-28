//! Results table modal component for displaying JSON data.
//!
//! This module provides a component for rendering the table modal, which displays
//! JSON results from command execution in a tabular format with scrolling and
//! navigation capabilities.
use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::*,
    text::{Line, Span},
    widgets::*,
};
use serde_json::{Value};

use crate::ui::utils::{get_scored_keys, infer_columns_from_json};
use crate::{
    app, theme,
    ui::{components::component::Component, utils::centered_rect},
};

/// Results table modal component for displaying JSON data.
///
/// This component renders a modal dialog containing tabular data from command
/// execution results. It automatically detects JSON arrays and displays them
/// in a scrollable table format with proper column detection and formatting.
///
/// # Features
///
/// - Automatically detects and displays JSON arrays as tables
/// - Provides scrollable navigation through large datasets
/// - Handles column detection and formatting
/// - Supports keyboard navigation (arrow keys, page up/down, home/end)
/// - Falls back to key-value display for non-array JSON
///
/// # Usage
///
/// The table component is typically activated when a command returns JSON
/// results containing arrays. It provides an optimal viewing experience for
/// tabular data like lists of apps, dynos, or other resources.
///
/// # Navigation
///
/// - **Arrow keys**: Scroll up/down through rows
/// - **Page Up/Down**: Scroll faster through the table
/// - **Home/End**: Jump to beginning/end of the table
/// - **Escape**: Close the table modal
///
/// # Examples
///
/// ```rust
/// use heroku_tui::ui::components::TableComponent;
///
/// let mut table = TableComponent::new();
/// table.init()?;
/// ```
#[derive(Default)]
pub struct TableComponent;

impl TableComponent {
    /// Creates a new table component instance.
    ///
    /// # Returns
    ///
    /// A new TableComponent with default state
    pub fn new() -> Self {
        Self
    }

    /// Renders a JSON array as a table with offset support using known columns.
    pub fn render_json_table_with_columns(
        &self,
        frame: &mut Frame,
        area: Rect,
        json: &Value,
        offset: usize,
        columns: &[String],
    ) {
        // Find the array to render: either the value itself, or the first array field of an object
        let arr = match json {
            Value::Array(a) => Some(a.as_slice()),
            Value::Object(m) => m.values().find_map(|v| match v {
                Value::Array(a) => Some(a.as_slice()),
                _ => None,
            }),
            _ => None,
        };
        if arr.is_none() {
            let p = Paragraph::new("No tabular data in JSON").style(theme::text_muted());
            frame.render_widget(p, area);
            return;
        }
        let arr = arr.unwrap();
        if arr.is_empty() {
            let p = Paragraph::new("No rows").style(theme::text_muted());
            frame.render_widget(p, area);
            return;
        }

        let headers: Vec<_> = columns
            .iter()
            .map(|header| Cell::from(self.normalize_header(header)).style(theme::title_style()))
            .collect();

        // Column widths: split area width evenly with a floor
        let col_count = columns.len() as u16;
        let mut widths: Vec<Constraint> = Vec::new();
        if col_count == 0 {
            widths.push(Constraint::Percentage(100));
        } else {
            let per = (100 / col_count.max(1)).max(1);
            for _ in 0..col_count {
                widths.push(Constraint::Percentage(per));
            }
        }

        // Determine visible height to slice rows for scrolling (account for borders + header)
        let inner_height = area.height.saturating_sub(2); // block borders
        let header_rows = 1u16;
        let visible = inner_height.saturating_sub(header_rows).max(1) as usize;
        let total_rows = arr.len();
        let start = offset.min(total_rows.saturating_sub(1));
        let end = (start + visible).min(total_rows);
        let mut rows_slice: Vec<Row> = Vec::with_capacity(end.saturating_sub(start));
        if start < end {
            for item in &arr[start..end] {
                let mut cells: Vec<Cell> = Vec::with_capacity(columns.len());
                for key in columns.iter() {
                    let val = item.get(key).unwrap_or(&Value::Null);
                    let txt = self.render_value(key, val);
                    cells.push(Cell::from(txt).style(theme::text_style()));
                }
                rows_slice.push(Row::new(cells));
            }
        }

        let table = Table::new(rows_slice, widths)
            .header(Row::new(headers))
            .block(
                Block::default()
                    .title(Span::styled("Results", theme::title_style()))
                    .borders(Borders::ALL)
                    .border_style(theme::border_style(false)),
            )
            .column_spacing(1)
            .row_highlight_style(theme::list_highlight_style());

        frame.render_widget(table, area);

        // Scrollbar indicating vertical position within table rows
        if total_rows > 0 {
            let mut sb_state = ScrollbarState::new(total_rows).position(start);
            let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .thumb_style(theme::title_style().fg(theme::ACCENT))
                .track_style(theme::text_muted());
            frame.render_stateful_widget(sb, area, &mut sb_state);
        }
    }

    /// Renders a JSON array as a table with offset support.
    pub fn render_json_table(&self, frame: &mut Frame, area: Rect, json: &Value, offset: usize) {
        let cols = infer_columns_from_json(json);
        self.render_json_table_with_columns(frame, area, json, offset, &cols);
    }

    /// Renders JSON as key-value pairs or plain text.
    pub fn render_kv_or_text(&self, frame: &mut Frame, area: Rect, json: &Value) {
        match json {
            Value::Object(map) => {
                // Sort keys using the same scoring
                let keys: Vec<String> = get_scored_keys(map);
                let mut lines: Vec<Line> = Vec::new();
                for header in keys.iter().take(24) {
                    let val = self.render_value(header, map.get(header).unwrap_or(&Value::Null));
                    lines.push(Line::from(vec![
                        Span::styled(self.normalize_header(header), theme::title_style()),
                        Span::raw(": "),
                        Span::styled(val, theme::text_style()),
                    ]));
                }
                let p = Paragraph::new(Text::from(lines))
                    .block(
                        Block::default()
                            .title(Span::styled("Details", theme::title_style()))
                            .borders(Borders::ALL)
                            .border_style(theme::border_style(false))
                            .style(Style::default().bg(theme::BG_PANEL)),
                    )
                    .wrap(Wrap { trim: false })
                    .style(theme::text_style());
                frame.render_widget(p, area);
            }
            other => {
                let s = match other {
                    Value::String(s) => s.clone(),
                    _ => other.to_string(),
                };
                let p = Paragraph::new(s)
                    .block(
                        Block::default()
                            .title(Span::styled("Result", theme::title_style()))
                            .borders(Borders::ALL)
                            .border_style(theme::border_style(false))
                            .style(Style::default().bg(theme::BG_PANEL)),
                    )
                    .wrap(Wrap { trim: false })
                    .style(theme::text_style());
                frame.render_widget(p, area);
            }
        }
    }

    fn normalize_header(&self, key: &str) -> String {
        key.replace('_', " ").to_string()
    }

    fn render_value(&self, key: &str, value: &Value) -> String {
        match value {
            Value::String(s) => {
                if self.is_sensitive_key(key) {
                    self.ellipsize_middle_if_sha_like(s, 12)
                } else {
                    s.clone()
                }
            }
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            // Take the highest scoring key from the object as a string
            Value::Object(map) => {
                if let Some(key) = get_scored_keys(&map).get(0) {
                    let value = map.get(key).unwrap();
                    if let Some(s) = value.as_str() {
                        return s.to_string();
                    } else {
                        return value.to_string();
                    }
                }

                return value.to_string();
            }
            _ => value.to_string(),
        }
    }

    fn is_sensitive_key(&self, key: &str) -> bool {
        matches!(key, "token" | "key" | "secret" | "password" | "api_key" | "auth_token")
    }

    fn ellipsize_middle_if_sha_like(&self, s: &str, keep_total: usize) -> String {
        // Heuristic: hex-looking and long → compress
        let is_hexish = s.len() >= 16 && s.chars().all(|c| c.is_ascii_hexdigit());
        if !is_hexish || s.len() <= keep_total {
            return s.to_string();
        }
        let head = keep_total / 2;
        let tail = keep_total - head;
        format!("{}…{}", &s[..head], &s[s.len() - tail..])
    }
}

impl Component for TableComponent {
    /// Renders the table modal with JSON results.
    ///
    /// This method handles the layout, styling, and table generation for the results display.
    ///
    /// # Arguments
    ///
    /// * `f` - The frame to render to
    /// * `rect` - The rectangular area to render in
    /// * `app` - The application state containing result data
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut app::App) {
        // Large modal to maximize space for tables
        let area = centered_rect(96, 90, rect);
        let title = "Results  [Esc] Close  ↑/↓ Scroll";
        let block = Block::default()
            .title(Span::styled(title, theme::title_style().fg(theme::ACCENT)))
            .borders(Borders::ALL)
            .border_style(theme::border_style(true));

        frame.render_widget(Clear, area);
        frame.render_widget(&block, area);
        let inner = block.inner(area);
        // Split for content + footer
        let splits = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);

        let cols = app.table.cached_columns().cloned();
        let json = app.table.selected_result_json();
        if let Some(json) = json {
            if let Some(cols) = cols {
                self.render_json_table_with_columns(frame, splits[0], json, app.table.count_offset(), &cols);
            } else {
                self.render_kv_or_text(frame, splits[0], json);
            }
        } else {
            let p = Paragraph::new("No results to display").style(theme::text_muted());
            frame.render_widget(p, splits[0]);
        }

        // Footer hint for table modal
        let footer = Paragraph::new(Line::from(vec![
            Span::styled("Hint: ", theme::text_muted()),
            Span::styled("Esc", theme::title_style().fg(theme::ACCENT)),
            Span::styled(" close  ", theme::text_muted()),
            Span::styled("↑/↓", theme::title_style().fg(theme::ACCENT)),
            Span::styled(" scroll  ", theme::text_muted()),
            Span::styled("PgUp/PgDn", theme::title_style().fg(theme::ACCENT)),
            Span::styled(" faster  ", theme::text_muted()),
            Span::styled("Home/End", theme::title_style().fg(theme::ACCENT)),
            Span::styled(" jump", theme::text_muted()),
        ]))
        .style(theme::text_muted());
        frame.render_widget(footer, splits[1]);
    }
}
