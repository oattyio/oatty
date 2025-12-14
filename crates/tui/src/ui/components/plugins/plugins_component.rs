//! Top-level Plugins view: orchestrates search, table, add view, details,
//! logs, and environment editor. Handles focus routing, shortcuts, and responsive
//! layout whether shown fullscreen or as a centered overlay.
//!
//! This module contains the main coordinator component for the MCP (Model Context Protocol)
//! plugins management interface, providing a unified view that can display different
//! subcomponents based on user interaction and app state.

use super::{PluginsEditComponent, PluginsTableComponent};
use crate::ui::components::plugins::plugin_editor::state::PluginEditViewState;
use crate::{
    app::App,
    ui::{components::component::Component, theme::theme_helpers::build_hint_spans},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use oatty_types::{Effect, Msg};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::Span,
};

/// Top-level Plugins view component that orchestrates all plugin-related UI elements.
///
/// This component manages the display and interaction of various plugin management
/// interfaces including the plugin list table, search functionality, add plugin,
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
    /// Child component for the add plugin
    edit_component: PluginsEditComponent,
}

impl Component for PluginsComponent {
    fn handle_message(&mut self, app: &mut App, msg: &Msg) -> Vec<Effect> {
        if let Msg::ExecCompleted(outcome) = msg {
            return app.plugins.handle_execution_completion(outcome);
        }
        Vec::new()
    }

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

    /// Handles mouse events by delegating them to various UI components and aggregating their effects.
    ///
    /// This function processes a given mouse event by passing it to the `table_component`,
    /// `logs_component`, and `edit_component`. Each component handles the mouse event independently,
    /// and any resulting effects are aggregated and returned as a single list.
    ///
    /// # Arguments
    ///
    /// * `app` - A mutable reference to the application's state.
    /// * `mouse` - The `MouseEvent` that needs to be handled.
    ///
    /// # Returns
    ///
    /// A vector of `Effect` instances representing the outcomes or side effects resulting from the
    /// mouse event as handled by the various components.
    ///
    /// # Components Handled
    ///
    /// * `table_component`
    /// * `logs_component`
    /// * `edit_component`
    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let mut effects = vec![];
        effects.extend(self.table_component.handle_mouse_events(app, mouse));
        effects.extend(self.edit_component.handle_mouse_events(app, mouse));
        effects
    }

    /// Renders the plugin component and all its children.
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
        self.render_body_section(frame, area, app);
    }

    /// Renders the hint bar content
    ///
    /// This method provides an area to render hints contextually
    /// and delegates to child components depending on focus.
    ///
    /// # Arguments
    ///
    /// * `frame` - Mutable reference to the terminal frame for rendering
    /// * `area` - The rectangular area available for rendering
    /// * `app` - Mutable reference to the app state
    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        let mut spans = Vec::new();

        // The add component is visible
        if let Some(add_state) = app.plugins.add.as_ref() {
            // use the add component hints
            if add_state.container_focus.get() {
                spans.extend(self.edit_component.get_hint_spans(app));
                return spans;
            }
            spans.extend(build_hint_spans(theme, &[("Esc", " Back  ")]));
        } else {
            // the add component is not visible
            spans.extend(build_hint_spans(theme, &[("Esc", " Clear  "), ("Ctrl+A", " Add  ")]));
        }
        // the grid is focused
        if app.plugins.table.f_grid.get() {
            spans.extend(self.table_component.get_hint_spans(app));
        }

        spans
    }
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

        if app.plugins.add.as_ref().is_some_and(|add_state| add_state.container_focus.get()) {
            return self.edit_component.handle_key_events(app, key_event);
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
            KeyCode::Char('a') if control_pressed => {
                let edit_view_state = PluginEditViewState::new();
                app.focus.focus(&edit_view_state.f_transport);
                app.plugins.add = Some(edit_view_state);
            }
            _ => {}
        }
        effects
    }

    /// Handles the search shortcut (Ctrl+F) which activates search in the appropriate context.
    fn handle_search_shortcut(&mut self, app: &mut App) {
        app.plugins.table.f_search.set(true);
        app.plugins.table.f_grid.set(false);
    }

    /// Handles the clear filter shortcut (Ctrl+K) which clears the search filter.
    fn handle_clear_filter_shortcut(&mut self, app: &mut App) {
        if app.plugins.table.f_search.get() {
            app.plugins.table.clear_filter();
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
        if app.plugins.table.container_focus.get() {
            return self.table_component.handle_key_events(app, key_event);
        }
        vec![]
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
            let columns = Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]).split(body_area);
            self.edit_component.render(frame, columns[0], app);
            self.table_component.render(frame, columns[1], app);
        } else if add_plugin_open {
            // Full-width add plugin when space is limited
            self.edit_component.render(frame, body_area, app);
        } else {
            // Default table view
            self.table_component.render(frame, body_area, app);
        }
    }
}
