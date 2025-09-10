//! Plugins logs component for displaying plugin logs in a drawer overlay.
//!
//! Supports search within the drawer, follow toggling (handled by parent), and
//! copying/exporting logs via effects. This component focuses on rendering the
//! list and handling in-drawer search input.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::Span,
    widgets::{Block, Borders, List, ListItem},
};

use crate::app::Effect;
use crate::ui::components::component::Component;
use crate::ui::theme::{Theme, helpers as th};

use super::state::PluginLogsState;

/// Component for rendering the plugin logs drawer overlay.
#[derive(Debug, Default)]
pub struct PluginsLogsComponent;

impl PluginsLogsComponent {
    /// Handle key events specific to the logs drawer.
    pub fn handle_key_events(&self, logs: &mut PluginLogsState, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Backspace if logs.search_active => { logs.search_query.pop(); Vec::new() }
            KeyCode::Char(c) if logs.search_active && !key.modifiers.contains(KeyModifiers::CONTROL) => { logs.search_query.push(c); Vec::new() }
            _ => Vec::new(),
        }
    }
}

impl Component for PluginsLogsComponent {
    fn handle_key_events(&mut self, _app: &mut crate::app::App, _key: KeyEvent) -> Vec<Effect> {
        Vec::new()
    }

    fn update(&mut self, _app: &mut crate::app::App, _msg: &crate::app::Msg) -> Vec<Effect> {
        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut crate::app::App) {
        if let Some(logs) = &app.plugins.logs {
            let theme = &*app.ctx.theme;
            self.render_logs_drawer(frame, area, theme, logs);
        }
    }
}

impl PluginsLogsComponent {
    /// Render the logs drawer title and filtered log lines as a list.
    fn render_logs_drawer(&self, frame: &mut Frame, area: Rect, theme: &dyn Theme, logs: &PluginLogsState) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border_style(true))
            .style(th::panel_style(theme))
            .title(Span::styled(
                format!(
                    "Logs — {}  [Ctrl-f] search  [Ctrl-l] follow  [Ctrl-y] copy  [Ctrl-u] copy all  [Ctrl-o] export",
                    logs.name
                ),
                theme.text_secondary_style().add_modifier(Modifier::BOLD),
            ));

        let items: Vec<ListItem> = logs.filtered().cloned().map(ListItem::new).collect();

        let list = List::new(items).block(block).style(theme.text_primary_style());
        frame.render_widget(list, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugins_logs_component_constructs() {
        let _c = PluginsLogsComponent::default();
        assert!(true);
    }
}
