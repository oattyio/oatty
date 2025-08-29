//! Logs hint bar showing keyboard shortcuts when logs are focused.

use ratatui::style::Style;
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

// theme helpers accessed via app.ctx.theme in render
use crate::{app, ui::components::component::Component};

#[derive(Default)]
pub struct LogsHintBarComponent;

impl LogsHintBarComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for LogsHintBarComponent {
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut app::App) {
        // Only render when logs are focused
        if !matches!(app.main_focus, app::MainFocus::Logs) {
            // Render empty line to avoid stale content
            frame.render_widget(Paragraph::new(""), rect);
            return;
        }

        use crate::ui::components::logs::state::LogEntry;

        // Decide if we should show the pretty/raw toggle hint
        let mut show_pretty_toggle = false;
        if app.logs.selection.is_single() {
            let idx = app.logs.selection.cursor;
            if let Some(LogEntry::Api { json: Some(_), .. }) = app.logs.rich_entries.get(idx) {
                show_pretty_toggle = true;
            }
        }

        let t = &*app.ctx.theme;
        let mut spans: Vec<Span> = vec![
            Span::styled("Logs: ", t.text_muted_style()),
            Span::styled("↑/↓", t.accent_emphasis_style()),
            Span::styled(" move  ", t.text_muted_style()),
            Span::styled("Shift+↑/↓", t.accent_emphasis_style()),
            Span::styled(" range  ", t.text_muted_style()),
            Span::styled("Enter", t.accent_emphasis_style()),
            Span::styled(" open  ", t.text_muted_style()),
            Span::styled("c", t.accent_emphasis_style()),
            Span::styled(" copy  ", t.text_muted_style()),
        ];
        if show_pretty_toggle {
            spans.push(Span::styled("v ", t.accent_emphasis_style()));
            // Show current mode with green highlight
            if app.logs.pretty_json {
                spans.push(Span::styled("pretty", Style::default().fg(t.roles().success)));
                spans.push(Span::styled("/raw  ", t.text_muted_style()));
            } else {
                spans.push(Span::styled("pretty/", t.text_muted_style()));
                spans.push(Span::styled("raw  ", Style::default().fg(t.roles().success)));
            }
        }
        spans.push(Span::styled("Tab", t.accent_emphasis_style()));
        spans.push(Span::styled(" focus", t.text_muted_style()));

        let hints = Paragraph::new(Line::from(spans)).style(t.text_muted_style());
        frame.render_widget(hints, rect);
    }
}
