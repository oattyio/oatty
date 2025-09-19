//! Key/value editor state for the plugin add form.
//!
//! This module defines reusable state management helpers for tabular editing of
//! key/value pairs in the Add Plugin flow. Environment variables for local
//! transports and HTTP headers for remote transports share the same editing
//! semantics, so the UI code can delegate navigation and mutation logic to the
//! types defined here.

use std::fmt;

use super::KeyValueEditorEditState;

/// Errors that can occur while committing key/value edits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyValueEditorCommitError {
    /// The edited row is missing a key.
    EmptyKey,
}

impl fmt::Display for KeyValueEditorCommitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyValueEditorCommitError::EmptyKey => {
                write!(f, "Key is required")
            }
        }
    }
}

/// Represents the interaction mode of the key/value editor.
#[derive(Debug, Clone)]
pub enum KeyValueEditorMode {
    /// No row is being edited; arrow keys move the selection.
    Browsing,
    /// An existing or newly created row is being edited.
    Editing(KeyValueEditorEditState),
}

impl Default for KeyValueEditorMode {
    fn default() -> Self {
        KeyValueEditorMode::Browsing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_editor_is_empty() {
        let editor = KeyValueEditorState::new();
        assert!(editor.rows.is_empty());
        assert!(editor.selected_row_index.is_none());
        assert!(!editor.is_editing());
    }

    #[test]
    fn begin_editing_new_row_creates_row() {
        let mut editor = KeyValueEditorState::new();
        editor.begin_editing_new_row();
        assert_eq!(editor.rows.len(), 1);
        assert!(editor.is_editing());
        assert_eq!(editor.selected_row_index, Some(0));
        assert_eq!(editor.editing_row_index(), Some(0));
    }

    #[test]
    fn commit_edit_updates_row() {
        let mut editor = KeyValueEditorState::new();
        editor.begin_editing_new_row();
        editor.push_character('A');
        editor.toggle_field();
        editor.push_character('B');
        editor.commit_edit().expect("commit should succeed");
        let row = editor.rows.first().expect("row present");
        assert_eq!(row.key, "A");
        assert_eq!(row.value, "B");
        assert!(!editor.is_editing());
    }

    #[test]
    fn cancel_edit_removes_new_row() {
        let mut editor = KeyValueEditorState::new();
        editor.begin_editing_new_row();
        editor.cancel_edit();
        assert!(editor.rows.is_empty());
        assert!(editor.selected_row_index.is_none());
        assert!(!editor.is_editing());
    }

    #[test]
    fn delete_selected_row_compacts_selection() {
        let mut editor = KeyValueEditorState::new();
        editor.begin_editing_new_row();
        editor.push_character('A');
        editor.commit_edit().expect("commit should succeed");
        editor.begin_editing_new_row();
        editor.push_character('B');
        editor.commit_edit().expect("commit should succeed");
        editor.ensure_selection();
        editor.select_previous();
        assert_eq!(editor.selected_row_index, Some(0));
        assert!(editor.delete_selected());
        assert_eq!(editor.rows.len(), 1);
        assert_eq!(editor.selected_row_index, Some(0));
    }
}
