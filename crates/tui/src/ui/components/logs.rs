//! Logs component for application logs and statuses.
//!
//! This component wraps the logs widget in a Component so it can be
//! orchestrated by the TEA root with a consistent API.

use ratatui::{layout::Rect, Frame};

use crate::{app, component::Component};

#[derive(Default)]
pub struct LogsComponent;

impl LogsComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for LogsComponent {
    fn render(&mut self, f: &mut Frame, rect: Rect, app: &mut app::App) {
        crate::ui::widgets::draw_logs(f, app, rect);
    }
}

