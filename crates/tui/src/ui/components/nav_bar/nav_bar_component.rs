use crate::{
    app::App,
    ui::theme::theme_helpers::{self as th, render_button},
};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use heroku_types::Effect;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders},
};

use super::VerticalNavBarState;
use crate::ui::components::component::Component;

/// A reusable vertical navigation bar component.
///
/// Renders a vertical column of icon buttons with selection and focus styling.
/// The component exposes navigation helpers and integrates with rat-focus via
/// `VerticalNavBarState`. It does not emit application-specific messages or
/// effects; consumers can map activation to their own `Msg`/`Effect` as needed.
#[derive(Debug, Default)]
pub struct VerticalNavBarComponent {
    /// Optional title for the surrounding block. When `None`, no title is shown.
    pub title: Option<String>,
}

impl VerticalNavBarComponent {
    /// Creates a new component.
    pub fn new() -> Self {
        Self {
            title: Some("Views".to_string()),
        }
    }

    /// Computes whether any item is focused (used to style borders/selection).
    fn any_item_focused(&self, state: &VerticalNavBarState) -> bool {
        state.item_focus_flags.iter().any(|f| f.get())
    }

    fn find_button_index_from_rect(rect: &Rect, targets: &Vec<Rect>, mouse_x: u16, mouse_y: u16) -> Option<usize> {
        if rect.width > 0 && rect.height > 0 {
            if let Some((idx, _)) = targets
                .iter()
                .enumerate()
                .find(|(_, r)| mouse_x >= r.x && mouse_x < r.x + r.width && mouse_y >= r.y && mouse_y < r.y + r.height)
            {
                return Some(idx);
            }
        }
        None
    }
}

impl Component for VerticalNavBarComponent {
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        // Ensure a valid initial child focus when the container gains focus
        let needs_init = {
            let st = &app.nav_bar;
            st.container_focus.get() && !st.item_focus_flags.iter().any(|f| f.get())
        };
        if needs_init {
            app.focus.first_in(&app.nav_bar);
        }

        let state = &mut app.nav_bar;
        let mut effects = vec![];
        match key.code {
            // Tab or BackTab is normal focus
            KeyCode::Tab => {
                app.focus.next();
            }
            KeyCode::BackTab => {
                app.focus.prev();
            }
            // Down or up arrow cycles back to end/start
            KeyCode::Down => {
                if let Some(flag) = state.cycle_focus(true) {
                    app.focus.by_widget_id(flag.widget_id());
                }
            }
            KeyCode::Up => {
                if let Some(flag) = state.cycle_focus(false) {
                    app.focus.by_widget_id(flag.widget_id());
                }
            }
            // Commit the selected index and route the app
            KeyCode::Enter => {
                if let Some((item, idx)) = state.get_focused_list_item() {
                    state.selected_index = idx;
                    effects.push(Effect::SwitchTo(item.route.clone()));
                }
            }
            _ => {}
        };
        effects
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let maybe_idx = if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let VerticalNavBarState {
                last_area,
                per_item_areas,
                ..
            } = &app.nav_bar;
            let x = mouse.column;
            let y = mouse.row;
            Self::find_button_index_from_rect(last_area, per_item_areas, x, y)
        } else {
            None
        };

        if let Some(idx) = maybe_idx {
            if let Some(item) = app.nav_bar.items.get(idx).cloned() {
                app.set_current_route(item.route);
            }

            if let Some(flag) = app.nav_bar.item_focus_flags.get(idx) {
                app.focus.focus(flag);
            }
        }
        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let state = &mut app.nav_bar;
        if !state.visible {
            return;
        }

        let theme = &*app.ctx.theme;
        let is_any_focused = self.any_item_focused(state);

        // Outer block with theme styling; focus color when any item is focused.
        let block: Block = th::block(theme, self.title.as_deref(), is_any_focused).borders(Borders::ALL);
        frame.render_widget(block, area);

        // Inner layout: a single vertical column with equal-height rows.
        // Each row is a Paragraph with the icon centered-ish via padding.
        if state.items.is_empty() {
            return;
        }

        // Determine row constraints: one per item.
        let row_count = state.items.len();
        let constraints = vec![Constraint::Length(3); row_count];
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .margin(1)
            .split(area);
        // Track layout for focus/mouse integration
        state.last_area = area;
        state.per_item_areas = chunks.to_vec();

        for (index, item) in state.items.iter().enumerate() {
            let is_selected = index == state.selected_index;
            let is_focused = state
                .item_focus_flags
                .get(index)
                .and_then(|f| Some(f.get()))
                .unwrap_or_default();
            // Safety: chunks length equals row_count; clamp if small area.
            if let Some(row_area) = chunks.get(index).copied() {
                // reversal of selected and focus intentional
                let borders = if is_focused { Borders::ALL } else { Borders::NONE };
                render_button(
                    frame,
                    row_area,
                    &item.icon,
                    true,
                    is_focused,
                    is_selected,
                    theme,
                    borders,
                );
            }
        }
    }
}
