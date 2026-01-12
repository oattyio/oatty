//! Combined key/value table and inline editor for the plugin add flow.
//!
//! This component encapsulates the tabular display of key/value pairs and the
//! inline editing experience that previously lived directly inside
//! `add.rs`. It centralizes keyboard handling, rendering, and cursor
//! positioning so the parent `PluginsAddComponent` can remain focused on the
//! surrounding form controls.
//!
//! The component follows the TUI component pattern by implementing the `Component`
//! trait and managing its own rendering and event handling. It delegates state
//! management to the parent `PluginAddViewState` while maintaining focus on
//! the user interaction experience.

use std::rc::Rc;

use crossterm::event::KeyEvent;
use oatty_types::Effect;
use ratatui::{Frame, layout::Rect, text::Span};

use crate::{
    app::App,
    ui::components::{Component, common::key_value_editor::KeyValueEditorView},
};

/// Component responsible for rendering and editing key/value pairs.
///
/// This component provides a tabular interface for managing key/value pairs
/// with inline editing capabilities. It supports both navigation and editing
/// modes, with keyboard shortcuts for common operations like adding new rows,
/// deleting existing rows, and switching between key and value fields.
///
/// The component is designed to be stateless, delegating all state management
/// to the parent `PluginAddViewState`. This allows for better separation of
/// concerns and easier testing.
#[derive(Debug, Default)]
pub struct KeyValueEditorComponent {
    view: KeyValueEditorView,
}

impl Component for KeyValueEditorComponent {
    /// Handle keyboard events for the key/value editor component.
    ///
    /// This method processes keyboard input when the component has focus and
    /// updates validation messaging based on inline editing actions.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state containing the plugin add view state
    /// * `key_event` - The keyboard event to process
    ///
    /// # Returns
    ///
    /// A vector of effects that should be processed by the application runtime.
    fn handle_key_events(&mut self, app: &mut App, key_event: KeyEvent) -> Vec<Effect> {
        let Some(add_state) = app.plugins.add.as_mut() else {
            return vec![];
        };
        self.view
            .handle_key_event(&mut add_state.kv_editor, key_event, Rc::clone(&app.focus));

        vec![]
    }

    /// Render the key/value editor component.
    ///
    /// This method renders the complete key/value editor interface.
    ///
    /// # Arguments
    ///
    /// * `frame` - The Ratatui frame for rendering
    /// * `area` - The available rendering area
    /// * `app` - The application state containing the plugin add view state
    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let Some(add_state) = app.plugins.add.as_mut() else {
            return;
        };
        add_state.update_key_value_table_label();
        let theme = &*app.ctx.theme;
        self.view.render_with_state(frame, area, theme, &mut add_state.kv_editor);
    }

    /// Get hint spans for the key/value editor component.
    ///
    /// This method provides contextual keyboard shortcuts and hints for
    /// inline table editing.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state containing the plugin add view state
    /// # Returns
    ///
    /// A vector of styled spans representing the available keyboard shortcuts.
    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        let mut spans = vec![];

        self.view.add_table_hints(&mut spans, theme);

        spans
    }
}
