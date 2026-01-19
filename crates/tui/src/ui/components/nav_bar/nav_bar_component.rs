use super::{NavItemAction, VerticalNavBarState};
use crate::ui::components::{Component, find_target_index_by_mouse_position};
use crate::{
    app::App,
    ui::theme::theme_helpers::{self as th, ButtonRenderOptions, ButtonType, render_button},
};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use oatty_types::Effect;
use ratatui::text::Span;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    widgets::{Block, Borders},
};

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
    fn push_action_effect(effects: &mut Vec<Effect>, action: &NavItemAction) {
        let NavItemAction::Route(route) = action;
        effects.push(Effect::SwitchTo(route.clone()));
    }
}
impl Component for VerticalNavBarComponent {
    /// Handles key events for the vertical navigation bar component.
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        // Ensure a valid initial child focus when the container gains focus
        let needs_init = {
            let st = &app.nav_bar;
            st.container_focus.get() && !st.item_focus_flags.iter().any(|f| f.get())
        };
        if needs_init {
            app.focus.focus(&app.nav_bar);
        }

        let state = &mut app.nav_bar;
        let mut effects = vec![];
        match key.code {
            // Tab or BackTab is a normal focus
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
                    Self::push_action_effect(&mut effects, &item.action);
                }
            }
            _ => {}
        };
        effects
    }

    /// Handles mouse events specific to the navigation bar and modifies application state accordingly.
    ///
    /// This function processes mouse events such as clicks within the navigation bar area.
    /// It determines if a mouse click corresponds to a button in the navigation bar, and if so,
    /// updates the application's state based on the button's associated action (route or modal) and focus flag.
    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let mut effects = vec![];
        let maybe_idx = if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let VerticalNavBarState {
                last_area, per_item_areas, ..
            } = &app.nav_bar;
            let x = mouse.column;
            let y = mouse.row;
            find_target_index_by_mouse_position(last_area, per_item_areas, x, y)
        } else {
            None
        };

        if let Some(idx) = maybe_idx {
            if let Some(item) = app.nav_bar.items.get(idx).cloned() {
                app.nav_bar.selected_index = idx;
                Self::push_action_effect(&mut effects, &item.action);
            }

            if let Some(flag) = app.nav_bar.item_focus_flags.get(idx) {
                app.focus.focus(flag);
            }
        }
        effects
    }

    /// Renders the navigation bar widget within the given area using the provided `Frame`.
    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let theme = &*app.ctx.theme;
        let is_any_focused = self.any_item_focused(&app.nav_bar);

        // Outer block with theme styling; focus color when any item is focused.
        let block: Block = th::block(theme, self.title.as_deref(), is_any_focused).borders(Borders::ALL);
        frame.render_widget(block, area);

        // Inner layout: a single vertical column with equal-height rows.
        // Each row is a Paragraph with the icon centered-ish via padding.
        if app.nav_bar.items.is_empty() {
            return;
        }

        let nav_bar_item_rects = self.get_preferred_layout(app, area);
        // Track layout for focus/mouse integration
        for (index, item) in app.nav_bar.items.iter().enumerate() {
            let is_selected = index == app.nav_bar.selected_index;
            let is_focused = app.nav_bar.item_focus_flags.get(index).map(|flag| flag.get()).unwrap_or_default();
            // Safety: chunks length equals row_count; clamp if a small area.
            if let Some(row_area) = nav_bar_item_rects.get(index).copied() {
                // reversal of selected and focus intentional
                let borders = if is_focused { Borders::ALL } else { Borders::NONE };
                render_button(
                    frame,
                    row_area,
                    &item.icon,
                    theme,
                    ButtonRenderOptions::new(true, is_focused, is_selected, borders, ButtonType::Secondary),
                );
            }
        }
        app.nav_bar.last_area = area;
        app.nav_bar.per_item_areas = nav_bar_item_rects
    }

    /// Generates a list of styled `Span` elements representing UI hints based on the application context.
    ///
    /// # Parameters
    ///
    /// * `self` - The instance of the struct implementing this method.
    /// * `app` - A reference to the current `App` instance, which provides access to context and theme data.
    ///
    /// # Returns
    /// A vector of `Span` elements representing the styled text elements for the UI hints.
    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        th::build_hint_spans(&*app.ctx.theme, &[(" Enter", " Select view"), (" ↑/↓", " Navigate")]).to_vec()
    }

    fn get_preferred_layout(&self, app: &App, area: Rect) -> Vec<Rect> {
        let row_count = app.nav_bar.items.len();
        let mut constraints = Vec::with_capacity(row_count + 1);
        constraints.extend(vec![Constraint::Length(3); row_count + 1]);
        constraints[row_count - 1] = Constraint::Min(0); // Pins the last item to the bottom

        let mut layout = Layout::vertical(constraints).margin(1).split(area).to_vec();
        layout.swap_remove(layout.len() - 2); // swap_remove possible since we're using len() - 2
        layout
    }
}
