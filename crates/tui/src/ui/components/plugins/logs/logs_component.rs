use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use heroku_types::{Effect, Msg};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::Span,
    widgets::{Block, Borders, List, ListItem},
};

use crate::ui::theme::{Theme, theme_helpers as th};
use crate::{app::App, ui::components::component::Component};

use super::state::PluginLogsState;

/// Component for rendering the plugin logs drawer overlay.
#[derive(Debug, Default)]
pub struct PluginsLogsComponent;

impl PluginsLogsComponent {
    /// Handle key events specific to the "logs" drawer.
    pub fn handle_key_events(&self, logs: &mut PluginLogsState, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Backspace if logs.search_active => {
                logs.search_query.pop();
                Vec::new()
            }
            KeyCode::Char(c) if logs.search_active && !key.modifiers.contains(KeyModifiers::CONTROL) => {
                logs.search_query.push(c);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }
}

impl Component for PluginsLogsComponent {
    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        if let Some(logs) = &app.plugins.logs {
            let theme = &*app.ctx.theme;
            self.render_logs_drawer(frame, area, theme, logs);
        }
    }

    fn handle_message(&mut self, app: &mut App, msg: &Msg) -> Vec<Effect> {
        match msg {
            Msg::ExecCompleted(outcome) => {
                app.logs.process_general_execution_result(&outcome);
            }
            _ => {}
        }

        Vec::new()
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        [
            Span::styled("Ctrl+F", theme.accent_emphasis_style()),
            Span::styled(" Search ", theme.text_muted_style()),
            Span::styled("Ctrl+L", theme.accent_emphasis_style()),
            Span::styled(" Follow ", theme.text_muted_style()),
            Span::styled("Ctrl+Y", theme.accent_emphasis_style()),
            Span::styled(" Copy ", theme.text_muted_style()),
            Span::styled("Ctrl+U", theme.accent_emphasis_style()),
            Span::styled(" Copy all ", theme.text_muted_style()),
            Span::styled("Ctrl+O", theme.accent_emphasis_style()),
            Span::styled(" Export ", theme.text_muted_style()),
        ]
        .to_vec()
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
                format!("Logs — {}", logs.name),
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
