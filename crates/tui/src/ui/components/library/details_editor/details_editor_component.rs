use rat_focus::HasFocus;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
};

use crate::{
    app::App,
    ui::{
        components::{Component, common::key_value_editor::KeyValueEditorView},
        theme::theme_helpers::block,
    },
};

#[derive(Default, Debug)]
pub struct DetailsEditorComponent {
    kv_view: KeyValueEditorView,
}

impl Component for DetailsEditorComponent {
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let block = block(&*app.ctx.theme, Some("Details"), app.library.is_focused());
        let inner = block.inner(rect);
        frame.render_widget(block, rect);

        let layout = self.get_preferred_layout(app, inner);
    }

    fn get_preferred_layout(&self, _app: &App, area: Rect) -> Vec<Rect> {
        Layout::vertical([
            Constraint::Min(10),         // Title + Description
            Constraint::Min(7),          // Summary
            Constraint::Percentage(100), // Headers
        ])
        .split(area)
        .to_vec()
    }
}

impl DetailsEditorComponent {}
