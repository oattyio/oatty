//! Logs component for application logs and statuses.
//!
//! This component wraps the logs widget in a Component so it can be
//! orchestrated by the TEA root with a consistent API.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use heroku_util::redact_sensitive;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::*,
};
use serde_json::Value;

use super::{state::{LogDetailView, LogEntry}, hint_bar::LogsHintBarComponent};
use crate::{
    app, theme,
    ui::{
        components::{component::Component, table::TableComponent},
        utils::{centered_rect, infer_columns_from_json},
    },
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
        let detail = if let Some(LogEntry::Api { json: Some(j), .. }) = self.is_single_api(app) {
            if self.json_has_array(&j) {
                LogDetailView::Table { offset: 0 }
            } else {
                LogDetailView::Text
            }
        } else {
            LogDetailView::Text
        };
        app.logs.detail = Some(detail);

        // Prepare cached redacted JSON when opening details
        if app.logs.selection.is_single() {
            let idx = app.logs.selection.cursor;
            match app.logs.rich_entries.get(idx) {
                Some(LogEntry::Api { json: Some(j), .. }) => {
                    app.logs.cached_detail_index = Some(idx);
                    app.logs.cached_redacted_json = Some(self.redact_json(j));
                    // Cache inferred columns for table view
                    app.logs.cached_columns = Some(infer_columns_from_json(j));
                }
                _ => {
                    app.logs.cached_detail_index = None;
                    app.logs.cached_redacted_json = None;
                    app.logs.cached_columns = None;
                }
            }
        } else {
            app.logs.cached_detail_index = None;
            app.logs.cached_redacted_json = None;
            app.logs.cached_columns = None;
        }
    }

    fn json_has_array(&self, v: &Value) -> bool {
        match v {
            Value::Array(a) => !a.is_empty(),
            Value::Object(m) => m.values().any(|v| matches!(v, Value::Array(_))),
            _ => false,
        }
    }

    fn redact_json(&self, v: &Value) -> Value {
        match v {
            Value::String(s) => Value::String(redact_sensitive(s)),
            Value::Array(arr) => Value::Array(arr.iter().map(|x| self.redact_json(x)).collect()),
            Value::Object(map) => {
                let mut out = serde_json::Map::new();
                for (k, val) in map.iter() {
                    out.insert(k.clone(), self.redact_json(val));
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
                if let Some(j) = json {
                    if app.logs.pretty_json {
                        let red = self.redact_json(j);
                        return serde_json::to_string_pretty(&red).unwrap_or_else(|_| redact_sensitive(raw));
                    }
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

    fn render(&mut self, f: &mut Frame, rect: Rect, app: &mut app::App) {
        let focused = matches!(app.main_focus, app::MainFocus::Logs);
        let block = Block::default()
            .title(Span::styled(
                format!("Logs ({})", app.logs.entries.len()),
                theme::title_style(),
            ))
            .borders(Borders::ALL)
            .border_style(theme::border_style(focused));
        let inner = block.inner(rect);

        // List items with redaction for safety
        // Entries are pre-redacted when appended
        let items: Vec<ListItem> = app
            .logs
            .entries
            .iter()
            .map(|l| ListItem::new(l.as_str()).style(theme::text_style()))
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(theme::list_highlight_style())
            .highlight_symbol(if focused { "► " } else { "" });
        let mut list_state = ListState::default();
        if focused {
            if let Some(sel) = self.selected_index(app) {
                list_state.select(Some(sel));
            }
        } else {
            list_state.select(None);
        }
        f.render_stateful_widget(list, rect, &mut list_state);

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
                    .thumb_style(theme::title_style().fg(theme::ACCENT))
                    .track_style(theme::text_muted());
                f.render_stateful_widget(sb, rect, &mut sb_state);
            }
        }

        // Inline logs hint bar pinned to bottom-left, only when focused
        if focused && inner.height >= 1 {
            let hint_area = Rect::new(inner.x, inner.y + inner.height.saturating_sub(1), inner.width, 1);
            let mut hints_comp = LogsHintBarComponent::new();
            hints_comp.render(f, hint_area, app);
        }

        // Detail modal if open
        if focused && app.logs.detail.is_some() {
            let detail = app.logs.detail.unwrap();
            let area = centered_rect(90, 85, rect);
            let title = "Log Details";
            let block = Block::default()
                .title(Span::styled(title, theme::title_style().fg(theme::ACCENT)))
                .borders(Borders::ALL)
                .border_style(theme::border_style(true));
            f.render_widget(Clear, area);
            f.render_widget(&block, area);
            let inner = block.inner(area);
            let splits = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(inner);

            self.render_detail_content(f, splits[0], app, detail);

            let footer = Paragraph::new(Line::from(vec![
                Span::styled("Hint: ", theme::text_muted()),
                Span::styled("Esc", theme::title_style().fg(theme::ACCENT)),
                Span::styled(" close  ", theme::text_muted()),
                Span::styled("c", theme::title_style().fg(theme::ACCENT)),
                Span::styled(" copy  ", theme::text_muted()),
            ]))
            .style(theme::text_muted());
            f.render_widget(footer, splits[1]);
        }
    }
}

impl LogsComponent {
    fn render_detail_content(&self, f: &mut Frame, area: Rect, app: &mut app::App, detail: LogDetailView) {
        let (start, end) = app.logs.selection.range();
        // Single API with JSON and array → table
        if start == end {
            if let Some(LogEntry::Api { json: Some(j), .. }) = app.logs.rich_entries.get(start) {
                let table = TableComponent::default();
                if self.json_has_array(j) {
                    let red_ref: &Value = match app.logs.cached_detail_index {
                        Some(i) if i == start => app.logs.cached_redacted_json.as_ref().unwrap_or(j),
                        _ => j,
                    };
                    let offset = match detail {
                        LogDetailView::Table { offset } => offset,
                        _ => 0,
                    };
                    if let Some(cols) = app.logs.cached_columns.as_ref() {
                        table.render_json_table_with_columns(f, area, red_ref, offset, cols);
                    } else {
                        table.render_json_table(f, area, red_ref, offset);
                    }
                    return;
                } else {
                    // Render KV or text
                    let red_ref: &Value = match app.logs.cached_detail_index {
                        Some(i) if i == start => app.logs.cached_redacted_json.as_ref().unwrap_or(j),
                        _ => j,
                    };
                    table.render_kv_or_text(f, area, red_ref);
                    return;
                }
            }
            // Single non-API or API without JSON → text
            let s = app.logs.entries.get(start).cloned().unwrap_or_default();
            let p = Paragraph::new(redact_sensitive(&s))
                .block(Block::default().borders(Borders::NONE))
                .wrap(Wrap { trim: false })
                .style(theme::text_style());
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
            .style(theme::text_style());
        f.render_widget(p, area);
    }
}
