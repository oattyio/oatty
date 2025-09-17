//! Plugins details component for displaying plugin information in a modal overlay.
use heroku_types::{Effect, Msg};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::Span,
    widgets::{Block, Borders, Paragraph},
};

use crate::ui::components::component::Component;
use crate::ui::theme::{Theme, helpers as th};

use super::PluginsState;

/// Component for rendering the plugin details modal overlay.
#[derive(Debug, Default)]
pub struct PluginsDetailsComponent;

impl Component for PluginsDetailsComponent {
    fn handle_key_events(&mut self, _app: &mut crate::app::App, _key: crossterm::event::KeyEvent) -> Vec<Effect> {
        Vec::new()
    }

    fn update(&mut self, _app: &mut crate::app::App, _msg: &Msg) -> Vec<Effect> {
        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut crate::app::App) {
        let theme = &*app.ctx.theme;
        self.render_details(frame, area, theme, &app.plugins);
    }
}

impl PluginsDetailsComponent {
    fn render_details(&self, frame: &mut Frame, area: Rect, theme: &dyn Theme, state: &PluginsState) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border_style(true))
            .style(th::panel_style(theme))
            .title(Span::styled(
                "Plugin Details",
                theme.text_secondary_style().add_modifier(Modifier::BOLD),
            ));
        frame.render_widget(block, area);

        // Inner layout
        let inner = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };

        let lines = if let Some(item) = state.selected_item() {
            vec![
                format!("Name: {}", item.name),
                format!("Status: {}", item.status),
                format!("Command/BaseUrl: {}", item.command_or_url),
                format!(
                    "Tags: {}",
                    if item.tags.is_empty() {
                        "-".to_string()
                    } else {
                        item.tags.join(", ")
                    }
                ),
                match item.latency_ms {
                    Some(ms) => format!("Latency: {} ms", ms),
                    None => "Latency: -".into(),
                },
                format!("Last error: {}", item.last_error.clone().unwrap_or_else(|| "-".into())),
                "".to_string(),
                "Actions: [S]tart  S[t]op  [R]estart  [E]nv  [L]ogs  [b]ack".to_string(),
            ]
        } else {
            vec!["No selection".to_string()]
        };

        let text = lines.join("\n");
        let para = Paragraph::new(text).style(theme.text_primary_style());
        frame.render_widget(para, inner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugins_details_component_constructs() {
        let _c = PluginsDetailsComponent::default();
        assert!(true);
    }
}
