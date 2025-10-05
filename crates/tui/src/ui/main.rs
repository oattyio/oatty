use ratatui::{
    prelude::*,
    style::Style,
    widgets::{Block, Paragraph},
};

use crate::app::App;
use crate::ui::components::component::Component;
use crate::ui::components::nav_bar::VerticalNavBarComponent;

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
pub fn draw(frame: &mut Frame, app: &mut App) {
    let size = frame.area();
    // Fill the entire background with the theme's background color for consistency
    let bg_fill = Paragraph::new("").style(Style::default().bg(app.ctx.theme.roles().background));
    frame.render_widget(bg_fill, size);

    // Temporarily take components to avoid borrow checker issues
    let mut main_view = std::mem::replace(&mut app.main_view, None);
    let mut open_modal = std::mem::replace(&mut app.open_modal, None);

    // Layout: left rail for nav bar, right for active main view
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(9), Constraint::Min(1)])
        .split(size);

    // Handle main view rendering
    if let Some(current) = main_view.as_mut() {
        // Render nav bar on the left
        let mut nav = VerticalNavBarComponent::new();
        nav.render(frame, chunks[0], app);
        // Render active view on the right
        current.render(frame, chunks[1], app);
    }

    // Render modal overlays if active
    if open_modal.is_some() {
        render_overlay(frame, app);
        if let Some(modal) = open_modal.as_mut() {
            modal.render(frame, size, app);
        }
    }

    // Move components back if they weren't replaced.
    if app.main_view.is_none() {
        app.main_view = main_view;
    }
    if app.open_modal.is_none() {
        app.open_modal = open_modal;
    }
}

/// Renders modal overlays based on application state.
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
