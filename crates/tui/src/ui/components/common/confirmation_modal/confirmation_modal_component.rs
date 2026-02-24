use crate::app::App;
use crate::ui::components::Component;
use crate::ui::theme::theme_helpers::{ButtonRenderOptions, block_with_severity, build_hint_spans, render_button};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use oatty_types::{Effect, Msg};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::prelude::Span;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

#[derive(Default, Debug, Clone)]
pub struct ConfirmationModal {
    button_areas: Vec<Rect>,
}

impl Component for ConfirmationModal {
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let mut effects = Vec::new();
        match key.code {
            KeyCode::Tab => {
                app.focus.next();
            }
            KeyCode::BackTab => {
                app.focus.prev();
            }
            KeyCode::Enter => {
                if let Some(button) = app.confirmation_modal_state.buttons().iter().find(|button| button.focus.get()) {
                    effects.extend([
                        Effect::CloseModal,
                        Effect::SendMsg(Msg::ConfirmationModalButtonClicked(button.focus.widget_id())),
                    ]);
                }
            }
            KeyCode::Esc => effects.extend([Effect::CloseModal, Effect::SendMsg(Msg::ConfirmationModalClosed)]),
            _ => {}
        }
        effects
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let MouseEvent { kind, column, row, .. } = mouse;
        if kind == MouseEventKind::Down(MouseButton::Left) {
            let position = Position::new(column, row);
            let button_index = self.button_areas.iter().position(|area| area.contains(position));
            if let Some(index) = button_index {
                let button_id = app.confirmation_modal_state.buttons()[index].focus.widget_id();
                return vec![Effect::CloseModal, Effect::SendMsg(Msg::ConfirmationModalButtonClicked(button_id))];
            }
        }
        vec![]
    }

    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let theme = &*app.ctx.theme;
        let confirmation_modal_state = &app.confirmation_modal_state;
        let title = confirmation_modal_state.title().unwrap_or_default();
        let severity = confirmation_modal_state.r#type().unwrap_or_default();
        let block = block_with_severity(theme, severity, Some(title), true);
        let inner = block.inner(rect);

        frame.render_widget(&block, rect);

        let [message_rect, _, button_rect, ..] = self.get_preferred_layout(app, inner)[..] else {
            return;
        };

        if let Some(message) = confirmation_modal_state.message() {
            let lines = message
                .lines()
                .map(|line| Line::from(Span::from(line.to_string())))
                .collect::<Vec<Line>>();
            let paragraph = Paragraph::new(lines).block(Block::default()).wrap(Wrap { trim: false });
            frame.render_widget(paragraph, message_rect);
        }

        let buttons = confirmation_modal_state.buttons();
        let width: u16 = 12;
        let spacer: u16 = 2;
        let mut button_areas = Vec::with_capacity(buttons.len());
        for (i, button) in buttons.iter().enumerate() {
            let mult = i as u16;
            let x = button_rect.x;
            let rect = Rect::new(x + (mult * width + mult * spacer), button_rect.y, width, button_rect.height);
            render_button(
                frame,
                rect,
                button.label.as_str(),
                theme,
                ButtonRenderOptions::new(
                    true,
                    confirmation_modal_state.is_button_focused(i),
                    false,
                    Borders::ALL,
                    button.button_type,
                ),
            );
            button_areas.push(rect);
        }

        self.button_areas = button_areas;
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        build_hint_spans(
            &*app.ctx.theme,
            &[("Tab/Shift+Tab", " Focus "), ("Enter", " Confirm "), ("Esc", " close/cancel")],
        )
    }

    fn get_preferred_layout(&self, app: &App, area: Rect) -> Vec<Rect> {
        let has_message = app.confirmation_modal_state.message().is_some();
        let outer = Layout::vertical([
            Constraint::Min(if has_message { 1 } else { 0 }), // Message
            Constraint::Length(1),                            // Spacer
            Constraint::Length(3),                            // Buttons
        ]);

        outer.split(area).to_vec()
    }
}
