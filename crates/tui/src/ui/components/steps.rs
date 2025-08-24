//! Workflow steps component.
//!
//! This component is the placeholder shell for a future Workflow Steps
//! panel. It conforms to the Component trait and can later be extended
//! to visualize steps/progress for workflows or multi-stage operations.

use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::{app, component::Component, theme};

#[derive(Default)]
pub struct StepsComponent;

impl StepsComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for StepsComponent {
    fn render(&mut self, f: &mut Frame, rect: Rect, _app: &mut app::App) {
        let block = Block::default()
            .title("Workflow Steps")
            .borders(Borders::ALL)
            .border_style(theme::border_style(false));
        let content = Paragraph::new("No workflow in progress. Start a run to see steps here.")
            .block(block)
            .style(theme::text_muted());
        f.render_widget(content, rect);
    }
}

