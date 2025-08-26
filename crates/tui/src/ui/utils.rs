//! UI utilities and helper functions for the TUI application.
//!
//! This module provides utility functions and helper traits that are used
//! across the UI components. It includes layout utilities, string helpers,
//! and other common functionality needed for UI rendering.

use ratatui::prelude::*;

/// Creates a centered rectangular area within a given rectangle.
///
/// This utility function calculates a centered rectangle based on percentage
/// dimensions relative to the parent rectangle. It's commonly used for
/// creating modal dialogs and popup windows.
///
/// # Arguments
///
/// * `percent_x` - The width of the centered rectangle as a percentage (0-100)
/// * `percent_y` - The height of the centered rectangle as a percentage (0-100)
/// * `r` - The parent rectangle to center within
///
/// # Returns
///
/// A new rectangle centered within the parent rectangle with the specified
/// percentage dimensions.
///
/// # Examples
///
/// ```rust
/// use ratatui::prelude::*;
/// use heroku_tui::ui::utils::centered_rect;
///
/// let parent = Rect::new(0, 0, 100, 50);
/// let centered = centered_rect(80, 70, parent);
/// // Creates a rectangle that's 80% wide and 70% tall, centered in parent
/// ```
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    let area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1]);
    area[1]
}

/// Extension trait for String to provide fallback values when empty.
///
/// This trait adds a method to String that returns an alternative value
/// when the string is empty, useful for displaying placeholder text
/// in UI components.
///
/// # Examples
///
/// ```rust
/// use heroku_tui::ui::utils::IfEmptyStr;
///
/// let empty = String::new();
/// let result = empty.if_empty_then("default".to_string());
/// assert_eq!(result, "default");
///
/// let non_empty = "hello".to_string();
/// let result = non_empty.if_empty_then("default".to_string());
/// assert_eq!(result, "hello");
/// ```
pub trait IfEmptyStr {
    /// Returns the string if non-empty, otherwise returns the alternative value.
    ///
    /// # Arguments
    ///
    /// * `alt` - The alternative string to return if self is empty
    ///
    /// # Returns
    ///
    /// The original string if non-empty, otherwise the alternative string.
    fn if_empty_then(self, alt: String) -> String;
}

impl IfEmptyStr for String {
    fn if_empty_then(self, alt: String) -> String {
        if self.is_empty() { alt } else { self }
    }
}
