//! Results table modal component for displaying JSON data.
//!
//! This module provides a component for rendering the table modal, which displays
//! JSON results from command execution in a tabular format with scrolling and
//! navigation capabilities.

use ratatui::{layout::Rect, Frame};

use crate::{app, component::Component};

/// Results table modal component for displaying JSON data.
///
/// This component renders a modal dialog containing tabular data from command
/// execution results. It automatically detects JSON arrays and displays them
/// in a scrollable table format with proper column detection and formatting.
///
/// # Features
///
/// - Automatically detects and displays JSON arrays as tables
/// - Provides scrollable navigation through large datasets
/// - Handles column detection and formatting
/// - Supports keyboard navigation (arrow keys, page up/down, home/end)
/// - Falls back to key-value display for non-array JSON
///
/// # Usage
///
/// The table component is typically activated when a command returns JSON
/// results containing arrays. It provides an optimal viewing experience for
/// tabular data like lists of apps, dynos, or other resources.
///
/// # Navigation
///
/// - **Arrow keys**: Scroll up/down through rows
/// - **Page Up/Down**: Scroll faster through the table
/// - **Home/End**: Jump to beginning/end of the table
/// - **Escape**: Close the table modal
///
/// # Examples
///
/// ```rust
/// use heroku_tui::ui::components::TableComponent;
///
/// let mut table = TableComponent::new();
/// table.init()?;
/// ```
#[derive(Default)]
pub struct TableComponent;

impl TableComponent {
    /// Creates a new table component instance.
    ///
    /// # Returns
    ///
    /// A new TableComponent with default state
    pub fn new() -> Self {
        Self
    }
}

impl Component for TableComponent {
    /// Renders the table modal with JSON results.
    ///
    /// This method delegates rendering to the modal system, which handles
    /// the layout, styling, and table generation for the results display.
    ///
    /// # Arguments
    ///
    /// * `f` - The frame to render to
    /// * `rect` - The rectangular area to render in
    /// * `app` - The application state containing result data
    fn render(&mut self, f: &mut Frame, rect: Rect, app: &mut app::App) {
        crate::ui::modals::draw_table_modal(f, app, rect);
    }
}
