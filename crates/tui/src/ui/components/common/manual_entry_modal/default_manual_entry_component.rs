use crossterm::event::{KeyCode, KeyEvent};
use oatty_types::{Effect, Msg};
use ratatui::{Frame, layout::Rect, text::Span};

use crate::{
    app::App,
    ui::components::{Component, common::ManualEntryView},
};

#[derive(Debug, Default)]
pub struct DefaultManualEntryComponent {
    inner: ManualEntryView,
}

impl Component for DefaultManualEntryComponent {
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let Some(state) = app.manual_entry_state.as_mut() else {
            return;
        };
        self.inner.render_with_state(frame, rect, &*app.ctx.theme, state);
    }

    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        if key.code == KeyCode::Esc {
            return vec![Effect::CloseModal];
        }
        let Some(state) = app.manual_entry_state.as_mut() else {
            return Vec::new();
        };

        match self.inner.handle_key_events(state, key) {
            Ok(Some(_)) => return vec![Effect::CloseModal, Effect::SendMsg(Msg::ManualEntryModalClosed)],
            Err(error) => state.set_error(format!("{}", error)),
            Ok(None) => {}
        }

        Vec::new()
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: crossterm::event::MouseEvent) -> Vec<Effect> {
        let Some(state) = app.manual_entry_state.as_mut() else {
            return Vec::new();
        };

        self.inner.handle_mouse_events(state, mouse)
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let Some(state) = app.manual_entry_state.as_ref() else {
            return Vec::new();
        };
        self.inner.get_hint_spans(&*app.ctx.theme, state)
    }
}
