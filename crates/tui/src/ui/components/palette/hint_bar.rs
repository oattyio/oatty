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
pub struct HintBarComponent<'a> {
    hints: Option<Paragraph<'a>>,
}

impl HintBarComponent<'_> {
    pub fn new() -> Self {
        HintBarComponent { hints: None }
    }

    fn hints(&mut self, app: &mut app::App) -> &Paragraph<'_> {
        if self.hints.is_none() {
            let theme = &*app.ctx.theme;
            let _ = self.hints.insert(
                Paragraph::new(Line::from(vec![
                    Span::styled("Hints: ", theme.text_muted_style()),
                    Span::styled("Tab", theme.accent_emphasis_style()),
                    Span::styled(" completions ", theme.text_muted_style()),
                    Span::styled("↑/↓", theme.accent_emphasis_style()),
                    Span::styled(" cycle  ", theme.text_muted_style()),
                    Span::styled("Enter", theme.accent_emphasis_style()),
                    Span::styled(" accept  ", theme.text_muted_style()),
                    Span::styled("Ctrl-F", theme.accent_emphasis_style()),
                    Span::styled(" builder  ", theme.text_muted_style()),
                    Span::styled("Esc", theme.accent_emphasis_style()),
                    Span::styled(" cancel", theme.text_muted_style()),
                ]))
                .style(theme.text_muted_style()),
            );
        }
        self.hints.as_ref().unwrap()
    }
}

impl Component for HintBarComponent<'_> {
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut app::App) {
        frame.render_widget(self.hints(app), rect);
    }
}
