//! Logs component for application logs and statuses.
//!
//! This component wraps the logs widget in a Component so it can be
//! orchestrated by the TEA root with a consistent API.

use ratatui::{
    Frame,
    layout::Rect,
    text::Span,
    widgets::{Block, Borders, List, ListItem},
};

use crate::{app, theme, ui::components::component::Component};

#[derive(Default)]
pub struct LogsComponent;

impl LogsComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for LogsComponent {
    fn render(&mut self, f: &mut Frame, rect: Rect, app: &mut app::App) {
        let block = Block::default()
            .title(Span::styled(
                format!("Logs ({})", app.logs.entries.len()),
                theme::title_style(),
            ))
            .borders(Borders::ALL)
            .border_style(theme::border_style(false));

        let items: Vec<ListItem> = app
            .logs
            .entries
            .iter()
            .map(|l| ListItem::new(l.as_str()).style(theme::text_style()))
            .collect();

        let list = List::new(items).block(block);
        f.render_widget(list, rect);
    }
}
