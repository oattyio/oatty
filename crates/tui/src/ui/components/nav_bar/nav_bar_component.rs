use super::VerticalNavBarState;
use crate::ui::components::{Component, find_target_index_by_mouse_position};
use crate::{
    app::App,
    ui::theme::theme_helpers::{self as th, ButtonRenderOptions, render_button},
};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use heroku_types::Effect;
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
                    effects.push(Effect::SwitchTo(item.route.clone()));
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
    /// updates the application's state based on the button's associated route and focus flag.
    ///
    /// # Arguments
    ///
    /// * `app` - A mutable reference to the application state, allowing the method to update routes and focus.
    /// * `mouse` - The mouse event containing details like the type of event and the location of the mouse cursor.
    ///
    /// # Returns
    ///
    /// A vector of `Effect` objects. Currently, it always returns an empty vector.
    ///
    /// # Behavior
    ///
    /// - When the `mouse` event is of kind `MouseEventKind::Down(MouseButton::Left)`:
    ///   - It calculates if the mouse click corresponds to any button in the navigation bar by using the `find_button_index_from_rect` method.
    ///   - If the index of the button is found:
    ///     - Updates the application's current route using `app.set_current_route` with the button's associated route.
    ///     - Updates the application's focus state using `app.focus.focus` with the button's associated focus flag, if applicable.
    /// - If no button is clicked, or the event is not a left-mouse down event, the application state remains unchanged.
    ///
    /// # Notes
    ///
    /// - Mouse coordinates (`column` and `row`) are used to map the event to specific button areas.
    /// - `find_button_index_from_rect` is a helper method assumed to determine the index of the button in the navigation bar based on the button areas.
    /// - The method relies on the navigation bar state (`app.nav_bar`) to resolve button areas, item routes, and focus flags.
    ///
    /// # Example
    ///
    /// ```
    /// // Assuming `app` is your application state and `mouse_event` is a MouseEvent:
    /// let effects = handle_mouse_events(&mut app, mouse_event);
    /// // The application state might now have an updated current route and focus state if a navigation bar button was clicked.
    /// ```
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
                effects.push(Effect::SwitchTo(item.route.clone()));
            }

            if let Some(flag) = app.nav_bar.item_focus_flags.get(idx) {
                app.focus.focus(flag);
            }
        }
        effects
    }

    /// Renders the navigation bar widget within the given area using the provided `Frame`.
    ///
    /// # Parameters
    /// - `frame`: The frame context used to render widgets.
    /// - `area`: The screen area where the navigation bar is rendered.
    /// - `app`: The application state, containing the navigation bar and theme styling.
    ///
    /// # Description
    /// This function is responsible for rendering a navigation bar and its associated icons and
    /// focus/hover interactions. If the navigation bar is not visible (`app.nav_bar.visible` is `false`),
    /// the function returns early without rendering anything. The rendering process consists of the
    /// following steps:
    ///
    /// 1. **Outer Block**:
    ///    - A styled boundary (block) is rendered around the entire navigation bar.
    ///    - The block's styling reflects theme settings and an active focus state if any item is focused.
    ///
    /// 2. **Inner Layout**:
    ///    - Organizes the navigation bar items into equal-height rows, stacked vertically.
    ///    - Each row displays an item with its icon and responds to hover/focus interactions.
    ///
    /// 3. **Item Rendering**:
    ///    - Each item is rendered as a button-like widget. When an item's `is_focused` or `is_selected`
    ///      states are active, appropriate visual indicators (borders) are applied.
    ///    - The item's layout is derived from the calculated `regions`, ensuring proper placement and sizing.
    ///
    /// # Focus and Interactions
    /// - Focus states for navigation bar items are determined using `app.nav_bar.item_focus_flags`.
    /// - These states, along with the selected index, drive both visual feedback and interactive behavior.
    ///
    /// # Notes
    /// If the navigation bar has no items (`app.nav_bar.items.is_empty()`), the function exits early
    /// without rendering further content.
    ///
    /// Any rendering issues related to insufficient area size are handled by clamping or skipping
    /// rows, ensuring stability even in constrained layouts.
    ///
    /// # Safety
    /// The function assumes a one-to-one correspondence between navigation bar items and layout rows
    /// (`regions`). Ensure the screen area provided is enough to accommodate all items.
    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        if !app.nav_bar.visible {
            return;
        }

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

        let nav_bar_items = self.get_preferred_layout(app, area);
        // Track layout for focus/mouse integration
        for (index, item) in app.nav_bar.items.iter().enumerate() {
            let is_selected = index == app.nav_bar.selected_index;
            let is_focused = app.nav_bar.item_focus_flags.get(index).map(|flag| flag.get()).unwrap_or_default();
            // Safety: chunks length equals row_count; clamp if a small area.
            if let Some(row_area) = nav_bar_items.get(index).copied() {
                // reversal of selected and focus intentional
                let borders = if is_focused { Borders::ALL } else { Borders::NONE };
                render_button(
                    frame,
                    row_area,
                    &item.icon,
                    theme,
                    ButtonRenderOptions::new(true, is_focused, is_selected, borders, false),
                );
            }
        }
        app.nav_bar.last_area = area;
        app.nav_bar.per_item_areas = nav_bar_items
    }

    /// Generates a list of rectangular layout regions based on the preferred layout configuration.
    ///
    /// This function calculates a vertical layout for the given area by dividing it into multiple
    /// rectangular regions. Each region's height is determined by the length specified for each
    /// row in the navigation bar state.
    ///
    /// # Parameters
    /// - `&self`: A reference to the instance of the type implementing this method.
    /// - `app: &App`: The application state, containing details like the navigation bar configuration.
    /// - `area: Rect`: The available rectangular area within which the layout should be computed.
    ///
    /// # Returns
    /// A vector of `Rect` representing the split layout configuration. Each `Rect` corresponds
    /// to a section of the area divided based on the navigation bar's row count.
    ///
    /// # Behavior
    /// 1. Reads the navigation bar's state (`nav_bar`) from the application state.
    /// 2. Determines the number of rows (or items) in the navigation bar.
    /// 3. Creates a list of layout constraints, where each row is allocated a fixed height of 3 units.
    /// 4. Configures a vertical layout (`Direction::Vertical`) with the specified constraints and a margin of 1.
    /// 5. Splits the provided `area` rectangle into sections as defined by the layout configuration.
    ///
    /// # Example
    /// ```rust
    /// let app = App {
    ///     nav_bar: NavBarState {
    ///         items: vec!["Home", "Settings", "About"],
    ///     },
    /// };
    /// let area = Rect::new(0, 0, 100, 30);
    ///
    /// let layout = get_preferred_layout(&app, area);
    /// assert_eq!(layout.len(), 3); // One `Rect` for each navigation item
    /// ```
    ///
    /// # Notes
    /// The margin of 1 unit is applied around the layout, which reduces the usable space for
    /// the `area` accordingly before splitting it into sections.
    ///
    /// # Dependencies
    /// This function assumes the use of a `Layout` object, configured with a direction, constraints,
    /// margin, and the ability to split a rectangle into multiple.
    fn get_preferred_layout(&self, app: &App, area: Rect) -> Vec<Rect> {
        let row_count = app.nav_bar.items.len();
        let constraints = vec![Constraint::Length(3); row_count];
        Layout::vertical(constraints).margin(1).split(area).to_vec()
    }
}
