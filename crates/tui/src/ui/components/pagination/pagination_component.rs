use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use heroku_types::Effect;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::*,
    text::{Line, Span},
    widgets::*,
};

use crate::ui::components::find_target_index_by_mouse_position;
use crate::ui::components::pagination::state::PaginationState;
use crate::{
    app::App,
    ui::{
        components::component::Component,
        theme::{roles::Theme as UiTheme, theme_helpers as th},
    },
};

/// Pagination component for range-based navigation and controls.
///
/// This component provides a comprehensive pagination interface with:
/// - Range field selection using a List widget
/// - Range start/end input fields for filtering
/// - Navigation controls (first/prev/next/last)
/// - Page information display
/// - Integration with the existing theme system
/// - Keyboard navigation and focus management
///
/// The component supports both traditional page-based pagination and
/// range-based pagination for API endpoints that support cursor-based
/// navigation.
#[derive(Debug, Default)]
pub struct PaginationComponent;
impl PaginationComponent {
    /// Synchronize navigation availability with the latest application pagination state.
    pub fn sync_navigation_state(&mut self, app: &mut App) {
        let pagination_state = &mut app.table.pagination_state;
        let history_depth = app.pagination_history.len();
        pagination_state.current_page = history_depth.saturating_sub(1);
        pagination_state.prev_available = history_depth > 1;

        if let Some(pagination) = &app.last_pagination {
            pagination_state.next_range = pagination.next_range.clone();
        } else {
            pagination_state.next_range = None;
        }
        pagination_state.next_available = pagination_state.next_range.is_some();
    }

    /// Renders the range display (read-only) values.
    ///
    /// Shows the current range field and start/end values as display-only
    /// widgets without any interactive input.
    fn render_range_controls(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let PaginationState {
            range_mode,
            field,
            range_start,
            range_end,
            ..
        } = &app.table.pagination_state;
        if !*range_mode {
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(25), // Field
                Constraint::Length(1),  // Spacer
                Constraint::Length(25), // Start
                Constraint::Length(1),  // Spacer
                Constraint::Length(25), // End
            ])
            .split(area);

        let theme = &*app.ctx.theme;
        // Field (display-only)
        let field_label = "Field";
        let field = Paragraph::new(field.as_str())
            .block(
                Block::default()
                    .title(Line::from(vec![Span::styled(field_label, theme.text_secondary_style())]))
                    .borders(Borders::ALL)
                    .border_style(theme.border_style(false)),
            )
            .style(theme.text_primary_style());
        frame.render_widget(field, chunks[0]);

        // Start (display-only)
        self.render_range_input(frame, chunks[2], "Start", range_start, false, theme);
        // End (display-only)
        self.render_range_input(frame, chunks[4], "End", range_end, false, theme);
    }

    /// Renders a styled range input widget within a specified area of the UI frame.
    ///
    /// This function displays a labeled input field with a given value, applying specific styles
    /// based on whether the input is focused. The appearance and visual elements of the input
    /// field are determined by the provided `UiTheme`.
    ///
    /// # Arguments
    ///
    /// * `frame` - A mutable reference to the `Frame` object that handles rendering widgets on
    ///   the terminal UI.
    /// * `area` - A `Rect` specifying the area within the UI frame where the input widget should
    ///   be rendered.
    /// * `label` - A string slice representing the label/title of the input widget.
    /// * `value` - A string slice representing the current value of the input field to be displayed.
    /// * `focused` - A boolean flag indicating whether the input field is currently focused. This
    ///   affects the styling of the input field.
    /// * `theme` - A reference to a `UiTheme` trait object that provides style settings for the
    ///   input field, such as text and border styles.
    ///
    /// # Behavior
    ///
    /// - The `label` is styled differently based on the `focused` state. A focused input will use
    ///   the theme's `accent_emphasis_style`, while a non-focused input will use the theme's
    ///   `text_secondary_style`.
    /// - The input value is styled using the theme's `text_primary_style`.
    /// - A bordered block encompasses
    fn render_range_input(&self, frame: &mut Frame, area: Rect, label: &str, value: &str, focused: bool, theme: &dyn UiTheme) {
        let title_style = if focused {
            theme.accent_emphasis_style()
        } else {
            theme.text_secondary_style()
        };

        let input = Paragraph::new(value)
            .block(
                Block::default()
                    .title(Line::from(vec![Span::styled(label, title_style)]))
                    .borders(Borders::ALL)
                    .border_style(theme.border_style(focused)),
            )
            .style(theme.text_primary_style());

        frame.render_widget(input, area);
    }

    /// Renders a horizontal divider line.
    ///
    /// Creates a simple horizontal line using the muted text style
    /// to separate different sections of the pagination interface.
    ///
    /// # Arguments
    /// * `frame` - The ratatui frame to render into
    /// * `area` - The rectangular area to render within
    /// * `theme` - The theme to use for styling
    fn render_divider(&self, frame: &mut Frame, area: Rect, theme: &dyn UiTheme) {
        let divider_style = Style::default().fg(theme.roles().divider);
        let divider = Line::from(vec![Span::styled("─".repeat(area.width as usize), divider_style)]);
        let paragraph = Paragraph::new(divider);
        frame.render_widget(paragraph, area);
    }

    /// Renders the navigation controls with page information.
    ///
    /// Creates the bottom navigation bar with:
    /// - First/Previous/Next/Last navigation buttons
    /// - Current page information display
    /// - Range information when in range mode
    /// - Proper button styling based on availability and focus state
    ///
    /// # Arguments
    /// * `frame` - The ratatui frame to render into
    /// * `area` - The rectangular area to render within
    /// * `app` - The application state for theme and focus information
    fn render_navigation_controls(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        self.sync_navigation_state(app);
        let PaginationState {
            nav_first_f,
            nav_prev_f,
            nav_next_f,
            nav_last_f,
            ..
        } = &app.table.pagination_state;
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(8), // First button
                Constraint::Length(1), // Spacer
                Constraint::Length(8), // Prev button
                Constraint::Length(1), // Spacer
                Constraint::Min(0),    // Page info
                Constraint::Length(1), // Spacer
                Constraint::Length(8), // Next button
                Constraint::Length(1), // Spacer
                Constraint::Length(8), // Last button
            ])
            .split(area);

        let has_prev = app.table.pagination_state.prev_available;
        let has_next_page = app.table.pagination_state.next_available;
        // First page button
        self.render_nav_button(frame, chunks[0], "First", has_prev, nav_first_f.get(), app);
        // Previous page button
        self.render_nav_button(frame, chunks[2], "Prev", has_prev, nav_prev_f.get(), app);
        // Page info
        self.render_page_info(frame, chunks[4], app);
        // Next page button
        self.render_nav_button(frame, chunks[6], "Next", has_next_page, nav_next_f.get(), app);
        // Last page button
        self.render_nav_button(frame, chunks[8], "Last", has_next_page, nav_last_f.get(), app);

        app.table.pagination_state.last_area = area;
        app.table.pagination_state.per_item_areas = vec![chunks[0], chunks[2], chunks[6], chunks[8]];
    }

    /// Renders the page information display.
    ///
    /// Shows current page information and range details when applicable.
    ///
    /// # Arguments
    /// * `frame` - The ratatui frame to render into
    /// * `area` - The rectangular area to render within
    /// * `theme` - The theme to use for styling
    fn render_page_info(&self, frame: &mut Frame, area: Rect, app: &App) {
        let info_text = if app.table.pagination_state.range_mode {
            format!(" | {}", app.table.pagination_state.range_info())
        } else {
            String::new()
        };

        let info_paragraph = Paragraph::new(info_text)
            .style(app.ctx.theme.text_secondary_style())
            .alignment(Alignment::Center);
        frame.render_widget(info_paragraph, area);
    }
    /// Renders a navigation button within the provided area on the frame with a specified label,
    /// visual style configurations, and state settings.
    ///
    /// # Arguments
    ///
    /// * `frame` - A mutable reference to the [`Frame`] used for rendering the button.
    /// * `area` - A [`Rect`] that defines the area in which the button will be rendered.
    /// * `label` - A string slice representing the label to display on the button.
    /// * `enabled` - A boolean indicating if the button is enabled. If `false`, the button will
    ///   be rendered in a "disabled" style.
    /// * `focused` - A boolean indicating if the button is focused. When `true`, the button will
    ///   have additional styling (e.g., border highlighting) to indicate focus.
    /// * `theme` - A reference to an implementation of the [`UiTheme`] trait, used to apply theme-specific
    ///   styles to the button.
    ///
    /// # Behavior
    ///
    /// - The function dynamically applies different styles to the button depending on whether it is `enabled`
    ///   or not:
    ///   - If `enabled` is `true`, the button will use the secondary style provided by the theme.
    ///   - If `enabled` is `false`, the button will apply a muted and dimmed style for the label and border.
    /// - When `enabled` and `focused` are both `true`, the border style will indicate focus. Enabling focus does
    ///   not modify the text's size, maintaining a stable button size.
    /// - The button's appearance is determined using elements such as text alignment, borders, and theme-based styles.
    ///
    /// # Example
    ///
    /// Here's an example of how you might use this function in a UI rendering loop:
    ///
    /// ```rust
    /// use some_ui_library::{Frame, Rect, UiTheme, MyCustomTheme};
    ///
    /// let frame = &mut some_frame;
    /// let area = Rect::new(0, 0, 10, 3);
    /// let label = "Click Me";
    /// let is_enabled = true;
    /// let is_focused = true;
    /// let theme: &dyn UiTheme = &MyCustomTheme::default();
    ///
    /// my_ui_component.render_nav_button(frame, area, label, is_enabled, is_focused, theme);
    /// ```
    ///
    /// This will render a navigation button labeled "Click Me" in the specified rectangular area.
    /// The button will be styled and focused according to the specified `theme`.
    ///
    /// [`Frame`]: <Reference_to_Frame_type>
    /// [`Rect`]: <Reference_to_Rect_type>
    /// [`UiTheme`]: <Reference_to_UiTheme_trait>
    fn render_nav_button(&self, frame: &mut Frame, area: Rect, label: &str, enabled: bool, focused: bool, app: &App) {
        let theme = &*app.ctx.theme;
        let button_style = if enabled {
            // Keep size stable on focus by avoiding bold; rely on border color
            // for focus indication.
            th::button_secondary_style(theme, true, false)
        } else {
            // Stronger disabled styling: muted text + dim
            theme.text_muted_style().add_modifier(Modifier::DIM)
        };

        let border_style = if enabled {
            theme.border_style(focused)
        } else {
            // Muted border for disabled state
            theme.text_muted_style()
        };

        let button = Paragraph::new(label)
            .block(Block::default().borders(Borders::ALL).border_style(border_style))
            .style(button_style)
            .alignment(Alignment::Center);

        frame.render_widget(button, area);
    }

    /// Handles navigation button actions.
    ///
    /// Processes left/right arrow keys and home/end keys for
    /// page navigation when the navigation area has focus.
    ///
    /// # Arguments
    /// * `event` - The key event that triggered the navigation
    fn handle_navigation_actions(&mut self, event: &KeyEvent, app: &mut App) -> Option<Effect> {
        let pagination_state = &mut app.table.pagination_state;
        match event.code {
            KeyCode::Left => {
                if pagination_state.prev_available {
                    pagination_state.prev_page();
                    Some(Effect::PrevPageRequested)
                } else {
                    None
                }
            }
            KeyCode::Right => {
                // Use Raw Next-Range header to request the next page when available
                if pagination_state.next_available {
                    pagination_state.next_range.clone().map(|next_range| {
                        pagination_state.current_page = pagination_state.current_page.saturating_add(1);
                        pagination_state.prev_available = true;
                        Effect::NextPageRequested(next_range)
                    })
                } else {
                    None
                }
            }
            KeyCode::End => {
                if pagination_state.next_available {
                    pagination_state.last_page();
                    Some(Effect::LastPageRequested)
                } else {
                    None
                }
            }
            KeyCode::Home => {
                if pagination_state.prev_available {
                    pagination_state.first_page();
                    Some(Effect::FirstPageRequested)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    // Removed: text input handling for range values (now display-only)
}

impl Component for PaginationComponent {
    /// Handles keyboard events for the pagination component.
    ///
    /// Processes keyboard input to manage:
    /// - Focus navigation between controls
    /// - Range field list navigation
    /// - Navigation button actions
    /// - Text input for range values
    ///
    /// # Arguments
    /// * `_app` - The application state (unused in current implementation)
    /// * `event` - The keyboard event to process
    ///
    /// # Returns
    /// Vector of effects to be processed by the application
    fn handle_key_events(&mut self, app: &mut App, event: KeyEvent) -> Vec<Effect> {
        let mut effects = vec![];
        if !app.table.pagination_state.is_visible {
            return vec![];
        }

        self.sync_navigation_state(app);

        match event.code {
            KeyCode::BackTab => {
                app.focus.prev();
            }
            KeyCode::Tab => {
                app.focus.next();
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                let pagination_state = &app.table.pagination_state;
                if (pagination_state.nav_first_f.get()
                    || pagination_state.nav_prev_f.get()
                    || pagination_state.nav_next_f.get()
                    || pagination_state.nav_last_f.get())
                    && let Some(effect) = self.handle_navigation_actions(&event, app)
                {
                    effects.push(effect);
                }
            }
            KeyCode::Enter => {
                let pagination_state = &mut app.table.pagination_state;
                // Activate the focused nav button
                if pagination_state.nav_first_f.get() && pagination_state.prev_available {
                    pagination_state.first_page();
                    effects.push(Effect::FirstPageRequested);
                }
                if pagination_state.nav_prev_f.get() && pagination_state.prev_available {
                    pagination_state.prev_page();
                    effects.push(Effect::PrevPageRequested);
                }
                if pagination_state.nav_next_f.get()
                    && pagination_state.next_available
                    && let Some(next_range) = pagination_state.next_range.clone()
                {
                    pagination_state.current_page = pagination_state.current_page.saturating_add(1);
                    pagination_state.prev_available = true;
                    effects.push(Effect::NextPageRequested(next_range));
                }
                if pagination_state.nav_last_f.get() && pagination_state.next_available {
                    effects.push(Effect::LastPageRequested);
                }
            }
            _ => {}
        }

        effects
    }

    /// Handles mouse events for navigating through a paginated UI.
    ///
    /// # Parameters
    /// - `app`: Mutable reference to the `App` instance, representing the application state.
    /// - `mouse`: The `MouseEvent` object containing details about the mouse event, such as column, row,
    ///   and event kind (e.g., `MouseEventKind::Down`).
    ///
    /// # Returns
    /// - A `Vec<Effect>` representing any effects generated by the mouse event. If no buttons are clicked
    ///   or the mouse event doesn't correspond to valid navigation actions, an empty vector is returned.
    ///
    /// # Behavior
    /// - The method checks if the mouse event corresponds to a left mouse button click. If true, it determines
    ///   the button index (`maybe_idx`) based on the mouse position relative to UI button areas (`state.last_area`
    ///   and `state.per_item_areas`).
    /// - If a valid button index (`idx`) is derived:
    ///   - It checks whether the navigation button is enabled, considering whether there are previous
    ///     or next pages (`has_next_page`, `has_prev_page`).
    ///   - If the button is enabled:
    ///     - Executes the corresponding navigation function (`nav_first_f`, `nav_prev_f`, `nav_next_f`, `nav_last_f`)
    ///       based on the button index.
    ///     - Simulates a key press event (`Enter`) to trigger the navigation logic.
    /// - Returns an empty vector if no valid button index is found or if the button is disabled.
    ///
    /// # Notes
    /// - This function relies on helper functions to determine button index (`find_button_index_from_rect`)
    ///   and simulate key press events (`handle_key_events`).
    /// - The button indices and their mapping to navigation functionality are as follows:
    ///   - Index `0`: Navigate to the first page.
    ///   - Index `1`: Navigate to the previous page.
    ///   - Index `2`: Navigate to the next page.
    ///   - Index `3`: Navigate to the last page.
    ///
    /// # Example
    /// Consider a scenario where a user clicks on a "Next Page" button (corresponding to index `2`):
    ///
    /// ```
    /// let mouse_event = MouseEvent::new(MouseEventKind::Down(MouseButton::Left), column, row, 0);
    /// let effects = my_handler.handle_mouse_events(&mut app, mouse_event);
    /// assert!(!effects.is_empty()); // Navigation logic is triggered.
    /// ```
    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        self.sync_navigation_state(app);
        let pagination_state = &app.table.pagination_state;
        let maybe_idx = if pagination_state.is_visible && mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            let MouseEvent { column, row, .. } = mouse;
            find_target_index_by_mouse_position(&pagination_state.last_area, &pagination_state.per_item_areas, column, row)
        } else {
            None
        };

        if let Some(idx) = maybe_idx {
            let has_next_page = pagination_state.next_available;
            let has_prev = pagination_state.prev_available;
            let enabled = idx < 2 && has_prev || idx > 1 && has_next_page;
            if !enabled {
                return Vec::new();
            }
            let ordered_f = [
                &pagination_state.nav_first_f,
                &pagination_state.nav_prev_f,
                &pagination_state.nav_next_f,
                &pagination_state.nav_last_f,
            ][idx];
            ordered_f.set(true);
            return self.handle_key_events(app, KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        }
        Vec::new()
    }

    /// Renders the pagination controls using the app's focus system for styling.
    ///
    /// This method renders the complete pagination interface including
    /// - Range controls (field selection and input fields)
    /// - Visual divider
    /// - Navigation controls with page information
    ///
    /// The rendering uses the app's focus system to style focused elements
    /// appropriately based on the current focus state.
    ///
    /// # Arguments
    /// * `frame` - The ratatui frame to render into
    /// * `area` - The rectangular area to render within
    /// * `app` - The application state for theme and focus information
    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        if !app.table.pagination_state.is_visible {
            return;
        }

        let chunks = self.get_preferred_layout(app, area);

        self.render_range_controls(frame, chunks[0], app);
        self.render_divider(frame, chunks[1], &*app.ctx.theme);
        self.render_navigation_controls(frame, chunks[2], app);
    }

    fn get_hint_spans(&self, app: &App, is_root: bool) -> Vec<Span<'_>> {
        let pagination_state = &app.table.pagination_state;
        if !pagination_state.is_visible {
            return Vec::new();
        }

        let nav_focused = pagination_state.nav_first_f.get()
            || pagination_state.nav_prev_f.get()
            || pagination_state.nav_next_f.get()
            || pagination_state.nav_last_f.get();
        if !nav_focused {
            return Vec::new();
        }

        let theme = &*app.ctx.theme;
        let mut spans = Vec::new();
        if is_root {
            spans.push(Span::styled("Pagination: ", theme.text_muted_style()));
        }
        spans.extend([
            Span::styled("←/→", theme.accent_emphasis_style()),
            Span::styled(" navigate  ", theme.text_muted_style()),
            Span::styled("Home/End", theme.accent_emphasis_style()),
            Span::styled(" jump  ", theme.text_muted_style()),
            Span::styled("Enter", theme.accent_emphasis_style()),
            Span::styled(" select", theme.text_muted_style()),
        ]);
        spans
    }

    fn get_preferred_layout(&self, _app: &App, area: Rect) -> Vec<Rect> {
        Layout::vertical([
            Constraint::Length(3), // Range display
            Constraint::Length(1), // Divider
            Constraint::Length(3), // Navigation controls
        ])
        .split(area)
        .to_vec()
    }
}
