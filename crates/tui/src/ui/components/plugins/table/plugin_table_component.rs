use crate::ui::theme::Theme;
use crate::ui::theme::theme_helpers::{ButtonRenderOptions, render_button};
use crate::ui::{
    components::plugins::{PluginsTableState, plugin_editor::state::PluginEditViewState},
    theme::theme_helpers,
};
use crate::{app::App, ui::components::component::Component};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use heroku_mcp::AuthStatus;
use heroku_types::{Effect, Modal, PluginStatus};
use rat_focus::FocusFlag;
use ratatui::layout::Position;
use ratatui::prelude::Layout;
use ratatui::widgets::Paragraph;
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::Modifier,
    text::Span,
    widgets::{Block, Borders, Row, Table},
};

/// Table column width constraints for the plugin table.
const NAME_COLUMN_WIDTH: u16 = 18;
const STATUS_COLUMN_WIDTH: u16 = 10;
const COMMAND_COLUMN_PERCENTAGE: u16 = 50;
const AUTH_COLUMN_WIDTH: u16 = 12;
const TAGS_COLUMN_PERCENTAGE: u16 = 20;

/// Table header labels for the plugin table.
const TABLE_HEADERS: &[&str] = &["Name", "Status", "Command/BaseUrl", "Auth", "Tags"];

/// The hit areas for the plugin table and their enabled state.
#[derive(Debug, PartialEq)]
enum PluginTableHitArea {
    Search(Rect, bool),
    Table(Rect, bool),
    AddButton(Rect, bool),
    StartButton(Rect, bool),
    StopButton(Rect, bool),
    EditButton(Rect, bool),
    DeleteButton(Rect, bool),
}
impl PluginTableHitArea {
    pub fn info(&self) -> (&Rect, bool) {
        match self {
            PluginTableHitArea::Search(r, e) => (r, *e),
            PluginTableHitArea::Table(r, e) => (r, *e),
            PluginTableHitArea::AddButton(r, e) => (r, *e),
            PluginTableHitArea::StartButton(r, e) => (r, *e),
            PluginTableHitArea::StopButton(r, e) => (r, *e),
            PluginTableHitArea::EditButton(r, e) => (r, *e),
            PluginTableHitArea::DeleteButton(r, e) => (r, *e),
        }
    }
}
impl PluginTableHitArea {
    fn focus_flag<'a>(&'a self, app: &'a App) -> &'a FocusFlag {
        match self {
            PluginTableHitArea::Search(_, _) => &app.plugins.table.f_search,
            PluginTableHitArea::Table(_, _) => &app.plugins.table.f_grid,
            PluginTableHitArea::AddButton(_, _) => &app.plugins.table.f_add,
            PluginTableHitArea::StartButton(_, _) => &app.plugins.table.f_start,
            PluginTableHitArea::StopButton(_, _) => &app.plugins.table.f_stop,
            PluginTableHitArea::EditButton(_, _) => &app.plugins.table.f_edit,
            PluginTableHitArea::DeleteButton(_, _) => &app.plugins.table.f_delete,
        }
    }

    fn key_code(&self) -> Option<KeyCode> {
        match self {
            PluginTableHitArea::Search(_, _) => None,
            PluginTableHitArea::Table(_, _) => None,
            PluginTableHitArea::AddButton(_, _) => Some(KeyCode::Char('a')),
            PluginTableHitArea::StartButton(_, _) => Some(KeyCode::Char('s')),
            PluginTableHitArea::StopButton(_, _) => Some(KeyCode::Char('t')),
            PluginTableHitArea::EditButton(_, _) => Some(KeyCode::Char('e')),
            PluginTableHitArea::DeleteButton(_, _) => Some(KeyCode::Char('d')),
        }
    }
}

/// Component for rendering the plugin table with selection and navigation.
///
/// This component displays a table of MCP plugins with their status, authentication state,
/// and other metadata. It supports keyboard navigation with up/down arrow keys and
/// provides visual feedback for the currently selected row.
#[derive(Debug, Default, PartialEq)]
pub struct PluginsTableComponent {
    hit_areas: Vec<PluginTableHitArea>,
}

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
        table_state.table_state.select_previous();
        table_state.normalize_selection();
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
        table_state.table_state.select_next();
        table_state.normalize_selection();
    }

    /// Creates the table header row with styled column headers.
    ///
    /// # Arguments
    /// * `theme` - The current theme for styling
    ///
    /// # Returns
    /// A styled Row containing the table headers
    fn create_table_header(theme: &dyn Theme) -> Row<'static> {
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
    fn create_table_rows(state: &PluginsTableState, theme: &dyn Theme) -> Vec<Row<'static>> {
        let filtered_indices = state.filtered_indices();
        let mut table_rows: Vec<Row<'static>> = Vec::with_capacity(filtered_indices.len());

        for (row_index, &item_index) in filtered_indices.iter().enumerate() {
            if let Some(plugin_item) = state.items.get(item_index) {
                let row_style = theme_helpers::table_row_style(theme, row_index);
                let formatted_tags = Self::format_tags(&plugin_item.tags);
                let formatted_auth_status = format_auth_status(&plugin_item.auth_status);
                let status_text = plugin_item.status.display().to_string();

                table_rows.push(
                    Row::new(vec![
                        Span::raw(plugin_item.name.clone()),
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

    fn render_action_buttons(&mut self, frame: &mut Frame, area: Rect, app: &App, theme: &dyn Theme) -> Vec<PluginTableHitArea> {
        let state = &app.plugins.table;

        let button_columns = Layout::horizontal([
            Constraint::Length(12), // Add button
            Constraint::Min(0),     // Flexible space to move buttons to the right
            Constraint::Length(12), // Start button
            Constraint::Length(2),  // Spacer
            Constraint::Length(10), // Stop button
            Constraint::Length(2),  // Spacer
            Constraint::Length(12), // Edit button
            Constraint::Length(2),  // Spacer
            Constraint::Length(12), // Delete button
        ])
        .split(area);
        let selected_item = state.selected_item();
        let is_selected = selected_item.is_some();
        let is_running = selected_item.map(|item| item.status == PluginStatus::Running).unwrap_or(false);
        let has_active_add = app.plugins.add.is_some();
        let buttons = vec![
            (&state.f_add, "Add", button_columns[0], is_selected && !has_active_add),
            (&state.f_start, "Start", button_columns[2], is_selected && !is_running),
            (&state.f_stop, "Stop", button_columns[4], is_selected && is_running),
            (&state.f_edit, "Edit", button_columns[6], is_selected && !has_active_add),
            (&state.f_delete, "Delete", button_columns[8], is_selected),
        ];
        for (button_flag, label, button_area, enabled) in &buttons {
            let focused = button_flag.get();
            let options = ButtonRenderOptions {
                selected: false,
                enabled: *enabled,
                focused,
                borders: Borders::ALL,
                is_primary: false,
            };
            render_button(frame, *button_area, label, theme, options);
        }
        vec![
            PluginTableHitArea::AddButton(button_columns[0], buttons[0].3),
            PluginTableHitArea::StartButton(button_columns[2], buttons[1].3),
            PluginTableHitArea::StopButton(button_columns[4], buttons[2].3),
            PluginTableHitArea::EditButton(button_columns[6], buttons[3].3),
            PluginTableHitArea::DeleteButton(button_columns[8], buttons[4].3),
        ]
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
    /// Handles key events for the grid view and performs actions based on the event.
    ///
    /// This function operates primarily in the context of managing plugins in the application.
    /// Key events are mapped to specific actions, which include moving selection, showing modals,
    /// and invoking plugin operations such as start, stop, restart, and more. Control modifiers
    /// often trigger specific behaviors when combined with certain keys.
    ///
    /// # Parameters
    ///
    /// - `app`: A mutable reference to the `App` instance, which holds the application state
    ///   and manages plugin operations.
    /// - `key`: The key event received, containing the key code and any active modifiers.
    ///
    /// # Returns
    ///
    /// A vector of `Effect` instances representing the actions to be performed as a result
    /// of the key event. Effects may include showing a modal, loading plugin details, or
    /// initiating plugin control operations.
    ///
    /// # Key Behaviors
    ///
    /// - **Navigation:**
    ///   - `Up`: Moves the selection up in the grid if the grid flag is enabled.
    ///   - `Down`: Moves the selection down in the grid if the grid flag is enabled.
    ///
    /// - **Plugin Details:**
    ///   - `Enter`: Opens the plugin details modal and loads the selected plugin's details.
    ///   - `Ctrl + D`: Same behavior as `Enter`.
    ///
    /// - **Plugin Operations:**
    ///   - `Ctrl + S`: Starts the selected plugin.
    ///   - `Ctrl + T`: Stops the selected plugin.
    ///   - `Ctrl + R`: Restarts the selected plugin.
    ///
    /// - **Logs:**
    ///   - `Ctrl + L`: Opens the logs for the selected plugin and marks logs as open.
    ///
    /// - **Plugin Management:**
    ///   - `Ctrl + A`: Opens the add-plugin view if allowed.
    ///   - `Ctrl + E`: Opens the edit view with the currently selected plugin's details if allowed.
    ///
    /// - **Ignored Inputs:**
    ///   - Any key events not specifically matched in the logic are ignored.
    ///
    /// # Notes
    ///
    /// - The `control_pressed` flag is derived from the key event's modifier state and is used
    ///   to differentiate between basic and modified key behaviors.
    /// - The function directly modifies the application state (`app`) and returns effects that
    ///   can be processed further by the caller.
    fn handle_grid_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let mut effects = vec![];
        let control_pressed = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Up if app.plugins.table.f_grid.get() => {
                Self::move_selection_up(app);
            }
            KeyCode::Down if app.plugins.table.f_grid.get() => {
                Self::move_selection_down(app);
            }
            KeyCode::Enter => {
                effects.push(Effect::ShowModal(Modal::PluginDetails));
                app.plugins.ensure_details_state();
                if let Some(selected_item) = app.plugins.table.selected_item() {
                    effects.push(Effect::PluginsLoadDetail(selected_item.name.clone()));
                }
            }
            KeyCode::Char('d') if control_pressed => {
                app.plugins.ensure_details_state();
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
            KeyCode::Char('a') if control_pressed => {
                app.plugins.add = Some(PluginEditViewState::new());
            }
            KeyCode::Char('e') if control_pressed => {
                if let Some(detail) = app.plugins.table.selected_item() {
                    app.plugins.add = Some(PluginEditViewState::from_detail(detail.clone()));
                }
            }
            _ => {}
        };
        effects
    }
    /// Handles search navigation for the plugin table.
    ///
    /// When the search box has focus, printable characters update the filter and backspace removes
    /// the previous character. Outside of search mode, arrow keys move the selected table row, and
    /// `Ctrl+A` opens the add-plugin workflow.
    fn handle_search_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.plugins.add = Some(PluginEditViewState::new());
            }
            KeyCode::Backspace if app.plugins.table.f_search.get() => {
                Self::remove_last_filter_character(app);
            }
            KeyCode::Char(character) if app.plugins.table.f_search.get() => {
                Self::insert_filter_character_unless_control(app, key, character);
            }
            KeyCode::Left => {
                app.plugins.table.reduce_move_cursor_left();
            }
            KeyCode::Right => {
                app.plugins.table.reduce_move_cursor_right();
            }
            _ => {}
        }
        Vec::new()
    }

    /// Removes the last character from the search filter and normalizes selection.
    fn remove_last_filter_character(application: &mut App) {
        application.plugins.table.pop_filter_character();
    }

    /// Inserts a character into the search filter unless a control modifier is pressed.
    fn insert_filter_character_unless_control(application: &mut App, key_event: KeyEvent, character: char) {
        if key_event.modifiers.contains(KeyModifiers::CONTROL) {
            return;
        }
        application.plugins.table.push_filter_character(character);
    }

    fn hit_test_table(&mut self, app: &mut App, table_area: Rect, mouse_position: Position) -> Vec<Effect> {
        let offset_y = table_area.y as usize;
        let idx = (mouse_position.y as usize).saturating_sub(offset_y);
        if app.plugins.table.filtered_indices().get(idx).is_some() {
            app.plugins.table.set_selected_index(Some(idx));
        }
        Vec::new()
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
        if app.plugins.table.f_grid.get() {
            effects.extend(self.handle_grid_key_events(app, key));
        }
        if app.plugins.table.f_search.get() {
            effects.extend(self.handle_search_key_events(app, key));
        }

        // If enter is pressed on any of the buttons,
        // map them to the hot key events and process them in
        // the handle_grid_key_events function.
        if key.code == KeyCode::Enter
            && let Some(key_code) = self
                .hit_areas
                .iter()
                .find_map(|h| if h.focus_flag(app).get() { h.key_code() } else { None })
        {
            effects.extend(self.handle_grid_key_events(app, KeyEvent::new(key_code, KeyModifiers::CONTROL)));
        }

        effects
    }
    /// Handles mouse events within the application, updating the plugins table state
    /// and potentially generating a set of effects based on the user's interaction.
    ///
    /// This function is responsible for processing mouse interactions by checking
    /// whether a left mouse click occurred within a certain area of the plugins table
    /// (determined by the `last_area` and `per_item_area` dimensions). If a valid index
    /// is identified based on the mouse position, the `f_grid` state of the plugin
    /// table's grid is updated.
    ///
    /// # Arguments
    ///
    /// * `app` - A mutable reference to the application state (`App`), allowing modifications
    ///   based on the interaction.
    /// * `mouse` - A `MouseEvent` object which represents the properties of the mouse event,
    ///   including its position, button type, and event kind (e.g., mouse down, mouse up).
    ///
    /// # Returns
    ///
    /// A `Vec<Effect>` which represents any effects resulting from the mouse interaction.
    /// In this implementation, the vector is always empty.
    ///
    /// # Details
    ///
    /// - If the `MouseEventKind` is a left mouse button press (`MouseEventKind::Down(MouseButton::Left)`),
    ///   the function attempts to compute an index, `maybe_idx`, corresponding to the mouse position by
    ///   using the helper function `find_target_index_by_mouse_position`.
    /// - If a valid index, `maybe_idx`, is found, the `f_grid` attribute of the plugins table grid
    ///   (`app.plugins.table.f_grid`) is set to `true`.
    /// - No effects are added to the returned `effects` vector in this implementation,
    ///   but the infrastructure exists for future enhancements.
    ///
    /// # Notes
    ///
    /// This function assumes that `last_area` and `per_item_area` are correctly initialized within
    /// the `plugins.table` before calling this function. Additionally, `find_target_index_by_mouse_position`
    /// should be properly implemented to determine the target index based on mouse coordinates.
    ///
    /// # Example
    ///
    /// ```rust
    /// let mut app = App::new();
    /// let mouse_event = MouseEvent {
    ///     kind: MouseEventKind::Down(MouseButton::Left),
    ///     column: 5,
    ///     row: 10,
    /// };
    /// let mut handler = EventHandler::new();
    /// let effects = handler.handle_mouse_events(&mut app, mouse_event);
    /// assert!(effects.is_empty());
    /// ```
    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return Vec::new();
        }
        let position = Position {
            x: mouse.column,
            y: mouse.row,
        };
        let maybe_focus_flag = self.hit_areas.iter().find_map(|h| {
            let (rect, enabled) = h.info();
            let hit = rect.contains(position);
            if hit && enabled { Some((h.focus_flag(app), rect)) } else { None }
        });

        if let Some((flag, rect)) = maybe_focus_flag {
            app.focus.focus(flag);
            if flag == &app.plugins.table.f_grid {
                return self.hit_test_table(app, *rect, position);
            }
            // note that the Enter key is a no-op for the search input
            // if this changes, we may need to handle Enter differently here
            return self.handle_key_events(app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        }
        Vec::new()
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
        let blocks = self.get_preferred_layout(app, area);

        let theme = &*app.ctx.theme;
        let table_state = &mut app.plugins.table;
        let is_search_focused = table_state.f_search.get();
        let header_block = theme_helpers::block(theme, Some("Search Plugins"), is_search_focused);

        // Search input
        let header_inner = header_block.inner(blocks[0]);
        let filter_text = table_state.filter_text();
        let header = Paragraph::new(filter_text).style(theme.text_primary_style()).block(header_block);
        frame.render_widget(header, blocks[0]);

        // Position the cursor at the end of input when focused
        if is_search_focused {
            let x = header_inner.x.saturating_add(table_state.cursor_position as u16);
            let y = header_inner.y;
            frame.set_cursor_position((x, y));
        }

        let table_header = Self::create_table_header(theme);
        let table_rows = Self::create_table_rows(table_state, theme);
        let column_constraints = Self::create_column_constraints();

        let highlight_style = theme.selection_style().add_modifier(Modifier::BOLD);
        let highlight_symbol = if table_state.f_grid.get() { "> " } else { "" };
        let table_widget = Table::new(table_rows, column_constraints)
            .header(table_header)
            .row_highlight_style(highlight_style)
            .highlight_symbol(highlight_symbol)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.border_style(table_state.f_grid.get()))
                    .style(theme_helpers::panel_style(theme)),
            );
        frame.render_stateful_widget(table_widget, blocks[1], &mut app.plugins.table.table_state);

        // Calculated table area
        let mut table_area = blocks[1];
        table_area.height = table_area.height.saturating_sub(3); // remove header, border-top and border-bottom
        table_area.y = table_area.y.saturating_add(2); // the first row is 2 rows below the border

        let mut hit_areas = self.render_action_buttons(frame, blocks[2], app, theme);
        hit_areas.extend(vec![
            PluginTableHitArea::Search(blocks[0], true),
            PluginTableHitArea::Table(table_area, true),
        ]);
        self.hit_areas = hit_areas;
    }
    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        let mut spans = Vec::with_capacity(5);

        if app.plugins.table.selected_item().is_some() {
            spans.extend(theme_helpers::build_hint_spans(
                theme,
                &[
                    ("Ctrl+E", " Edit  "),
                    ("Enter/Ctrl+D", " Details  "),
                    ("Ctrl+S", " start  "),
                    ("Ctrl+T", " Stop  "),
                    ("Ctrl+R", " Restart  "),
                ],
            ));
        }

        spans
    }

    fn get_preferred_layout(&self, _app: &App, area: Rect) -> Vec<Rect> {
        Layout::vertical([
            Constraint::Length(3), // search bar
            Constraint::Min(6),    // table
            Constraint::Length(3), // Action buttons
        ])
        .split(area)
        .to_vec()
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
    use heroku_mcp::AuthStatus;

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
