use std::borrow::Cow;

use crate::ui::components::common::TextInputState;
use anyhow::{Result, anyhow};
use oatty_types::value_objects::EnvRow;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::{layout::Rect, widgets::TableState};

/// Identifies which field is active within the focused row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyValueEditorField {
    /// The key column input.
    Key,
    /// The value column input.
    Value,
}

/// State container for results-driven editing of key/value pairs.
#[derive(Debug, Default)]
pub struct KeyValueEditorState {
    pub f_table: FocusFlag,
    pub f_key_field: FocusFlag,
    pub f_value_field: FocusFlag,
    pub f_add_button: FocusFlag,
    pub f_remove_button: FocusFlag,
    pub f_show_secrets_button: FocusFlag,

    rows: Vec<EnvRow>,
    table_state: TableState,
    block_label: Cow<'static, str>,
    key_label: Cow<'static, str>,
    value_label: Cow<'static, str>,
    container: FocusFlag,
    key_input_state: TextInputState,
    value_input_state: TextInputState,
    is_dirty: bool,
    show_secrets: bool,
}

impl KeyValueEditorState {
    pub fn new(block_label: Cow<'static, str>, key_label: Cow<'static, str>, value_label: Cow<'static, str>) -> Self {
        Self {
            key_label,
            value_label,
            block_label,
            ..Default::default()
        }
    }

    pub fn show_secrets(&self) -> bool {
        self.show_secrets
    }

    pub fn toggle_show_secrets(&mut self) {
        self.show_secrets = !self.show_secrets;
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn reset_dirty(&mut self) {
        self.is_dirty = false;
    }

    pub fn rows(&self) -> &Vec<EnvRow> {
        &self.rows
    }

    pub fn set_rows(&mut self, rows: Vec<EnvRow>) {
        self.rows = rows;
        self.is_dirty = false;
        let normalized_selection = self
            .table_state
            .selected()
            .filter(|selected_index| *selected_index < self.rows.len());
        self.set_selected_row(normalized_selection);
    }

    pub fn is_focused(&self) -> bool {
        self.container.get()
    }

    pub fn set_selected_row(&mut self, maybe_idx: Option<usize>) {
        self.table_state.select(maybe_idx);
        self.ensure_input_states_for_selected_row();
    }

    pub fn table_state_mut(&mut self) -> &mut TableState {
        &mut self.table_state
    }

    pub fn table_state(&self) -> &TableState {
        &self.table_state
    }

    /// Returns the label of the key-value editor.
    pub fn block_label(&self) -> Cow<'static, str> {
        self.block_label.clone()
    }

    /// Sets the label of the key-value editor.
    pub fn set_block_label(&mut self, label: Cow<'static, str>) {
        self.block_label = label;
    }

    pub fn key_label(&self) -> Cow<'static, str> {
        self.key_label.clone()
    }

    pub fn value_label(&self) -> Cow<'static, str> {
        self.value_label.clone()
    }

    /// Returns the currently active field, defaulting to the value column.
    pub fn active_field(&self) -> KeyValueEditorField {
        if self.f_key_field.get() {
            KeyValueEditorField::Key
        } else {
            KeyValueEditorField::Value
        }
    }

    /// Focuses the key column within the selected row.
    pub fn prepare_key_field_for_input(&mut self) {
        self.ensure_input_states_for_selected_row();
        self.reset_cursor_for_key_field();
    }

    /// Focuses the value column within the selected row.
    pub fn prepare_value_field_for_input(&mut self) {
        self.ensure_input_states_for_selected_row();
        self.reset_cursor_for_value_field();
    }

    pub fn selected_row(&self) -> Option<usize> {
        self.table_state.selected()
    }

    /// Selects the previous row when possible.
    ///
    /// Returns `true` when the selection moved.
    pub fn select_previous_row(&mut self) {
        self.table_state.select_previous();
        self.load_input_states_for_selected_row();
    }

    /// Selects the next row when possible.
    ///
    /// Returns `true` when the selection moved.
    pub fn select_next_row(&mut self) {
        self.table_state.select_next();
        self.load_input_states_for_selected_row();
    }

    /// Selects the first row if any rows exist.
    ///
    /// Returns `true` when the selection moved.
    pub fn select_first_row(&mut self) {
        self.table_state.select_first();
        self.load_input_states_for_selected_row();
    }

    /// Selects the last row if any rows exist.
    ///
    /// Returns `true` when the selection moved.
    pub fn select_last_row(&mut self) {
        self.table_state.select_last();
        self.load_input_states_for_selected_row();
    }

    /// Inserts a new empty row and focuses the key column.
    pub fn add_new_row(&mut self) {
        self.rows.push(EnvRow {
            key: String::new(),
            value: String::new(),
            is_secret: false,
        });
        self.table_state.select(Some(self.rows.len() - 1));
        self.load_input_states_for_selected_row();
        self.prepare_key_field_for_input();
        self.is_dirty = true;
    }

    /// Deletes the currently selected row.
    pub fn delete_selected_row(&mut self) {
        let Some(index) = self.table_state.selected() else {
            return;
        };
        if index >= self.rows.len() {
            return;
        }
        self.rows.remove(index);
        if self.rows.is_empty() {
            self.clear_input_states();
        }
        self.table_state.select_next();
        self.load_input_states_for_selected_row();
        self.prepare_value_field_for_input();
        self.is_dirty = true;
    }

    /// Inserts a character into the focused input and syncs the backing row.
    ///
    /// # Arguments
    ///
    /// * `character` - The character to insert at the current cursor position
    pub fn insert_character(&mut self, character: char) {
        let Some(row_index) = self.table_state.selected() else {
            return;
        };
        match self.active_field() {
            KeyValueEditorField::Key => {
                self.key_input_state.insert_char(character);
                self.rows[row_index].key = self.key_input_state.input().to_string();
            }
            KeyValueEditorField::Value => {
                self.value_input_state.insert_char(character);
                self.rows[row_index].value = self.value_input_state.input().to_string();
            }
        }
        self.is_dirty = true;
    }

    /// Removes the character before the cursor in the focused input.
    pub fn delete_previous_character(&mut self) {
        let Some(row_index) = self.table_state.selected() else {
            return;
        };
        match self.active_field() {
            KeyValueEditorField::Key => {
                self.key_input_state.backspace();
                self.rows[row_index].key = self.key_input_state.input().to_string();
            }
            KeyValueEditorField::Value => {
                self.value_input_state.backspace();
                self.rows[row_index].value = self.value_input_state.input().to_string();
            }
        }
        self.is_dirty = true;
    }

    /// Removes the character after the cursor in the focused input.
    pub fn delete_next_character(&mut self) {
        let Some(row_index) = self.table_state.selected() else {
            return;
        };
        match self.active_field() {
            KeyValueEditorField::Key => {
                self.key_input_state.delete();
                self.rows[row_index].key = self.key_input_state.input().to_string();
            }
            KeyValueEditorField::Value => {
                self.value_input_state.delete();
                self.rows[row_index].value = self.value_input_state.input().to_string();
            }
        }
        self.is_dirty = true;
    }

    /// Moves the cursor left in the focused input.
    pub fn move_cursor_left(&mut self) {
        match self.active_field() {
            KeyValueEditorField::Key => self.key_input_state.move_left(),
            KeyValueEditorField::Value => self.value_input_state.move_left(),
        }
    }

    /// Moves the cursor right in the focused input.
    pub fn move_cursor_right(&mut self) {
        match self.active_field() {
            KeyValueEditorField::Key => self.key_input_state.move_right(),
            KeyValueEditorField::Value => self.value_input_state.move_right(),
        }
    }

    /// Returns the key input state for cursor placement.
    pub fn key_input_state(&self) -> &TextInputState {
        &self.key_input_state
    }

    /// Returns the value input state for cursor placement.
    pub fn value_input_state(&self) -> &TextInputState {
        &self.value_input_state
    }

    /// Validates the focused row and commits trimmed key text.
    ///
    /// # Returns
    ///
    /// A success message when the key is present, or an error message when missing.
    pub fn validate_focused_row(&mut self) -> Result<String> {
        let Some(row_index) = self.table_state.selected() else {
            return Ok(String::new());
        };
        self.validate_row(row_index)
    }

    /// Validates the row and commits trimmed key text.
    ///
    /// # Returns
    ///
    /// A success message when the key is present, or an error message when missing.
    pub fn validate_row(&self, row_index: usize) -> Result<String> {
        // do not validate a focused row since it may
        // still be actively editing
        if self
            .table_state
            .selected()
            .is_some_and(|idx| idx == row_index && self.f_key_field.get())
        {
            return Ok(String::new());
        }
        let maybe_trimmed_key = self.rows.get(row_index).map(|row| row.key.trim().to_string());
        if let Some(trimmed_key) = maybe_trimmed_key
            && trimmed_key.is_empty()
        {
            return Err(anyhow!("✘ Key name missing"));
        }
        Ok("✓ Looks good!".to_string())
    }

    /// Return a Vec of valid EnvRow. A valid EnvRow is one that has a non-empty key.
    pub fn valid_rows(&self) -> Vec<EnvRow> {
        self.rows.iter().filter(|row| !row.key.trim().is_empty()).cloned().collect()
    }

    pub fn is_selected_row_empty(&self) -> bool {
        let Some(row_index) = self.table_state.selected() else {
            return true;
        };
        self.rows.get(row_index).is_none_or(EnvRow::is_empty)
    }

    fn ensure_input_states_for_selected_row(&mut self) {
        if self.table_state.selected().is_none() {
            self.clear_input_states();
            return;
        };
        self.load_input_states_for_selected_row();
    }

    fn load_input_states_for_selected_row(&mut self) {
        if let Some(idx) = self.table_state.selected()
            && let Some(row) = self.rows.get(idx)
        {
            self.key_input_state.set_input(row.key.clone());
            self.value_input_state.set_input(row.value.clone());
        };

        self.reset_cursor_for_key_field();
        self.reset_cursor_for_value_field();
    }

    fn clear_input_states(&mut self) {
        self.key_input_state.clear();
        self.value_input_state.clear();
    }

    fn reset_cursor_for_key_field(&mut self) {
        let cursor = self.key_input_state.input().len();
        self.key_input_state.set_cursor(cursor);
    }

    fn reset_cursor_for_value_field(&mut self) {
        let cursor = self.value_input_state.input().len();
        self.value_input_state.set_cursor(cursor);
    }
}

impl HasFocus for KeyValueEditorState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.leaf_widget(&self.f_add_button);
        builder.leaf_widget(&self.f_remove_button);
        builder.leaf_widget(&self.f_show_secrets_button);
        builder.leaf_widget(&self.f_table);
        if !self.rows.is_empty() || !self.f_table.get() {
            builder.leaf_widget(&self.f_key_field);
            builder.leaf_widget(&self.f_value_field);
        }

        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_focused_row_rejects_empty_key() {
        let mut state = KeyValueEditorState::default();
        state.rows.push(EnvRow {
            key: String::new(),
            value: String::new(),
            is_secret: false,
        });
        state.select_first_row();
        state.prepare_key_field_for_input();

        let validation = state.validate_focused_row();

        assert!(validation.is_err());
    }
}
