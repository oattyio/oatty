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
use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use oatty_types::{Effect, Msg};

use crate::ui::components::common::handle_table_mouse_actions;
use ratatui::widgets::{Borders, Padding};
use ratatui::{Frame, layout::Rect, text::Span};

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
        if key.code == KeyCode::Esc {
            return vec![Effect::CloseModal];
        }
        if handle_table_navigation_key(key.code, &mut app.table, app.focus.as_ref()) {
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
        handle_table_mouse_actions(&mut app.table, mouse, self.table_area);

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
        let title = "Results  [Esc] Close  ↑/↓ Scroll";
        let block = th::block(&*app.ctx.theme, Some(title), app.table.container_focus.get());

        frame.render_widget(&block, rect);
        let inner = block.inner(rect);
        // Split for content + pagination and footer
        let is_grid_focused = app.table.grid_f.get();
        let table_block = th::block::<String>(&*app.ctx.theme, None, is_grid_focused)
            .borders(Borders::NONE)
            .padding(Padding::uniform(1));
        let table_inner = table_block.inner(inner);
        frame.render_widget(table_block, inner);
        self.view
            .render_results(frame, table_inner, &mut app.table, is_grid_focused, &*app.ctx.theme);

        self.table_area = table_inner;
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let has_rows = app.table.has_rows();
        let has_kv = !app.table.kv_entries().is_empty();
        if !has_rows && !has_kv {
            return Vec::new();
        }

        let theme = &*app.ctx.theme;
        th::build_hint_spans(
            theme,
            &[
                ("Esc", " close "),
                ("C", " copy row "),
                ("↑/↓", " scroll  "),
                ("PgUp/PgDn", " faster  "),
                ("Home/End", " jump"),
            ],
        )
    }
}
