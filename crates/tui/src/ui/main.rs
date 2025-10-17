use crate::app::App;
use ratatui::widgets::Clear;
use ratatui::{
    prelude::*,
    style::Style,
    widgets::{Block, Paragraph},
};

/// Renders the main user interface layout and coordinates all UI components.
///
/// This function creates the main application layout and orchestrates the
/// rendering of all UI components, including the command palette, hints,
/// logs, and modal overlays.
///
/// # Arguments
///
/// * `f` - The frame to render to
/// * `app` - The application state containing all UI data
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    // Fill the entire background with the theme's background color for consistency
    let bg_fill = Paragraph::new("").style(Style::default().bg(app.ctx.theme.roles().background));
    frame.render_widget(bg_fill, area);

    // Temporarily take components to avoid borrow checker issues
    let mut main_view = app.main_view.take();
    let mut open_modal = app.open_modal.take();
    let mut nav_bar = app.nav_bar_view.take();
    let mut logs_view = app.logs_view.take();

    let layout = get_preferred_layout(app, area);
    let mut hint_spans: Vec<Span> = vec![Span::styled("Hints: ", app.ctx.theme.text_muted_style())];
    // Handle main view rendering
    if let Some(current) = main_view.as_mut() {
        // Render nav bar on the left
        if let Some(nav) = nav_bar.as_mut() {
            nav.render(frame, layout[0], app);
        }
        // Render an active view on the right
        current.render(frame, layout[2], app);
        hint_spans.extend(current.get_hint_spans(app));
    }

    if let Some(logs) = logs_view.as_mut() {
        let logs_area = layout[3];
        logs.render(frame, logs_area, app);
        hint_spans.extend(logs.get_hint_spans(app));
    }
    let hints_widget = Paragraph::new(Line::from(hint_spans)).style(app.ctx.theme.text_muted_style());
    frame.render_widget(hints_widget, layout[1]);

    // Render modal overlays if active
    if open_modal.is_some() {
        render_overlay(frame, app);
        if let Some((modal, position)) = open_modal.as_mut() {
            let modal_area = position(area);
            frame.render_widget(Clear, modal_area);
            modal.render(frame, modal_area, app);
        }
    }

    // Move components back if they weren't replaced.
    if app.main_view.is_none() {
        app.main_view = main_view;
    }
    if app.open_modal.is_none() {
        app.open_modal = open_modal;
    }
    if app.nav_bar_view.is_none() {
        app.nav_bar_view = nav_bar;
    }
    if app.logs_view.is_none() {
        app.logs_view = logs_view;
    }
}
fn get_preferred_layout(app: &App, area: Rect) -> Vec<Rect> {
    // a wider area displays 2 columns with the leftmost
    // column split into 2 rows totaling 3 rendering areas.
    let outer_areas = Layout::horizontal([
        Constraint::Length(9), // Nav bar width
        Constraint::Min(1),    // Wrapper
    ])
    .split(area);
    // Split the wrapper area into 2 areas for the main view
    // and hints stacked vertically.
    let content_areas = Layout::vertical([
        Constraint::Percentage(100), // Main view width
        Constraint::Min(1),          // Hints area
    ])
    .split(outer_areas[1]);

    let main_view_areas = if content_areas[0].width >= 141 {
        let constraints = if app.logs_view.is_some() {
            [
                Constraint::Percentage(50), // Main view width
                Constraint::Percentage(50), // Logs width
            ]
        } else {
            [
                Constraint::Percentage(100), // Main view width
                Constraint::Percentage(0),   // No logs shown
            ]
        };

        Layout::vertical(constraints).split(content_areas[0])
    } else {
        // Smaller screens display 3 stacked rows.
        let constraints = [
            Constraint::Percentage(60), // Command palette area (+ suggestions)
            Constraint::Percentage(40), // logs / output content
        ];

        Layout::vertical(constraints).split(content_areas[0])
    };

    vec![
        outer_areas[0],     // navigation
        content_areas[1],   // Hints bar
        main_view_areas[0], // Main view
        main_view_areas[1], // Logs / output content (if open)
    ]
}

/// Renders modal overlays based on the application state.
///
/// # Arguments
///
/// * `f` - The frame to render to
/// * `app` - The application state
fn render_overlay(frame: &mut Frame, app: &mut App) {
    // Draw the theme-specific modal overlay when any modal is visible
    let area = frame.area();
    frame.render_widget(Block::default().style(app.ctx.theme.modal_background_style()).dim(), area);
}
