//! Theme and styling for the Heroku TUI application.
//!
//! This module defines the color scheme and styling functions used throughout
//! the TUI interface. It provides a consistent, Heroku-branded visual design
//! with a dark theme and blue accent colors.

use ratatui::style::{Color, Modifier, Style};

/// Heroku Cloud Blue accent color for highlights and focus indicators.
///
/// This is an approximation of the official Heroku Cloud Blue color (#08ABED)
/// used for accent elements throughout the interface.
pub const ACCENT: Color = Color::Rgb(8, 171, 237);

/// Primary foreground color for normal text.
///
/// A light gray color used for most text content in the interface.
pub const FG: Color = Color::Rgb(224, 224, 230);

/// Muted foreground color for secondary text.
///
/// A darker gray used for less prominent text like hints, labels, and
/// secondary information.
pub const FG_MUTED: Color = Color::Rgb(168, 168, 175);

/// Default border color for UI elements.
///
/// A medium gray used for borders of unfocused UI components.
pub const BORDER: Color = Color::Rgb(72, 72, 80);

/// Focused border color for UI elements.
///
/// Uses the accent color to indicate which element currently has focus.
pub const BORDER_FOCUS: Color = ACCENT;

/// Background color for panels and containers.
///
/// A very dark gray that serves as the primary background color.
pub const BG_PANEL: Color = Color::Rgb(18, 18, 24);

/// Background color for highlighted elements.
///
/// A subtle blue-tinted dark color used for general highlighting and
/// selection backgrounds.
pub const BG_HIGHLIGHT: Color = Color::Rgb(20, 32, 44);

/// Background color for selected list items.
///
/// An even subtler blue-tinted dark color used specifically for
/// list and table selection highlighting.
pub const BG_SELECT: Color = Color::Rgb(18, 28, 38);

/// Warning color for error states and alerts.
///
/// A red-orange color used to indicate warnings, errors, and
/// validation failures.
pub const WARN: Color = Color::Rgb(220, 96, 110);

/// Creates a border style based on focus state.
///
/// This function returns a style with appropriate border color based on
/// whether the element is currently focused or not.
///
/// # Arguments
///
/// * `focused` - Whether the element should appear focused
///
/// # Returns
///
/// A Style with the appropriate border color.
///
/// # Examples
///
/// ```rust
/// use crate::theme::border_style;
///
/// let focused_style = border_style(true);   // Uses ACCENT color
/// let normal_style = border_style(false);   // Uses BORDER color
/// ```
pub fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(BORDER_FOCUS)
    } else {
        Style::default().fg(BORDER)
    }
}

/// Creates a style for titles and headers.
///
/// This function returns a style with muted foreground color and bold
/// modifier, suitable for titles, headers, and prominent labels.
///
/// # Returns
///
/// A Style with muted color and bold modifier.
///
/// # Examples
///
/// ```rust
/// use crate::theme::title_style;
///
/// let title = "Command List";
/// // Apply title_style() to render the title with proper styling
/// ```
pub fn title_style() -> Style {
    Style::default().fg(FG_MUTED).add_modifier(Modifier::BOLD)
}

/// Creates a style for normal text content.
///
/// This function returns a style with the primary foreground color,
/// suitable for most text content in the interface.
///
/// # Returns
///
/// A Style with the primary foreground color.
///
/// # Examples
///
/// ```rust
/// use crate::theme::text_style;
///
/// let content = "This is normal text content";
/// // Apply text_style() to render with primary foreground color
/// ```
pub fn text_style() -> Style {
    Style::default().fg(FG)
}

/// Creates a style for muted or secondary text.
///
/// This function returns a style with the muted foreground color,
/// suitable for hints, secondary information, and less prominent text.
///
/// # Returns
///
/// A Style with the muted foreground color.
///
/// # Examples
///
/// ```rust
/// use crate::theme::text_muted;
///
/// let hint = "Press Tab to complete";
/// // Apply text_muted() to render as secondary text
/// ```
pub fn text_muted() -> Style {
    Style::default().fg(FG_MUTED)
}

/// Creates a style for highlighted or focused elements.
///
/// This function returns a style with the primary foreground color
/// and a subtle background highlight, suitable for focused input
/// fields and active elements.
///
/// # Returns
///
/// A Style with foreground color and highlight background.
///
/// # Examples
///
/// ```rust
/// use crate::theme::highlight_style;
///
/// let focused_field = "app-name";
/// // Apply highlight_style() to show the field is focused
/// ```
pub fn highlight_style() -> Style {
    // Used for focused input rows; keep a subtle background hint
    Style::default().fg(FG).bg(BG_HIGHLIGHT)
}

/// Creates a style for selected list and table items.
///
/// This function returns a style with the accent color and bold
/// modifier, suitable for highlighting selected items in lists
/// and tables without using background colors.
///
/// # Returns
///
/// A Style with accent color and bold modifier.
///
/// # Examples
///
/// ```rust
/// use crate::theme::list_highlight_style;
///
/// let selected_item = "apps:list";
/// // Apply list_highlight_style() to show the item is selected
/// ```
pub fn list_highlight_style() -> Style {
    // Used for list/table selection; emphasize subtly via accent + bold, no fill
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}
