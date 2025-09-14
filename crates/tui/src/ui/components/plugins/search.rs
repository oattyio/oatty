//! Plugins search component for handling search input and filtering.
//!
//! Renders a simple header block containing the current filter, places the
//! cursor at the end when focused, and updates filter text based on keystrokes
//! (excluding Ctrl-modified keys).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};

use crate::app::Effect;
use crate::ui::components::component::Component;
use crate::ui::theme::{Theme, helpers as th};

use super::state::PluginsState;

/// Component for rendering the plugins search input.
#[derive(Debug, Default)]
pub struct PluginsSearchComponent;

impl PluginsSearchComponent {
    /// Handle key events specific to the search input.
    pub fn handle_key_events(&self, app: &mut crate::app::App, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Backspace if app.plugins.search_flag.get() => {
                Self::remove_last(app);
                Vec::new()
            }
            KeyCode::Char(c) if app.plugins.search_flag.get() => {
                Self::insert_char(app, key, c);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn remove_last(app: &mut crate::app::App) {
        app.plugins.filter.pop();
        app.plugins.selected = Some(0);
        app.mark_dirty();
    }

    fn insert_char(app: &mut crate::app::App, key: KeyEvent, c: char) {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return;
        }
        app.plugins.filter.push(c);
        app.plugins.selected = Some(0);
        app.mark_dirty();
    }
}

impl Component for PluginsSearchComponent {
    fn handle_key_events(&mut self, app: &mut crate::app::App, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Backspace if app.plugins.search_flag.get() => {
                app.plugins.filter.pop();
                app.plugins.selected = Some(0);
                app.mark_dirty();
                Vec::new()
            }
            KeyCode::Char(c) if app.plugins.search_flag.get() => {
                if !key.modifiers.contains(KeyModifiers::CONTROL) {
                    app.plugins.filter.push(c);
                    app.plugins.selected = Some(0);
                    app.mark_dirty();
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn update(&mut self, _app: &mut crate::app::App, _msg: &crate::app::Msg) -> Vec<Effect> {
        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut crate::app::App) {
        let theme = &*app.ctx.theme;
        self.render_search_panel(frame, area, theme, &app.plugins);
    }
}

impl PluginsSearchComponent {
    /// Render the search header with the current filter value.
    fn render_search_panel(&self, frame: &mut Frame, area: Rect, theme: &dyn Theme, state: &PluginsState) {
        let search_focused = state.search_flag.get();
        let header_block = th::block(theme, Some("Search Plugins"), search_focused);

        // Render input inside the block
        let header_inner = header_block.inner(area);
        let header = Paragraph::new(state.filter.as_str())
            .style(theme.text_primary_style())
            .block(header_block);
        frame.render_widget(header, area);

        // Position cursor at end of input when focused
        if search_focused {
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
