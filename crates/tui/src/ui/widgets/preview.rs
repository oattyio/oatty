//! Preview widget for displaying command previews and results.
//!
//! This module provides a widget for rendering command previews, showing
//! the CLI command that would be executed and any JSON results.

use crate::app::App;
use crate::theme;
use ratatui::{prelude::*, widgets::*};

/// Renders the command preview area showing the generated CLI command.
///
/// This function displays a preview of the command that would be executed
/// based on the current field values. It shows either the CLI command string
/// or JSON results in a table format when available.
///
/// # Arguments
///
/// * `f` - The frame to render to
/// * `app` - The application state containing preview data
/// * `area` - The rectangular area to render the preview in
///
/// # Features
///
/// - Shows CLI command preview when no JSON result is available
/// - Automatically switches to table view for JSON arrays
/// - Falls back to key-value display for JSON objects
/// - Includes copy hint in title
/// - Uses themed styling for borders and text
///
/// # Display Logic
///
/// 1. If JSON result exists and contains arrays → Show table view
/// 2. If JSON result exists but no arrays → Show key-value view
/// 3. If no JSON result → Show CLI command preview
///
/// # Styling
///
/// - Uses themed border styling
/// - Applies text styling to content
/// - Title includes copy hint with accent color
///
/// # Examples
///
/// ```rust
/// use ratatui::Frame;
/// use heroku_tui::app::App;
///
/// let app = App::new(registry);
/// let area = Rect::new(0, 0, 50, 10);
/// draw_preview(&mut frame, &app, area);
/// ```
pub fn draw_preview(f: &mut Frame, app: &App, area: Rect) {
    // If we have a JSON result, prefer a table when an array is present; else fallback to key/values
    if let Some(json) = &app.table.result_json {
        let has_array = match json {
            serde_json::Value::Array(a) => !a.is_empty(),
            serde_json::Value::Object(m) => {
                m.values().any(|v| matches!(v, serde_json::Value::Array(_)))
            }
            _ => false,
        };
        if has_array {
            crate::tables::draw_json_table(f, area, json);
        } else {
            crate::tables::draw_kv_or_text(f, area, json);
        }
        return;
    }

    let block = Block::default()
        .title(Span::styled("Command  [Ctrl+Y] Copy", theme::title_style()))
        .borders(Borders::ALL)
        .border_style(theme::border_style(false));

    let mut text = String::new();
    if let Some(spec) = &app.builder.picked {
        let cli = crate::preview::cli_preview(spec, &app.builder.fields);
        text = cli;
    } else {
        text.push_str("Select a command to see preview.");
    }

    let p = Paragraph::new(text)
        .style(theme::text_style())
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}
