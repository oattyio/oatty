use ratatui::{
    prelude::*,
    style::{Modifier, Style},
    widgets::Paragraph,
};

use super::components::{BuilderComponent, HelpComponent, LogsComponent, TableComponent};
use crate::{
    app::App,
    ui::{
        components::{
            component::Component,
            palette::{HintBarComponent, PaletteComponent},
        },
        layout::MainLayout,
    },
};

/// Renders the main user interface layout and coordinates all UI components.
///
/// This function creates the main application layout and orchestrates the
/// rendering of all UI components including the command palette, hints,
/// logs, and modal overlays.
///
/// # Arguments
///
/// * `f` - The frame to render to
/// * `app` - The application state containing all UI data
#[allow(clippy::too_many_arguments)]
pub fn draw(
    frame: &mut Frame,
    app: &mut App,
    palette: &mut PaletteComponent,
    hints: &mut HintBarComponent,
    logs: &mut LogsComponent,
    builder: &mut BuilderComponent,
    help: &mut HelpComponent,
    table: &mut TableComponent,
) {
    let size = frame.area();

    // Fill the entire background with the theme's background color for consistency
    let bg_fill = Paragraph::new("").style(Style::default().bg(app.ctx.theme.roles().background));
    frame.render_widget(bg_fill, size);

    // Create main layout with vertical sections
    let chunks = MainLayout::vertical_layout(size, app);

    // Render main UI components
    render_command_palette(frame, app, palette, chunks[0]);
    render_hints(frame, app, hints, chunks[1]);
    render_logs(frame, app, logs, chunks[2]);

    // Render modal overlays if active
    render_modals(frame, app, builder, help, table);
}

// Creates the main vertical layout for the application.
//
// Arguments:
// * `size` - The total available screen area
//
// Returns:
// Vector of rectangular areas for each UI section
// (helper moved to ui/layout.rs)

/// Renders the command palette area.
///
/// # Arguments
///
/// * `f` - The frame to render to
/// * `app` - The application state
/// * `area` - The area to render the palette in
fn render_command_palette(f: &mut Frame, app: &mut App, palette: &mut PaletteComponent, area: Rect) {
    palette.render(f, area, app);
}

/// Renders the hints area with keyboard shortcuts.
///
/// # Arguments
///
/// * `f` - The frame to render to
/// * `app` - The application state
/// * `area` - The area to render hints in
fn render_hints(f: &mut Frame, app: &mut App, hints: &mut HintBarComponent, area: Rect) {
    // Always render palette hints here; logs hints are drawn inside the logs block
    // now.
    hints.render(f, area, app);
}

// Render logs area via component
fn render_logs(f: &mut Frame, app: &mut App, logs: &mut LogsComponent, area: Rect) {
    logs.render(f, area, app);
}

/// Renders modal overlays based on application state.
///
/// # Arguments
///
/// * `f` - The frame to render to
/// * `app` - The application state
fn render_modals(
    f: &mut Frame,
    app: &mut App,
    builder: &mut BuilderComponent,
    help: &mut HelpComponent,
    table: &mut TableComponent,
) {
    // Draw a dim overlay when any modal is visible
    let any_modal = app.help.is_visible() || app.table.is_visible() || app.builder.is_visible();
    if any_modal {
        use ratatui::widgets::Block;
        let area = f.area();
        f.render_widget(Block::default().bg(app.ctx.theme.roles().background).add_modifier(Modifier::DIM), area);
    }

    if app.help.is_visible() {
        help.render(f, f.area(), app);
    }
    if app.table.is_visible() {
        table.render(f, f.area(), app);
    }
    if app.builder.is_visible() {
        builder.render(f, f.area(), app);
    }
}
