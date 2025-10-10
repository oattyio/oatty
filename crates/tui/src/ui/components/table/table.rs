//! Results table modal component for displaying JSON data.
//!
//! This module provides a component for rendering the table modal, which
//! displays JSON results from command execution in a tabular format with
//! scrolling and navigation capabilities.
use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use heroku_types::{Effect, Msg};
use heroku_util::format_date_mmddyyyy;
use rat_focus::HasFocus;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::Modifier,
    text::{Line, Span},
    widgets::{Scrollbar, ScrollbarState, *},
};
use serde_json::Value;

use super::state::KeyValueEntry;
use crate::app::App;
use crate::ui::{
    components::{PaginationComponent, component::Component},
    theme::{roles::Theme as UiTheme, theme_helpers as th},
    utils::centered_rect,
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
/// - **Home/End**: Jump to the beginning /end of the table
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
#[derive(Debug, Default)]
pub struct TableComponent<'a> {
    table: Table<'a>,
    table_state: TableState,
    scrollbar: Scrollbar<'a>,
    scrollbar_state: ScrollbarState,
    pagination: PaginationComponent,
}

impl TableComponent<'_> {
    /// Renders a JSON array as a table with offset support using known columns.
    #[allow(clippy::too_many_arguments)]
    pub fn render_json_table_with_columns(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        offset: usize,
        selected: usize,
        rows: &[Row],
        widths: &[Constraint],
        headers: &[Cell],
        focused: bool,
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
            .rows(rows[start..end].to_owned())
            .widths(widths)
            .header(Row::new(headers.to_owned()).style(th::table_header_row_style(theme)))
            .block(
                th::block(theme, None, false)
                    .borders(Borders::ALL)
                    .border_style(theme.border_style(focused)),
            )
            .column_spacing(1)
            .row_highlight_style(th::table_selected_style(theme))
            // Ensure the table fills with background surface and text color
            .style(th::panel_style(theme));

        // Highlight the selected row relative to the visible window
        let sel = selected.saturating_sub(start);
        self.table_state.select(Some(sel));
        frame.render_stateful_widget(table, area, &mut self.table_state);

        // Scrollbar indicating vertical position within table rows
        if !rows.is_empty() {
            let mut sb_state = self.scrollbar_state.content_length(max_start).position(start);
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
    fn render_kv_detail(&self, frame: &mut Frame, area: Rect, json: &Value, app: &mut App) {
        let entries = app.table.kv_entries();
        let focused = app.table.grid_f.get();
        let selection = if entries.is_empty() {
            None
        } else {
            Some(app.table.selected_index().min(entries.len().saturating_sub(1)))
        };
        let offset = if entries.is_empty() {
            0
        } else {
            app.table.count_offset().min(entries.len().saturating_sub(1))
        };

        self.render_kv_or_text(frame, area, entries, selection, offset, focused, json, &*app.ctx.theme);
    }

    /// Renders JSON as key-value pairs or plain text.
    #[allow(clippy::too_many_arguments)]
    pub fn render_kv_or_text(
        &self,
        frame: &mut Frame,
        area: Rect,
        entries: &[KeyValueEntry],
        selection: Option<usize>,
        offset: usize,
        focused: bool,
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
                if let Some(selected) = selection {
                    list_state.select(Some(selected.min(entries.len().saturating_sub(1))));
                }
                if !entries.is_empty() {
                    let capped_offset = offset.min(entries.len().saturating_sub(1));
                    *list_state.offset_mut() = capped_offset;
                }

                let list = List::new(items)
                    .block(th::block(theme, Some("Details"), focused))
                    .highlight_style(th::table_selected_style(theme))
                    .style(th::panel_style(theme));

                frame.render_stateful_widget(list, area, &mut list_state);
            }
            other => {
                let date = match other {
                    Value::String(s) => format_date_mmddyyyy(s).unwrap_or_else(|| s.clone()),
                    _ => other.to_string(),
                };
                let paragraph = Paragraph::new(date)
                    .block(th::block(theme, Some("Result"), false))
                    .wrap(Wrap { trim: false })
                    .style(theme.text_primary_style());
                frame.render_widget(paragraph, area);
            }
        }
    }
}

impl Component for TableComponent<'_> {
    /// Handle key events for the result table modal.
    ///
    /// Applies local state updates directly to `app.table` for scrolling and
    /// navigation. Returns `Ok(true)` if the table handled the key,
    /// otherwise `Ok(false)`.
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let mut effects: Vec<Effect> = vec![];
        if key.code == KeyCode::Esc {
            return vec![Effect::CloseModal];
        }
        self.pagination.sync_navigation_state(app);
        // Delegate to pagination when pagination subcontrols are focused
        let pagination_state = &app.table.pagination_state;
        let focus_on_grid = app.table.grid_f.get();
        let focus_on_pagination = pagination_state.is_focused();
        // Let the table handle Tab/BackTab to cycle grid <-> pagination; otherwise delegate
        if !focus_on_grid && focus_on_pagination {
            effects.extend(self.pagination.handle_key_events(app, key));
            return effects;
        }

        match key.code {
            KeyCode::BackTab => {
                app.focus.prev();
            }
            KeyCode::Tab => {
                app.focus.next();
            }
            KeyCode::Up => {
                app.table.reduce_scroll(-1);
            }
            KeyCode::Down => {
                app.table.reduce_scroll(1);
            }
            KeyCode::PageUp => {
                let step = app.table.visible_rows().saturating_sub(1);
                let step = if step == 0 { 10 } else { step } as isize;
                app.table.reduce_scroll(-step);
            }
            KeyCode::PageDown => {
                let step = app.table.visible_rows().saturating_sub(1);
                let step = if step == 0 { 10 } else { step } as isize;
                app.table.reduce_scroll(step);
            }
            KeyCode::Home => {
                app.table.reduce_home();
            }
            KeyCode::End => {
                app.table.reduce_end();
            }
            KeyCode::Char('c') => {
                if let Some(value) = app.table.selected_data() {
                    let s = serde_json::to_string(value).ok().unwrap_or_default();
                    effects.extend(app.update(Msg::CopyToClipboard(s)));
                } else if let Some(entry) = app.table.selected_kv_entry() {
                    let serialized = serde_json::to_string(&entry.raw_value).unwrap_or_else(|_| entry.raw_value.to_string());
                    let payload = format!("{}: {}", entry.key, serialized);
                    effects.extend(app.update(Msg::CopyToClipboard(payload)));
                }
            }
            _ => {}
        }
        effects
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let mut effects: Vec<Effect> = vec![];
        effects.extend(self.pagination.handle_mouse_events(app, mouse));
        effects
    }

    /// Renders the table modal with JSON results.
    ///
    /// This method handles the layout, styling, and table generation for the
    /// result display.
    ///
    /// # Arguments
    ///
    /// * `f` - The frame to render to
    /// * `rect` - The rectangular area to render in
    /// * `app` - The application state containing result data
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        // Set up pagination if the command has range support
        if let Some(pagination) = app.last_pagination.clone() {
            app.table.pagination_state.set_pagination(pagination);
            app.table.pagination_state.show_pagination();
        } else {
            app.table.pagination_state.hide_pagination();
        }
        // Large modal to maximize space for tables
        let area = centered_rect(96, 90, rect);
        let title = "Results  [Esc] Close  ↑/↓ Scroll";
        let block = th::block(&*app.ctx.theme, Some(title), app.table.container_focus.get());

        frame.render_widget(Clear, area);
        frame.render_widget(&block, area);
        let inner = block.inner(area);
        // Split for content + pagination and footer
        let splits = self.get_preferred_layout(app, inner);

        app.table.set_visible_rows(splits[0].height as usize);
        let json = app.table.selected_result_json().cloned();
        let widths = app.table.column_constraints();
        let headers = app.table.headers();
        let maybe_rows = app.table.rows();
        let mut rendered_table = false;

        if let Some(json_value) = json.as_ref() {
            if let Some(rows) = maybe_rows {
                if !rows.is_empty() {
                    self.render_json_table_with_columns(
                        frame,
                        splits[0],
                        app.table.count_offset(),
                        app.table.selected_index(),
                        rows,
                        widths.unwrap(),
                        headers.unwrap(),
                        app.table.grid_f.get(),
                        &*app.ctx.theme,
                    );
                    rendered_table = true;
                } else {
                    self.render_kv_detail(frame, splits[0], json_value, app);
                }
            } else {
                self.render_kv_detail(frame, splits[0], json_value, app);
            }
        } else {
            let p = Paragraph::new("No results to display").style(app.ctx.theme.text_muted_style());
            frame.render_widget(p, splits[0]);
        }

        if rendered_table {
            self.pagination.render(frame, splits[1], app);
        }

        let hint_spans = self.get_hint_spans(app, true);
        let hint_line = if hint_spans.is_empty() {
            Line::default()
        } else {
            Line::from(hint_spans)
        };
        let hints_widget = Paragraph::new(hint_line).style(app.ctx.theme.text_muted_style());
        frame.render_widget(hints_widget, splits[2]);
    }

    fn get_hint_spans(&self, app: &App, is_root: bool) -> Vec<Span<'_>> {
        let has_rows = app.table.rows().map(|rows| !rows.is_empty()).unwrap_or(false);
        let has_kv = !app.table.kv_entries().is_empty();
        if !has_rows && !has_kv {
            return Vec::new();
        }

        let theme = &*app.ctx.theme;
        let mut spans: Vec<Span> = Vec::new();
        if is_root {
            spans.push(Span::styled("Hints: ", theme.text_muted_style()));
        }

        spans.extend([
            Span::styled("Esc", theme.accent_emphasis_style()),
            Span::styled(" close ", theme.text_muted_style()),
            Span::styled("c", theme.accent_emphasis_style()),
            Span::styled(" copy ", theme.text_muted_style()),
            Span::styled("↑/↓", theme.accent_emphasis_style()),
            Span::styled(" scroll  ", theme.text_muted_style()),
            Span::styled("PgUp/PgDn", theme.accent_emphasis_style()),
            Span::styled(" faster  ", theme.text_muted_style()),
            Span::styled("Home/End", theme.accent_emphasis_style()),
            Span::styled(" jump", theme.text_muted_style()),
        ]);

        if has_rows && app.table.pagination_state.is_visible() {
            spans.extend(self.pagination.get_hint_spans(app, false));
        }
        spans
    }

    fn get_preferred_layout(&self, app: &App, area: Rect) -> Vec<Rect> {
        Layout::vertical([
            Constraint::Min(1),                                                              // Table content
            Constraint::Length(if app.table.pagination_state.is_visible() { 7 } else { 0 }), // Pagination controls
            Constraint::Length(1),                                                           // Footer
        ])
        .split(area)
        .to_vec()
    }
}
