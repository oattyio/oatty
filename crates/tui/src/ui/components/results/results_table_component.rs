//! Results table modal component for displaying JSON data.
//!
//! This module provides a component for rendering the results modal, which
//! displays JSON results from command execution in a tabular format with
//! scrolling and navigation capabilities.
use crate::app::App;
use crate::ui::{
    components::{
        common::{ResultsTableView, handle_table_navigation_key},
        component::Component,
    },
    theme::theme_helpers as th,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use oatty_types::{Effect, Msg};

use crate::ui::components::common::handle_table_mouse_actions;
use rat_focus::Focus;
use ratatui::layout::Position;
use ratatui::widgets::{Borders, Padding};
use ratatui::{Frame, layout::Rect, text::Span};
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

/// Results table modal component for displaying JSON data.
///
/// This component renders a modal dialog containing tabular data from command
/// execution results. It automatically detects JSON arrays and displays them
/// in a scrollable results format with proper column detection and formatting.
///
/// # Features
///
/// - Automatically detects and displays JSON arrays as results
/// - Provides scrollable navigation through large datasets
/// - Handles column detection and formatting
/// - Supports keyboard navigation (arrow keys, page up/down, home/end)
/// - Falls back to key-value display for non-array JSON
///
/// # Navigation
///
/// - **Arrow keys**: Scroll up/down through rows
/// - **Page Up/Down**: Scroll faster through the results
/// - **Home/End**: Jump to the beginning /end of the results
/// - **Escape**: Close the results modal
#[derive(Debug, Default)]
pub struct TableComponent {
    view: ResultsTableView,
    table_area: Rect,
    breadcrumb_area: Rect,
    last_click: Option<(usize, usize, Instant)>,
}

impl Component for TableComponent {
    fn handle_message(&mut self, app: &mut App, msg: Msg) -> Vec<Effect> {
        if let Msg::ExecCompleted(exec_outcome) = msg {
            app.table.process_general_execution_result(*exec_outcome, &*app.ctx.theme);
        }
        Vec::new()
    }

    /// Handle key events for the result results modal.
    ///
    /// Applies local state updates directly to `app.results` for scrolling and
    /// navigation. Returns `Ok(true)` if the results handled the key,
    /// otherwise `Ok(false)`.
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let mut effects: Vec<Effect> = vec![];
        if key.code == KeyCode::Esc && app.table.drill_up(&*app.ctx.theme) {
            return effects;
        }
        if key.code == KeyCode::Esc {
            return vec![Effect::CloseModal];
        }
        if key.code == KeyCode::Enter && app.table.drill_into_selection(&*app.ctx.theme) {
            return effects;
        }
        if app.table.has_rows() && handle_table_navigation_key(key.code, &mut app.table, app.focus.as_ref()) {
            return effects;
        }
        if !app.table.has_rows() && matches!(key.code, KeyCode::Char('v') | KeyCode::Char('V')) {
            app.table.toggle_split_preview_pinned();
            return effects;
        }
        if !app.table.has_rows() && handle_split_preview_navigation_key(key, &mut app.table) {
            return effects;
        }
        if !app.table.has_rows() && handle_fallback_navigation_key(key.code, &mut app.table, app.focus.as_ref()) {
            return effects;
        }
        if let KeyCode::Char('c') = key.code {
            if let Some(idx) = app.table.table_state.selected()
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
        effects
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        if self.try_handle_breadcrumb_click(app, mouse) {
            return Vec::new();
        }
        if app.table.has_rows() {
            handle_table_mouse_actions(&mut app.table, mouse, self.table_area);
            self.select_table_cell_from_mouse(app, mouse);
            self.try_handle_double_click_drill(app, mouse);
        } else {
            let list_area = self.view.fallback_list_area().unwrap_or(self.table_area);
            let preview_area = self.view.split_preview_area();
            handle_fallback_mouse_actions(&mut app.table, mouse, list_area, preview_area);
            let position = Position {
                x: mouse.column,
                y: mouse.row,
            };
            if mouse.kind == MouseEventKind::Down(crossterm::event::MouseButton::Left)
                && preview_area.is_some_and(|area| area.contains(position))
            {
                return Vec::new();
            }
            self.try_handle_double_click_drill(app, mouse);
        }

        Vec::new()
    }

    /// Renders the results modal with JSON results.
    ///
    /// This method handles the layout, styling, and results generation for the
    /// result display.
    ///
    /// # Arguments
    ///
    /// * `f` - The frame to render to
    /// * `rect` - The rectangular area to render in
    /// * `app` - The application state containing result data
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        // Large modal to maximize space for tables
        let title = if app.table.is_in_drill_mode() {
            "Results  [Esc] Up  [Enter] Drill  ↑/↓ Scroll"
        } else {
            "Results  [Esc] Close  [Enter] Drill  ↑/↓ Scroll"
        };
        let block = th::block(&*app.ctx.theme, Some(title), app.table.container_focus.get());

        frame.render_widget(&block, rect);
        let inner = block.inner(rect);
        let layout_chunks =
            ratatui::layout::Layout::vertical([ratatui::layout::Constraint::Length(1), ratatui::layout::Constraint::Min(1)]).split(inner);
        let breadcrumb_area = layout_chunks[0];
        let content_area = layout_chunks[1];
        self.render_breadcrumbs(frame, breadcrumb_area, app);
        // Split for content + pagination and footer
        let is_grid_focused = app.table.grid_f.get();
        let table_block = th::block::<String>(&*app.ctx.theme, None, is_grid_focused)
            .borders(Borders::NONE)
            .padding(Padding::uniform(1));
        let table_inner = table_block.inner(content_area);
        frame.render_widget(table_block, content_area);
        self.view
            .render_results(frame, table_inner, &mut app.table, is_grid_focused, &*app.ctx.theme);

        self.table_area = table_inner;
        self.breadcrumb_area = breadcrumb_area;
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let has_rows = app.table.has_rows();
        let has_kv = !app.table.kv_entries().is_empty();
        if !has_rows && !has_kv {
            return Vec::new();
        }

        let theme = &*app.ctx.theme;
        if has_rows {
            return th::build_hint_spans(
                theme,
                &[
                    ("Esc", if app.table.is_in_drill_mode() { " up " } else { " close " }),
                    ("Enter", " drill "),
                    ("C", " copy row "),
                    ("↑/↓", " scroll  "),
                    ("PgUp/PgDn", " faster  "),
                    ("Home/End", " jump"),
                ],
            );
        }

        th::build_hint_spans(
            theme,
            &[
                ("Esc", if app.table.is_in_drill_mode() { " up " } else { " close " }),
                ("Enter", " drill "),
                ("V", " preview  "),
                ("↑/↓", " list  "),
                ("Ctrl+↑/↓", " preview  "),
                ("Ctrl+PgUp/PgDn", " preview page"),
            ],
        )
    }
}

impl TableComponent {
    fn render_breadcrumbs(&self, frame: &mut Frame, area: Rect, app: &App) {
        let breadcrumbs = app.table.breadcrumbs();
        let mut spans = Vec::new();
        for (index, breadcrumb) in breadcrumbs.iter().enumerate() {
            let style = if index + 1 == breadcrumbs.len() {
                app.ctx.theme.accent_emphasis_style()
            } else {
                app.ctx.theme.text_secondary_style()
            };
            spans.push(Span::styled(breadcrumb.clone(), style));
            if index + 1 < breadcrumbs.len() {
                spans.push(Span::styled(" / ", app.ctx.theme.text_muted_style()));
            }
        }
        let breadcrumb = ratatui::widgets::Paragraph::new(ratatui::text::Line::from(spans)).style(app.ctx.theme.text_secondary_style());
        frame.render_widget(breadcrumb, area);
    }

    fn try_handle_breadcrumb_click(&mut self, app: &mut App, mouse: MouseEvent) -> bool {
        let position = Position {
            x: mouse.column,
            y: mouse.row,
        };
        if mouse.kind != crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
            || !self.breadcrumb_area.contains(position)
        {
            return false;
        }

        let breadcrumbs = app.table.breadcrumbs();
        let mut cursor_x = self.breadcrumb_area.x;
        for (index, label) in breadcrumbs.iter().enumerate() {
            let width = label.width() as u16;
            if position.x >= cursor_x && position.x < cursor_x.saturating_add(width) {
                return app.table.drill_to_breadcrumb(index, &*app.ctx.theme);
            }
            cursor_x = cursor_x.saturating_add(width);
            if index + 1 < breadcrumbs.len() {
                cursor_x = cursor_x.saturating_add(3);
            }
        }
        false
    }

    fn try_handle_double_click_drill(&mut self, app: &mut App, mouse: MouseEvent) {
        if mouse.kind != crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) {
            return;
        }
        let now = Instant::now();
        let (row_index, column_index) = if app.table.has_rows() {
            let Some(row_index) = app.table.table_state.selected() else {
                self.last_click = None;
                return;
            };
            let Some(column_index) = app.table.table_state.selected_column() else {
                self.last_click = None;
                return;
            };
            (row_index, column_index)
        } else {
            let Some(row_index) = app.table.list_state.selected() else {
                self.last_click = None;
                return;
            };
            (row_index, 0usize)
        };
        if let Some((last_row_index, last_column_index, last_instant)) = self.last_click
            && last_row_index == row_index
            && last_column_index == column_index
            && now.duration_since(last_instant) <= Duration::from_millis(350)
        {
            let _ = app.table.drill_into_selection(&*app.ctx.theme);
            self.last_click = None;
            return;
        }
        self.last_click = Some((row_index, column_index, now));
    }

    fn select_table_cell_from_mouse(&mut self, app: &mut App, mouse: MouseEvent) {
        if mouse.kind != crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) {
            return;
        }
        let position = Position {
            x: mouse.column,
            y: mouse.row,
        };
        if !self.table_area.contains(position) {
            return;
        }
        let row_index = find_table_row_index(self.table_area, position, app.table.table_state.offset());
        let Some(row_index) = row_index else {
            return;
        };
        let relative_x = position.x.saturating_sub(self.table_area.x);
        let column_index = app.table.hit_test_column(relative_x, self.table_area.width).unwrap_or(0usize);
        app.table.select_cell(row_index, column_index, self.table_area.width);
    }
}

fn find_table_row_index(table_area: Rect, position: Position, table_offset: usize) -> Option<usize> {
    if !table_area.contains(position) {
        return None;
    }
    Some(position.y.saturating_sub(table_area.y + 1) as usize + table_offset)
}

fn handle_fallback_navigation_key(
    key_code: KeyCode,
    state: &mut crate::ui::components::results::state::ResultsTableState,
    focus: &Focus,
) -> bool {
    let list_state = &mut state.list_state;
    match key_code {
        KeyCode::BackTab => {
            focus.prev();
        }
        KeyCode::Tab => {
            focus.next();
        }
        KeyCode::Up => {
            list_state.scroll_up_by(1);
        }
        KeyCode::Down => {
            list_state.scroll_down_by(1);
        }
        KeyCode::PageUp => {
            list_state.scroll_up_by(10);
        }
        KeyCode::PageDown => {
            list_state.scroll_down_by(10);
        }
        KeyCode::Home => {
            list_state.scroll_up_by(u16::MAX);
        }
        KeyCode::End => {
            list_state.scroll_down_by(u16::MAX);
        }
        _ => return false,
    }
    true
}

fn handle_fallback_mouse_actions(
    state: &mut crate::ui::components::results::state::ResultsTableState,
    mouse: MouseEvent,
    list_area: Rect,
    preview_area: Option<Rect>,
) -> bool {
    let position = Position {
        x: mouse.column,
        y: mouse.row,
    };
    if let Some(preview_area) = preview_area
        && preview_area.contains(position)
    {
        match mouse.kind {
            MouseEventKind::ScrollDown => state.scroll_split_preview_lines(1),
            MouseEventKind::ScrollUp => state.scroll_split_preview_lines(-1),
            MouseEventKind::Down(crossterm::event::MouseButton::Left) => {}
            _ => return false,
        }
        return true;
    }

    if !list_area.contains(position) {
        return false;
    }

    match mouse.kind {
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            let relative_row = position.y.saturating_sub(list_area.y) as usize;
            let index = state.list_state.offset() + relative_row;
            if index < state.kv_entries().len() {
                state.list_state.select(Some(index));
            }
        }
        crossterm::event::MouseEventKind::ScrollUp => state.list_state.scroll_up_by(1),
        crossterm::event::MouseEventKind::ScrollDown => state.list_state.scroll_down_by(1),
        _ => return false,
    }
    true
}

fn handle_split_preview_navigation_key(key_event: KeyEvent, state: &mut crate::ui::components::results::state::ResultsTableState) -> bool {
    if !state.should_show_split_preview() {
        return false;
    }
    let has_control = key_event.modifiers.contains(KeyModifiers::CONTROL);
    if !has_control {
        return false;
    }
    match key_event.code {
        KeyCode::Up | KeyCode::Char('k') => {
            state.scroll_split_preview_lines(-1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.scroll_split_preview_lines(1);
        }
        KeyCode::PageUp => {
            state.scroll_split_preview_pages(-1);
        }
        KeyCode::PageDown => {
            state.scroll_split_preview_pages(1);
        }
        KeyCode::Home => {
            state.scroll_split_preview_to_top();
        }
        KeyCode::End => {
            state.scroll_split_preview_to_bottom();
        }
        _ => return false,
    }
    true
}
