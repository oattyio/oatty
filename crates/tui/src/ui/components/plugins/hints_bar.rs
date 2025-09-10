use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{app::App, ui::components::component::Component};

#[derive(Debug, Default)]
pub struct PluginHintsBar<'a> {
    hints: Option<Paragraph<'a>>,
}

impl PluginHintsBar<'_> {
    fn hints(&mut self, app: &mut App) -> &Paragraph<'_> {
        if self.hints.is_none() {
            let theme = &*app.ctx.theme;
            let hints_line = Line::from(vec![
                Span::styled("Hints: ", theme.text_muted_style()),
                Span::styled("Ctrl-c", theme.accent_emphasis_style()),
                Span::styled(" palette  ", theme.text_muted_style()),
                Span::styled("Ctrl-f", theme.accent_emphasis_style()),
                Span::styled(" search  ", theme.text_muted_style()),
                Span::styled("Ctrl-k", theme.accent_emphasis_style()),
                Span::styled(" clear  ", theme.text_muted_style()),
                Span::styled("Ctrl-d", theme.accent_emphasis_style()),
                Span::styled(" details  ", theme.text_muted_style()),
                Span::styled("Ctrl-a", theme.accent_emphasis_style()),
                Span::styled(" add  ", theme.text_muted_style()),
                Span::styled("Ctrl-l", theme.accent_emphasis_style()),
                Span::styled(" logs/follow  ", theme.text_muted_style()),
                Span::styled("Ctrl-e", theme.accent_emphasis_style()),
                Span::styled(" env  ", theme.text_muted_style()),
                Span::styled("Ctrl-s", theme.accent_emphasis_style()),
                Span::styled(" start  ", theme.text_muted_style()),
                Span::styled("Ctrl-t", theme.accent_emphasis_style()),
                Span::styled(" stop  ", theme.text_muted_style()),
                Span::styled("Ctrl-r", theme.accent_emphasis_style()),
                Span::styled(" restart  ", theme.text_muted_style()),
                Span::styled("Ctrl-y", theme.accent_emphasis_style()),
                Span::styled(" copy  ", theme.text_muted_style()),
                Span::styled("Ctrl-u", theme.accent_emphasis_style()),
                Span::styled(" copy all  ", theme.text_muted_style()),
                Span::styled("Ctrl-o", theme.accent_emphasis_style()),
                Span::styled(" export  ", theme.text_muted_style()),
                Span::styled("Ctrl-b", theme.accent_emphasis_style()),
                Span::styled(" back", theme.text_muted_style()),
            ]);
            self.hints = Some(Paragraph::new(hints_line).style(theme.text_muted_style()));
        }
        self.hints.as_ref().unwrap()
    }
}

impl Component for PluginHintsBar<'_> {
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        frame.render_widget(self.hints(app), rect);
    }
}
