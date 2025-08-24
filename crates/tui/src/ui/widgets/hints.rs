//! Hints widget for displaying keyboard shortcuts and help text.
//!
//! This module provides a widget for rendering keyboard shortcuts and
//! contextual help information to guide users through the interface.

use crate::theme;
use ratatui::{prelude::*, widgets::*};

/// Renders the hints strip with keyboard shortcuts and help text.
///
/// This function displays a horizontal line of keyboard shortcuts that help
/// users understand available commands and navigation options. The hints
/// are styled consistently with the application theme.
///
/// # Arguments
///
/// * `f` - The frame to render to
/// * `area` - The rectangular area to render the hints in
///
/// # Displayed Shortcuts
///
/// The hints widget displays the following keyboard shortcuts:
///
/// - **↑/↓** - Cycle through suggestions or options
/// - **Tab** - Accept the current suggestion
/// - **Ctrl+R** - Access command history
/// - **Ctrl+F** - Open the command builder modal
/// - **Esc** - Cancel current operation
///
/// # Styling
///
/// - Uses muted text color for labels
/// - Uses accent color for key combinations
/// - Consistent with application theme
///
/// # Examples
///
/// ```rust
/// use ratatui::Frame;
///
/// let area = Rect::new(0, 0, 80, 1);
/// draw_hints(&mut frame, area);
/// ```
pub fn draw_hints(f: &mut Frame, area: Rect) {
    let hints = Paragraph::new(Line::from(vec![
        Span::styled("Hints: ", theme::text_muted()),
        Span::styled("↑/↓", theme::title_style().fg(theme::ACCENT)),
        Span::styled(" cycle  ", theme::text_muted()),
        Span::styled("Tab", theme::title_style().fg(theme::ACCENT)),
        Span::styled(" accept  ", theme::text_muted()),
        Span::styled("Ctrl-R", theme::title_style().fg(theme::ACCENT)),
        Span::styled(" history  ", theme::text_muted()),
        Span::styled("Ctrl-F", theme::title_style().fg(theme::ACCENT)),
        Span::styled(" builder  ", theme::text_muted()),
        Span::styled("Esc", theme::title_style().fg(theme::ACCENT)),
        Span::styled(" cancel", theme::text_muted()),
    ]))
    .style(theme::text_muted());
    f.render_widget(hints, area);
}
