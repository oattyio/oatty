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

use crate::{app, ui::components::component::Component};

#[derive(Default)]
pub struct HintBarComponent;

impl HintBarComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for HintBarComponent {
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut app::App) {
        let t = &*app.ctx.theme;
        let hints = Paragraph::new(Line::from(vec![
            Span::styled("Hints: ", t.text_muted_style()),
            Span::styled("↑/↓", t.accent_emphasis_style()),
            Span::styled(" cycle  ", t.text_muted_style()),
            Span::styled("Tab", t.accent_emphasis_style()),
            Span::styled(" accept  ", t.text_muted_style()),
            Span::styled("Ctrl-F", t.accent_emphasis_style()),
            Span::styled(" builder  ", t.text_muted_style()),
            Span::styled("Esc", t.accent_emphasis_style()),
            Span::styled(" cancel", t.text_muted_style()),
        ]))
        .style(t.text_muted_style());
        frame.render_widget(hints, rect);
    }
}
