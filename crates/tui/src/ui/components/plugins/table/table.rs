use crossterm::event::{KeyCode, KeyEvent};
use heroku_types::Effect;
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    widgets::{Block, Borders, Row, Table},
};

use crate::ui::theme::helpers as th;
use crate::{app::App, ui::components::component::Component};
use heroku_mcp::types::plugin::AuthStatus;

/// Component for rendering the plugins table with selection and navigation.
#[derive(Debug, Default)]
pub struct PluginsTableComponent;

impl PluginsTableComponent {
    fn move_selection_up(app: &mut App) {
        let filtered_indices = app.plugins.filtered_indices();
        if filtered_indices.is_empty() {
            return;
        }
        let selected_index = app
            .plugins
            .selected
            .unwrap_or(0)
            .min(filtered_indices.len().saturating_sub(1));
        let new_position = selected_index.saturating_sub(1);
        app.plugins.selected = Some(new_position);
    }

    fn move_selection_down(app: &mut App) {
        let filtered_indices = app.plugins.filtered_indices();
        if filtered_indices.is_empty() {
            return;
        }
        let selected_index = app
            .plugins
            .selected
            .unwrap_or(0)
            .min(filtered_indices.len().saturating_sub(1));
        let new_position = (selected_index + 1).min(filtered_indices.len().saturating_sub(1));
        app.plugins.selected = Some(new_position);
    }
}

impl Component for PluginsTableComponent {
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Up if app.plugins.grid_flag.get() => {
                Self::move_selection_up(app);
                Vec::new()
            }
            KeyCode::Down if app.plugins.grid_flag.get() => {
                Self::move_selection_down(app);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let theme = &*app.ctx.theme;
        let state = &app.plugins;
        let header_cells = ["Name", "Status", "Command/BaseUrl", "Auth", "Tags"]
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
                if row_index == selected_row_index {
                    row_style = selected_row_style;
                }
                let tags = if item.tags.is_empty() {
                    String::new()
                } else {
                    item.tags.join(",")
                };
                let display_name = if row_index == selected_row_index {
                    format!("› {}", item.name)
                } else {
                    item.name.clone()
                };
                let auth_status = format_auth_status(&item.auth_status);
                rows.push(
                    Row::new(vec![
                        display_name,
                        item.status.clone(),
                        item.command_or_url.clone(),
                        auth_status,
                        tags,
                    ])
                    .style(row_style),
                );
            }
        }

        let widths = [
            ratatui::layout::Constraint::Length(18),
            ratatui::layout::Constraint::Length(10),
            ratatui::layout::Constraint::Percentage(50),
            ratatui::layout::Constraint::Length(12),
            ratatui::layout::Constraint::Percentage(20),
        ];
        let table = Table::new(rows, widths).header(header).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border_style(state.grid_flag.get()))
                .style(th::panel_style(theme)),
        );

        frame.render_widget(table, area);
    }
}

/// Format authentication status for display in the table.
fn format_auth_status(status: &AuthStatus) -> String {
    match status {
        AuthStatus::Unknown => "?".to_string(),
        AuthStatus::Authorized => "✓".to_string(),
        AuthStatus::Required => "!".to_string(),
        AuthStatus::Failed => "✗".to_string(),
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
