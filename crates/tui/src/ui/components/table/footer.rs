use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{app, ui::components::component::Component};

#[derive(Debug, Default)]
pub struct TableFooter<'a> {
    footer: Option<Paragraph<'a>>,
}

impl TableFooter<'_> {
    pub fn footer(&mut self, app: &app::App) -> &Paragraph<'_> {
        if self.footer.is_none() {
            let _ = self.footer.insert(
                Paragraph::new(Line::from(vec![
                    Span::styled("Hint: ", app.ctx.theme.text_muted_style()),
                    Span::styled("Esc", app.ctx.theme.accent_emphasis_style()),
                    Span::styled(" close  ", app.ctx.theme.text_muted_style()),
                    Span::styled("↑/↓", app.ctx.theme.accent_emphasis_style()),
                    Span::styled(" scroll  ", app.ctx.theme.text_muted_style()),
                    Span::styled("PgUp/PgDn", app.ctx.theme.accent_emphasis_style()),
                    Span::styled(" faster  ", app.ctx.theme.text_muted_style()),
                    Span::styled("Home/End", app.ctx.theme.accent_emphasis_style()),
                    Span::styled(" jump", app.ctx.theme.text_muted_style()),
                ]))
                .style(app.ctx.theme.text_muted_style()),
            );
        }
        self.footer.as_ref().unwrap()
    }
}

impl Component for TableFooter<'_> {
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut app::App) {
        frame.render_widget(self.footer(app), rect);
    }
}
