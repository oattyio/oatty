use crate::{
    app::App,
    ui::{
        components::{
            common::ResultsTableView,
            component::Component,
            logs::state::{LogDetailView, LogEntry},
            table::build_key_value_entries,
        },
        theme::theme_helpers::{self, build_hint_spans},
        utils::build_copy_text,
    },
};
use crossterm::event::KeyCode;
use heroku_types::Effect;
use heroku_util::redact_sensitive;
use ratatui::{
    Frame,
    layout::Rect,
    text::Span,
    widgets::{Block, Borders, Paragraph, Wrap},
};
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct LogDetailsComponent;

impl LogDetailsComponent {
    /// Renders the content of the detail modal for selected log entries.
    ///
    /// This method handles different rendering modes based on the selection:
    ///
    /// - **Single API entry with JSON**: Renders formatted JSON using
    ///   TableComponent
    /// - **Single non-API entry**: Renders plain text with word wrapping
    /// - **Multi-selection**: Renders concatenated text from all selected
    ///   entries
    ///
    /// All content is automatically redacted for security before display.
    ///
    /// # Arguments
    ///
    /// * `f` - The terminal frame to render to
    /// * `area` - The rectangular area allocated for the detail content
    /// * `app` - The application state containing logs and selection
    fn render_detail_content(&self, frame: &mut Frame, area: Rect, app: &mut App) {
        let (start, end) = app.logs.selection.range();

        // Handle single selection
        if start == end {
            if let Some(entry) = app.logs.rich_entries.get(start) {
                match entry {
                    LogEntry::Api { json: Some(j), .. } | LogEntry::Mcp { json: Some(j), .. } => {
                        // Use cached redacted JSON if available, otherwise redact on-the-fly
                        // Note: Only non-array JSON renders here; arrays are routed to global table
                        // modal
                        let red_ref: &Value = match app.logs.cached_detail_index {
                            Some(i) if i == start => app.logs.cached_redacted_json.as_ref().unwrap_or(j),
                            _ => j,
                        };

                        // Render formatted JSON using TableComponent for better presentation
                        let entries = build_key_value_entries(red_ref);
                        let offset = match app.logs.detail {
                            Some(LogDetailView::Table { offset }) => offset.min(entries.len().saturating_sub(1)),
                            _ => 0,
                        };
                        let selection = if entries.is_empty() { None } else { Some(offset) };
                        app.logs.detail = Some(LogDetailView::Table { offset });
                        let detail_block = theme_helpers::block(&*app.ctx.theme, Some("Details"), true);
                        let inner_area = detail_block.inner(area);
                        frame.render_widget(detail_block, area);
                        ResultsTableView::render_kv_or_text(frame, inner_area, &entries, selection, offset, red_ref, &*app.ctx.theme);
                        return;
                    }
                    LogEntry::Api { status, raw, .. } => {
                        let detail_text = format!("HTTP {status}\n\n{raw}");
                        let paragraph = Paragraph::new(redact_sensitive(&detail_text))
                            .block(Block::default().borders(Borders::NONE))
                            .wrap(Wrap { trim: false })
                            .style(app.ctx.theme.text_primary_style());
                        frame.render_widget(paragraph, area);
                        return;
                    }
                    LogEntry::Text { level, msg } => {
                        let heading = level.as_deref().unwrap_or("info");
                        let detail_text = format!("[{heading}] {msg}");
                        let paragraph = Paragraph::new(redact_sensitive(&detail_text))
                            .block(Block::default().borders(Borders::NONE))
                            .wrap(Wrap { trim: false })
                            .style(app.ctx.theme.text_primary_style());
                        frame.render_widget(paragraph, area);
                        return;
                    }
                    LogEntry::Mcp { raw, .. } => {
                        let paragraph = Paragraph::new(redact_sensitive(raw))
                            .block(Block::default().borders(Borders::NONE))
                            .wrap(Wrap { trim: false })
                            .style(app.ctx.theme.text_primary_style());
                        frame.render_widget(paragraph, area);
                        return;
                    }
                }
            }

            // Handle a single non-API entry or API without JSON
            let s = app.logs.entries.get(start).cloned().unwrap_or_default();
            let p = Paragraph::new(redact_sensitive(&s))
                .block(Block::default().borders(Borders::NONE))
                .wrap(Wrap { trim: false })
                .style(app.ctx.theme.text_primary_style());
            frame.render_widget(p, area);
            return;
        }

        // Handle multi-selection: concatenate all selected log entries
        let mut buf = String::new();
        let max = app.logs.entries.len().saturating_sub(1);
        for i in start..=end.min(max) {
            if !buf.is_empty() {
                buf.push('\n');
            }
            buf.push_str(app.logs.entries.get(i).map(|s| s.as_str()).unwrap_or(""));
        }

        // Render concatenated text with word wrapping
        let p = Paragraph::new(redact_sensitive(&buf))
            .block(Block::default().borders(Borders::NONE))
            .wrap(Wrap { trim: false })
            .style(app.ctx.theme.text_primary_style());
        frame.render_widget(p, area);
    }
}

impl Component for LogDetailsComponent {
    fn handle_key_events(&mut self, app: &mut App, key: crossterm::event::KeyEvent) -> Vec<Effect> {
        let mut effects = Vec::with_capacity(1);

        // keys not requiring the detail
        match key.code {
            KeyCode::Esc => {
                app.logs.detail = None;
                effects.push(Effect::CloseModal);
            }
            KeyCode::Char('c') => {
                effects.push(Effect::CopyLogsRequested(build_copy_text(app)));
            }
            _ => {}
        }

        if let Some(LogDetailView::Table { offset }) = app.logs.detail {
            let detail_json = app.logs.cached_redacted_json.as_ref().or_else(|| {
                let (start, end) = app.logs.selection.range();
                if start == end {
                    app.logs.rich_entries.get(start).and_then(|entry| match entry {
                        LogEntry::Api { json: Some(value), .. } | LogEntry::Mcp { json: Some(value), .. } => Some(value),
                        _ => None,
                    })
                } else {
                    None
                }
            });

            if let Some(json) = detail_json {
                let entries = build_key_value_entries(json);
                if !entries.is_empty() {
                    let max_index = entries.len().saturating_sub(1);
                    match key.code {
                        KeyCode::Up => {
                            let next_offset = offset.saturating_sub(1);
                            app.logs.detail = Some(LogDetailView::Table { offset: next_offset });
                        }
                        KeyCode::Down => {
                            let next_offset = offset.saturating_add(1).min(max_index);
                            app.logs.detail = Some(LogDetailView::Table { offset: next_offset });
                        }
                        _ => {}
                    }
                }
            }
        }

        effects
    }

    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let title = "Log Details";
        let block = theme_helpers::block(&*app.ctx.theme, Some(title), true);
        // Clear the modal area and render the border
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        // Render the main detail content
        self.render_detail_content(frame, inner, app);
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        build_hint_spans(theme, &[("Esc", " Close  "), ("C", " Copy  ")])
    }
}
