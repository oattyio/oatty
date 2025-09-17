//! Logs hint bar showing keyboard shortcuts when logs are focused.

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

// theme helpers accessed via app.ctx.theme in render
use crate::{app, ui::components::component::Component};

#[derive(Default, Debug)]
pub struct LogsHintBar;

impl Component for LogsHintBar {
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut app::App) {
        // Only render when logs are focused (rat-focus)
        if !app.logs.focus.get() {
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

        let theme = &*app.ctx.theme;
        let mut spans: Vec<Span> = vec![
            Span::styled("Logs: ", theme.text_muted_style()),
            Span::styled("↑/↓", theme.accent_emphasis_style()),
            Span::styled(" move  ", theme.text_muted_style()),
            Span::styled("Shift+↑/↓", theme.accent_emphasis_style()),
            Span::styled(" range  ", theme.text_muted_style()),
            Span::styled("Enter", theme.accent_emphasis_style()),
            Span::styled(" open  ", theme.text_muted_style()),
            Span::styled("c", theme.accent_emphasis_style()),
            Span::styled(" copy  ", theme.text_muted_style()),
        ];
        if show_pretty_toggle {
            spans.push(Span::styled("v ", theme.accent_emphasis_style()));
            // Show current mode with green highlight
            if app.logs.pretty_json {
                spans.push(Span::styled("pretty", Style::default().fg(theme.roles().success)));
                spans.push(Span::styled("/raw  ", theme.text_muted_style()));
            } else {
                spans.push(Span::styled("pretty/", theme.text_muted_style()));
                spans.push(Span::styled("raw  ", Style::default().fg(theme.roles().success)));
            }
        }
        spans.push(Span::styled("Tab", theme.accent_emphasis_style()));
        spans.push(Span::styled(" focus", theme.text_muted_style()));

        let hints = Paragraph::new(Line::from(spans)).style(theme.text_muted_style());
        frame.render_widget(hints, rect);
    }
}
