//! Combined key/value table and inline editor for the plugin add flow.
//!
//! This component encapsulates the tabular display of key/value pairs and the
//! inline editing experience that previously lived directly inside
//! `add.rs`. It centralizes keyboard handling, rendering, and cursor
//! positioning so the parent `PluginsAddComponent` can remain focused on the
//! surrounding form controls.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use heroku_types::Effect;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
};

use crate::ui::{
    components::{
        component::Component,
        plugins::{KeyValueEditorField, add_plugin::state::PluginAddViewState},
    },
    theme::{Theme, theme_helpers},
};

/// Component responsible for rendering and editing key/value pairs.
#[derive(Debug, Default)]
pub struct KeyValueEditorComponent;

impl KeyValueEditorComponent {
    /// Process a key event for the key/value editor when it has focus.
    ///
    /// The caller is responsible for ensuring that the component should handle
    /// the event (typically by checking the key/value focus flag).
    fn process_key_event(&mut self, add_state: &mut PluginAddViewState, key_event: &KeyEvent) -> Vec<Effect> {
        if !add_state.f_key_value_pairs.get() {
            return vec![];
        }
        let mut validation_update: Option<Option<String>> = None;
        let modifiers = key_event.modifiers;

        {
            let editor = add_state.active_key_value_editor_mut();

            if editor.is_editing() {
                match key_event.code {
                    KeyCode::Esc => {
                        editor.cancel_edit();
                        validation_update = Some(None);
                    }
                    KeyCode::Enter => match editor.commit_edit() {
                        Ok(()) => validation_update = Some(None),
                        Err(error) => validation_update = Some(Some(error.to_string())),
                    },
                    KeyCode::Char('e') if modifiers.contains(KeyModifiers::CONTROL) => match editor.commit_edit() {
                        Ok(()) => validation_update = Some(None),
                        Err(error) => validation_update = Some(Some(error.to_string())),
                    },
                    KeyCode::Backspace => {
                        editor.pop_character();
                    }
                    KeyCode::Tab | KeyCode::BackTab => {
                        editor.toggle_field();
                    }
                    KeyCode::Left => {
                        editor.set_active_field(KeyValueEditorField::Key);
                    }
                    KeyCode::Right => {
                        editor.set_active_field(KeyValueEditorField::Value);
                    }
                    KeyCode::Char(character)
                        if !modifiers.contains(KeyModifiers::CONTROL) && !modifiers.contains(KeyModifiers::ALT) =>
                    {
                        editor.push_character(character);
                    }
                    _ => {}
                }
            } else {
                editor.ensure_selection();
                match key_event.code {
                    KeyCode::Up => {
                        editor.select_previous();
                    }
                    KeyCode::Down => {
                        editor.select_next();
                    }
                    KeyCode::Enter => {
                        editor.begin_editing_selected();
                        validation_update = Some(None);
                    }
                    KeyCode::Char('e') if modifiers.contains(KeyModifiers::CONTROL) => {
                        editor.begin_editing_selected();
                        validation_update = Some(None);
                    }
                    KeyCode::Char('n') if modifiers.contains(KeyModifiers::CONTROL) => {
                        editor.begin_editing_new_row();
                        validation_update = Some(None);
                    }
                    KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                        if editor.delete_selected() {
                            validation_update = Some(None);
                        }
                    }
                    KeyCode::Delete => {
                        if editor.delete_selected() {
                            validation_update = Some(None);
                        }
                    }
                    KeyCode::Char(character)
                        if !modifiers.contains(KeyModifiers::CONTROL) && !modifiers.contains(KeyModifiers::ALT) =>
                    {
                        editor.begin_editing_selected();
                        editor.push_character(character);
                        validation_update = Some(None);
                    }
                    KeyCode::Home => {
                        if !editor.rows.is_empty() {
                            editor.selected_row_index = Some(0);
                        }
                    }
                    KeyCode::End => {
                        if !editor.rows.is_empty() {
                            editor.selected_row_index = Some(editor.rows.len().saturating_sub(1));
                        }
                    }
                    _ => {}
                }
            }
        }

        if let Some(update) = validation_update {
            add_state.validation = update;
        }

        vec![]
    }

    /// Render the combined table and inline editor.
    pub fn render_with_state(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        theme: &dyn Theme,
        add_state: &PluginAddViewState,
    ) {
        let editor = add_state.active_key_value_editor();
        let is_editing = editor.is_editing();

        let mut constraints = vec![Constraint::Min(4)];
        if is_editing {
            constraints.push(Constraint::Length(4));
        }

        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        let table_area = sections[0];
        self.render_table(frame, table_area, theme, add_state);

        if is_editing {
            let editor_area = sections[1];
            self.render_inline_editor(frame, editor_area, theme, add_state);
        }
    }

    fn render_table(&self, frame: &mut Frame, area: Rect, theme: &dyn Theme, add_state: &PluginAddViewState) {
        let editor = add_state.active_key_value_editor();
        let label = add_state.key_value_table_label();
        let is_focused = add_state.f_key_value_pairs.get();

        let editing_row_index = editor.editing_row_index();
        let editing_buffers = editor.editing_buffers();

        let title = if let Some(row_index) = editing_row_index {
            let field_name = editing_buffers
                .map(|(_, _, field)| match field {
                    KeyValueEditorField::Key => "key",
                    KeyValueEditorField::Value => "value",
                })
                .unwrap_or("key");
            format!("{} — editing row {} ({})", label, row_index + 1, field_name)
        } else {
            label.to_string()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border_style(is_focused))
            .style(theme_helpers::panel_style(theme))
            .title(Span::styled(title, theme.text_secondary_style()));

        let header = Row::new(vec![
            Span::styled("KEY", theme_helpers::table_header_style(theme)),
            Span::styled("VALUE", theme_helpers::table_header_style(theme)),
        ])
        .style(theme_helpers::table_header_row_style(theme));

        let rows: Vec<Row> = if editor.rows.is_empty() {
            vec![Row::new(vec!["  --".to_string(), "Press Ctrl+N to add".to_string()]).style(theme.text_muted_style())]
        } else {
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

                    let (display_key, display_value) = if is_edit_row {
                        if let Some((key, value, _)) = editing_buffers {
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
                    };

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
        };

        let table = Table::new(rows, [Constraint::Percentage(35), Constraint::Percentage(65)])
            .header(header)
            .block(block);

        frame.render_widget(table, area);
    }

    fn render_inline_editor(&self, frame: &mut Frame, area: Rect, theme: &dyn Theme, add_state: &PluginAddViewState) {
        let editor = add_state.active_key_value_editor();
        let (key_buffer, value_buffer, active_field) =
            editor.editing_buffers().unwrap_or(("", "", KeyValueEditorField::Key));
        let row_number = editor.editing_row_index().map(|idx| idx + 1).unwrap_or(1);

        let title = format!(
            "Editing {} Row {} — {} field",
            add_state.key_value_table_label(),
            row_number,
            match active_field {
                KeyValueEditorField::Key => "key",
                KeyValueEditorField::Value => "value",
            }
        );

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border_style(add_state.f_key_value_pairs.get()))
            .style(theme_helpers::panel_style(theme))
            .title(Span::styled(title, theme.text_secondary_style()));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let fields = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(inner);

        self.render_inline_field(
            frame,
            fields[0],
            theme,
            "Key",
            key_buffer,
            "(required)",
            matches!(active_field, KeyValueEditorField::Key),
        );
        self.render_inline_field(
            frame,
            fields[1],
            theme,
            "Value",
            value_buffer,
            "(optional)",
            matches!(active_field, KeyValueEditorField::Value),
        );

        self.position_cursor_for_inline_field(frame, &fields, active_field, key_buffer, value_buffer);
    }

    fn render_inline_field(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &dyn Theme,
        label: &str,
        value: &str,
        placeholder: &str,
        focused: bool,
    ) {
        let mut spans = Vec::new();
        spans.push(Span::styled(
            if focused { "› " } else { "  " },
            theme.text_secondary_style(),
        ));
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

    fn position_cursor_for_inline_field(
        &self,
        frame: &mut Frame,
        fields: &[Rect],
        active_field: KeyValueEditorField,
        key_buffer: &str,
        value_buffer: &str,
    ) {
        if fields.len() < 2 {
            return;
        }
        let (target_area, label, value_length) = match active_field {
            KeyValueEditorField::Key => (fields[0], "Key", key_buffer.chars().count()),
            KeyValueEditorField::Value => (fields[1], "Value", value_buffer.chars().count()),
        };
        let label_prefix = format!("{}: ", label);
        let offset = 2 + label_prefix.chars().count();
        let cursor_x = target_area.x + offset as u16 + value_length as u16;
        let cursor_y = target_area.y;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

impl Component for KeyValueEditorComponent {
    fn handle_key_events(&mut self, app: &mut crate::app::App, key_event: KeyEvent) -> Vec<Effect> {
        let Some(add_state) = app.plugins.add.as_mut() else {
            return Vec::new();
        };
        self.process_key_event(add_state, &key_event)
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut crate::app::App) {
        let Some(add_state) = app.plugins.add.as_ref() else {
            return;
        };
        let theme = &*app.ctx.theme;
        self.render_with_state(frame, area, theme, add_state);
    }
}
