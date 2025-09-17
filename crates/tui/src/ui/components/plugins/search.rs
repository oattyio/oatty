//! Plugins search component for handling search input and filtering.
//!
//! Renders a simple header block containing the current filter, places the
//! cursor at the end when focused, and updates filter text based on keystrokes
//! (excluding Ctrl-modified keys).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use heroku_types::{Effect, Msg};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};

use crate::app::App;
use crate::ui::components::component::Component;
use crate::ui::theme::{Theme, helpers as th};

use super::PluginsState;

/// Component for rendering and handling the plugins search input.
///
/// This component is responsible for:
/// - Processing simple text input for the search filter
/// - Ignoring control-modified character inputs
/// - Rendering the search header and positioning the cursor when focused
///
/// It follows the TUI component contract and mutates application state under
/// `app.plugins` directly for local UI interactions.
#[derive(Debug, Default)]
pub struct PluginsSearchComponent;

impl PluginsSearchComponent {
    /// Handles key events specific to the search input (convenience method).
    ///
    /// This mirrors the component trait handler and is provided as a thin
    /// wrapper for callers that may not use the trait object directly.
    ///
    /// # Arguments
    /// - `application`: Mutable reference to the application state
    /// - `key_event`: The keyboard event to process
    ///
    /// # Returns
    /// Returns a vector of effects to be processed by the application runtime.
    pub fn handle_key_events(&self, application: &mut App, key_event: KeyEvent) -> Vec<Effect> {
        Self::process_key_event(application, key_event)
    }

    /// Removes the last character from the search filter and normalizes selection.
    fn remove_last_filter_character(application: &mut App) {
        application.plugins.filter.pop();
        application.plugins.selected = Some(0);
    }

    /// Inserts a character into the search filter unless a control modifier is pressed.
    fn insert_filter_character_unless_control(application: &mut App, key_event: KeyEvent, character: char) {
        if key_event.modifiers.contains(KeyModifiers::CONTROL) {
            return;
        }
        application.plugins.filter.push(character);
        application.plugins.selected = Some(0);
    }

    /// Core key event processor shared by both inherent and trait implementations.
    fn process_key_event(application: &mut App, key_event: KeyEvent) -> Vec<Effect> {
        match key_event.code {
            KeyCode::Backspace if application.plugins.search_flag.get() => {
                Self::remove_last_filter_character(application);
                Vec::new()
            }
            KeyCode::Char(character) if application.plugins.search_flag.get() => {
                Self::insert_filter_character_unless_control(application, key_event, character);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }
}

impl Component for PluginsSearchComponent {
    fn handle_key_events(&mut self, application: &mut App, key_event: KeyEvent) -> Vec<Effect> {
        Self::process_key_event(application, key_event)
    }

    fn update(&mut self, _app: &mut App, _msg: &Msg) -> Vec<Effect> {
        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let theme = &*app.ctx.theme;
        self.render_search_panel(frame, area, theme, &app.plugins);
    }
}

impl PluginsSearchComponent {
    /// Render the search header with the current filter value.
    ///
    /// # Arguments
    /// - `frame`: Terminal frame used for rendering
    /// - `area`: The rectangular area to render into
    /// - `theme`: Active theme used for styles
    /// - `state`: Reference to the plugins view state
    fn render_search_panel(&self, frame: &mut Frame, area: Rect, theme: &dyn Theme, state: &PluginsState) {
        let is_search_focused = state.search_flag.get();
        let header_block = th::block(theme, Some("Search Plugins"), is_search_focused);

        // Render input inside the block
        let header_inner = header_block.inner(area);
        let header = Paragraph::new(state.filter.as_str())
            .style(theme.text_primary_style())
            .block(header_block);
        frame.render_widget(header, area);

        // Position cursor at end of input when focused
        if is_search_focused {
            let x = header_inner.x.saturating_add(state.filter.chars().count() as u16);
            let y = header_inner.y;
            frame.set_cursor_position((x, y));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugins_search_component_constructs() {
        let _c = PluginsSearchComponent::default();
        assert!(true);
    }
}
