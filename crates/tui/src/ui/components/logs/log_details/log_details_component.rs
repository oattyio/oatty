//! Log detail modal rendering and input handling.
//!
//! This module focuses on rendering a selected log entry in a modal with
//! redaction, along with key handling for table navigation and copy/close
//! actions.

use crate::ui::components::common::handle_table_mouse_actions;
use crate::ui::theme::Theme;
use crate::{
    app::App,
    ui::{
        components::{
            common::{
                ResultsTableView, ScrollMetrics, handle_table_navigation_key, highlight_pretty_json_lines, render_vertical_scrollbar,
            },
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
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum DetailViewMode {
    #[default]
    Empty,
    Table,
    Text,
}

#[derive(Debug, Clone)]
struct CachedPrettyJson {
    selected_index: usize,
    formatted: Arc<str>,
}

/// Renders and manages the log detail modal.
#[derive(Debug, Default)]
pub struct LogDetailsComponent {
    details_table: ResultsTableView,
    table_area: Rect,
    detail_area: Rect,
    detail_view_mode: DetailViewMode,
    last_selected_index: Option<usize>,
    text_scroll_metrics: ScrollMetrics,
    cached_pretty_json: Option<CachedPrettyJson>,
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
            self.detail_view_mode = DetailViewMode::Empty;
            render_empty_detail_message(frame, area);
            return;
        };

        if selected_index >= app.logs.rich_entries.len() {
            self.detail_view_mode = DetailViewMode::Empty;
            render_empty_detail_message(frame, area);
            return;
        }

        if self.last_selected_index != Some(selected_index) {
            self.reset_text_scroll_state();
            self.last_selected_index = Some(selected_index);
            self.cached_pretty_json = None;
        }

        if let Some(parsed_json) = selected_mcp_parsed_response_json(app, selected_index) {
            self.detail_view_mode = DetailViewMode::Text;
            self.render_scrollable_parsed_response_json(frame, area, &*app.ctx.theme, selected_index, parsed_json, app);
            return;
        }

        match app.logs.rich_entries.get(selected_index).expect("selected index is valid") {
            LogEntry::Api { json: Some(value), .. } | LogEntry::Mcp { json: Some(value), .. } => {
                self.detail_view_mode = DetailViewMode::Table;
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
                self.detail_view_mode = DetailViewMode::Text;
                self.render_scrollable_redacted_paragraph(frame, area, &*app.ctx.theme, &detail_text, app);
            }
            LogEntry::Text { level, msg } => {
                let heading = level.unwrap_or(LogLevel::Info);
                let detail_text = format!("[{heading}] {msg}");
                self.detail_view_mode = DetailViewMode::Text;
                self.render_scrollable_redacted_paragraph(frame, area, &*app.ctx.theme, &detail_text, app);
            }
            LogEntry::Mcp { raw, .. } => {
                self.detail_view_mode = DetailViewMode::Text;
                self.render_scrollable_redacted_paragraph(frame, area, &*app.ctx.theme, raw, app);
            }
        }
    }

    fn selected_index(&self, app: &App) -> Option<usize> {
        if app.logs.rich_entries.is_empty() {
            return None;
        }
        app.logs.selected_rich_index()
    }

    fn reset_text_scroll_state(&mut self) {
        self.text_scroll_metrics.reset();
    }

    fn update_text_viewport_height(&mut self, height: u16) {
        self.text_scroll_metrics.update_viewport_height(height);
    }

    fn update_text_content_height(&mut self, height: u16) {
        self.text_scroll_metrics.update_content_height(height);
    }

    fn max_text_scroll_offset(&self) -> u16 {
        self.text_scroll_metrics.max_offset()
    }

    fn is_text_scrollable(&self) -> bool {
        self.text_scroll_metrics.is_scrollable()
    }

    fn scroll_text_lines(&mut self, delta: i16) {
        self.text_scroll_metrics.scroll_lines(delta);
    }

    fn scroll_text_pages(&mut self, delta_pages: i16) {
        self.text_scroll_metrics.scroll_pages(delta_pages);
    }

    fn scroll_text_to_top(&mut self) {
        self.text_scroll_metrics.scroll_to_top();
    }

    fn scroll_text_to_bottom(&mut self) {
        self.text_scroll_metrics.scroll_to_bottom();
    }

    fn handle_text_scroll_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_text_lines(-1);
                true
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_text_lines(1);
                true
            }
            KeyCode::PageUp => {
                self.scroll_text_pages(-1);
                true
            }
            KeyCode::PageDown => {
                self.scroll_text_pages(1);
                true
            }
            KeyCode::Home => {
                self.scroll_text_to_top();
                true
            }
            KeyCode::End => {
                self.scroll_text_to_bottom();
                true
            }
            _ => false,
        }
    }

    fn render_text_scrollbar(&self, frame: &mut Frame, area: Rect, app: &App) {
        if !self.is_text_scrollable() {
            return;
        }

        let viewport_height = usize::from(self.text_scroll_metrics.viewport_height().max(1));
        let max_scroll_offset = self.max_text_scroll_offset();
        let content_length = usize::from(max_scroll_offset.saturating_add(1));
        render_vertical_scrollbar(
            frame,
            area,
            &*app.ctx.theme,
            content_length,
            self.text_scroll_metrics.offset() as usize,
            viewport_height,
        );
    }

    fn render_scrollable_redacted_paragraph(&mut self, frame: &mut Frame, area: Rect, theme: &dyn Theme, text: &str, app: &App) {
        self.update_text_viewport_height(area.height);
        let mut paragraph = Paragraph::new(redact_sensitive(text))
            .block(Block::default().borders(Borders::NONE))
            .wrap(Wrap { trim: false })
            .style(theme.text_primary_style());
        let line_count = paragraph.line_count(area.width);
        let capped_height = line_count.min(u16::MAX as usize) as u16;
        self.update_text_content_height(capped_height);

        paragraph = paragraph.scroll((self.text_scroll_metrics.offset(), 0));
        frame.render_widget(paragraph, area);
        self.render_text_scrollbar(frame, area, app);
    }

    fn render_scrollable_parsed_response_json(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        theme: &dyn Theme,
        selected_index: usize,
        json_value: &Value,
        app: &App,
    ) {
        let mut lines = Vec::new();
        lines.push(Line::from(vec![Span::styled("parsed_response_text", theme.syntax_keyword_style())]));
        lines.push(Line::from(Span::raw("")));
        self.ensure_formatted_json_cached(selected_index, json_value);
        let formatted_json = self.formatted_json_for_selection(selected_index).unwrap_or_else(|| Arc::from(""));
        lines.extend(highlight_pretty_json_lines(formatted_json.as_ref(), theme));

        self.update_text_viewport_height(area.height);
        let mut paragraph = Paragraph::new(lines)
            .block(Block::default().borders(Borders::NONE))
            .wrap(Wrap { trim: false })
            .style(theme.text_primary_style());
        let line_count = paragraph.line_count(area.width);
        let capped_height = line_count.min(u16::MAX as usize) as u16;
        self.update_text_content_height(capped_height);

        paragraph = paragraph.scroll((self.text_scroll_metrics.offset(), 0));
        frame.render_widget(paragraph, area);
        self.render_text_scrollbar(frame, area, app);
    }

    fn ensure_formatted_json_cached(&mut self, selected_index: usize, json_value: &Value) {
        let already_cached = self
            .cached_pretty_json
            .as_ref()
            .map(|cached| cached.selected_index == selected_index)
            .unwrap_or(false);
        if already_cached {
            return;
        }

        let formatted_json: Arc<str> = Arc::from(serde_json::to_string_pretty(json_value).unwrap_or_else(|_| json_value.to_string()));
        self.cached_pretty_json = Some(CachedPrettyJson {
            selected_index,
            formatted: formatted_json,
        });
    }

    fn formatted_json_for_selection(&self, selected_index: usize) -> Option<Arc<str>> {
        self.cached_pretty_json.as_ref().and_then(|cached| {
            if cached.selected_index == selected_index {
                Some(Arc::clone(&cached.formatted))
            } else {
                None
            }
        })
    }
}

impl Component for LogDetailsComponent {
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let mut effects = Vec::with_capacity(1);
        if key.code == KeyCode::Esc {
            effects.push(Effect::CloseModal);
            return effects;
        }
        if self.detail_view_mode == DetailViewMode::Table {
            if handle_table_navigation_key(key.code, &mut app.logs.results_table, app.focus.as_ref()) {
                return effects;
            }
        } else if self.handle_text_scroll_key(key.code) {
            return effects;
        }
        if let KeyCode::Char('c') = key.code {
            effects.push(Effect::CopyLogsRequested(build_copy_text(app)));
        }

        effects
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        if self.detail_view_mode == DetailViewMode::Table {
            handle_table_mouse_actions(&mut app.logs.results_table, mouse, self.table_area);
        } else if self.detail_area.contains((mouse.column, mouse.row).into()) {
            match mouse.kind {
                crossterm::event::MouseEventKind::ScrollDown => self.scroll_text_lines(1),
                crossterm::event::MouseEventKind::ScrollUp => self.scroll_text_lines(-1),
                _ => {}
            }
        }
        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let title = "Log Details";
        let block = theme_helpers::block(&*app.ctx.theme, Some(title), true);
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        self.render_detail_content(frame, inner, app);
        self.table_area = inner;
        self.detail_area = inner;
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        match self.detail_view_mode {
            DetailViewMode::Table => build_hint_spans(
                theme,
                &[
                    ("↑/↓", " Scroll  "),
                    ("PgUp/PgDn", " Page  "),
                    ("Home/End", " Jump  "),
                    ("Esc", " Close  "),
                    ("C", " Copy  "),
                ],
            ),
            DetailViewMode::Text => build_hint_spans(
                theme,
                &[
                    ("↑/↓", " Scroll  "),
                    ("PgUp/PgDn", " Page  "),
                    ("Home/End", " Jump  "),
                    ("Esc", " Close  "),
                    ("C", " Copy  "),
                ],
            ),
            DetailViewMode::Empty => build_hint_spans(theme, &[("Esc", " Close  "), ("C", " Copy  ")]),
        }
    }
}

fn render_empty_detail_message(frame: &mut Frame, area: Rect) {
    frame.render_widget(Span::raw("Nothing here"), area);
}

fn selected_mcp_parsed_response_json<'application>(app: &'application App<'_>, selected_index: usize) -> Option<&'application Value> {
    let entry = app.logs.rich_entries.get(selected_index)?;
    let payload = match entry {
        LogEntry::Mcp { json: Some(value), .. } => value,
        _ => return None,
    };
    payload
        .get("parsed_response_text")
        .filter(|value| value.is_object() || value.is_array())
}
