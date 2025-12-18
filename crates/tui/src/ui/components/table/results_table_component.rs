//! Results table modal component for displaying JSON data.
//!
//! This module provides a component for rendering the table modal, which
//! displays JSON results from command execution in a tabular format with
//! scrolling and navigation capabilities.
use crate::app::App;
use crate::ui::{
    components::{PaginationComponent, common::ResultsTableView, component::Component},
    theme::theme_helpers as th,
};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use oatty_types::{Effect, Msg};
use rat_focus::HasFocus;
use ratatui::layout::Position;
use ratatui::widgets::{Borders, Padding};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::Span,
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
pub struct TableComponent {
    view: ResultsTableView,
    pagination: PaginationComponent,
    table_area: Rect,
}

impl TableComponent {
    fn hit_test_table(&mut self, table_area: Rect, mouse_position: Position, list_offset: usize) -> Option<usize> {
        let idx = mouse_position.y.saturating_sub(table_area.y + 1) as usize + list_offset; // +1 for header
        if self.table_area.contains(mouse_position) {
            Some(idx)
        } else {
            None
        }
    }
}

impl Component for TableComponent {
    fn handle_message(&mut self, app: &mut App, msg: &Msg) -> Vec<Effect> {
        if let Msg::ExecCompleted(exec_outcome) = msg {
            app.table.process_general_execution_result(exec_outcome, &*app.ctx.theme);
        }
        self.pagination.handle_message(app, msg)
    }

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
        // Delegate to pagination when pagination subcontrols are focused
        let pagination_state = &app.table.pagination_state;
        let focus_on_grid = app.table.grid_f.get();
        let focus_on_pagination = pagination_state.is_focused();
        // Let the table handle Tab/BackTab to cycle grid <-> pagination; otherwise delegate
        if !focus_on_grid && focus_on_pagination {
            effects.extend(self.pagination.handle_key_events(app, key));
            return effects;
        }
        let table_state = &mut app.table.table_state;
        match key.code {
            KeyCode::BackTab => {
                app.focus.prev();
            }
            KeyCode::Tab => {
                app.focus.next();
            }
            KeyCode::Up => {
                table_state.scroll_up_by(1);
            }
            KeyCode::Down => {
                table_state.scroll_down_by(1);
            }
            KeyCode::PageUp => {
                table_state.scroll_up_by(10);
            }
            KeyCode::PageDown => {
                table_state.scroll_down_by(10);
            }
            KeyCode::Home => {
                table_state.scroll_up_by(u16::MAX);
            }
            KeyCode::End => {
                table_state.scroll_down_by(u16::MAX);
            }
            KeyCode::Char('c') => {
                if let Some(idx) = table_state.selected()
                    && let Some(value) = app.table.selected_data(idx)
                {
                    let s = serde_json::to_string(value).ok().unwrap_or_default();
                    effects.push(Effect::CopyToClipboardRequested(s));
                } else if let Some(idx) = app.table.list_state.selected()
                    && let Some(entry) = app.table.selected_kv_entry(idx)
                {
                    let serialized = serde_json::to_string(&entry.raw_value).unwrap_or_else(|_| entry.raw_value.to_string());
                    let payload = format!("{}: {}", entry.key, serialized);
                    effects.push(Effect::CopyToClipboardRequested(payload));
                }
            }
            _ => {}
        }
        effects
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let mut effects: Vec<Effect> = vec![];
        effects.extend(self.pagination.handle_mouse_events(app, mouse));
        let pos = Position {
            x: mouse.column,
            y: mouse.row,
        };
        if self.table_area.contains(pos) {
            let table_state = &mut app.table.table_state;
            match mouse.kind {
                MouseEventKind::ScrollUp => {
                    table_state.scroll_up_by(1);
                }
                MouseEventKind::ScrollDown => {
                    table_state.scroll_down_by(1);
                }
                _ => {}
            }
        }
        // Update the mouse over index when the mouse moves or is released
        if mouse.kind == MouseEventKind::Moved || mouse.kind == MouseEventKind::Up(MouseButton::Left) {
            app.table.mouse_over_idx = self.hit_test_table(self.table_area, pos, app.table.table_state.offset());
        }

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
        // Large modal to maximize space for tables
        let title = "Results  [Esc] Close  ↑/↓ Scroll";
        let block = th::block(&*app.ctx.theme, Some(title), app.table.container_focus.get());

        frame.render_widget(&block, rect);
        let inner = block.inner(rect);
        // Split for content + pagination and footer
        let splits = self.get_preferred_layout(app, inner);
        let is_grid_focused = app.table.grid_f.get();
        let table_block = th::block::<String>(&*app.ctx.theme, None, is_grid_focused)
            .borders(Borders::NONE)
            .padding(Padding::uniform(1));
        let table_inner = table_block.inner(splits[0]);
        frame.render_widget(table_block, splits[0]);
        let rendered_table = self
            .view
            .render_results(frame, table_inner, &mut app.table, is_grid_focused, &*app.ctx.theme);

        if rendered_table {
            self.pagination.render(frame, splits[1], app);
        }

        self.table_area = table_inner;
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let has_rows = app.table.rows().map(|rows| !rows.is_empty()).unwrap_or(false);
        let has_kv = !app.table.kv_entries().is_empty();
        if !has_rows && !has_kv {
            return Vec::new();
        }

        let theme = &*app.ctx.theme;
        let mut spans = th::build_hint_spans(
            theme,
            &[
                ("Esc", " close "),
                ("C", " copy "),
                ("↑/↓", " scroll  "),
                ("PgUp/PgDn", " faster  "),
                ("Home/End", " jump"),
            ],
        );

        if has_rows && app.table.pagination_state.is_visible() {
            spans.extend(self.pagination.get_hint_spans(app));
        }
        spans
    }

    fn get_preferred_layout(&self, app: &App, area: Rect) -> Vec<Rect> {
        Layout::vertical([
            Constraint::Min(1),                                                              // Table content
            Constraint::Length(if app.table.pagination_state.is_visible() { 7 } else { 0 }), // Pagination controls
        ])
        .split(area)
        .to_vec()
    }
}
