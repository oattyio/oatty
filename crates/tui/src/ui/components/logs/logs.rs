//! Logs component for application logs and statuses.
//!
//! This component wraps the logs widget in a Component so it can be
//! orchestrated by the TEA root with a consistent API.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use heroku_util::redact_sensitive;
use once_cell::sync::Lazy;
use ratatui::style::{Modifier, Style};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::*,
};
use regex::Regex;
use serde_json::Value;

use super::{
    hint_bar::LogsHintBarComponent,
    state::{LogDetailView, LogEntry},
};
use crate::ui::components::TableComponent;
use crate::ui::theme::helpers as th;
use crate::ui::theme::roles::Theme as UiTheme;
use crate::{
    app,
    ui::{components::component::Component, utils::centered_rect},
};

#[derive(Default)]
pub struct LogsComponent;

impl LogsComponent {
    pub fn new() -> Self {
        Self
    }

    fn selected_index(&self, app: &app::App) -> Option<usize> {
        if app.logs.entries.is_empty() {
            None
        } else {
            Some(app.logs.selection.cursor.min(app.logs.entries.len() - 1))
        }
    }

    fn move_cursor(&self, app: &mut app::App, delta: isize) {
        if app.logs.entries.is_empty() {
            return;
        }
        let len = app.logs.entries.len() as isize;
        let cur = app.logs.selection.cursor as isize;
        let next = (cur + delta).clamp(0, len - 1) as usize;
        app.logs.selection.cursor = next;
        app.logs.selection.anchor = next;
    }

    fn extend_selection(&self, app: &mut app::App, delta: isize) {
        if app.logs.entries.is_empty() {
            return;
        }
        let len = app.logs.entries.len() as isize;
        let cur = app.logs.selection.cursor as isize;
        let next = (cur + delta).clamp(0, len - 1) as usize;
        app.logs.selection.cursor = next;
    }

    fn is_single_api(&self, app: &app::App) -> Option<LogEntry> {
        if app.logs.selection.is_single() {
            let idx = app.logs.selection.cursor;
            return app.logs.rich_entries.get(idx).cloned();
        }
        None
    }

    fn choose_detail(&self, app: &mut app::App) {
        // Single selection logic only
        if !app.logs.selection.is_single() {
            app.logs.detail = Some(LogDetailView::Text);
            app.logs.cached_detail_index = None;
            app.logs.cached_redacted_json = None;
            return;
        }

        let idx = app.logs.selection.cursor;
        match app.logs.rich_entries.get(idx) {
            Some(LogEntry::Api { json: Some(j), .. }) => {
                if self.json_has_array(j) {
                    // Route array JSON to the global Table modal
                    let redacted = Self::redact_json(j);
                    app.table.apply_result_json(Some(redacted), &*app.ctx.theme);
                    app.table.apply_show(true);
                    // Do not open a Logs detail modal in this case
                    app.logs.detail = None;
                    app.logs.cached_detail_index = None;
                    app.logs.cached_redacted_json = None;
                } else {
                    app.logs.detail = Some(LogDetailView::Text);
                    app.logs.cached_detail_index = Some(idx);
                    app.logs.cached_redacted_json = Some(Self::redact_json(j));
                }
            }
            _ => {
                app.logs.detail = Some(LogDetailView::Text);
                app.logs.cached_detail_index = None;
                app.logs.cached_redacted_json = None;
            }
        }
    }

    fn json_has_array(&self, v: &Value) -> bool {
        match v {
            Value::Array(a) => !a.is_empty(),
            Value::Object(m) => m.values().any(|v| matches!(v, Value::Array(_))),
            _ => false,
        }
    }

    fn redact_json(v: &Value) -> Value {
        match v {
            Value::String(s) => Value::String(redact_sensitive(s)),
            Value::Array(arr) => Value::Array(arr.iter().map(Self::redact_json).collect()),
            Value::Object(map) => {
                let mut out = serde_json::Map::new();
                for (k, val) in map.iter() {
                    out.insert(k.clone(), Self::redact_json(val));
                }
                Value::Object(out)
            }
            other => other.clone(),
        }
    }

    fn build_copy_text(&self, app: &app::App) -> String {
        if app.logs.entries.is_empty() {
            return String::new();
        }
        let (start, end) = app.logs.selection.range();
        if start >= app.logs.entries.len() {
            return String::new();
        }
        if start == end {
            // Single
            if let Some(LogEntry::Api { json, raw, .. }) = app.logs.rich_entries.get(start) {
                if let Some(j) = json
                    && app.logs.pretty_json
                {
                    let red = Self::redact_json(j);
                    return serde_json::to_string_pretty(&red).unwrap_or_else(|_| redact_sensitive(raw));
                }
                return redact_sensitive(raw);
            }
        }
        // Multi-select or text fallback: join visible strings
        let mut buf = String::new();
        for i in start..=end.min(app.logs.entries.len() - 1) {
            let line = app.logs.entries.get(i).cloned().unwrap_or_default();
            if !buf.is_empty() {
                buf.push('\n');
            }
            buf.push_str(&line);
        }
        redact_sensitive(&buf)
    }

    fn styled_line<'a>(&self, theme: &dyn UiTheme, line: &'a str) -> Line<'a> {
        static TS_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^\[?\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2}(?:\.\d+)?Z?\]?").unwrap());
        static UUID_RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-5][0-9a-fA-F]{3}-[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}\b")
                .unwrap()
        });
        static HEXID_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b[0-9a-fA-F]{12,}\b").unwrap());

        let mut spans: Vec<Span> = Vec::new();
        let mut i = 0usize;
        // Timestamp at start
        if let Some(m) = TS_RE.find(line)
            && m.start() == 0 && m.end() > 0 {
                spans.push(Span::styled(
                    &line[m.start()..m.end()],
                    Style::default().fg(theme.roles().accent_secondary),
                ));
                i = m.end();
            }
        // Remaining text; color UUID/hex IDs subtly
        let rest = &line[i..];
        let mut last = 0usize;
        for m in UUID_RE.find_iter(rest).chain(HEXID_RE.find_iter(rest)) {
            if m.start() > last {
                spans.push(Span::styled(&rest[last..m.start()], theme.text_primary_style()));
            }
            spans.push(Span::styled(&rest[m.start()..m.end()], theme.accent_emphasis_style()));
            last = m.end();
        }
        if last < rest.len() {
            spans.push(Span::styled(&rest[last..], theme.text_primary_style()));
        }
        Line::from(spans)
    }
}

impl Component for LogsComponent {
    fn init(&mut self) -> Result<()> {
        Ok(())
    }

    fn handle_key_events(&mut self, app: &mut app::App, key: KeyEvent) -> Vec<app::Effect> {
        let mut effects = Vec::new();
        // If detail is open, some keys control the detail view
        if let Some(detail) = app.logs.detail {
            match key.code {
                KeyCode::Esc | KeyCode::Backspace => {
                    app.logs.detail = None;
                    return effects;
                }
                KeyCode::Up => {
                    if let LogDetailView::Table { offset } = detail {
                        app.logs.detail = Some(LogDetailView::Table {
                            offset: offset.saturating_sub(1),
                        });
                    }
                    return effects;
                }
                KeyCode::Down => {
                    if let LogDetailView::Table { offset } = detail {
                        app.logs.detail = Some(LogDetailView::Table {
                            offset: offset.saturating_add(1),
                        });
                    }
                    return effects;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Up => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.extend_selection(app, -1);
                } else {
                    self.move_cursor(app, -1);
                }
            }
            KeyCode::Down => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.extend_selection(app, 1);
                } else {
                    self.move_cursor(app, 1);
                }
            }
            KeyCode::Enter => {
                self.choose_detail(app);
            }
            KeyCode::Char('c') => {
                let text = self.build_copy_text(app);
                effects.push(app::Effect::CopyLogsRequested(text));
            }
            KeyCode::Char('v') => {
                if matches!(self.is_single_api(app), Some(LogEntry::Api { .. })) {
                    app.logs.pretty_json = !app.logs.pretty_json;
                }
                return effects;
            }
            _ => {}
        }
        effects
    }

    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut app::App) {
        let focused = matches!(app.main_focus, app::MainFocus::Logs);
        let title = format!("Logs ({})", app.logs.entries.len());
        let block = th::block(&*app.ctx.theme, Some(&title), focused);
        let inner = block.inner(rect);

        // List items with redaction for safety
        // Entries are pre-redacted when appended
        let items: Vec<ListItem> = app
            .logs
            .entries
            .iter()
            .map(|l| ListItem::new(self.styled_line(&*app.ctx.theme, l)))
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(app.ctx.theme.selection_style().add_modifier(Modifier::BOLD))
            .style(th::panel_style(&*app.ctx.theme))
            .highlight_symbol(if focused { "► " } else { "" });
        let mut list_state = ListState::default();
        if focused {
            if let Some(sel) = self.selected_index(app) {
                list_state.select(Some(sel));
            }
        } else {
            list_state.select(None);
        }
        frame.render_stateful_widget(list, rect, &mut list_state);

        // Draw a vertical scrollbar to indicate position when focused
        if focused {
            use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
            let content_len = app.logs.entries.len();
            if content_len > 0 {
                let visible = rect.height.saturating_sub(2) as usize; // borders
                let sel = self.selected_index(app).unwrap_or(0);
                let max_top = content_len.saturating_sub(visible);
                let top = sel.min(max_top);
                let mut sb_state = ScrollbarState::new(content_len).position(top);
                let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_style(Style::default().fg(app.ctx.theme.roles().scrollbar_thumb))
                    .track_style(Style::default().fg(app.ctx.theme.roles().scrollbar_track));
                frame.render_stateful_widget(sb, rect, &mut sb_state);
            }
        }

        // Inline logs hint bar pinned to bottom-left, only when focused
        if focused && inner.height >= 1 {
            let hint_area = Rect::new(inner.x, inner.y + inner.height.saturating_sub(1), inner.width, 1);
            let mut hints_comp = LogsHintBarComponent::new();
            hints_comp.render(frame, hint_area, app);
        }

        // Detail modal if open
        if focused && app.logs.detail.is_some() {
            let area = centered_rect(90, 85, rect);
            let title = "Log Details";
            let block = th::block(&*app.ctx.theme, Some(title), true);
            frame.render_widget(Clear, area);
            frame.render_widget(&block, area);
            let inner = block.inner(area);
            let splits = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(inner);

            self.render_detail_content(frame, splits[0], app);

            let footer = Paragraph::new(Line::from(vec![
                Span::styled("Hint: ", app.ctx.theme.text_muted_style()),
                Span::styled("Esc", app.ctx.theme.accent_emphasis_style()),
                Span::styled(" close  ", app.ctx.theme.text_muted_style()),
                Span::styled("c", app.ctx.theme.accent_emphasis_style()),
                Span::styled(" copy  ", app.ctx.theme.text_muted_style()),
            ]))
            .style(app.ctx.theme.text_muted_style());
            frame.render_widget(footer, splits[1]);
        }
    }
}

impl LogsComponent {
    fn render_detail_content(&self, f: &mut Frame, area: Rect, app: &mut app::App) {
        let (start, end) = app.logs.selection.range();
        // Single API with JSON and array → table
        if start == end {
            if let Some(LogEntry::Api { json: Some(j), .. }) = app.logs.rich_entries.get(start) {
                // Only non-array JSON renders here; arrays are routed to the global table modal
                let red_ref: &Value = match app.logs.cached_detail_index {
                    Some(i) if i == start => app.logs.cached_redacted_json.as_ref().unwrap_or(j),
                    _ => j,
                };
                // Render KV or text for non-array JSON
                let table = TableComponent::default();
                table.render_kv_or_text(f, area, red_ref, &*app.ctx.theme);
                return;
            }
            // Single non-API or API without JSON → text
            let s = app.logs.entries.get(start).cloned().unwrap_or_default();
            let p = Paragraph::new(redact_sensitive(&s))
                .block(Block::default().borders(Borders::NONE))
                .wrap(Wrap { trim: false })
                .style(app.ctx.theme.text_primary_style());
            f.render_widget(p, area);
            return;
        }

        // Multi-select: concatenate text lines
        let mut buf = String::new();
        let max = app.logs.entries.len().saturating_sub(1);
        for i in start..=end.min(max) {
            if !buf.is_empty() {
                buf.push('\n');
            }
            buf.push_str(app.logs.entries.get(i).map(|s| s.as_str()).unwrap_or(""));
        }
        let p = Paragraph::new(redact_sensitive(&buf))
            .block(Block::default().borders(Borders::NONE))
            .wrap(Wrap { trim: false })
            .style(app.ctx.theme.text_primary_style());
        f.render_widget(p, area);
    }
}
