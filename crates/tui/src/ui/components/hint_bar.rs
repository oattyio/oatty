//! Hint bar component for keyboard shortcuts and contextual help.
//!
//! This component renders the single-line hints strip that shows
//! useful key bindings and tips. It implements the shared Component
//! trait to align with the app-wide component architecture.

use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{app, component::Component, theme};

#[derive(Default)]
pub struct HintBarComponent;

impl HintBarComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for HintBarComponent {
    fn render(&mut self, f: &mut Frame, rect: Rect, _app: &mut app::App) {
        let hints = Paragraph::new(Line::from(vec![
            Span::styled("Hints: ", theme::text_muted()),
            Span::styled("↑/↓", theme::title_style().fg(theme::ACCENT)),
            Span::styled(" cycle  ", theme::text_muted()),
            Span::styled("Tab", theme::title_style().fg(theme::ACCENT)),
            Span::styled(" accept  ", theme::text_muted()),
            Span::styled("Ctrl-F", theme::title_style().fg(theme::ACCENT)),
            Span::styled(" builder  ", theme::text_muted()),
            Span::styled("Esc", theme::title_style().fg(theme::ACCENT)),
            Span::styled(" cancel", theme::text_muted()),
        ]))
        .style(theme::text_muted());
        f.render_widget(hints, rect);
    }
}
