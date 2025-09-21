//! Plugins details component for displaying plugin information in a modal overlay.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use heroku_types::Effect;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::ui::theme::{Theme, theme_helpers as th};
use crate::{
    app::App,
    ui::{components::component::Component, utils::centered_rect},
};

use super::{PluginListItem, PluginsState};

/// Component for rendering the plugin details modal overlay.
#[derive(Debug, Default)]
pub struct PluginsDetailsComponent;

impl Component for PluginsDetailsComponent {
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let control_pressed: bool = key.modifiers.contains(KeyModifiers::CONTROL);
        let mut effects = vec![];
        match key.code {
            KeyCode::Char('y') if control_pressed => {
                if let Some(item) = app.plugins.table.selected_item() {
                    effects.push(Effect::CopyToClipboardRequested(item.to_string()));
                }
            }
            KeyCode::Esc => {
                effects.push(Effect::CloseModal);
            }
            _ => {}
        };
        effects
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut crate::app::App) {
        let theme = &*app.ctx.theme;
        let area = centered_rect(60, 50, area);
        frame.render_widget(Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border_style(true))
            .style(th::panel_style(theme))
            .title(Span::styled(
                "Plugin Details",
                theme.text_secondary_style().add_modifier(Modifier::BOLD),
            ));

        let content_area = block.inner(area);
        frame.render_widget(block, area);

        let layouts = Layout::vertical([Constraint::Percentage(100), Constraint::Min(1)]).split(content_area);

        self.render_details(frame, layouts[0], theme, &app.plugins);

        let hints = Line::from(self.get_hint_spans(app, true)).style(theme.text_muted_style());
        let paragraph = Paragraph::new(hints);
        frame.render_widget(paragraph, layouts[1]);
    }

    fn get_hint_spans(&self, app: &App, is_root: bool) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        let mut spans = vec![];
        if is_root {
            spans.push(Span::styled("Hints: ", theme.text_muted_style()));
        }
        spans.extend([
            Span::styled("Esc", theme.accent_emphasis_style()),
            Span::styled(" Close  ", theme.text_muted_style()),
            Span::styled("Ctrl+Y", theme.accent_emphasis_style()),
            Span::styled(" Copy  ", theme.text_muted_style()),
        ]);

        spans
    }
}

impl PluginsDetailsComponent {
    /// Render the modal container and its detailed content for the currently selected plugin.
    fn render_details(&self, frame: &mut Frame, area: Rect, theme: &dyn Theme, plugins_state: &PluginsState) {
        let detail_lines = self.build_detail_lines(theme, plugins_state);
        let paragraph = Paragraph::new(detail_lines).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    }

    /// Build the styled text lines that describe the selected plugin, or show an empty-state message when nothing is selected.
    fn build_detail_lines(&self, theme: &dyn Theme, plugins_state: &PluginsState) -> Vec<Line<'static>> {
        if let Some(item) = plugins_state.table.selected_item() {
            self.build_selected_item_lines(item, theme)
        } else {
            self.build_empty_state_lines(theme)
        }
    }

    /// Construct the full list of detail rows for a specific plugin.
    fn build_selected_item_lines(&self, item: &PluginListItem, theme: &dyn Theme) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        lines.push(self.label_value_line("Name", item.name.clone(), theme));
        lines.push(self.label_value_line("Status", item.status.clone(), theme));
        lines.push(self.label_value_line("Command/BaseUrl", item.command_or_url.clone(), theme));

        let tags_value = if item.tags.is_empty() {
            "-".to_string()
        } else {
            item.tags.join(", ")
        };
        lines.push(self.label_value_line("Tags", tags_value, theme));

        let latency_value = item
            .latency_ms
            .map(|milliseconds| format!("{milliseconds} ms"))
            .unwrap_or_else(|| "-".to_string());
        lines.push(self.label_value_line("Latency", latency_value, theme));

        let last_error_value = item.last_error.clone().unwrap_or_else(|| "-".to_string());
        lines.push(self.last_error_line("Last error", last_error_value, theme));

        lines
    }

    /// Provide messaging when no plugin is currently selected.
    fn build_empty_state_lines(&self, theme: &dyn Theme) -> Vec<Line<'static>> {
        vec![Line::from(Span::styled("No selection", theme.text_muted_style()))]
    }

    /// Format a `Label: value` line with emphasis on the label and muted styling for the value.
    fn label_value_line(&self, label: &str, value: String, theme: &dyn Theme) -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("{label}: "), theme.accent_emphasis_style()),
            Span::styled(value, theme.text_muted_style()),
        ])
    }

    /// Format the last error line, highlighting actual error text with the theme's error status styling.
    fn last_error_line(&self, label: &str, value: String, theme: &dyn Theme) -> Line<'static> {
        let value_style = if value == "-" {
            theme.text_muted_style()
        } else {
            theme.status_error()
        };

        Line::from(vec![
            Span::styled(format!("{label}: "), theme.accent_emphasis_style()),
            Span::styled(value, value_style),
        ])
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
