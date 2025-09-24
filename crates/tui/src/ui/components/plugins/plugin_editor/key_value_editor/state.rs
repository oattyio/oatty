use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;

use crate::ui::components::plugins::EnvRow;

/// Represents the interaction mode of the key/value editor.
#[derive(Debug, Clone, Default)]
pub enum KeyValueEditorMode {
    /// No row is being edited; arrow keys move the selection.
    #[default]
    Browsing,
    /// An existing or newly created row is being edited.
    Editing(KeyValueEditorEditState),
}

/// State container for table-driven editing of key/value pairs.
#[derive(Debug, Clone)]
pub struct KeyValueEditorState {
    /// Persisted rows backing the table.
    pub rows: Vec<EnvRow>,
    /// Currently selected row when browsing.
    pub selected_row_index: Option<usize>,
    /// Interaction mode for the editor.
    pub mode: KeyValueEditorMode,
    /// Container focus flag used when integrating with the global focus ring.
    focus: FocusFlag,
    /// Focus flag associated with the table surface while browsing rows.
    table_focus: FocusFlag,
    /// Focus flag associated with the inline key editor when editing a row.
    key_field_focus: FocusFlag,
    /// Focus flag associated with the inline value editor when editing a row.
    value_field_focus: FocusFlag,
}

impl KeyValueEditorState {
    /// Constructs an empty editor with a deterministic focus namespace.
    ///
    /// The `namespace` argument is used to derive unique names for the
    /// container, table, key field, and value field focus flags. Callers should
    /// pass a stable namespace to avoid rebuilding focus structures unnecessarily.
    pub fn new(namespace: &str) -> Self {
        let focus = FocusFlag::named(&format!("{namespace}.container"));
        let table_focus = FocusFlag::named(&format!("{namespace}.table"));
        let key_field_focus = FocusFlag::named(&format!("{namespace}.field.key"));
        let value_field_focus = FocusFlag::named(&format!("{namespace}.field.value"));
        let mut state = Self {
            rows: Vec::new(),
            selected_row_index: None,
            mode: KeyValueEditorMode::Browsing,
            focus,
            table_focus,
            key_field_focus,
            value_field_focus,
        };
        state.focus_table();
        state
    }

    /// Returns `true` when the inline editor is active.
    pub fn is_editing(&self) -> bool {
        matches!(self.mode, KeyValueEditorMode::Editing(_))
    }

    /// Provides the currently edited buffers if the editor is in edit mode.
    pub fn editing_buffers(&self) -> Option<(&str, &str)> {
        match &self.mode {
            KeyValueEditorMode::Browsing => None,
            KeyValueEditorMode::Editing(edit) => Some((&edit.key_buffer, &edit.value_buffer)),
        }
    }

    /// Returns the index of the row currently being edited, if any.
    pub fn editing_row_index(&self) -> Option<usize> {
        match &self.mode {
            KeyValueEditorMode::Browsing => None,
            KeyValueEditorMode::Editing(edit) => Some(edit.row_index),
        }
    }

    /// Indicates whether the key buffer currently owns the editing focus.
    pub fn is_key_field_focused(&self) -> bool {
        self.key_field_focus.get()
    }

    /// Indicates whether the value buffer currently owns the editing focus.
    pub fn is_value_field_focused(&self) -> bool {
        self.value_field_focus.get()
    }

    /// Returns a human-readable label for the active inline editor field.
    pub fn active_field_label(&self) -> &'static str {
        if self.is_value_field_focused() { "value" } else { "key" }
    }

    /// Exposes the container focus flag for integration with other state.
    pub fn focus_flag(&self) -> FocusFlag {
        self.focus.clone()
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
        self.focus_key_field();
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
                self.focus_table();
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
        self.focus_table();
    }

    /// Commits the current edit session into the underlying row storage.
    pub fn commit_edit(&mut self) -> Result<String, String> {
        let ok = Ok("✓ Looks good!".to_string());
        let KeyValueEditorMode::Editing(edit) = &self.mode else {
            return ok;
        };
        if edit.key_buffer.trim().is_empty() {
            return Err("✘ Key name missing".to_string());
        }
        if edit.row_index >= self.rows.len() {
            return ok;
        }
        let target = &mut self.rows[edit.row_index];
        target.key = edit.key_buffer.trim().to_string();
        target.value = edit.value_buffer.clone();
        self.selected_row_index = Some(edit.row_index);
        self.mode = KeyValueEditorMode::Browsing;
        self.focus_table();

        ok
    }

    /// Toggles the active editing field between key and value.
    pub fn toggle_field(&mut self) {
        if !self.is_editing() {
            return;
        }
        if self.is_value_field_focused() {
            self.focus_key_field();
        } else {
            self.focus_value_field();
        }
    }

    /// Focuses the key buffer when editing an inline row.
    pub fn focus_key_input(&mut self) {
        if self.is_editing() {
            self.focus_key_field();
        }
    }

    /// Focuses the value buffer when editing an inline row.
    pub fn focus_value_input(&mut self) {
        if self.is_editing() {
            self.focus_value_field();
        }
    }

    /// Appends a character to the active buffer while editing.
    pub fn push_character(&mut self, character: char) {
        if !self.is_editing() {
            return;
        }
        let value_field_active = self.is_value_field_focused();
        if let KeyValueEditorMode::Editing(edit) = &mut self.mode {
            if value_field_active {
                edit.value_buffer.push(character);
            } else {
                edit.key_buffer.push(character);
            }
        }
    }

    /// Removes the last character from the active buffer while editing.
    pub fn pop_character(&mut self) {
        if !self.is_editing() {
            return;
        }
        let value_field_active = self.is_value_field_focused();
        if let KeyValueEditorMode::Editing(edit) = &mut self.mode {
            if value_field_active {
                edit.value_buffer.pop();
            } else {
                edit.key_buffer.pop();
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

    fn focus_table(&mut self) {
        self.table_focus.set(true);
        self.key_field_focus.set(false);
        self.value_field_focus.set(false);
    }

    fn focus_key_field(&mut self) {
        self.table_focus.set(false);
        self.key_field_focus.set(true);
        self.value_field_focus.set(false);
    }

    fn focus_value_field(&mut self) {
        self.table_focus.set(false);
        self.key_field_focus.set(false);
        self.value_field_focus.set(true);
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
        self.focus_key_field();
    }
}

impl HasFocus for KeyValueEditorState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.leaf_widget(&self.table_focus);
        if self.is_editing() {
            builder.leaf_widget(&self.key_field_focus);
            builder.leaf_widget(&self.value_field_focus);
        }
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}

/// Editing state for a key/value row when in edit mode.
#[derive(Debug, Clone)]
pub struct KeyValueEditorEditState {
    /// Index of the row being edited.
    pub row_index: usize,
    /// Working buffer for the key column.
    pub key_buffer: String,
    /// Working buffer for the value column.
    pub value_buffer: String,
    /// Original row snapshot for cancellation or change detection.
    pub original_row: EnvRow,
    /// Indicates whether the editor created a new row for this edit session.
    pub is_new_row: bool,
}

impl KeyValueEditorEditState {
    /// Creates an edit state from an existing row.
    pub(super) fn from_existing(row_index: usize, row: &EnvRow) -> Self {
        Self {
            row_index,
            key_buffer: row.key.clone(),
            value_buffer: row.value.clone(),
            original_row: row.clone(),
            is_new_row: false,
        }
    }

    /// Creates an edit state for a newly inserted row.
    pub(super) fn new_row(row_index: usize) -> Self {
        Self {
            row_index,
            key_buffer: String::new(),
            value_buffer: String::new(),
            original_row: EnvRow {
                key: String::new(),
                value: String::new(),
                is_secret: false,
            },
            is_new_row: true,
        }
    }
}
