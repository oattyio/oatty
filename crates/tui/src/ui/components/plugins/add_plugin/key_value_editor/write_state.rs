use crate::ui::components::plugins::{EnvRow, KeyValueEditorField};

/// Editing state for a key/value row when in edit mode.
#[derive(Debug, Clone)]
pub struct KeyValueEditorEditState {
    /// Index of the row being edited.
    pub row_index: usize,
    /// Currently active field (key or value).
    pub active_field: KeyValueEditorField,
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
            active_field: KeyValueEditorField::Key,
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
            active_field: KeyValueEditorField::Key,
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
