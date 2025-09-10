//! Plugins table component for displaying and navigating the main plugins list.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    widgets::{Block, Borders, Row, Table},
};

use crate::app::Effect;
use crate::ui::components::component::Component;
use crate::ui::theme::{Theme, helpers as th};

use super::state::PluginsState;

/// Component for rendering the plugins table with selection and navigation.
#[derive(Debug, Default)]
pub struct PluginsTableComponent;

impl PluginsTableComponent {
    fn move_selection_up(app: &mut crate::app::App) {
        let filtered_indices = app.plugins.filtered_indices();
        if filtered_indices.is_empty() { return; }
        let selected_index = app.plugins
            .selected
            .unwrap_or(0)
            .min(filtered_indices.len().saturating_sub(1));
        let new_position = selected_index.saturating_sub(1);
        app.plugins.selected = Some(new_position);
        app.mark_dirty();
    }

    fn move_selection_down(app: &mut crate::app::App) {
        let filtered_indices = app.plugins.filtered_indices();
        if filtered_indices.is_empty() { return; }
        let selected_index = app.plugins
            .selected
            .unwrap_or(0)
            .min(filtered_indices.len().saturating_sub(1));
        let new_position = (selected_index + 1).min(filtered_indices.len().saturating_sub(1));
        app.plugins.selected = Some(new_position);
        app.mark_dirty();
    }
}

impl Component for PluginsTableComponent {
    fn handle_key_events(&mut self, app: &mut crate::app::App, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Up if app.plugins.grid_flag.get() => { Self::move_selection_up(app); Vec::new() }
            KeyCode::Down if app.plugins.grid_flag.get() => { Self::move_selection_down(app); Vec::new() }
            _ => Vec::new(),
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut crate::app::App) {
        let theme = &*app.ctx.theme;
        self.render_plugins_table(frame, area, theme, &app.plugins);
    }
}

impl PluginsTableComponent {
    /// Render the plugins table with header and selection styling.
    fn render_plugins_table(&self, frame: &mut Frame, area: Rect, theme: &dyn Theme, state: &PluginsState) {
        let header_cells = ["Name", "Status", "Command/BaseUrl", "Tags"]
            .into_iter()
            .map(|h| ratatui::text::Span::styled(h, th::table_header_style(theme)));
        let header = Row::new(header_cells).style(th::table_header_row_style(theme));

        let filtered = state.filtered_indices();
        let mut rows: Vec<Row> = Vec::with_capacity(filtered.len());
        let selected_row_style = theme.selection_style().add_modifier(Modifier::BOLD);
        let selected_row_index = state.selected.unwrap_or(0);

        for (row_index, &item_index) in filtered.iter().enumerate() {
            if let Some(item) = state.items.get(item_index) {
                let mut row_style = th::table_row_style(theme, row_index);
                if row_index == selected_row_index { row_style = selected_row_style; }
                let tags = if item.tags.is_empty() { String::new() } else { item.tags.join(",") };
                let display_name = if row_index == selected_row_index { format!("› {}", item.name) } else { item.name.clone() };
                rows.push(Row::new(vec![display_name, item.status.clone(), item.command_or_url.clone(), tags]).style(row_style));
            }
        }

        let widths = [
            ratatui::layout::Constraint::Length(18),
            ratatui::layout::Constraint::Length(10),
            ratatui::layout::Constraint::Percentage(60),
            ratatui::layout::Constraint::Percentage(20),
        ];
        let table = Table::new(rows, widths).header(header).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border_style(false))
                .style(th::panel_style(theme)),
        );

        frame.render_widget(table, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugins_table_component_constructs() {
        let _c = PluginsTableComponent::default();
        assert!(true);
    }
}
