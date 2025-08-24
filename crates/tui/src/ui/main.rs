use super::components::{
    BuilderComponent, HelpComponent, HintBarComponent, LogsComponent, PaletteComponent,
    StepsComponent, TableComponent,
};
use crate::app::App;
use crate::component::Component;
use ratatui::prelude::*;

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
pub fn draw(
    f: &mut Frame,
    app: &mut App,
    palette: &mut PaletteComponent,
    hints: &mut HintBarComponent,
    steps: &mut StepsComponent,
    logs: &mut LogsComponent,
    builder: &mut BuilderComponent,
    help: &mut HelpComponent,
    table: &mut TableComponent,
) {
    let size = f.area();

    // Create main layout with vertical sections
    let chunks = super::layout::create_main_layout(size, app);

    // Render main UI components
    render_command_palette(f, app, palette, chunks[0]);
    render_hints(f, app, hints, chunks[1]);
    render_steps(f, app, steps, chunks[2]);
    render_logs(f, app, logs, chunks[3]);

    // Render modal overlays if active
    render_modals(f, app, builder, help, table);
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
fn render_command_palette(
    f: &mut Frame,
    app: &mut App,
    palette: &mut PaletteComponent,
    area: Rect,
) {
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
    // Only show hints when no error present and either no popup or no suggestions
    if app.palette.error.is_none()
        && (!app.palette.popup_open || app.palette.suggestions.is_empty())
    {
        hints.render(f, area, app);
    }
}

// Render the steps/workflow area (center panel)
fn render_steps(f: &mut Frame, _app: &mut App, steps: &mut StepsComponent, area: Rect) {
    steps.render(f, area, _app);
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
    if app.show_help {
        help.render(f, f.area(), app);
    }
    if app.show_table {
        table.render(f, f.area(), app);
    }
    if app.show_builder {
        builder.render(f, f.area(), app);
    }
}
