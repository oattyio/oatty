//! Results table modal component for displaying JSON data.
//!
//! This module provides a component for rendering the table modal, which displays
//! JSON results from command execution in a tabular format with scrolling and
//! navigation capabilities.
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::{Scrollbar, ScrollbarState};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::*,
    text::{Line, Span},
    widgets::*,
};
use serde_json::Value;

use crate::app::Effect;
use crate::ui::components::table::TableFooter;
use crate::ui::theme::helpers as th;
use crate::ui::theme::roles::Theme as UiTheme;
use crate::ui::utils::{get_scored_keys, normalize_header, render_value};
use crate::{
    app,
    ui::{components::component::Component, utils::centered_rect},
};
use heroku_util::format_date_mmddyyyy;

// Date field keys and formatting are provided by heroku-util.

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
pub struct TableComponent<'a> {
    table: Table<'a>,
    table_state: TableState,
    scrollbar: Scrollbar<'a>,
    scrollbar_state: ScrollbarState,
    footer: TableFooter<'a>,
}

impl Default for TableComponent<'_> {
    fn default() -> Self {
        TableComponent { 
            table: Table::default(), 
            table_state: TableState::default(), 
            scrollbar: Scrollbar::default(), 
            scrollbar_state: ScrollbarState::default(), 
            footer: TableFooter::default() }
    }
}

impl<'a> TableComponent<'_> {
    /// Renders a JSON array as a table with offset support using known columns.
    pub fn render_json_table_with_columns(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        offset: usize,
        selected: usize,
        rows: &[Row],
        widths: &[Constraint],
        headers: &Vec<Cell>,
        theme: &dyn UiTheme,
    ) {
        if rows.is_empty() {
            let p = Paragraph::new("No results to display").style(theme.text_muted_style());
            frame.render_widget(p, area);
            return;
        }
        // Compute visible rows
        let visible = area.height as usize;
        let max_start = rows.len().saturating_sub(visible.max(1));
        let start = offset.min(max_start);
        // Render only the visible window of rows
        let end = (start + visible).min(rows.len());
        let table = self
            .table
            .clone()
            .rows(rows[start..end].iter().cloned())
            .widths(widths)
            .header(Row::new(headers.clone()).style(th::table_header_row_style(theme)))
            .block(th::block(theme, None, false))
            .column_spacing(1)
            .row_highlight_style(th::table_selected_style(theme))
            // Ensure table fills with background surface and text color
            .style(th::panel_style(theme));

        // Highlight the selected row relative to the visible window
        let sel = selected.saturating_sub(start);
        self.table_state.select(Some(sel));
        frame.render_stateful_widget(table, area, &mut self.table_state);

        // Scrollbar indicating vertical position within table rows
        if rows.len() > 0 {
            let mut sb_state = self
                .scrollbar_state
                .content_length(max_start)
                .position(start);
            let scrollbar = self
                .scrollbar
                .clone()
                .thumb_style(Style::default().fg(theme.roles().scrollbar_thumb))
                .track_style(Style::default().fg(theme.roles().scrollbar_track));
            frame.render_stateful_widget(scrollbar, area, &mut sb_state);
            self.scrollbar_state = sb_state;
        }
    }

    /// Renders JSON as key-value pairs or plain text.
    pub fn render_kv_or_text(&self, frame: &mut Frame, area: Rect, json: &Value, theme: &dyn UiTheme) {
        match json {
            Value::Object(map) => {
                // Sort keys using the same scoring
                let keys: Vec<String> = get_scored_keys(map);
                let mut lines: Vec<Line> = Vec::new();
                for header in keys.iter().take(24) {
                    let val = render_value(header, map.get(header).unwrap_or(&Value::Null));
                    lines.push(Line::from(vec![
                        Span::styled(
                            normalize_header(header),
                            theme.text_secondary_style().add_modifier(Modifier::BOLD),
                        ),
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
                    Value::String(s) => format_date_mmddyyyy(s).unwrap_or_else(|| s.clone()),
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
}

impl Component for TableComponent<'_> {
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

        app.table.set_visible_rows(splits[0].height as usize);
        let json = app.table.selected_result_json();
        let widths = app.table.column_constraints();
        let headers = app.table.headers();
        let maybe_rows = app.table.rows();
        if let Some(json) = json {
            if let Some(rows) = maybe_rows {                
                self.render_json_table_with_columns(
                    frame,
                    splits[0],
                    app.table.count_offset(),
                    app.table.selected_index(),
                    &rows,
                    widths.unwrap(),
                    headers.unwrap(),
                    &*app.ctx.theme,
                );
            } else {
                self.render_kv_or_text(frame, splits[0], json, &*app.ctx.theme);
            }
        } else {
            let p = Paragraph::new("No results to display").style(app.ctx.theme.text_muted_style());
            frame.render_widget(p, splits[0]);
        }
        self.footer.render(frame, splits[1], app);
    }

    /// Handle key events for the results table modal.
    ///
    /// Applies local state updates directly to `app.table` for scrolling and navigation.
    /// Returns `Ok(true)` if the key was handled by the table, otherwise `Ok(false)`.
    fn handle_key_events(&mut self, app: &mut app::App, key: KeyEvent) -> Vec<Effect> {
        let effects: Vec<Effect> = vec![];
        match key.code {
            KeyCode::Up => {
                app.table.reduce_scroll(-1);
            }
            KeyCode::Down => {
                app.table.reduce_scroll(1);
            }
            KeyCode::PageUp => {
                let step = app
                    .table
                    .visible_rows()
                    .saturating_sub(1);
                let step = if step == 0 { 10 } else { step } as isize;
                app.table.reduce_scroll(-step);
            }
            KeyCode::PageDown => {
                let step = app
                    .table
                    .visible_rows()
                    .saturating_sub(1);
                let step = if step == 0 { 10 } else { step } as isize;
                app.table.reduce_scroll(step);
            }
            KeyCode::Home => {
                app.table.reduce_home();
            }
            KeyCode::End => {
                app.table.reduce_end();
            }
            // Toggle handled via App message; keep consistent with global actions
            KeyCode::Char('t') => {
                let _ = app.update(app::Msg::ToggleTable);
            }
            _ => {}
        }
        effects
    }
}
