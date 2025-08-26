//! UI rendering module for the TUI application.
//!
//! This module provides all the user interface rendering functionality,
//! including main layout, modals, components, and utilities.

pub mod components;
pub mod layout;
pub mod main;
pub mod modals;
pub mod utils;

use crate::app::App;
use crate::ui::components::{
    BuilderComponent, HelpComponent, HintBarComponent, LogsComponent, TableComponent,
    palette::PaletteComponent,
};
use ratatui::Frame;

/// Renders the main user interface for the TUI application.
///
/// This function is the primary entry point for all UI rendering. It creates the
/// main layout with command palette, hints, logs, and handles modal overlays.
/// The layout is divided into vertical sections with specific constraints for
/// each area.
///
/// # Arguments
///
/// * `f` - The frame to render to
/// * `app` - The application state containing all UI data
///
/// # Layout Structure
///
/// The main layout consists of:
/// - Command palette area (3 lines) - Input field and suggestions
/// - Hints area (1 line) - Keyboard shortcuts and help text
/// - Spacer area (minimum 1 line) - Future content area
/// - Logs area (6 lines) - Application logs and status messages
///
/// # Modal Overlays
///
/// The function also handles rendering of modal overlays when active:
/// - Help modal (`app.show_help`)
/// - Table modal (`app.show_table`)
/// - Builder modal (`app.show_builder`)
///
/// # Examples
///
/// ```rust,no_run
/// use ratatui::prelude::*;
/// use heroku_tui::app::App;
/// use heroku_tui::ui::draw;
/// use heroku_tui::ui::components::*;
/// use heroku_registry::Registry;
///
/// let registry = Registry::from_embedded_schema().unwrap();
/// let mut app = App::new(registry);
/// let mut frame = Frame::new(/* terminal setup */);
/// let mut palette = PaletteComponent::new();
/// let mut hints = HintBarComponent::new();
/// let mut logs = LogsComponent::new();
/// let mut builder = BuilderComponent::new();
/// let mut help = HelpComponent::new();
/// let mut table = TableComponent::new();
/// draw(&mut frame, &mut app, &mut palette, &mut hints, &mut logs, &mut builder, &mut help, &mut table);
/// ```
pub fn draw(
    f: &mut Frame,
    app: &mut App,
    palette: &mut PaletteComponent,
    hints: &mut HintBarComponent,
    logs: &mut LogsComponent,
    builder: &mut BuilderComponent,
    help: &mut HelpComponent,
    table: &mut TableComponent,
) {
    main::draw(f, app, palette, hints, logs, builder, help, table);
}

// Re-export commonly used components for convenience
// Re-export select components if needed by external callers.
// Currently unused internally; kept minimal to avoid warnings.
