//! Logs widget for displaying application logs and status messages.
//!
//! This module provides a widget for rendering application logs in a
//! scrollable list format with proper styling and organization.

use crate::app::App;
use crate::theme;
use ratatui::{prelude::*, widgets::*};

/// Renders the logs area displaying application logs and status messages.
///
/// This function displays application logs in a scrollable list format with
/// a title showing the log count. The logs area provides feedback to users
/// about command execution, errors, and application status.
///
/// # Arguments
///
/// * `f` - The frame to render to
/// * `app` - The application state containing log data
/// * `area` - The rectangular area to render the logs in
///
/// # Features
///
/// - Shows log count in the title
/// - Displays logs as a scrollable list
/// - Uses themed styling for borders and text
/// - Each log entry is rendered as a separate list item
/// - Handles empty log states gracefully
///
/// # Styling
///
/// - Uses themed border styling
/// - Applies text styling to log entries
/// - Title shows log count with accent color
///
/// # Examples
///
/// ```rust
/// use ratatui::Frame;
/// use heroku_tui::app::App;
///
/// let app = App::new(registry);
/// let area = Rect::new(0, 0, 80, 6);
/// draw_logs(&mut frame, &app, area);
/// ```
pub fn draw_logs(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(Span::styled(
            format!("Logs ({})", app.logs.len()),
            theme::title_style(),
        ))
        .borders(Borders::ALL)
        .border_style(theme::border_style(false));

    let items: Vec<ListItem> = app
        .logs
        .iter()
        .map(|l| ListItem::new(l.as_str()).style(theme::text_style()))
        .collect();

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}
