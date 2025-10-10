use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use heroku_types::{Effect, Modal};
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::Modifier,
    text::Span,
    widgets::{Block, Borders, Row, Table},
};

use crate::ui::{
    components::plugins::{PluginDetail, PluginsTableState, plugin_editor::state::PluginEditViewState},
    theme::theme_helpers,
};
use crate::{app::App, ui::components::component::Component};
use heroku_mcp::AuthStatus;

/// Table column width constraints for the plugin table.
const NAME_COLUMN_WIDTH: u16 = 18;
const STATUS_COLUMN_WIDTH: u16 = 10;
const COMMAND_COLUMN_PERCENTAGE: u16 = 50;
const AUTH_COLUMN_WIDTH: u16 = 12;
const TAGS_COLUMN_PERCENTAGE: u16 = 20;

/// Table header labels for the plugin table.
const TABLE_HEADERS: &[&str] = &["Name", "Status", "Command/BaseUrl", "Auth", "Tags"];

/// Component for rendering the plugin table with selection and navigation.
///
/// This component displays a table of MCP plugins with their status, authentication state,
/// and other metadata. It supports keyboard navigation with up/down arrow keys and
/// provides visual feedback for the currently selected row.
#[derive(Debug, Default, PartialEq)]
pub struct PluginsTableComponent;

impl PluginsTableComponent {
    /// Moves the table selection up by one row.
    ///
    /// If the table is empty or already at the top, this function has no effect.
    /// The selection is bounded by the number of filtered items.
    ///
    /// # Arguments
    /// * `app` - The application state containing the plugin data
    fn move_selection_up(app: &mut App) {
        let table_state = &mut app.plugins.table;
        let filtered_indices = table_state.filtered_indices();
        if filtered_indices.is_empty() {
            return;
        }

        let current_selected_index = table_state.selected.unwrap_or(0).min(filtered_indices.len().saturating_sub(1));
        let new_position = current_selected_index.saturating_sub(1);
        table_state.selected = Some(new_position);
    }

    /// Moves the table selection down by one row.
    ///
    /// If the table is empty or already at the bottom, this function has no effect.
    /// The selection is bounded by the number of filtered items.
    ///
    /// # Arguments
    /// * `app` - The application state containing the plugin data
    fn move_selection_down(app: &mut App) {
        let table_state = &mut app.plugins.table;
        let filtered_indices = table_state.filtered_indices();
        if filtered_indices.is_empty() {
            return;
        }

        let current_selected_index = table_state.selected.unwrap_or(0).min(filtered_indices.len().saturating_sub(1));
        let new_position = (current_selected_index + 1).min(filtered_indices.len().saturating_sub(1));
        table_state.selected = Some(new_position);
    }

    /// Creates the table header row with styled column headers.
    ///
    /// # Arguments
    /// * `theme` - The current theme for styling
    ///
    /// # Returns
    /// A styled Row containing the table headers
    fn create_table_header(theme: &dyn crate::ui::theme::Theme) -> Row<'static> {
        let header_cells = TABLE_HEADERS
            .iter()
            .map(|&header_text| Span::styled(header_text, theme_helpers::table_header_style(theme)));

        Row::new(header_cells).style(theme_helpers::table_header_row_style(theme))
    }

    /// Creates table rows for all filtered plugin items.
    ///
    /// # Arguments
    /// * `state` - The plugins state containing items and selection
    /// * `theme` - The current theme for styling
    ///
    /// # Returns
    /// A vector of styled Row widgets representing the plugin data
    fn create_table_rows(state: &PluginsTableState, theme: &dyn crate::ui::theme::Theme) -> Vec<Row<'static>> {
        let filtered_indices = state.filtered_indices();
        let mut table_rows: Vec<Row<'static>> = Vec::with_capacity(filtered_indices.len());
        let selected_row_style = theme.selection_style().add_modifier(Modifier::BOLD);
        let selected_row_index = state.selected.unwrap_or(0);
        let is_focused = state.grid_flag.get();

        for (row_index, &item_index) in filtered_indices.iter().enumerate() {
            if let Some(plugin_item) = state.items.get(item_index) {
                let row_style =
                    Self::determine_row_style(theme, row_index, is_focused, row_index == selected_row_index, selected_row_style);

                let display_name = Self::format_display_name(plugin_item, is_focused, row_index == selected_row_index);
                let formatted_tags = Self::format_tags(&plugin_item.tags);
                let formatted_auth_status = format_auth_status(&plugin_item.auth_status);
                let status_text = plugin_item.status.display().to_string();

                table_rows.push(
                    Row::new(vec![
                        Span::raw(display_name),
                        Span::raw(status_text),
                        Span::raw(plugin_item.command_or_url.clone()),
                        Span::raw(formatted_auth_status),
                        Span::raw(formatted_tags),
                    ])
                    .style(row_style),
                );
            }
        }

        table_rows
    }

    /// Determines the appropriate style for a table row based on its state.
    ///
    /// # Arguments
    /// * `theme` - The current theme for styling
    /// * `row_index` - The index of the row in the filtered list
    /// * `is_focused` - Whether the table is currently focused
    /// * `is_selected` - Whether this row is currently selected
    /// * `selected_style` - The style to apply to selected rows
    ///
    /// # Returns
    /// The appropriate style for the row
    fn determine_row_style(
        theme: &dyn crate::ui::theme::Theme,
        row_index: usize,
        is_focused: bool,
        is_selected: bool,
        selected_style: ratatui::style::Style,
    ) -> ratatui::style::Style {
        if is_focused && is_selected {
            selected_style
        } else {
            theme_helpers::table_row_style(theme, row_index)
        }
    }

    /// Formats the display name for a plugin item, adding a selection indicator if needed.
    ///
    /// # Arguments
    /// * `plugin_item` - The plugin item to format
    /// * `is_focused` - Whether the table is currently focused
    /// * `is_selected` - Whether this item is currently selected
    ///
    /// # Returns
    /// The formatted display name string
    fn format_display_name(plugin_item: &PluginDetail, is_focused: bool, is_selected: bool) -> String {
        if is_focused && is_selected {
            format!("› {}", plugin_item.name)
        } else {
            plugin_item.name.clone()
        }
    }

    /// Formats the tag list for display in the table.
    ///
    /// # Arguments
    /// * `tags` - The tags to format
    ///
    /// # Returns
    /// A comma-separated string of tags, or empty string if no tags
    fn format_tags(tags: &[String]) -> String {
        if tags.is_empty() { String::new() } else { tags.join(",") }
    }

    /// Creates the column width constraints for the table.
    ///
    /// # Returns
    /// An array of Constraint values defining column widths
    fn create_column_constraints() -> [Constraint; 5] {
        [
            Constraint::Length(NAME_COLUMN_WIDTH),
            Constraint::Length(STATUS_COLUMN_WIDTH),
            Constraint::Percentage(COMMAND_COLUMN_PERCENTAGE),
            Constraint::Length(AUTH_COLUMN_WIDTH),
            Constraint::Percentage(TAGS_COLUMN_PERCENTAGE),
        ]
    }
}

impl Component for PluginsTableComponent {
    /// Handles keyboard events for table navigation.
    ///
    /// Supports up/down arrow keys for row selection when the table is focused.
    ///
    /// # Arguments
    /// * `app` - The application state
    /// * `key` - The key event to handle
    ///
    /// # Returns
    /// A vector of effects to be processed by the runtime
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let mut effects = vec![];
        let control_pressed = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Up if app.plugins.table.grid_flag.get() => {
                Self::move_selection_up(app);
            }
            KeyCode::Down if app.plugins.table.grid_flag.get() => {
                Self::move_selection_down(app);
            }
            KeyCode::Enter => {
                effects.push(Effect::ShowModal(Modal::PluginDetails));
                if let Some(selected_item) = app.plugins.table.selected_item() {
                    effects.push(Effect::PluginsLoadDetail(selected_item.name.clone()));
                }
            }
            KeyCode::Char('d') if control_pressed => {
                effects.push(Effect::ShowModal(Modal::PluginDetails));
                if let Some(selected_item) = app.plugins.table.selected_item() {
                    effects.push(Effect::PluginsLoadDetail(selected_item.name.clone()));
                }
            }
            KeyCode::Char('s') if control_pressed => {
                if let Some(selected_item) = app.plugins.table.selected_item() {
                    effects.push(Effect::PluginsStart(selected_item.name.clone()));
                }
            }
            KeyCode::Char('t') if control_pressed => {
                if let Some(selected_item) = app.plugins.table.selected_item() {
                    effects.push(Effect::PluginsStop(selected_item.name.clone()));
                }
            }
            KeyCode::Char('r') if control_pressed => {
                if let Some(selected_item) = app.plugins.table.selected_item() {
                    effects.push(Effect::PluginsRestart(selected_item.name.clone()));
                }
            }
            KeyCode::Char('l') if control_pressed => {
                if let Some(selected_item) = app.plugins.table.selected_item() {
                    let plugin_name = selected_item.name.clone();
                    app.plugins.open_logs(plugin_name.clone());
                    app.plugins.logs_open = true;
                }
            }
            KeyCode::Char('a') if control_pressed && app.plugins.can_open_add_plugin() => {
                app.plugins.add = Some(PluginEditViewState::new());
            }
            KeyCode::Char('e') if control_pressed && app.plugins.can_open_add_plugin() => {
                if let Some(detail) = app.plugins.table.selected_item() {
                    app.plugins.add = Some(PluginEditViewState::from_detail(detail.clone()));
                }
            }
            _ => {}
        };

        effects
    }

    /// Renders the plugin table component.
    ///
    /// Creates and displays a table showing all filtered plugin items with their
    /// status, authentication state, and metadata. The table supports keyboard
    /// navigation and visual selection feedback.
    ///
    /// # Arguments
    /// * `frame` - The ratatui frame for rendering
    /// * `area` - The rectangular area to render within
    /// * `app` - The application state containing plugin data
    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let theme = &*app.ctx.theme;
        let table_state = &app.plugins.table;

        let table_header = Self::create_table_header(theme);
        let table_rows = Self::create_table_rows(table_state, theme);
        let column_constraints = Self::create_column_constraints();

        let table_widget = Table::new(table_rows, column_constraints).header(table_header).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border_style(table_state.grid_flag.get()))
                .style(theme_helpers::panel_style(theme)),
        );

        frame.render_widget(table_widget, area);
    }
    fn get_hint_spans(&self, app: &App, is_root: bool) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        let mut spans = Vec::with_capacity(10);
        if is_root {
            spans.push(Span::styled("Hints: ", theme.text_muted_style()));
            // the plugin component adds this
            if app.plugins.can_open_add_plugin() {
                spans.extend([
                    Span::styled("Ctrl-A", theme.accent_emphasis_style()),
                    Span::styled(" Add  ", theme.text_muted_style()),
                ]);
            }
        }

        if app.plugins.table.selected_item().is_some() {
            spans.extend([
                Span::styled("Ctrl-E", theme.accent_emphasis_style()),
                Span::styled(" Edit  ", theme.text_muted_style()),
            ]);
        }

        spans.extend([
            Span::styled("Enter/Ctrl-D", theme.accent_emphasis_style()),
            Span::styled(" Details  ", theme.text_muted_style()),
            Span::styled("Ctrl-S", theme.accent_emphasis_style()),
            Span::styled(" start  ", theme.text_muted_style()),
            Span::styled("Ctrl-T", theme.accent_emphasis_style()),
            Span::styled(" Stop  ", theme.text_muted_style()),
            Span::styled("Ctrl-R", theme.accent_emphasis_style()),
            Span::styled(" Restart  ", theme.text_muted_style()),
            Span::styled("Ctrl-L", theme.accent_emphasis_style()),
            Span::styled(" Logs  ", theme.text_muted_style()),
        ]);

        spans
    }
}

/// Formats authentication status for display in the table.
///
/// Converts the AuthStatus enum into user-friendly symbols for display.
///
/// # Arguments
/// * `status` - The authentication status to format
///
/// # Returns
/// A string representation of the authentication status
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
    use crate::ui::components::plugins::PluginDetail;
    use heroku_mcp::AuthStatus;
    use heroku_mcp::PluginStatus;

    #[test]
    fn plugins_table_component_constructs() {
        let component = PluginsTableComponent::default();
        assert_eq!(component, PluginsTableComponent::default());
    }

    #[test]
    fn format_auth_status_returns_correct_symbols() {
        assert_eq!(format_auth_status(&AuthStatus::Unknown), "?");
        assert_eq!(format_auth_status(&AuthStatus::Authorized), "✓");
        assert_eq!(format_auth_status(&AuthStatus::Required), "!");
        assert_eq!(format_auth_status(&AuthStatus::Failed), "✗");
    }

    #[test]
    fn format_tags_handles_empty_list() {
        let empty_tags: Vec<String> = vec![];
        assert_eq!(PluginsTableComponent::format_tags(&empty_tags), "");
    }

    #[test]
    fn format_tags_joins_multiple_tags() {
        let tags = vec!["tag1".to_string(), "tag2".to_string(), "tag3".to_string()];
        assert_eq!(PluginsTableComponent::format_tags(&tags), "tag1,tag2,tag3");
    }

    #[test]
    fn format_display_name_adds_selection_indicator() {
        let mut plugin_item = PluginDetail::new("test-plugin".to_string(), "test-command".to_string(), None);
        plugin_item.status = PluginStatus::Running;

        let display_name_focused = PluginsTableComponent::format_display_name(&plugin_item, true, true);
        assert_eq!(display_name_focused, "› test-plugin");

        let display_name_unfocused = PluginsTableComponent::format_display_name(&plugin_item, false, false);
        assert_eq!(display_name_unfocused, "test-plugin");
    }

    #[test]
    fn create_column_constraints_returns_correct_structure() {
        let constraints = PluginsTableComponent::create_column_constraints();
        assert_eq!(constraints.len(), 5);

        // Verify the constraint types match our expectations
        match constraints[0] {
            Constraint::Length(width) => assert_eq!(width, NAME_COLUMN_WIDTH),
            _ => panic!("First constraint should be Length"),
        }

        match constraints[2] {
            Constraint::Percentage(percentage) => assert_eq!(percentage, COMMAND_COLUMN_PERCENTAGE),
            _ => panic!("Third constraint should be Percentage"),
        }
    }
}
