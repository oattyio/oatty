//! Combined key/value results and inline editor for the plugin add flow.
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

use std::rc::Rc;

use anyhow::Error;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use oatty_types::value_objects::EnvRow;
use rat_focus::Focus;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Cell, Row, Table},
};

use crate::ui::theme::{
    Theme,
    theme_helpers::{self, block, create_checkbox},
};
use crate::ui::{
    components::common::key_value_editor::{KeyValueEditorField, KeyValueEditorState},
    theme::theme_helpers::{button_primary_style, button_secondary_style},
};

#[derive(Debug, Clone, Copy)]
enum RowNavigation {
    Previous,
    Next,
    First,
    Last,
}

#[derive(Debug, Default)]
struct KeyValueLayout {
    add_button_area: Rect,
    remove_button_area: Rect,
    show_secrets_area: Rect,
    table_area: Rect,
}

impl KeyValueLayout {
    fn new(add_button_area: Rect, remove_button_area: Rect, show_secrets_rect: Rect, table_area: Rect) -> Self {
        KeyValueLayout {
            add_button_area,
            remove_button_area,
            show_secrets_area: show_secrets_rect,
            table_area,
        }
    }
}

const TABLE_COLUMN_SPACING: u16 = 1;
const SELECTION_PREFIX_WIDTH: u16 = 2;

/// Returns the column constraints for the results.
fn table_column_constraints() -> [Constraint; 2] {
    [Constraint::Percentage(35), Constraint::Percentage(65)]
}

/// Returns the column areas for the results.
fn column_areas(inner_area: Rect) -> Rc<[Rect]> {
    Layout::horizontal(table_column_constraints())
        .spacing(TABLE_COLUMN_SPACING)
        .split(inner_area)
}

/// Component responsible for rendering and editing key/value pairs.
///
/// This component provides a tabular interface for managing key/value pairs
/// with inline editing capabilities. It supports row navigation, field focus
/// cycling, and common operations like adding or deleting rows.
///
/// The component is designed to be stateless, delegating all state management
/// to the parent `PluginAddViewState`. This allows for better separation of
/// concerns and easier testing.
#[derive(Debug, Default)]
pub struct KeyValueEditorView {
    last_layout: KeyValueLayout,
}

impl KeyValueEditorView {
    /// Handle keyboard input for the inline results editor.
    ///
    /// This method routes keyboard input for inline key/value editing,
    /// row navigation, and focus cycling between the key and value fields.
    ///
    /// # Arguments
    ///
    /// * `state` - The mutable editor state to update
    /// * `key_event` - The keyboard event to process
    ///
    /// # Returns
    ///
    /// Outcome metadata describing edits and validation results.
    pub fn handle_key_event(&mut self, state: &mut KeyValueEditorState, key_event: KeyEvent, focus: Rc<Focus>) {
        let modifiers = key_event.modifiers;

        match key_event.code {
            KeyCode::Up => {
                self.handle_row_navigation(state, RowNavigation::Previous);
            }
            KeyCode::Down => {
                self.handle_row_navigation(state, RowNavigation::Next);
            }
            KeyCode::Home => {
                self.handle_row_navigation(state, RowNavigation::First);
            }
            KeyCode::End => {
                self.handle_row_navigation(state, RowNavigation::Last);
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.handle_tab_navigation(key_event.code, focus);
            }
            KeyCode::Char('a') if modifiers.contains(KeyModifiers::CONTROL) => {
                let validation = state.validate_focused_row();
                if validation.is_ok() {
                    state.add_new_row();
                    focus.focus(&state.f_key_field);
                }
            }
            KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                state.delete_selected_row();
            }
            KeyCode::Left => {
                state.move_cursor_left();
            }
            KeyCode::Right => {
                state.move_cursor_right();
            }
            KeyCode::Backspace => {
                state.delete_previous_character();
            }
            KeyCode::Delete => {
                state.delete_next_character();
            }
            KeyCode::Char(' ') | KeyCode::Enter if state.f_show_secrets_button.get() => {
                state.toggle_show_secrets();
            }
            KeyCode::Char(character) if self.is_regular_character_input(modifiers) => {
                if state.selected_row().is_none() {
                    state.add_new_row();
                    focus.focus(&state.f_key_field);
                }
                state.insert_character(character);
            }
            _ => {}
        }
    }

    pub fn handle_mouse_event(&mut self, state: &mut KeyValueEditorState, mouse_event: MouseEvent, focus: Rc<Focus>) {
        let pos = Position::new(mouse_event.column, mouse_event.row);
        let hit_test_table = self.last_layout.table_area.contains(pos);
        let list_offset = state.table_state().offset();
        let idx = pos.y.saturating_sub(self.last_layout.table_area.y + 1) as usize + list_offset;

        match mouse_event.kind {
            MouseEventKind::Down(MouseButton::Left) => match () {
                _ if hit_test_table && idx < state.rows().len() => {
                    if !state.f_table.get() {
                        focus.focus(&state.f_table);
                    }
                    state.set_selected_row(Some(idx));
                    // determine if the mouse was clicked over the key or value field.
                    let column_areas = column_areas(self.last_layout.table_area);
                    match (column_areas[0].contains(pos), column_areas[1].contains(pos)) {
                        (true, false) => {
                            focus.focus(&state.f_key_field);
                        }
                        (false, true) => {
                            focus.focus(&state.f_value_field);
                        }
                        _ => {}
                    }
                    state.prepare_value_field_for_input();
                }
                _ if self.last_layout.add_button_area.contains(pos) => {
                    focus.focus(&state.f_add_button);
                    let validation = state.validate_focused_row();
                    if validation.is_ok() {
                        state.add_new_row();
                        focus.focus(&state.f_key_field);
                    }
                }
                _ if self.last_layout.remove_button_area.contains(pos) => {
                    focus.focus(&state.f_remove_button);
                    state.delete_selected_row();
                }
                _ if self.last_layout.show_secrets_area.contains(pos) => {
                    focus.focus(&state.f_show_secrets_button);
                    state.toggle_show_secrets();
                }
                () => {}
            },
            MouseEventKind::Moved => {}
            _ => {}
        }
    }

    /// Render the inline results editor.
    ///
    /// This method renders the results and positions the cursor for the focused
    /// input field inside the selected row.
    ///
    /// # Arguments
    ///
    /// * `frame` - The Ratatui frame for rendering
    /// * `area` - The available rendering area
    /// * `theme` - The theme for styling
    /// * `state` - The key/value editor state
    pub fn render_with_state(&mut self, frame: &mut Frame, area: Rect, theme: &dyn Theme, state: &mut KeyValueEditorState) {
        let title = state.block_label();
        let is_focused = state.is_focused();

        let block = block(theme, Some(title), is_focused);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let layout = Layout::vertical([
            Constraint::Length(1),       // Buttons
            Constraint::Length(1),       // Spacer
            Constraint::Percentage(100), // Table
        ])
        .split(inner);

        let buttons_layout = Layout::horizontal([
            Constraint::Min(1),     // Error message
            Constraint::Length(7),  // Add
            Constraint::Length(1),  // spacer
            Constraint::Length(10), // Remove
            Constraint::Length(1),  // spacer
            Constraint::Length(17), // view secrets
        ])
        .split(layout[0]);

        let errors: Vec<Error> = (0..state.rows().len()).filter_map(|i| state.validate_row(i).err()).collect();
        if !errors.is_empty() {
            let message = if let Some(e) = errors.first()
                && errors.len() == 1
            {
                format!(" {}", e)
            } else {
                " ✘ One or more headers contain errors".to_string()
            };
            frame.render_widget(Span::styled(message, theme.status_error()), buttons_layout[0]);
        }

        let add = Line::from(vec![
            Span::styled(" + ", theme.status_success()),
            Span::styled("Add ", theme.text_primary_style()),
        ])
        .style(button_primary_style(theme, true, state.f_add_button.get()));
        frame.render_widget(add, buttons_layout[1]);

        let remove = Line::from(vec![
            Span::styled(" – ", theme.status_error()),
            Span::styled("Remove ", theme.text_secondary_style()),
        ])
        .style(button_secondary_style(theme, true, state.f_remove_button.get()));
        frame.render_widget(remove, buttons_layout[3]);

        let view_secrets = create_checkbox(
            Some("Show secrets "),
            state.show_secrets(),
            state.f_show_secrets_button.get(),
            theme,
        );
        frame.render_widget(view_secrets, buttons_layout[5]);

        self.render_table(frame, layout[2], theme, state);

        self.last_layout = KeyValueLayout::new(buttons_layout[1], buttons_layout[3], buttons_layout[5], layout[2]);
        self.position_cursor_for_focused_input(frame, state);
    }

    /// Render the key/value results with proper styling and selection indicators.
    ///
    /// This method renders the main results view showing all key/value pairs
    /// and applies styling based on selection and focus state.
    ///
    /// # Arguments
    ///
    /// * `frame` - The Ratatui frame for rendering
    /// * `area` - The available rendering area for the results
    /// * `theme` - The theme for styling
    /// * `state` - The key/value editor state
    fn render_table(&self, frame: &mut Frame, area: Rect, theme: &dyn Theme, state: &mut KeyValueEditorState) {
        let header = self.build_table_header(state, theme);
        let rows = self.build_table_rows(state, theme);

        let table = Table::new(rows, table_column_constraints()).header(header);

        frame.render_stateful_widget(table, area, state.table_state_mut());
    }

    /// Build the header row for the results.
    ///
    /// This method creates the column headers for the key/value results
    /// with appropriate styling.
    ///
    /// # Arguments
    ///
    /// * `state` - The key/value editor state
    /// * `theme` - The theme for styling
    ///
    /// # Returns
    ///
    /// A styled Row widget for the results header.
    fn build_table_header<'a>(&self, state: &KeyValueEditorState, theme: &'a dyn Theme) -> Row<'a> {
        let style = theme_helpers::table_header_style(theme);
        Row::new(vec![
            Span::styled(state.key_label(), style),
            Span::styled(state.value_label(), style),
        ])
        .style(theme_helpers::table_header_row_style(theme))
    }

    /// Build the data rows for the results.
    ///
    /// This method creates all the data rows for the results and applies
    /// appropriate styling based on selection and focus state.
    ///
    /// # Arguments
    ///
    /// * `editor` - The key/value editor state
    /// * `theme` - The theme for styling
    ///
    /// # Returns
    ///
    /// A vector of styled Row widgets for the results data.
    fn build_table_rows<'a>(&self, state: &KeyValueEditorState, theme: &'a dyn Theme) -> Vec<Row<'a>> {
        let selected_row_index = state.selected_row();
        let is_editor_focused = state.is_focused();
        let active_field = state.active_field();
        let show_secrets = state.show_secrets();

        state
            .rows()
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let is_selected = selected_row_index == Some(index);
                let is_error = state.validate_row(index).is_err();
                let arrow = if is_error {
                    "✘"
                } else if is_selected {
                    "›"
                } else {
                    " "
                };
                let key_focused = is_selected && is_editor_focused && active_field == KeyValueEditorField::Key;
                let value_focused = is_selected && is_editor_focused && active_field == KeyValueEditorField::Value;

                let key_cell = build_table_key_cell(&row.key, arrow, theme, key_focused, is_error);
                let value_cell = build_table_value_cell(row, theme, value_focused, show_secrets);
                let row_style = if is_selected {
                    theme_helpers::table_selected_style(theme)
                } else {
                    theme_helpers::table_row_style(theme, index)
                };

                Row::new(vec![key_cell, value_cell]).style(row_style)
            })
            .collect()
    }

    fn handle_tab_navigation(&self, key_code: KeyCode, focus: Rc<Focus>) {
        match key_code {
            KeyCode::Tab => {
                focus.next();
            }
            KeyCode::BackTab => {
                focus.prev();
            }
            _ => {}
        }
    }

    fn handle_row_navigation(&self, state: &mut KeyValueEditorState, navigation: RowNavigation) {
        match navigation {
            RowNavigation::Previous => state.select_previous_row(),
            RowNavigation::Next => state.select_next_row(),
            RowNavigation::First => state.select_first_row(),
            RowNavigation::Last => state.select_last_row(),
        };
        state.prepare_value_field_for_input();
    }

    fn position_cursor_for_focused_input(&self, frame: &mut Frame, state: &KeyValueEditorState) {
        if !state.f_key_field.get() && !state.f_value_field.get() {
            return;
        }
        let Some(row_index) = state.selected_row() else {
            return;
        };
        let inner_area = self.last_layout.table_area;
        if inner_area.height < 2 {
            return;
        }
        let row_y = inner_area.y.saturating_add(1).saturating_add(row_index as u16);
        if row_y >= inner_area.y.saturating_add(inner_area.height) {
            return;
        }
        let column_areas = column_areas(inner_area);
        if column_areas.len() < 2 {
            return;
        }
        let (target_area, cursor_columns, prefix_width) = match state.active_field() {
            KeyValueEditorField::Key => (column_areas[0], state.key_input_state().cursor_columns(), SELECTION_PREFIX_WIDTH),
            KeyValueEditorField::Value => (column_areas[1], state.value_input_state().cursor_columns(), 0),
        };
        let mut cursor_x = target_area.x.saturating_add(prefix_width).saturating_add(cursor_columns as u16);
        let max_x = target_area.x.saturating_add(target_area.width.saturating_sub(1));
        cursor_x = cursor_x.min(max_x);
        frame.set_cursor_position((cursor_x, row_y));
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

    /// Add hints for inline results editing.
    ///
    /// This method adds keyboard shortcuts that are available while the
    /// key/value editor has focus.
    ///
    /// # Arguments
    ///
    /// * `spans` - The vector of spans to add hints to
    /// * `theme` - The theme for styling
    pub fn add_table_hints(&self, spans: &mut Vec<Span<'_>>, theme: &dyn Theme) {
        spans.extend(theme_helpers::build_hint_spans(
            theme,
            &[
                ("Tab/Shift+Tab", " Field  "),
                ("↑/↓", " Navigate  "),
                ("Home/End", " First/Last  "),
                ("Ctrl+A", " Add  "),
                ("Ctrl+D", " Delete "),
            ],
        ));
    }
}

/// Build the results cell for the key column, mixing the selection arrow and syntax colors.
fn build_table_key_cell<'a>(display_key: &str, arrow: &str, theme: &dyn Theme, focused: bool, is_error: bool) -> Cell<'a> {
    let arrow_style = if is_error {
        theme.status_error()
    } else {
        theme.text_secondary_style()
    };
    let spans = vec![
        Span::styled(format!("{arrow} "), arrow_style),
        Span::styled(display_key.to_string(), theme.syntax_type_style()),
    ];
    let style = if focused { theme.selection_style() } else { Style::default() };
    Cell::from(Line::from(spans)).style(style)
}

/// Build the results cell for the value column using syntax colors.
fn build_table_value_cell<'a>(env_row: &EnvRow, theme: &dyn Theme, focused: bool, show_secrets: bool) -> Cell<'a> {
    let value_style = theme.syntax_string_style();
    let style = if focused { theme.selection_style() } else { Style::default() };
    let display = if !show_secrets && env_row.is_secret {
        "*".repeat(env_row.value.len())
    } else {
        env_row.value.clone()
    };
    Cell::from(Span::styled(display, value_style)).style(style)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    #[test]
    fn test_is_regular_character_input_with_control_modifier() {
        let component = KeyValueEditorView::default();
        let modifiers = KeyModifiers::CONTROL;

        assert!(!component.is_regular_character_input(modifiers));
    }

    #[test]
    fn test_is_regular_character_input_with_alt_modifier() {
        let component = KeyValueEditorView::default();
        let modifiers = KeyModifiers::ALT;

        assert!(!component.is_regular_character_input(modifiers));
    }

    #[test]
    fn test_is_regular_character_input_with_no_modifiers() {
        let component = KeyValueEditorView::default();
        let modifiers = KeyModifiers::empty();

        assert!(component.is_regular_character_input(modifiers));
    }

    #[test]
    fn test_is_regular_character_input_with_shift_modifier() {
        let component = KeyValueEditorView::default();
        let modifiers = KeyModifiers::SHIFT;

        assert!(component.is_regular_character_input(modifiers));
    }
}
