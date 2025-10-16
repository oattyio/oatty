//! Combined key/value table and inline editor for the plugin add flow.
//!
//! This component encapsulates the tabular display of key/value pairs and the
//! inline editing experience that previously lived directly inside
//! `add.rs`. It centralizes keyboard handling, rendering, and cursor
//! positioning so the parent `PluginsAddComponent` can remain focused on the
//! surrounding form controls.
//!
//! The component follows the TUI component pattern by implementing the `Component`
//! trait and managing its own rendering and event handling. It delegates state
//! management to the parent `PluginAddViewState` while maintaining focus on
//! the user interaction experience.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use heroku_types::Effect;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
};

use crate::{
    app::App,
    ui::{
        components::{component::Component, plugins::plugin_editor::state::PluginEditViewState},
        theme::{Theme, theme_helpers},
    },
};

/// Component responsible for rendering and editing key/value pairs.
///
/// This component provides a tabular interface for managing key/value pairs
/// with inline editing capabilities. It supports both navigation and editing
/// modes, with keyboard shortcuts for common operations like adding new rows,
/// deleting existing rows, and switching between key and value fields.
///
/// The component is designed to be stateless, delegating all state management
/// to the parent `PluginAddViewState`. This allows for better separation of
/// concerns and easier testing.
#[derive(Debug, Default)]
pub struct KeyValueEditorComponent;

/// Presentation metadata for rendering an inline key/value field.
///
/// Bundles styling inputs so helper functions avoid long parameter lists while
/// keeping rendering logic cohesive.
#[derive(Debug)]
struct InlineFieldDisplay<'a> {
    /// Label displayed before the field's value (for example, `Key`).
    label: &'a str,
    /// Current contents of the field.
    value: &'a str,
    /// Placeholder text shown when the field is empty.
    placeholder: &'a str,
    /// Indicates whether this field currently has focus.
    focused: bool,
}

/// Aggregated cursor metadata for positioning the inline editor caret.
#[derive(Debug)]
struct InlineFieldCursorState<'a> {
    /// Indicates whether the key field is currently focused.
    key_field_active: bool,
    /// Text buffer for the key field.
    key_buffer: &'a str,
    /// Text buffer for the value field.
    value_buffer: &'a str,
    /// Cursor position relative to the key buffer.
    key_cursor: usize,
    /// Cursor position relative to the value buffer.
    value_cursor: usize,
}

impl KeyValueEditorComponent {
    /// Handle keyboard input when the editor is in editing mode.
    ///
    /// This method processes keyboard events for inline editing, including
    /// character input, field navigation, and edit completion/cancellation.
    ///
    /// # Arguments
    ///
    /// * `editor` - The mutable reference to the active key/value editor
    /// * `key_event` - The keyboard event to process
    ///
    /// # Returns
    ///
    /// An optional validation message if the input triggered validation.
    fn handle_editing_mode_input(&mut self, app: &mut App, key_event: KeyEvent) -> Vec<Effect> {
        let effects = vec![];
        let modifiers = key_event.modifiers;

        match key_event.code {
            KeyCode::Enter => {
                self.commit_edit_with_validation(app);
            }
            KeyCode::Char('e') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.commit_edit_with_validation(app);
            }
            _ => {}
        }

        // avoid 2 mutable references to app
        let Some(editor) = app.plugins.add.as_mut() else { return effects };

        match key_event.code {
            KeyCode::Esc => {
                editor.kv_editor.cancel_edit();
            }
            KeyCode::Backspace => {
                editor.kv_editor.pop_character();
            }
            KeyCode::Left => {
                editor.kv_editor.move_cursor_left();
            }
            KeyCode::Right => {
                editor.kv_editor.move_cursor_right();
            }
            KeyCode::Tab | KeyCode::BackTab => {
                editor.kv_editor.toggle_field();
            }
            KeyCode::Up => {
                editor.kv_editor.focus_key_input();
            }
            KeyCode::Down => {
                editor.kv_editor.focus_value_input();
            }
            KeyCode::Char(character) if self.is_regular_character_input(modifiers) => {
                editor.kv_editor.push_character(character);
            }
            _ => {}
        }
        effects
    }

    /// Handle keyboard input when the editor is in navigation mode.
    ///
    /// This method processes keyboard events for table navigation, row selection,
    /// and initiating edit operations.
    ///
    /// # Arguments
    ///
    /// * `editor` - The mutable reference to the active key/value editor
    /// * `key_event` - The keyboard event to process
    ///
    /// # Returns
    ///
    /// An optional validation message if the input triggered validation.
    fn handle_navigation_mode_input(&mut self, app: &mut App, key_event: KeyEvent) {
        let Some(add_state) = app.plugins.add.as_mut() else {
            return;
        };
        let modifiers = key_event.modifiers;
        add_state.kv_editor.ensure_selection();

        match key_event.code {
            KeyCode::Up => {
                add_state.kv_editor.select_previous();
            }
            KeyCode::Down => {
                add_state.kv_editor.select_next();
            }
            KeyCode::Enter => {
                add_state.kv_editor.begin_editing_selected();
            }
            KeyCode::Char('e') if modifiers.contains(KeyModifiers::CONTROL) => {
                add_state.kv_editor.begin_editing_selected();
            }
            KeyCode::Char('n') if modifiers.contains(KeyModifiers::CONTROL) => {
                add_state.kv_editor.begin_editing_new_row();
            }
            KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                add_state.kv_editor.delete_selected();
            }
            KeyCode::Delete => {
                add_state.kv_editor.delete_selected();
            }
            KeyCode::Char(character) if self.is_regular_character_input(modifiers) => {
                add_state.kv_editor.begin_editing_selected();
                add_state.kv_editor.push_character(character);
            }
            KeyCode::Home => {
                self.navigate_to_first_row(&mut add_state.kv_editor);
            }
            KeyCode::End => {
                self.navigate_to_last_row(&mut add_state.kv_editor);
            }
            _ => {}
        }
    }

    /// Check if the key event represents regular character input.
    ///
    /// This helper method determines whether a character key event should be
    /// treated as regular text input (as opposed to a command shortcut).
    ///
    /// # Arguments
    ///
    /// * `modifiers` - The key modifiers for the event
    ///
    /// # Returns
    ///
    /// True if this should be treated as regular character input.
    fn is_regular_character_input(&self, modifiers: KeyModifiers) -> bool {
        !modifiers.contains(KeyModifiers::CONTROL) && !modifiers.contains(KeyModifiers::ALT)
    }

    /// Commit the current edit and handle validation.
    ///
    /// This method attempts to commit the current edit and returns an appropriate
    /// validation message based on the result.
    ///
    /// # Arguments
    ///
    /// * `editor` - The mutable reference to the active key/value editor
    ///
    /// # Returns
    ///
    /// An optional validation message if the commit failed.
    fn commit_edit_with_validation(&mut self, app: &mut App) {
        let Some(add_state) = app.plugins.add.as_mut() else { return };
        add_state.validation = add_state.kv_editor.commit_edit();
    }

    /// Navigate to the first row in the editor.
    ///
    /// This method selects the first row if any rows exist.
    ///
    /// # Arguments
    ///
    /// * `editor` - The mutable reference to the active key/value editor
    fn navigate_to_first_row(
        &mut self,
        editor: &mut crate::ui::components::plugins::plugin_editor::key_value_editor::state::KeyValueEditorState,
    ) {
        if !editor.rows.is_empty() {
            editor.selected_row_index = Some(0);
        }
    }

    /// Navigate to the last row in the editor.
    ///
    /// This method selects the last row if any rows exist.
    ///
    /// # Arguments
    ///
    /// * `editor` - The mutable reference to the active key/value editor
    fn navigate_to_last_row(
        &mut self,
        editor: &mut crate::ui::components::plugins::plugin_editor::key_value_editor::state::KeyValueEditorState,
    ) {
        if !editor.rows.is_empty() {
            editor.selected_row_index = Some(editor.rows.len().saturating_sub(1));
        }
    }

    /// Render the combined table and inline editor.
    ///
    /// This method orchestrates the rendering of both the key/value table and
    /// the inline editor (when active). It handles layout management and delegates
    /// to specialized rendering methods for each component.
    ///
    /// # Arguments
    ///
    /// * `frame` - The Ratatui frame for rendering
    /// * `area` - The available rendering area
    /// * `theme` - The theme for styling
    /// * `add_state` - The plugin "add view" state containing editor state
    pub fn render_with_state(&mut self, frame: &mut Frame, area: Rect, theme: &dyn Theme, add_state: &PluginEditViewState) {
        let editor = &add_state.kv_editor;
        let is_editing = editor.is_editing();

        let mut constraints = vec![Constraint::Min(4)];
        if is_editing {
            constraints.push(Constraint::Length(4));
        }

        let sections = Layout::vertical(constraints).split(area);

        let table_area = sections[0];
        self.render_table(frame, table_area, theme, add_state);

        if is_editing {
            let editor_area = sections[1];
            self.render_inline_editor(frame, editor_area, theme, add_state);
        }
    }

    /// Render the key/value table with proper styling and selection indicators.
    ///
    /// This method renders the main table view showing all key/value pairs.
    /// It handles different display modes for editing vs. browsing and applies
    /// appropriate styling based on focus and selection state.
    ///
    /// # Arguments
    ///
    /// * `frame` - The Ratatui frame for rendering
    /// * `area` - The available rendering area for the table
    /// * `theme` - The theme for styling
    /// * `add_state` - The plugin add view state containing editor state
    fn render_table(&self, frame: &mut Frame, area: Rect, theme: &dyn Theme, add_state: &PluginEditViewState) {
        let editor = &add_state.kv_editor;
        let label = add_state.key_value_table_label();
        let is_focused = add_state.is_key_value_editor_focused();

        let title = self.build_table_title(editor, label);
        let block = self.build_table_block(theme, title, is_focused);
        let header = self.build_table_header(theme);
        let rows = self.build_table_rows(editor, theme);

        let table = Table::new(rows, [Constraint::Percentage(35), Constraint::Percentage(65)])
            .header(header)
            .block(block);

        frame.render_widget(table, area);
    }

    /// Build the title for the table based on current editing state.
    ///
    /// This method creates an appropriate title that indicates whether the
    /// table is in browsing mode or editing mode, and which field is active.
    ///
    /// # Arguments
    ///
    /// * `editor` - The key/value editor state
    /// * `label` - The base label for the table
    ///
    /// # Returns
    ///
    /// A formatted title string for the table.
    fn build_table_title(
        &self,
        editor: &crate::ui::components::plugins::plugin_editor::key_value_editor::state::KeyValueEditorState,
        label: &str,
    ) -> String {
        let editing_row_index = editor.editing_row_index();

        if let Some(row_index) = editing_row_index {
            format!("{} — editing row {} ({})", label, row_index + 1, editor.active_field_label())
        } else {
            label.to_string()
        }
    }

    /// Build the block widget for the table with appropriate styling.
    ///
    /// This method creates a styled block widget that serves as the container
    /// for the table, with borders and title styling based on focus state.
    ///
    /// # Arguments
    ///
    /// * `theme` - The theme for styling
    /// * `title` - The title text for the block
    /// * `is_focused` - Whether the table currently has focus
    ///
    /// # Returns
    ///
    /// A styled Block widget for the table container.
    fn build_table_block<'a>(&self, theme: &'a dyn Theme, title: String, is_focused: bool) -> Block<'a> {
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border_style(is_focused))
            .style(theme_helpers::panel_style(theme))
            .title(Span::styled(title, theme.text_secondary_style()))
    }

    /// Build the header row for the table.
    ///
    /// This method creates the column headers for the key/value table
    /// with appropriate styling.
    ///
    /// # Arguments
    ///
    /// * `theme` - The theme for styling
    ///
    /// # Returns
    ///
    /// A styled Row widget for the table header.
    fn build_table_header<'a>(&self, theme: &'a dyn Theme) -> Row<'a> {
        Row::new(vec![
            Span::styled("KEY", theme_helpers::table_header_style(theme)),
            Span::styled("VALUE", theme_helpers::table_header_style(theme)),
        ])
        .style(theme_helpers::table_header_row_style(theme))
    }

    /// Build the data rows for the table.
    ///
    /// This method creates all the data rows for the table, handling
    /// different display modes for editing vs browsing, and applying
    /// appropriate styling based on selection state.
    ///
    /// # Arguments
    ///
    /// * `editor` - The key/value editor state
    /// * `theme` - The theme for styling
    ///
    /// # Returns
    ///
    /// A vector of styled Row widgets for the table data.
    fn build_table_rows<'a>(
        &self,
        editor: &crate::ui::components::plugins::plugin_editor::key_value_editor::state::KeyValueEditorState,
        theme: &'a dyn Theme,
    ) -> Vec<Row<'a>> {
        let editing_row_index = editor.editing_row_index();
        let editing_buffers = editor.editing_buffers();

        editor
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let is_edit_row = editing_row_index == Some(index);
                let is_selected = if let Some(edit_index) = editing_row_index {
                    edit_index == index
                } else {
                    editor.selected_row_index == Some(index)
                };

                let (display_key, display_value) = self.build_row_display_values(row, is_edit_row, editing_buffers);
                let arrow = if is_selected { "›" } else { " " };
                let key_cell = format!("{} {}", arrow, display_key);
                let row_style = if is_selected {
                    theme_helpers::table_selected_style(theme)
                } else {
                    theme_helpers::table_row_style(theme, index)
                };

                Row::new(vec![key_cell, display_value]).style(row_style)
            })
            .collect()
    }

    /// Build the display values for a table row.
    ///
    /// This method determines what values to display for a row based on
    /// whether it's being edited and whether the value should be masked.
    ///
    /// # Arguments
    ///
    /// * `row` - The environment row data
    /// * `is_edit_row` - Whether this row is currently being edited
    /// * `editing_buffers` - The current editing buffer values
    ///
    /// # Returns
    ///
    /// A tuple of (key, value) strings to display.
    fn build_row_display_values(
        &self,
        row: &crate::ui::components::plugins::EnvRow,
        is_edit_row: bool,
        editing_buffers: Option<(&str, &str)>,
    ) -> (String, String) {
        if is_edit_row {
            if let Some((key, value)) = editing_buffers {
                (key.to_string(), value.to_string())
            } else {
                (row.key.clone(), row.value.clone())
            }
        } else {
            let masked_value = if row.is_secret {
                "••••••••••".to_string()
            } else {
                row.value.clone()
            };
            (row.key.clone(), masked_value)
        }
    }

    /// Render the inline editor for editing key/value pairs.
    ///
    /// This method renders the inline editing interface that appears below
    /// the table when a row is being edited. It shows separate fields for
    /// key and value editing with appropriate focus indicators.
    ///
    /// # Arguments
    ///
    /// * `frame` - The Ratatui frame for rendering
    /// * `area` - The available rendering area for the editor
    /// * `theme` - The theme for styling
    /// * `add_state` - The plugin add view state containing editor state
    fn render_inline_editor(&self, frame: &mut Frame, area: Rect, theme: &dyn Theme, add_state: &PluginEditViewState) {
        let editor = &add_state.kv_editor;
        let (key_buffer, value_buffer) = editor.editing_buffers().unwrap_or(("", ""));
        let row_number = editor.editing_row_index().map(|idx| idx + 1).unwrap_or(1);

        let title = format!(
            "Editing {} Row {} — {} field",
            add_state.key_value_table_label(),
            row_number,
            editor.active_field_label()
        );

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border_style(add_state.is_key_value_editor_focused()))
            .style(theme_helpers::panel_style(theme))
            .title(Span::styled(title, theme.text_secondary_style()));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let fields = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(inner);

        let key_field_active = editor.is_key_field_focused();
        let value_field_active = editor.is_value_field_focused();

        self.render_inline_field(
            frame,
            fields[0],
            theme,
            InlineFieldDisplay {
                label: "Key",
                value: key_buffer,
                placeholder: "(required)",
                focused: key_field_active,
            },
        );
        self.render_inline_field(
            frame,
            fields[1],
            theme,
            InlineFieldDisplay {
                label: "Value",
                value: value_buffer,
                placeholder: "(optional)",
                focused: value_field_active,
            },
        );

        let (key_cursor, value_cursor) = editor.editing_cursors().unwrap_or((key_buffer.len(), value_buffer.len()));
        self.position_cursor_for_inline_field(
            frame,
            &fields,
            InlineFieldCursorState {
                key_field_active,
                key_buffer,
                value_buffer,
                key_cursor,
                value_cursor,
            },
        );
    }

    /// Render a single inline field (key or value) with appropriate styling.
    ///
    /// This method renders an individual field within the inline editor,
    /// showing the label, current value, and placeholder text with
    /// appropriate focus styling.
    ///
    /// # Arguments
    ///
    /// * `frame` - The Ratatui frame for rendering
    /// * `area` - The available rendering area for the field
    /// * `theme` - The theme for styling
    /// * `display` - Presentation metadata for the inline field
    fn render_inline_field(&self, frame: &mut Frame, area: Rect, theme: &dyn Theme, display: InlineFieldDisplay<'_>) {
        let InlineFieldDisplay {
            label,
            value,
            placeholder,
            focused,
        } = display;
        let mut spans = Vec::new();
        spans.push(Span::styled(if focused { "› " } else { "  " }, theme.text_secondary_style()));
        spans.push(Span::styled(format!("{}: ", label), theme.text_primary_style()));
        if value.is_empty() {
            spans.push(Span::styled(placeholder.to_string(), theme.text_muted_style()));
        } else {
            spans.push(Span::styled(value.to_string(), theme.text_primary_style()));
        }

        let style = if focused {
            theme.selection_style()
        } else {
            theme.text_primary_style()
        };
        let paragraph = Paragraph::new(Line::from(spans)).style(style);
        frame.render_widget(paragraph, area);
    }

    /// Position the cursor for the currently active inline field.
    ///
    /// This method calculates and sets the cursor position within the active
    /// field, accounting for the field label and current content length.
    /// The cursor is positioned at the end of the current field content.
    ///
    /// # Arguments
    ///
    /// * `frame` - The Ratatui frame for cursor positioning
    /// * `fields` - The layout areas for the key and value fields
    /// * `cursor_state` - Aggregated cursor metadata for the editor
    fn position_cursor_for_inline_field(&self, frame: &mut Frame, fields: &[Rect], cursor_state: InlineFieldCursorState<'_>) {
        if fields.len() < 2 {
            return;
        }
        let InlineFieldCursorState {
            key_field_active,
            key_buffer,
            value_buffer,
            key_cursor,
            value_cursor,
        } = cursor_state;
        let (target_area, field_label, content_length) = if key_field_active {
            let prefix = &key_buffer[..key_cursor.min(key_buffer.len())];
            (fields[0], "Key", prefix.chars().count())
        } else {
            let prefix = &value_buffer[..value_cursor.min(value_buffer.len())];
            (fields[1], "Value", prefix.chars().count())
        };
        let label_prefix = format!("{}: ", field_label);
        let offset = 2 + label_prefix.chars().count();
        let cursor_x = target_area.x + offset as u16 + content_length as u16;
        let cursor_y = target_area.y;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

impl Component for KeyValueEditorComponent {
    /// Handle keyboard events for the key/value editor component.
    ///
    /// This method processes keyboard input when the component has focus,
    /// delegating to the appropriate handler based on the current editing state.
    /// It returns a vector of effects that should be processed by the application runtime.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state containing the plugin add view state
    /// * `key_event` - The keyboard event to process
    ///
    /// # Returns
    ///
    /// A vector of effects that should be processed by the application runtime.
    fn handle_key_events(&mut self, app: &mut App, key_event: KeyEvent) -> Vec<Effect> {
        let editing = app.plugins.add.as_ref().map(|editor| editor.kv_editor.is_editing());
        if editing.unwrap_or(false) {
            self.handle_editing_mode_input(app, key_event);
        } else {
            self.handle_navigation_mode_input(app, key_event)
        }

        vec![]
    }

    /// Render the key/value editor component.
    ///
    /// This method renders the complete key/value editor interface, including
    /// the table view and inline editor (when active). It delegates to the
    /// specialized rendering method with the current application state.
    ///
    /// # Arguments
    ///
    /// * `frame` - The Ratatui frame for rendering
    /// * `area` - The available rendering area
    /// * `app` - The application state containing the plugin add view state
    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let Some(add_state) = app.plugins.add.as_ref() else {
            return;
        };
        let theme = &*app.ctx.theme;
        self.render_with_state(frame, area, theme, add_state);
    }

    /// Get hint spans for the key/value editor component.
    ///
    /// This method provides contextual keyboard shortcuts and hints based on
    /// the current editing state. It shows different hints for navigation
    /// mode vs editing mode to help users understand available actions.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state containing the plugin add view state
    /// * `is_root` - Whether this is the root component (affects hint formatting)
    ///
    /// # Returns
    ///
    /// A vector of styled spans representing the available keyboard shortcuts.
    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let add_state = app.plugins.add.as_ref().expect("add state should be something");
        let theme = &app.ctx.theme;
        let mut spans = vec![];

        let is_editing = add_state.kv_editor.is_editing();

        // Add common navigation hints
        self.add_common_hints(&mut spans, theme.as_ref(), is_editing);

        // Add mode-specific hints
        if is_editing {
            self.add_editing_mode_hints(&mut spans, theme.as_ref());
        } else {
            self.add_navigation_mode_hints(&mut spans, theme.as_ref());
        }

        spans
    }
}

impl KeyValueEditorComponent {
    /// Add common hints that appear in both editing and navigation modes.
    ///
    /// This method adds the basic navigation hints that are available
    /// regardless of the current editing state.
    ///
    /// # Arguments
    ///
    /// * `spans` - The vector of spans to add hints to
    /// * `theme` - The theme for styling
    /// * `is_editing` - Whether the editor is currently in editing mode
    fn add_common_hints(&self, spans: &mut Vec<Span<'_>>, theme: &dyn Theme, is_editing: bool) {
        spans.extend([
            Span::styled("↑/↓", theme.accent_emphasis_style()),
            Span::styled(if is_editing { " Focus  " } else { " Navigate  " }, theme.text_muted_style()),
        ]);
    }

    /// Add hints specific to editing mode.
    ///
    /// This method adds keyboard shortcuts that are only available
    /// when the editor is in editing mode.
    ///
    /// # Arguments
    ///
    /// * `spans` - The vector of spans to add hints to
    /// * `theme` - The theme for styling
    fn add_editing_mode_hints(&self, spans: &mut Vec<Span<'_>>, theme: &dyn Theme) {
        spans.extend([
            Span::styled("Enter/Ctrl+E", theme.accent_emphasis_style()),
            Span::styled(" Apply  ", theme.text_muted_style()),
            Span::styled("Esc", theme.accent_emphasis_style()),
            Span::styled(" Cancel  ", theme.text_muted_style()),
        ]);
    }

    /// Add hints specific to navigation mode.
    ///
    /// This method adds keyboard shortcuts that are only available
    /// when the editor is in navigation mode.
    ///
    /// # Arguments
    ///
    /// * `spans` - The vector of spans to add hints to
    /// * `theme` - The theme for styling
    fn add_navigation_mode_hints(&self, spans: &mut Vec<Span<'_>>, theme: &dyn Theme) {
        spans.extend([
            Span::styled("Home/End", theme.accent_emphasis_style()),
            Span::styled(" First/Last  ", theme.text_muted_style()),
            Span::styled("Enter/Ctrl+E", theme.accent_emphasis_style()),
            Span::styled(" Edit  ", theme.text_muted_style()),
            Span::styled("Ctrl+N", theme.accent_emphasis_style()),
            Span::styled(" New  ", theme.text_muted_style()),
            Span::styled("Delete/Ctrl+D", theme.accent_emphasis_style()),
            Span::styled(" Delete ", theme.text_muted_style()),
        ]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::components::plugins::EnvRow;
    use crate::ui::components::plugins::plugin_editor::key_value_editor::state::KeyValueEditorState;
    use crossterm::event::KeyModifiers;
    #[test]
    fn test_is_regular_character_input_with_control_modifier() {
        let component = KeyValueEditorComponent;
        let modifiers = KeyModifiers::CONTROL;

        assert!(!component.is_regular_character_input(modifiers));
    }

    #[test]
    fn test_is_regular_character_input_with_alt_modifier() {
        let component = KeyValueEditorComponent;
        let modifiers = KeyModifiers::ALT;

        assert!(!component.is_regular_character_input(modifiers));
    }

    #[test]
    fn test_is_regular_character_input_with_no_modifiers() {
        let component = KeyValueEditorComponent;
        let modifiers = KeyModifiers::empty();

        assert!(component.is_regular_character_input(modifiers));
    }

    #[test]
    fn test_is_regular_character_input_with_shift_modifier() {
        let component = KeyValueEditorComponent;
        let modifiers = KeyModifiers::SHIFT;

        assert!(component.is_regular_character_input(modifiers));
    }

    #[test]
    fn test_build_table_title_when_not_editing() {
        let component = KeyValueEditorComponent;
        let editor = KeyValueEditorState::new("test");
        let label = "Environment Variables";

        let title = component.build_table_title(&editor, label);

        assert_eq!(title, "Environment Variables");
    }

    #[test]
    fn test_build_row_display_values_for_regular_row() {
        let component = KeyValueEditorComponent;
        let row = EnvRow {
            key: "API_KEY".to_string(),
            value: "secret123".to_string(),
            is_secret: false,
        };

        let (key, value) = component.build_row_display_values(&row, false, None);

        assert_eq!(key, "API_KEY");
        assert_eq!(value, "secret123");
    }

    #[test]
    fn test_build_row_display_values_for_secret_row() {
        let component = KeyValueEditorComponent;
        let row = EnvRow {
            key: "PASSWORD".to_string(),
            value: "secret123".to_string(),
            is_secret: true,
        };

        let (key, value) = component.build_row_display_values(&row, false, None);

        assert_eq!(key, "PASSWORD");
        assert_eq!(value, "••••••••••");
    }

    #[test]
    fn test_build_row_display_values_for_editing_row() {
        let component = KeyValueEditorComponent;
        let row = EnvRow {
            key: "API_KEY".to_string(),
            value: "secret123".to_string(),
            is_secret: true,
        };
        let editing_buffers = Some(("NEW_KEY", "new_value"));

        let (key, value) = component.build_row_display_values(&row, true, editing_buffers);

        assert_eq!(key, "NEW_KEY");
        assert_eq!(value, "new_value");
    }
}
