//! Results table modal component for displaying JSON data.
//!
//! This module provides a component for rendering the table modal, which displays
//! JSON results from command execution in a tabular format with scrolling and
//! navigation capabilities.
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::*,
    text::{Line, Span},
    widgets::*,
};
use serde_json::Value;
use heck::ToTitleCase;
use chrono::{DateTime, NaiveDate, Datelike};

use crate::ui::theme::helpers as th;
use crate::ui::theme::roles::Theme as UiTheme;
use crate::ui::utils::{get_scored_keys, infer_columns_from_json};
use crate::{
    app,
    ui::{components::component::Component, utils::centered_rect},
};

// Generated at build-time from schemas/heroku-schema.json
mod generated_date_fields {
    include!(concat!(env!("OUT_DIR"), "/date_fields.rs"));
}

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
/// ```rust,ignore
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

    /// Handle key events for the results table modal.
    ///
    /// Applies local state updates directly to `app.table` for scrolling and navigation.
    /// Returns `Ok(true)` if the key was handled by the table, otherwise `Ok(false)`.
    pub fn handle_key(&self, app: &mut app::App, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Up => {
                app.table.reduce_scroll(-1);
                Ok(true)
            }
            KeyCode::Down => {
                app.table.reduce_scroll(1);
                Ok(true)
            }
            KeyCode::PageUp => {
                app.table.reduce_scroll(-10);
                Ok(true)
            }
            KeyCode::PageDown => {
                app.table.reduce_scroll(10);
                Ok(true)
            }
            KeyCode::Home => {
                app.table.reduce_home();
                Ok(true)
            }
            KeyCode::End => {
                app.table.reduce_end();
                Ok(true)
            }
            // Toggle handled via App message; keep consistent with global actions
            KeyCode::Char('t') => {
                let _ = app.update(app::Msg::ToggleTable);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Renders a JSON array as a table with offset support using known columns.
    pub fn render_json_table_with_columns(
        &self,
        frame: &mut Frame,
        area: Rect,
        json: &Value,
        offset: usize,
        columns: &[String],
        theme: &dyn UiTheme,
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
            let p = Paragraph::new("No tabular data in JSON").style(theme.text_muted_style());
            frame.render_widget(p, area);
            return;
        }
        let arr = arr.unwrap();
        if arr.is_empty() {
            let p = Paragraph::new("No rows").style(theme.text_muted_style());
            frame.render_widget(p, area);
            return;
        }

        let headers: Vec<_> = columns
            .iter()
            .map(|header| Cell::from(self.normalize_header(header)).style(th::table_header_style(theme)))
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
            for (idx, item) in arr[start..end].iter().enumerate() {
                let mut cells: Vec<Cell> = Vec::with_capacity(columns.len());
                for key in columns.iter() {
                    let val = item.get(key).unwrap_or(&Value::Null);
                    let txt = self.render_value(key, val);
                    let mut style = theme.text_primary_style();
                    if self.is_status_like(key) {
                        if let Some(color) = self.status_color_for_value(&txt, theme) {
                            style = Style::default().fg(color);
                        }
                    }
                    cells.push(Cell::from(txt).style(style));
                }
                // Alternating row backgrounds using theme helper (no dim modifier).
                let absolute_index = start + idx;
                let row_style = th::table_row_style(theme, absolute_index);
                rows_slice.push(Row::new(cells).style(row_style));
            }
        }

        let table = Table::new(rows_slice, widths)
            .header(Row::new(headers))
            .block(th::block(theme, None, false))
            .column_spacing(1)
            .row_highlight_style(th::table_selected_style(theme))
            // Ensure table fills with background main and body text color
            .style(Style::default().fg(theme.roles().text));

        frame.render_widget(table, area);

        // Scrollbar indicating vertical position within table rows
        if total_rows > 0 {
            let mut sb_state = ScrollbarState::new(total_rows).position(start);
            let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .thumb_style(Style::default().fg(theme.roles().scrollbar_thumb))
                .track_style(Style::default().fg(theme.roles().scrollbar_track));
            frame.render_stateful_widget(sb, area, &mut sb_state);
        }
    }

    /// Renders a JSON array as a table with offset support.
    pub fn render_json_table(
        &self,
        frame: &mut Frame,
        area: Rect,
        json: &Value,
        offset: usize,
        theme: &dyn UiTheme,
    ) {
        let cols = infer_columns_from_json(json);
        self.render_json_table_with_columns(frame, area, json, offset, &cols, theme);
    }

    /// Renders JSON as key-value pairs or plain text.
    pub fn render_kv_or_text(&self, frame: &mut Frame, area: Rect, json: &Value, theme: &dyn UiTheme) {
        match json {
            Value::Object(map) => {
                // Sort keys using the same scoring
                let keys: Vec<String> = get_scored_keys(map);
                let mut lines: Vec<Line> = Vec::new();
                for header in keys.iter().take(24) {
                    let val = self.render_value(header, map.get(header).unwrap_or(&Value::Null));
                    lines.push(Line::from(vec![
                        Span::styled(self.normalize_header(header), theme.text_secondary_style().add_modifier(Modifier::BOLD)),
                        Span::raw(": "),
                        Span::styled(val, theme.text_primary_style()),
                    ]));
                }
                let p = Paragraph::new(Text::from(lines))
                    .block(th::block(theme, Some("Details"), false))
                    .wrap(Wrap { trim: false })
                    .style(theme.text_primary_style());
                frame.render_widget(p, area);
            }
            other => {
                let s = match other {
                    Value::String(s) => self.format_date_mmddyyyy(s).unwrap_or_else(|| s.clone()),
                    _ => other.to_string(),
                };
                let p = Paragraph::new(s)
                    .block(th::block(theme, Some("Result"), false))
                    .wrap(Wrap { trim: false })
                    .style(theme.text_primary_style());
                frame.render_widget(p, area);
            }
        }
    }

    fn normalize_header(&self, key: &str) -> String {
        key.replace('_', " ").to_string().to_title_case()
    }

    fn is_status_like(&self, key: &str) -> bool {
        matches!(key.to_ascii_lowercase().as_str(), "status" | "state")
    }

    fn status_color_for_value(&self, value: &str, theme: &dyn UiTheme) -> Option<ratatui::style::Color> {
        let v = value.to_ascii_lowercase();
        if matches!(v.as_str(), "ok" | "succeeded" | "success" | "passed") {
            Some(theme.roles().success)
        } else if matches!(v.as_str(), "error" | "failed" | "fail") {
            Some(theme.roles().error)
        } else {
            None
        }
    }

    fn render_value(&self, key: &str, value: &Value) -> String {
        match value {
            Value::String(s) => {
                if self.is_sensitive_key(key) {
                    self.ellipsize_middle_if_sha_like(s, 12)
                } else if self.is_date_like_key(key) {
                    self.format_date_mmddyyyy(s).unwrap_or_else(|| s.clone())
                } else {
                    s.clone()
                }
            }
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            // Take the highest scoring key from the object as a string
            Value::Object(map) => {
                if let Some(key) = get_scored_keys(map).first() {
                    let value = map.get(key).unwrap();
                    if let Some(s) = value.as_str() {
                        s.to_string()
                    } else {
                        value.to_string()
                    }
                } else {
                    value.to_string()
                }
            }
            _ => value.to_string(),
        }
    }

    fn is_sensitive_key(&self, key: &str) -> bool {
        matches!(key, "token" | "key" | "secret" | "password" | "api_key" | "auth_token")
    }

    fn is_date_like_key(&self, key: &str) -> bool {
        let k = key.to_ascii_lowercase().replace([' ', '-'], "_");
        // Prefer schema-derived keys; fall back to heuristics
        if generated_date_fields::DATE_FIELD_KEYS.contains(&k.as_str()) {
            return true;
        }
        k.ends_with("_at") || k.ends_with("_on") || k.ends_with("_date")
            || k == "created" || k == "updated" || k == "released"
    }

    fn format_date_mmddyyyy(&self, s: &str) -> Option<String> {
        // Try RFC3339/ISO8601 with timezone (e.g., 2024-01-01T12:00:00Z)
        if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
            let d = dt.date_naive();
            return Some(format!("{:02}/{:02}/{}", d.month(), d.day(), d.year()));
        }
        // Try common date-only forms
        if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
            return Some(format!("{:02}/{:02}/{}", d.month(), d.day(), d.year()));
        }
        if let Ok(d) = NaiveDate::parse_from_str(s, "%Y/%m/%d") {
            return Some(format!("{:02}/{:02}/{}", d.month(), d.day(), d.year()));
        }
        None
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
        let block = th::block(&*app.ctx.theme, Some(title), true);

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
                self.render_json_table_with_columns(frame, splits[0], json, app.table.count_offset(), &cols, &*app.ctx.theme);
            } else {
                self.render_kv_or_text(frame, splits[0], json, &*app.ctx.theme);
            }
        } else {
            let p = Paragraph::new("No results to display").style(app.ctx.theme.text_muted_style());
            frame.render_widget(p, splits[0]);
        }

        // Footer hint for table modal
        let footer = Paragraph::new(Line::from(vec![
            Span::styled("Hint: ", app.ctx.theme.text_muted_style()),
            Span::styled("Esc", app.ctx.theme.accent_emphasis_style()),
            Span::styled(" close  ", app.ctx.theme.text_muted_style()),
            Span::styled("↑/↓", app.ctx.theme.accent_emphasis_style()),
            Span::styled(" scroll  ", app.ctx.theme.text_muted_style()),
            Span::styled("PgUp/PgDn", app.ctx.theme.accent_emphasis_style()),
            Span::styled(" faster  ", app.ctx.theme.text_muted_style()),
            Span::styled("Home/End", app.ctx.theme.accent_emphasis_style()),
            Span::styled(" jump", app.ctx.theme.text_muted_style()),
        ]))
        .style(app.ctx.theme.text_muted_style());
        frame.render_widget(footer, splits[1]);
    }
}
