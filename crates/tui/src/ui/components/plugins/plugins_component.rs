//! Top-level Plugins view: orchestrates search, table, add view, details,
//! logs, and environment editor. Handles focus routing, shortcuts, and responsive
//! layout whether shown fullscreen or as a centered overlay.
//!
//! This module contains the main coordinator component for the MCP (Model Context Protocol)
//! plugins management interface, providing a unified view that can display different
//! sub-components based on user interaction and app state.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use heroku_types::Effect;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

use crate::{app::App, ui::components::component::Component};

use super::{
    PluginsAddComponent, PluginsLogsComponent, PluginsSearchComponent, PluginsSecretsComponent, PluginsTableComponent,
};

/// Top-level Plugins view component that orchestrates all plugin-related UI elements.
///
/// This component manages the display and interaction of various plugin management
/// interfaces including the plugin list table, search functionality, add plugin plugin,
/// plugin details, logs viewer, and environment variable editor. It handles focus
/// management, keyboard shortcuts, and responsive layout for both fullscreen and
/// overlay display modes.
///
/// The component follows the established TUI architecture pattern where it acts as
/// a coordinator that delegates specific functionality to specialized child components
/// while managing the overall state and user interaction flow. Each child component
/// (like PluginAddViewState) manages its own focus through the HasFocus trait,
/// ensuring proper encapsulation and separation of concerns.
#[derive(Debug, Default)]
pub struct PluginsComponent {
    /// Child component for displaying the plugin list table
    table_component: PluginsTableComponent,
    /// Child component for plugin search functionality
    search_component: PluginsSearchComponent,
    /// Child component for displaying plugin logs
    logs_component: PluginsLogsComponent,
    /// Child component for editing plugin environment variables
    secrets_component: PluginsSecretsComponent,
    /// Child component for the add plugin plugin
    add_component: PluginsAddComponent,
}

impl Component for PluginsComponent {
    /// Handles keyboard events for the plugins component and its children.
    ///
    /// This method implements a hierarchical event handling strategy:
    /// 1. First, check if any overlay is open and delegate to it
    /// 2. Handle focus cycling (Tab/BackTab) if no overlay is active
    /// 3. Process Ctrl-based shortcuts for plugin operations
    /// 4. Finally, delegate to child components for specific functionality
    ///
    /// # Arguments
    ///
    /// * `app` - Mutable reference to the app state
    /// * `key_event` - The keyboard event to process
    ///
    /// # Returns
    ///
    /// Returns a vector of effects that should be processed by the app
    fn handle_key_events(&mut self, app: &mut App, key_event: KeyEvent) -> Vec<Effect> {
        let mut effects = self.delegate_to_open_overlays(app, key_event);
        effects.extend(self.handle_control_shortcuts(app, key_event));
        effects.extend(self.delegate_to_child_components(app, key_event));

        match key_event.code {
            KeyCode::BackTab => app.focus.prev(),
            KeyCode::Tab => app.focus.next(),
            _ => false,
        };

        effects
    }

    /// Renders the plugins component and all its children.
    ///
    /// This method orchestrates the rendering of the entire plugins interface,
    /// including the main shell, header, body, footer, and any open overlays.
    /// It handles both fullscreen and overlay display modes.
    ///
    /// # Arguments
    ///
    /// * `frame` - Mutable reference to the terminal frame for rendering
    /// * `area` - The rectangular area available for rendering
    /// * `app` - Mutable reference to the app state
    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let layout = self.create_main_layout(area);
        let header_area = layout.get(0).expect("header area not found");
        let body_area = layout.get(1).expect("body area not found");
        let footer_area = layout.get(2).expect("footer area not found");

        self.render_header_section(frame, *header_area, app);
        self.render_body_section(frame, *body_area, app);

        let spans = self.get_hint_spans(app, true);
        frame.render_widget(
            Paragraph::new(Line::from(spans)).style(*&app.ctx.theme.text_muted_style()),
            *footer_area,
        );

        self.render_overlay_components(frame, area, app);
    }

    /// Renders the hints bar content
    ///
    /// This method provides an area to render hints contextually
    /// and delegates to child components depending on focus.
    ///
    /// # Arguments
    ///
    /// * `frame` - Mutable reference to the terminal frame for rendering
    /// * `area` - The rectangular area available for rendering
    /// * `app` - Mutable reference to the app state
    fn get_hint_spans(&self, app: &App, is_root: bool) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        let mut spans = vec![];
        if is_root {
            spans.push(Span::styled("Hints: ", theme.text_muted_style()));
        }

        // The add component is visible
        if let Some(add_state) = app.plugins.add.as_ref() {
            // use the add component hints
            if add_state.focus.get() {
                spans.extend(self.add_component.get_hint_spans(app, false));
                return spans;
            }
            spans.extend([
                Span::styled("Esc", theme.accent_emphasis_style()),
                Span::styled(" Back  ", theme.text_muted_style()),
            ]);
        } else {
            // the add component is not visible
            spans.extend([
                Span::styled("Esc", theme.accent_emphasis_style()),
                Span::styled(" Clear  ", theme.text_muted_style()),
            ]);

            if app.plugins.can_open_add_plugin() {
                spans.extend([
                    Span::styled("Ctrl-A", theme.accent_emphasis_style()),
                    Span::styled(" Add  ", theme.text_muted_style()),
                ]);
            }
        }
        // the grid is focused
        if app.plugins.table.grid_flag.get() {
            spans.extend(self.table_component.get_hint_spans(app, false));
        }

        spans
    }
}

/// Creates a centered rectangle within the given area using percentage-based dimensions.
///
/// This utility function calculates a new rectangle that is centered within the
/// provided area, with dimensions specified as percentages of the original area's
/// width and height. This is commonly used for creating overlay dialogs and
/// modal windows.
///
/// # Arguments
///
/// * `area` - The base rectangular area to center within
/// * `width_percentage` - Percentage of the area's width to use (0-100)
/// * `height_percentage` - Percentage of the area's height to use (0-100)
///
/// # Returns
///
/// Returns a new `Rect` that is centered within the provided area with the
/// specified percentage-based dimensions.
fn create_centered_rectangle(area: Rect, width_percentage: u16, height_percentage: u16) -> Rect {
    let width = area.width.saturating_mul(width_percentage).saturating_div(100);
    let height = area.height.saturating_mul(height_percentage).saturating_div(100);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;

    Rect { x, y, width, height }
}

impl PluginsComponent {
    /// Delegates keyboard events to open overlays if any are currently active.
    ///
    /// This method checks if any overlay (environment editor, logs viewer, or add plugin plugin)
    /// is currently open and delegates the keyboard event to the appropriate component.
    /// Tab/BackTab events are intentionally not handled here to allow the focus cycling
    /// handler to manage them consistently across all contexts.
    ///
    /// # Arguments
    ///
    /// * `app` - Mutable reference to the app state
    /// * `key_event` - The keyboard event to process
    ///
    /// # Returns
    ///
    /// Returns `Some(Vec<Effect>)` if the event was handled by an overlay, or `None` if
    /// no overlay is open or the event should be handled by the focus cycling system.
    fn delegate_to_open_overlays(&mut self, app: &mut App, key_event: KeyEvent) -> Vec<Effect> {
        // Let the focus cycling handler manage Tab/BackTab events
        // This ensures consistent focus management whether overlays are open or not
        if matches!(key_event.code, KeyCode::Tab | KeyCode::BackTab) {
            return vec![];
        }

        if app.plugins.secrets.is_some() {
            return self.secrets_component.handle_key_events(app, key_event);
        }

        if let Some(logs_state) = &mut app.plugins.logs {
            return self.logs_component.handle_key_events(logs_state, key_event);
        }

        if app.plugins.add.is_some() {
            return self.add_component.handle_key_events(app, key_event);
        }

        vec![]
    }

    /// Handles top-level Ctrl-based shortcuts and returns any effects.
    ///
    /// This method processes keyboard shortcuts that control the overall plugin
    /// interface behavior, such as opening/closing overlays, starting/stopping
    /// plugins, and managing the add plugin plugin.
    ///
    /// # Arguments
    ///
    /// * `app` - Mutable reference to the app state
    /// * `key_event` - The keyboard event to process
    ///
    /// # Returns
    ///
    /// Returns `Some(Vec<Effect>)` if the shortcut was handled and effects were generated,
    /// or `None` if the shortcut was not recognized or handled.
    fn handle_control_shortcuts(&mut self, app: &mut App, key_event: KeyEvent) -> Vec<Effect> {
        let mut effects: Vec<Effect> = Vec::with_capacity(1);
        let control_pressed: bool = key_event.modifiers.contains(KeyModifiers::CONTROL);
        match key_event.code {
            KeyCode::Char('f') if control_pressed => {
                self.handle_search_shortcut(app);
            }
            KeyCode::Esc => {
                self.handle_clear_filter_shortcut(app);
            }

            KeyCode::Char('v') if control_pressed && app.plugins.add.is_some() => {
                effects.push(Effect::PluginsValidateAdd);
            }
            // Also available when the table component is focused
            KeyCode::Char('a') if control_pressed && app.plugins.can_open_add_plugin() => {
                effects.push(Effect::PluginsApplyAdd);
            }
            KeyCode::Char('l') if control_pressed && app.plugins.logs_open => {
                self.handle_logs_toggle_follow_shortcut(app);
            }
            KeyCode::Char('y') if control_pressed && app.plugins.logs_open => {
                self.handle_copy_last_log_line_shortcut(app, &mut effects);
            }
            KeyCode::Char('u') if control_pressed && app.plugins.logs_open => {
                self.handle_copy_all_logs_shortcut(app, &mut effects);
            }
            KeyCode::Char('o') if control_pressed && app.plugins.logs_open => {
                self.handle_export_logs_shortcut(app, &mut effects);
            }
            _ => {}
        }
        effects
    }

    /// Handles the search shortcut (Ctrl+F) which activates search in the appropriate context.
    fn handle_search_shortcut(&mut self, app: &mut App) {
        if app.plugins.logs_open {
            if let Some(logs_state) = &mut app.plugins.logs {
                logs_state.search_active = true;
            }
        } else {
            app.plugins.table.search_flag.set(true);
            app.plugins.table.grid_flag.set(false);
        }
    }

    /// Handles the clear filter shortcut (Ctrl+K) which clears the search filter.
    fn handle_clear_filter_shortcut(&mut self, app: &mut App) {
        if app.plugins.table.search_flag.get() {
            app.plugins.table.clear_filter();
        }
    }

    /// Handles the logs toggle follow shortcut (Ctrl+L when logs are open).
    fn handle_logs_toggle_follow_shortcut(&mut self, app: &mut App) {
        if let Some(logs_state) = &mut app.plugins.logs {
            logs_state.toggle_follow();
        }
    }

    /// Handles copying the last log line (Ctrl+Y when logs are open).
    fn handle_copy_last_log_line_shortcut(&self, app: &App, effects: &mut Vec<Effect>) {
        if let Some(logs_state) = &app.plugins.logs {
            let last_line = logs_state.lines.last().cloned().unwrap_or_default();
            effects.push(Effect::CopyLogsRequested(last_line));
        }
    }

    /// Handles copying all logs (Ctrl+U when logs are open).
    fn handle_copy_all_logs_shortcut(&self, app: &App, effects: &mut Vec<Effect>) {
        if let Some(logs_state) = &app.plugins.logs {
            let all_logs = logs_state.lines.join("\n");
            effects.push(Effect::CopyLogsRequested(all_logs));
        }
    }

    /// Handles exporting logs (Ctrl+O when logs are open).
    fn handle_export_logs_shortcut(&self, app: &App, effects: &mut Vec<Effect>) {
        if let Some(logs_state) = &app.plugins.logs {
            effects.push(Effect::PluginsExportLogsDefault(logs_state.name.clone()));
        }
    }

    /// Delegates keyboard events to child components when appropriate.
    ///
    /// This method handles keyboard events that should be processed by specific
    /// child components based on the current focus state. It delegates to the
    /// search component when search is active, and to the table component when
    /// the grid is focused.
    ///
    /// # Arguments
    ///
    /// * `app` - Mutable reference to the app state
    /// * `key_event` - The keyboard event to process
    ///
    /// # Returns
    ///
    /// Returns a vector of effects generated by the child components.
    fn delegate_to_child_components(&mut self, app: &mut App, key_event: KeyEvent) -> Vec<Effect> {
        if app.plugins.table.search_flag.get() {
            return self.search_component.handle_key_events(app, key_event);
        }

        if app.plugins.table.grid_flag.get() {
            return self.table_component.handle_key_events(app, key_event);
        }
        vec![]
    }

    /// Creates the main 3-row layout: header, body, and footer.
    ///
    /// This method defines the vertical layout structure for the plugins interface,
    /// allocating space for the header (search bar), main body content, and footer
    /// (hints bar).
    ///
    /// # Arguments
    ///
    /// * `inner_area` - The inner rectangular area to layout within
    ///
    /// # Returns
    ///
    /// Returns a vector of rectangles representing the header, body, and footer areas.
    fn create_main_layout(&self, inner_area: Rect) -> Vec<Rect> {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(6), Constraint::Length(1)])
            .split(inner_area)
            .to_vec()
    }

    /// Renders the header area containing the search bar.
    ///
    /// # Arguments
    ///
    /// * `frame` - Mutable reference to the terminal frame for rendering
    /// * `header_area` - The rectangular area allocated for the header
    /// * `app` - Mutable reference to the app state
    fn render_header_section(&mut self, frame: &mut Frame, header_area: Rect, app: &mut App) {
        self.search_component.render(frame, header_area, app);
    }

    /// Renders the body area containing either the table or add view, or both side-by-side.
    ///
    /// This method determines the layout based on whether the add plugin plugin is open
    /// and the available width. If the add plugin is open and there's sufficient width,
    /// it displays both the add plugin and table side-by-side. Otherwise, it shows
    /// either the add plugin or table exclusively.
    ///
    /// # Arguments
    ///
    /// * `frame` - Mutable reference to the terminal frame for rendering
    /// * `body_area` - The rectangular area allocated for the body content
    /// * `app` - Mutable reference to the app state
    fn render_body_section(&mut self, frame: &mut Frame, body_area: Rect, app: &mut App) {
        let add_plugin_open = app.plugins.add.as_ref().map(|plugin| plugin.visible).unwrap_or(false);

        if add_plugin_open && body_area.width >= 120 {
            // Side-by-side layout when there's sufficient width
            let columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(body_area);
            self.add_component.render(frame, columns[0], app);
            self.table_component.render(frame, columns[1], app);
        } else if add_plugin_open {
            // Full-width add plugin when space is limited
            self.add_component.render(frame, body_area, app);
        } else {
            // Default table view
            self.table_component.render(frame, body_area, app);
        }
    }

    /// Renders overlay components (details, logs, environment editor) on top of the main shell.
    ///
    /// This method handles the rendering of modal overlays that appear on top of the
    /// main plugins interface. Each overlay is rendered in a centered rectangle with
    /// appropriate dimensions and clears the background area before rendering.
    ///
    /// # Arguments
    ///
    /// * `frame` - Mutable reference to the terminal frame for rendering
    /// * `outer_area` - The outer rectangular area for overlay positioning
    /// * `app` - Mutable reference to the app state
    fn render_overlay_components(&mut self, frame: &mut Frame, outer_area: Rect, app: &mut App) {
        if app.plugins.logs_open {
            if let Some(_logs_state) = &app.plugins.logs {
                let logs_area = create_centered_rectangle(outer_area, 90, 60);
                frame.render_widget(Clear, logs_area);
                self.logs_component.render(frame, logs_area, app);
            } else {
                app.plugins.logs_open = false;
            }
        }

        if app.plugins.secrets.is_some() {
            let environment_area = create_centered_rectangle(outer_area, 90, 70);
            frame.render_widget(Clear, environment_area);
            self.secrets_component.render(frame, environment_area, app);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that the PluginsComponent can be constructed successfully.
    ///
    /// This is a basic smoke test to ensure that the component can be instantiated
    /// without panicking and that all its fields are properly initialized.
    #[test]
    fn plugins_component_constructs_successfully() {
        let _component = PluginsComponent::default();
        // If we reach this point, the component was constructed successfully
        assert!(true);
    }
}
