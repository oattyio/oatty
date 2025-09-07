//! Results table modal component for displaying JSON data.
//!
//! This module provides a component for rendering the table modal, which
//! displays JSON results from command execution in a tabular format with
//! scrolling and navigation capabilities.
use crossterm::event::{KeyCode, KeyEvent};
use heroku_types::Pagination;
use heroku_util::format_date_mmddyyyy;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::*,
    text::{Line, Span},
    widgets::{Scrollbar, ScrollbarState, *},
};
use serde_json::Value;

use crate::{
    app,
    app::Effect,
    ui::{
        components::{PaginationComponent, component::Component, table::TableFooter},
        theme::{helpers as th, roles::Theme as UiTheme},
        utils::{centered_rect, get_scored_keys, normalize_header, render_value},
    },
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
/// ```rust,ignore
/// use heroku_tui::ui::components::TableComponent;
///
/// let mut table = TableComponent::new();
/// table.init()?;
/// ```
#[derive(Default)]
pub struct TableComponent<'a> {
    table: Table<'a>,
    table_state: TableState,
    scrollbar: Scrollbar<'a>,
    scrollbar_state: ScrollbarState,
    footer: TableFooter<'a>,
    pagination: PaginationComponent,
}

impl TableComponent<'_> {
    /// Sets the available range fields for pagination
    pub fn set_pagination(&mut self, pagination: Pagination) {
        self.pagination.set_pagination(pagination);
    }

    /// Shows the pagination controls
    pub fn show_pagination(&mut self) {
        self.pagination.show();
    }

    /// Hides the pagination controls
    pub fn hide_pagination(&mut self) {
        self.pagination.hide();
    }

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
            // Ensure table fills with background surface and text color
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
    /// Renders the table modal with JSON results.
    ///
    /// This method handles the layout, styling, and table generation for the
    /// results display.
    ///
    /// # Arguments
    ///
    /// * `f` - The frame to render to
    /// * `rect` - The rectangular area to render in
    /// * `app` - The application state containing result data
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut app::App) {
        // Set up pagination if the command has range support
        if let Some(pagination) = app.last_pagination.clone() {
            self.set_pagination(pagination);
            self.show_pagination();
        } else {
            self.hide_pagination();
        }
        // Large modal to maximize space for tables
        let area = centered_rect(96, 90, rect);
        let title = "Results  [Esc] Close  ↑/↓ Scroll";
        let block = th::block(&*app.ctx.theme, Some(title), true);

        frame.render_widget(Clear, area);
        frame.render_widget(&block, area);
        let inner = block.inner(area);
        // Split for content + pagination + footer
        let splits = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),                                                         // Table content
                Constraint::Length(if self.pagination.state().is_visible { 7 } else { 0 }), // Pagination controls
                Constraint::Length(1),                                                      // Footer
            ])
            .split(inner);

        app.table.set_visible_rows(splits[0].height as usize);
        let json = app.table.selected_result_json();
        let widths = app.table.column_constraints();
        let headers = app.table.headers();
        let maybe_rows = app.table.rows();
        let is_table = if let Some(json) = json {
            if let Some(rows) = maybe_rows {
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
                true
            } else {
                self.render_kv_or_text(frame, splits[0], json, &*app.ctx.theme);
                false
            }
        } else {
            let p = Paragraph::new("No results to display").style(app.ctx.theme.text_muted_style());
            frame.render_widget(p, splits[0]);
            false
        };

        if is_table {
            // Render pagination controls
            self.pagination.render(frame, splits[1], app);
            self.footer.render(frame, splits[2], app);
        }
    }

    /// Handle key events for the results table modal.
    ///
    /// Applies local state updates directly to `app.table` for scrolling and
    /// navigation. Returns `Ok(true)` if the key was handled by the table,
    /// otherwise `Ok(false)`.
    fn handle_key_events(&mut self, app: &mut app::App, key: KeyEvent) -> Vec<Effect> {
        let mut effects: Vec<Effect> = vec![];

        // Keep pagination focus in sync with enabled state
        self.pagination.normalize_focus();

        // Delegate to pagination when pagination subcontrols are focused
        let p = self.pagination.state();
        let focus_on_grid = app.table.grid_f.get();
        let focus_on_pagination = p.nav_first_f.get() || p.nav_prev_f.get() || p.nav_next_f.get() || p.nav_last_f.get();
        // Let table handle Tab/BackTab to cycle grid <-> pagination; otherwise delegate
        if !focus_on_grid && focus_on_pagination && key.code != KeyCode::Tab && key.code != KeyCode::BackTab {
            effects.extend(self.pagination.handle_key_events(app, key));
            return effects;
        }

        match key.code {
            KeyCode::Tab | KeyCode::BackTab => {
                // Cycle grid + pagination subcontrols
                let p = self.pagination.state();
                let mut b = rat_focus::FocusBuilder::new(None);
                b.widget(&PanelLeaf(app.table.grid_f.clone()));
                // Only include enabled nav buttons
                if p.has_prev_page() {
                    b.widget(&PanelLeaf(p.nav_first_f.clone()));
                    b.widget(&PanelLeaf(p.nav_prev_f.clone()));
                }
                if p.has_next_page() {
                    b.widget(&PanelLeaf(p.nav_next_f.clone()));
                    b.widget(&PanelLeaf(p.nav_last_f.clone()));
                }
                let f = b.build();
                if key.code == KeyCode::Tab {
                    let _ = f.next();
                } else {
                    let _ = f.prev();
                }
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
            // Toggle handled via App message; keep consistent with global actions
            KeyCode::Char('t') => {
                let _ = app.update(app::Msg::ToggleTable);
            }
            KeyCode::Char('c') => {
                effects.extend(app.update(app::Msg::CopyCommand));
            }
            _ => {}
        }
        effects
    }
}

// Local leaf wrapper used for table grid and pagination focus items
struct PanelLeaf(FocusFlag);
impl HasFocus for PanelLeaf {
    fn build(&self, builder: &mut FocusBuilder) {
        builder.leaf_widget(self);
    }
    fn focus(&self) -> FocusFlag {
        self.0.clone()
    }
    fn area(&self) -> ratatui::layout::Rect {
        ratatui::layout::Rect::default()
    }
}
