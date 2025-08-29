//! Layout system for the Heroku TUI application.
//!
//! This module provides layout utilities and helpers for organizing the UI
//! components in a consistent and maintainable way. It defines the main
//! application layout structure and provides reusable layout functions.

use crate::app::App;
use ratatui::prelude::*;
pub(super) struct MainLayout;

impl MainLayout {
    /// Creates the main vertical layout for the application.
    ///
    /// This function defines the primary layout structure of the TUI application,
    /// dividing the screen into logical sections for different UI components.
    /// The layout follows a vertical arrangement with specific constraints for
    /// each area.
    ///
    /// # Layout Structure
    ///
    /// The main layout consists of four vertical sections:
    ///
    /// 1. **Command Palette Area** (3 lines) - Input field and suggestions
    /// 2. **Hints Area** (1 line) - Keyboard shortcuts and help text
    /// 3. **Spacer Area** (minimum 1 line) - Future content area
    /// 4. **Logs Area** (6 lines) - Application logs and status messages
    ///
    /// # Arguments
    ///
    /// * `size` - The total available screen area (Rect)
    ///
    /// # Returns
    ///
    /// Vector of rectangular areas for each UI section, ordered from top to bottom
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use ratatui::prelude::*;
    /// use heroku_tui::ui::layout::create_main_layout;
    /// use heroku_tui::app::App;
    /// use heroku_registry::Registry;
    ///
    /// let registry = Registry::from_embedded_schema().unwrap();
    /// let app = App::new(registry);
    /// let screen_size = Rect::new(0, 0, 80, 24);
    /// let layout_areas = create_main_layout(screen_size, &app);
    ///
    /// // layout_areas[0] = Command palette area
    /// // layout_areas[1] = Hints area  
    /// // layout_areas[2] = Logs area
    /// ```
    pub fn vertical_layout(size: Rect, app: &App) -> Vec<Rect> {
        // Dynamically expand the palette area when suggestions popup is visible
        let mut palette_extra: u16 = 0;
        let show_popup = app.palette.error_message().is_none()
            && app.palette.is_suggestions_open()
            && !app.builder.is_visible()
            && !app.help.is_visible()
            && !app.palette.suggestions().is_empty();
        if show_popup {
            let rows = app.palette.suggestions().len().min(10) as u16; // match palette.rs max_rows
            let popup_height = rows + 3;
            palette_extra = popup_height;
        }

        let constraints = [
            Constraint::Length(2 + palette_extra), // Command palette area (+ suggestions)
            Constraint::Length(1),                 // Hints area
            Constraint::Min(1),                    // logs / output content
        ];

        Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(size)
            .to_vec()
    }
}
