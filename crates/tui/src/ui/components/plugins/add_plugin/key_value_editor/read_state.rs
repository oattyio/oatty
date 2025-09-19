use crate::ui::components::plugins::{EnvRow, KeyValueEditorField};

use super::{KeyValueEditorCommitError, KeyValueEditorEditState, KeyValueEditorMode};
/// State container for table-driven editing of key/value pairs.
#[derive(Debug, Clone)]
pub struct KeyValueEditorState {
    /// Persisted rows backing the table.
    pub rows: Vec<EnvRow>,
    /// Currently selected row when browsing.
    pub selected_row_index: Option<usize>,
    /// Interaction mode for the editor.
    pub mode: KeyValueEditorMode,
}

impl KeyValueEditorState {
    /// Constructs an empty editor.
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            selected_row_index: None,
            mode: KeyValueEditorMode::Browsing,
        }
    }

    /// Returns `true` when the editor is in edit mode.
    pub fn is_editing(&self) -> bool {
        matches!(self.mode, KeyValueEditorMode::Editing(_))
    }

    /// Provides the currently edited buffers and active field.
    pub fn editing_buffers(&self) -> Option<(&str, &str, KeyValueEditorField)> {
        match &self.mode {
            KeyValueEditorMode::Browsing => None,
            KeyValueEditorMode::Editing(edit) => Some((&edit.key_buffer, &edit.value_buffer, edit.active_field)),
        }
    }

    /// Returns the index of the row currently being edited, if any.
    pub fn editing_row_index(&self) -> Option<usize> {
        match &self.mode {
            KeyValueEditorMode::Browsing => None,
            KeyValueEditorMode::Editing(edit) => Some(edit.row_index),
        }
    }

    /// Selects the previous row when browsing.
    pub fn select_previous(&mut self) {
        if self.rows.is_empty() {
            self.selected_row_index = None;
            return;
        }
        let current = self.selected_row_index.unwrap_or(0);
        if current == 0 {
            self.selected_row_index = Some(0);
        } else {
            self.selected_row_index = Some(current - 1);
        }
    }

    /// Selects the next row when browsing.
    pub fn select_next(&mut self) {
        if self.rows.is_empty() {
            self.selected_row_index = None;
            return;
        }
        let last_index = self.rows.len() - 1;
        let current = self.selected_row_index.unwrap_or(0);
        if current >= last_index {
            self.selected_row_index = Some(last_index);
        } else {
            self.selected_row_index = Some(current + 1);
        }
    }

    /// Ensures there is an active selection when rows exist.
    pub fn ensure_selection(&mut self) {
        if self.rows.is_empty() {
            self.selected_row_index = None;
        } else if self.selected_row_index.is_none() {
            self.selected_row_index = Some(0);
        }
    }

    /// Begins editing the currently selected row or creates a new row if none exist.
    pub fn begin_editing_selected(&mut self) {
        if let Some(index) = self.selected_row_index {
            self.start_editing_existing(index);
        } else {
            self.begin_editing_new_row();
        }
    }

    /// Begins editing a new row appended to the end of the table.
    pub fn begin_editing_new_row(&mut self) {
        let row_index = self.rows.len();
        self.rows.push(EnvRow {
            key: String::new(),
            value: String::new(),
            is_secret: false,
        });
        self.mode = KeyValueEditorMode::Editing(KeyValueEditorEditState::new_row(row_index));
        self.selected_row_index = Some(row_index);
    }

    /// Deletes the currently selected row, returning `true` when a row was removed.
    pub fn delete_selected(&mut self) -> bool {
        if self.rows.is_empty() {
            self.selected_row_index = None;
            return false;
        }
        if let Some(index) = self.selected_row_index {
            if index < self.rows.len() {
                self.rows.remove(index);
                if self.rows.is_empty() {
                    self.selected_row_index = None;
                } else if index >= self.rows.len() {
                    self.selected_row_index = Some(self.rows.len() - 1);
                }
                return true;
            }
        }
        false
    }

    /// Cancels the current edit session, discarding uncommitted changes.
    pub fn cancel_edit(&mut self) {
        if let KeyValueEditorMode::Editing(edit) = &self.mode {
            if edit.is_new_row && edit.row_index < self.rows.len() {
                self.rows.remove(edit.row_index);
                if self.rows.is_empty() {
                    self.selected_row_index = None;
                } else if edit.row_index >= self.rows.len() {
                    self.selected_row_index = Some(self.rows.len() - 1);
                }
            } else if edit.row_index < self.rows.len() {
                self.rows[edit.row_index] = edit.original_row.clone();
                self.selected_row_index = Some(edit.row_index);
            }
        }
        self.mode = KeyValueEditorMode::Browsing;
    }

    /// Commits the current edit session into the underlying row storage.
    pub fn commit_edit(&mut self) -> Result<(), KeyValueEditorCommitError> {
        let KeyValueEditorMode::Editing(edit) = &self.mode else {
            return Ok(());
        };
        if edit.key_buffer.trim().is_empty() {
            return Err(KeyValueEditorCommitError::EmptyKey);
        }
        if edit.row_index >= self.rows.len() {
            return Ok(());
        }
        let target = &mut self.rows[edit.row_index];
        target.key = edit.key_buffer.trim().to_string();
        target.value = edit.value_buffer.clone();
        self.selected_row_index = Some(edit.row_index);
        self.mode = KeyValueEditorMode::Browsing;
        Ok(())
    }

    /// Toggles the active editing field between key and value.
    pub fn toggle_field(&mut self) {
        if let KeyValueEditorMode::Editing(edit) = &mut self.mode {
            edit.active_field = match edit.active_field {
                KeyValueEditorField::Key => KeyValueEditorField::Value,
                KeyValueEditorField::Value => KeyValueEditorField::Key,
            };
        }
    }

    /// Sets the active editing field explicitly.
    pub fn set_active_field(&mut self, field: KeyValueEditorField) {
        if let KeyValueEditorMode::Editing(edit) = &mut self.mode {
            edit.active_field = field;
        }
    }

    /// Appends a character to the active buffer while editing.
    pub fn push_character(&mut self, character: char) {
        if let KeyValueEditorMode::Editing(edit) = &mut self.mode {
            match edit.active_field {
                KeyValueEditorField::Key => edit.key_buffer.push(character),
                KeyValueEditorField::Value => edit.value_buffer.push(character),
            }
        }
    }

    /// Removes the last character from the active buffer while editing.
    pub fn pop_character(&mut self) {
        if let KeyValueEditorMode::Editing(edit) = &mut self.mode {
            match edit.active_field {
                KeyValueEditorField::Key => {
                    edit.key_buffer.pop();
                }
                KeyValueEditorField::Value => {
                    edit.value_buffer.pop();
                }
            }
        }
    }

    /// Provides an immutable view of the selected row, if any.
    pub fn selected_row(&self) -> Option<&EnvRow> {
        self.selected_row_index.and_then(|index| self.rows.get(index))
    }

    /// Provides a mutable view of the selected row, if any.
    pub fn selected_row_mut(&mut self) -> Option<&mut EnvRow> {
        let index = self.selected_row_index?;
        self.rows.get_mut(index)
    }

    fn start_editing_existing(&mut self, index: usize) {
        if index >= self.rows.len() {
            return;
        }
        let snapshot = self.rows[index].clone();
        self.mode = KeyValueEditorMode::Editing(KeyValueEditorEditState::from_existing(index, &snapshot));
        if let KeyValueEditorMode::Editing(edit) = &mut self.mode {
            edit.original_row = snapshot;
        }
    }
}
