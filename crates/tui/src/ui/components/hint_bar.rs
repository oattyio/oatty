//! Hint bar component for keyboard shortcuts and contextual help.
//!
//! This component renders the single-line hints strip that shows
//! useful key bindings and tips. It implements the shared Component
//! trait to align with the app-wide component architecture.

use ratatui::{layout::Rect, Frame};

use crate::{app, component::Component};

#[derive(Default)]
pub struct HintBarComponent;

impl HintBarComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for HintBarComponent {
    fn render(&mut self, f: &mut Frame, rect: Rect, _app: &mut app::App) {
        crate::ui::widgets::draw_hints(f, rect);
    }
}

