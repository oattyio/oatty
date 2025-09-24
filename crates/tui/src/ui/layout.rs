//! Layout system for the Heroku TUI application.
//!
//! This module provides layout utilities and helpers for organizing the UI
//! components in a consistent and maintainable way. It defines the main
//! application layout structure and provides reusable layout functions.
use ratatui::prelude::*;

use crate::app::App;
pub(super) struct MainLayout;

impl MainLayout {
    /// Creates the main vertical layout for the application.
    ///
    /// This function defines the primary layout structure of the TUI
    /// application, dividing the screen into logical sections for different
    /// UI components. The layout follows a vertical arrangement with
    /// specific constraints for each area.
    ///
    /// # Layout Structure
    ///
    /// The main layout consists of four vertical sections:
    ///
    /// 1. **Command Palette Area** - Input field and suggestions
    /// 2. **Hints Area** - Keyboard shortcuts and help text
    /// 4. **Logs Area** - Application logs and status messages
    ///
    /// # Arguments
    ///
    /// * `size` - The total available screen area (Rect)
    ///
    /// # Returns
    ///
    /// Vector of rectangular areas for each UI section, ordered from top to
    /// bottom
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
    pub fn responsive_layout(size: Rect, app: &App) -> Vec<Rect> {
        // Dynamically expand the palette area when suggestions popup is visible
        let mut palette_extra: u16 = 0;
        let show_popup =
            app.palette.error_message().is_none() && app.palette.is_suggestions_open() && !app.palette.suggestions().is_empty();

        if show_popup {
            let rows = app.palette.suggestions().len().min(10) as u16; // match palette.rs max_rows
            let popup_height = rows + 3;
            palette_extra = popup_height;
        }

        // wider area displays 2 columns with the leftmost
        // column split into 2 rows totaling 3 rendering areas.
        if size.width >= 141 {
            // start with left and right columns
            let two_col = Layout::horizontal([
                Constraint::Percentage(65), // Command palette + hints
                Constraint::Min(20),        // logs
            ])
            .split(size);

            // split the left col into two stacked rows
            let constraints = [
                Constraint::Length(5 + palette_extra), // Command palette area (+ suggestions)
                Constraint::Percentage(100),           // Gap
                Constraint::Min(1),                    // Hints area
            ];

            let vertical = Layout::vertical(constraints).split(two_col[0]);

            return vec![vertical[0], two_col[1], vertical[2]]; // ordering intentional
        }

        // Smaller screens display 3 stacked rows.
        let constraints = [
            Constraint::Length(5 + palette_extra), // Command palette area (+ suggestions)
            Constraint::Percentage(100),           // logs / output content
            Constraint::Min(1),                    // Hints area
        ];

        Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(size)
            .to_vec()
    }
}
