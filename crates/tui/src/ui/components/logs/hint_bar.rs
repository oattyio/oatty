//! Logs hint bar showing keyboard shortcuts when logs are focused.

use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};
use ratatui::style::{Color, Style};

use crate::{app, theme, ui::components::component::Component};

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

        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::styled("Logs: ", theme::text_muted()));
        spans.push(Span::styled("↑/↓", theme::title_style().fg(theme::ACCENT)));
        spans.push(Span::styled(" move  ", theme::text_muted()));
        spans.push(Span::styled("Shift+↑/↓", theme::title_style().fg(theme::ACCENT)));
        spans.push(Span::styled(" range  ", theme::text_muted()));
        spans.push(Span::styled("Enter", theme::title_style().fg(theme::ACCENT)));
        spans.push(Span::styled(" open  ", theme::text_muted()));
        spans.push(Span::styled("c", theme::title_style().fg(theme::ACCENT)));
        spans.push(Span::styled(" copy  ", theme::text_muted()));
        if show_pretty_toggle {
            spans.push(Span::styled("v ", theme::title_style().fg(theme::ACCENT)));
            // Show current mode with green highlight
            if app.logs.pretty_json {
                spans.push(Span::styled("pretty", Style::default().fg(Color::Green)));
                spans.push(Span::styled("/raw  ", theme::text_muted()));
            } else {
                spans.push(Span::styled("pretty/", theme::text_muted()));
                spans.push(Span::styled("raw  ", Style::default().fg(Color::Green)));
            }
        }
        spans.push(Span::styled("Tab", theme::title_style().fg(theme::ACCENT)));
        spans.push(Span::styled(" focus", theme::text_muted()));

        let hints = Paragraph::new(Line::from(spans)).style(theme::text_muted());
        frame.render_widget(hints, rect);
    }
}
