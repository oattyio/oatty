//! Log detail modal rendering and input handling.
//!
//! This module focuses on rendering a selected log entry in a modal with
//! redaction, along with key handling for table navigation and copy/close
//! actions.

use crate::ui::components::common::handle_table_mouse_actions;
use crate::{
    app::App,
    ui::{
        components::{
            common::{ResultsTableView, handle_table_navigation_key},
            component::Component,
            logs::state::LogEntry,
            results::build_key_value_entries,
        },
        theme::theme_helpers::{self, build_hint_spans},
        utils::build_copy_text,
    },
};
use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use oatty_types::{Effect, LogLevel};
use oatty_util::redact_sensitive;
use ratatui::{
    Frame,
    layout::Rect,
    text::Span,
    widgets::{Block, Borders, Paragraph, Wrap},
};

/// Renders and manages the log detail modal.
#[derive(Debug, Default)]
pub struct LogDetailsComponent {
    details_table: ResultsTableView,
    table_area: Rect,
}

impl LogDetailsComponent {
    /// Renders the content of the detail modal for selected log entries.
    ///
    /// This method handles different rendering modes based on the selection:
    ///
    /// - **Single API entry with JSON**: Renders formatted JSON using
    ///   TableComponent
    /// - **Single non-API entry**: Renders plain text with word wrapping
    ///
    /// All content is automatically redacted for security before display.
    ///
    /// # Arguments
    ///
    /// * `f` - The terminal frame to render to
    /// * `area` - The rectangular area allocated for the detail content
    /// * `app` - The application state containing logs and selection
    fn render_detail_content(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let Some(selected_index) = self.selected_index(app) else {
            render_empty_detail_message(frame, area);
            return;
        };

        if selected_index >= app.logs.rich_entries.len() {
            render_empty_detail_message(frame, area);
            return;
        }

        match app.logs.rich_entries.get(selected_index).expect("selected index is valid") {
            LogEntry::Api { json: Some(value), .. } | LogEntry::Mcp { json: Some(value), .. } => {
                let entries = build_key_value_entries(value);
                app.logs.results_table.set_kv_entries(entries);
                self.details_table
                    .render_results(frame, area, &mut app.logs.results_table, true, &*app.ctx.theme);
                return;
            }
            _ => {}
        };

        match app.logs.rich_entries.get(selected_index).expect("selected index is valid") {
            LogEntry::Api { status, raw, .. } => {
                let detail_text = format!("HTTP {status}\n\n{raw}");
                render_redacted_paragraph(frame, area, &*app.ctx.theme, &detail_text);
            }
            LogEntry::Text { level, msg } => {
                let heading = level.unwrap_or(LogLevel::Info);
                let detail_text = format!("[{heading}] {msg}");
                render_redacted_paragraph(frame, area, &*app.ctx.theme, &detail_text);
            }
            LogEntry::Mcp { raw, .. } => {
                render_redacted_paragraph(frame, area, &*app.ctx.theme, raw);
            }
        }
    }

    fn selected_index(&self, app: &App) -> Option<usize> {
        if app.logs.rich_entries.is_empty() {
            return None;
        }
        app.logs.list_state.selected()
    }
}

impl Component for LogDetailsComponent {
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let mut effects = Vec::with_capacity(1);
        if key.code == KeyCode::Esc {
            effects.push(Effect::CloseModal);
            return effects;
        }
        if handle_table_navigation_key(key.code, &mut app.logs.results_table, app.focus.as_ref()) {
            return effects;
        }
        if let KeyCode::Char('c') = key.code {
            effects.push(Effect::CopyLogsRequested(build_copy_text(app)));
        }

        effects
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        handle_table_mouse_actions(&mut app.logs.results_table, mouse, self.table_area);
        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let title = "Log Details";
        let block = theme_helpers::block(&*app.ctx.theme, Some(title), true);
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        self.render_detail_content(frame, inner, app);
        self.table_area = inner
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        build_hint_spans(theme, &[("Esc", " Close  "), ("C", " Copy  ")])
    }
}

fn render_empty_detail_message(frame: &mut Frame, area: Rect) {
    frame.render_widget(Span::raw("Nothing here"), area);
}

fn render_redacted_paragraph(frame: &mut Frame, area: Rect, theme: &dyn crate::ui::theme::roles::Theme, text: &str) {
    let paragraph = Paragraph::new(redact_sensitive(text))
        .block(Block::default().borders(Borders::NONE))
        .wrap(Wrap { trim: false })
        .style(theme.text_primary_style());
    frame.render_widget(paragraph, area);
}
