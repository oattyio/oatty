use crossterm::event::{KeyCode, KeyEvent};
use heroku_types::{Effect, Pagination};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::*,
    text::{Line, Span},
    widgets::*,
};

use super::state::PaginationState;
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
pub struct PaginationComponent {
    /// Internal state managing pagination data and UI state
    state: PaginationState,
}

impl PaginationComponent {
    /// Gets a reference to the pagination state.
    ///
    /// Returns a reference to the internal state that can be used to
    /// query current pagination information.
    pub fn state(&self) -> &PaginationState {
        &self.state
    }

    /// Sets the pagination configuration from a Pagination object.
    ///
    /// This method updates the component's state with pagination data
    /// from the API response, including range fields, current values,
    /// and navigation metadata.
    ///
    /// # Arguments
    /// * `pagination` - The pagination data from the API response
    pub fn set_pagination(&mut self, pagination: Pagination) {
        self.state.set_pagination(pagination);
    }

    // Removed: interactive range fields; no longer configurable by user

    /// Shows the pagination controls.
    ///
    /// Makes the pagination component visible in the UI.
    pub fn show(&mut self) {
        self.state.is_visible = true;
    }

    /// Hides the pagination controls.
    ///
    /// Makes the pagination component invisible in the UI.
    pub fn hide(&mut self) {
        self.state.is_visible = false;
    }

    /// Ensure focus is not set on disabled nav buttons.
    pub fn normalize_focus(&mut self) {
        let prev_enabled = self.state.has_prev_page();
        let next_enabled = self.state.has_next_page();
        if self.state.nav_first_f.get() && !prev_enabled {
            self.state.nav_first_f.set(false);
        }
        if self.state.nav_prev_f.get() && !prev_enabled {
            self.state.nav_prev_f.set(false);
        }
        if self.state.nav_next_f.get() && !next_enabled {
            self.state.nav_next_f.set(false);
        }
        if self.state.nav_last_f.get() && !next_enabled {
            self.state.nav_last_f.set(false);
        }
    }

    /// Renders the range display (read-only) values.
    ///
    /// Shows the current range field and start/end values as display-only
    /// widgets without any interactive input.
    fn render_range_controls(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        if !self.state.range_mode {
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
        let field = Paragraph::new(self.state.field.as_str())
            .block(
                Block::default()
                    .title(Line::from(vec![Span::styled(field_label, theme.text_secondary_style())]))
                    .borders(Borders::ALL)
                    .border_style(theme.border_style(false)),
            )
            .style(theme.text_primary_style());
        frame.render_widget(field, chunks[0]);

        // Start (display-only)
        self.render_range_input(frame, chunks[2], "Start", &self.state.range_start, false, theme);
        // End (display-only)
        self.render_range_input(frame, chunks[4], "End", &self.state.range_end, false, theme);
    }

    // Removed: interactive range field selection list (now display-only)

    /// Renders a range input field with label and value.
    ///
    /// Creates a bordered input field showing the current range value
    /// with proper focus styling.
    ///
    /// # Arguments
    /// * `frame` - The ratatui frame to render into
    /// * `area` - The rectangular area to render within
    /// * `label` - The label to display above the input field
    /// * `value` - The current value to display in the input field
    /// * `focused` - Whether this input field currently has focus
    /// * `theme` - The theme to use for styling
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
        let divider = Line::from(vec![Span::styled("─".repeat(area.width as usize), theme.text_muted_style())]);
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
    fn render_navigation_controls(&self, frame: &mut Frame, area: Rect, app: &mut App) {
        let theme = &*app.ctx.theme;
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

        // First page button
        self.render_nav_button(
            frame,
            chunks[0],
            "First",
            self.state.has_prev_page(),
            self.state.nav_first_f.get(),
            theme,
        );

        // Previous page button
        self.render_nav_button(
            frame,
            chunks[2],
            "Prev",
            self.state.has_prev_page(),
            self.state.nav_prev_f.get(),
            theme,
        );

        // Page info
        self.render_page_info(frame, chunks[4], theme);

        // Next page button
        self.render_nav_button(
            frame,
            chunks[6],
            "Next",
            self.state.has_next_page(),
            self.state.nav_next_f.get(),
            theme,
        );

        // Last page button
        self.render_nav_button(
            frame,
            chunks[8],
            "Last",
            self.state.has_next_page(),
            self.state.nav_last_f.get(),
            theme,
        );
    }

    /// Renders the page information display.
    ///
    /// Shows current page information and range details when applicable.
    ///
    /// # Arguments
    /// * `frame` - The ratatui frame to render into
    /// * `area` - The rectangular area to render within
    /// * `theme` - The theme to use for styling
    fn render_page_info(&self, frame: &mut Frame, area: Rect, theme: &dyn UiTheme) {
        let info_text = if self.state.range_mode {
            format!(" | {}", self.state.range_info())
        } else {
            String::new()
        };

        let info_paragraph = Paragraph::new(info_text)
            .style(theme.text_secondary_style())
            .alignment(Alignment::Center);
        frame.render_widget(info_paragraph, area);
    }

    /// Renders a navigation button with proper styling.
    ///
    /// Creates a bordered button with appropriate styling based on
    /// whether it's enabled, focused, or disabled.
    ///
    /// # Arguments
    /// * `frame` - The ratatui frame to render into
    /// * `area` - The rectangular area to render within
    /// * `label` - The text to display on the button
    /// * `enabled` - Whether the button should be enabled
    /// * `focused` - Whether the button currently has focus
    /// * `theme` - The theme to use for styling
    fn render_nav_button(&self, frame: &mut Frame, area: Rect, label: &str, enabled: bool, focused: bool, theme: &dyn UiTheme) {
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

    // Handles focus navigation between pagination controls.
    //
    // Manages tab navigation between the different focusable elements
    // in the pagination component.
    //
    // # Arguments
    // * `event` - The key event that triggered the focus change
    // * `forward` - Whether to move focus forward (true) or backward (false)
    // Removed: focus navigation handled by table-level ring across grid and nav buttons

    // Removed: interactive range field navigation (no longer applicable)

    /// Handles navigation button actions.
    ///
    /// Processes left/right arrow keys and home/end keys for
    /// page navigation when the navigation area has focus.
    ///
    /// # Arguments
    /// * `event` - The key event that triggered the navigation
    fn handle_navigation_actions(&mut self, event: &KeyEvent) -> Option<Effect> {
        match event.code {
            KeyCode::Left => {
                if self.state.has_prev_page() {
                    self.state.prev_page();
                    Some(Effect::PrevPageRequested)
                } else {
                    None
                }
            }
            KeyCode::Right | KeyCode::End => {
                // Use Raw Next-Range header to request the next page when available
                if self.state.has_next_page() {
                    self.state.next_range.clone().map(|next_range| {
                        self.state.current_page = self.state.current_page.saturating_add(1);
                        Effect::NextPageRequested(next_range)
                    })
                } else {
                    None
                }
            }
            KeyCode::Home => {
                if self.state.has_prev_page() {
                    self.state.first_page();
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
    /// Renders the pagination controls using the app's focus system for styling.
    ///
    /// This method renders the complete pagination interface including:
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
        if !self.state.is_visible {
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Range display
                Constraint::Length(1), // Divider
                Constraint::Length(3), // Navigation controls
            ])
            .split(area);

        self.render_range_controls(frame, chunks[0], app);
        self.render_divider(frame, chunks[1], &*app.ctx.theme);
        self.render_navigation_controls(frame, chunks[2], app);
    }

    fn get_hint_spans(&self, app: &App, is_root: bool) -> Vec<Span<'_>> {
        if !self.state.is_visible {
            return Vec::new();
        }

        let nav_focused =
            self.state.nav_first_f.get() || self.state.nav_prev_f.get() || self.state.nav_next_f.get() || self.state.nav_last_f.get();
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
    fn handle_key_events(&mut self, _app: &mut App, event: KeyEvent) -> Vec<Effect> {
        if !self.state.is_visible {
            return vec![];
        }

        match event.code {
            KeyCode::Tab | KeyCode::BackTab => { /* Tab handled at table level */ }
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                if (self.state.nav_first_f.get()
                    || self.state.nav_prev_f.get()
                    || self.state.nav_next_f.get()
                    || self.state.nav_last_f.get())
                    && let Some(effect) = self.handle_navigation_actions(&event)
                {
                    return vec![effect];
                }
            }
            KeyCode::Enter => {
                // Activate the focused nav button
                if self.state.nav_first_f.get() && self.state.has_prev_page() {
                    self.state.first_page();
                    return vec![Effect::FirstPageRequested];
                }
                if self.state.nav_prev_f.get() && self.state.has_prev_page() {
                    self.state.prev_page();
                    return vec![Effect::PrevPageRequested];
                }
                if (self.state.nav_next_f.get() || self.state.nav_last_f.get())
                    && self.state.has_next_page()
                    && let Some(next_range) = self.state.next_range.clone()
                {
                    self.state.current_page = self.state.current_page.saturating_add(1);
                    return vec![Effect::NextPageRequested(next_range)];
                }
            }
            _ => {}
        }

        vec![]
    }
}
