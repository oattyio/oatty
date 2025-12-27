//! Reusable UTF-8 safe text input state with cursor management.
//!
//! This module extracts the cursor/text editing primitives used by the
//! command palette so they can be reused by other components (e.g.,
//! workflow inputs), without bringing along suggestion/history logic.

#[derive(Clone, Debug, Default)]
pub struct TextInputState {
    /// The underlying text buffer
    input: String,
    /// Cursor byte index into `input` (always on a UTF-8 boundary)
    cursor: usize,
}

impl TextInputState {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            cursor: 0,
        }
    }

    // ----- Getters -----
    pub fn input(&self) -> &str {
        &self.input
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn clear(&mut self) {
        self.input.clear();
        self.cursor = 0;
    }
    pub fn is_empty(&self) -> bool {
        self.input.trim().is_empty()
    }
    /// Returns the cursor position in display columns (character count).
    pub fn cursor_columns(&self) -> usize {
        let cursor_index = self.cursor.min(self.input.len());
        self.input[..cursor_index].chars().count()
    }
    /// Returns the byte index for a cursor placed at the provided display column.
    ///
    /// The `column` value is expected to be relative to the start of the input text.
    pub fn cursor_index_for_column(&self, column: u16) -> usize {
        cursor_index_for_column(self.input(), column)
    }

    // ----- Setters -----
    pub fn set_input<S: Into<String>>(&mut self, s: S) {
        self.input = s.into();
        self.cursor = self.input.len().min(self.cursor);
    }

    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursor = cursor.min(self.input.len());
    }

    // ----- Editing primitives (UTF-8 safe) -----

    /// Move cursor one Unicode scalar to the left.
    pub fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let prev_len = self.input[..self.cursor].chars().last().map(|c| c.len_utf8()).unwrap_or(1);
        self.cursor = self.cursor.saturating_sub(prev_len);
    }

    /// Move cursor one Unicode scalar to the right.
    pub fn move_right(&mut self) {
        if self.cursor >= self.input.len() {
            return;
        }
        let mut iter = self.input[self.cursor..].chars();
        if let Some(next) = iter.next() {
            self.cursor = self.cursor.saturating_add(next.len_utf8());
        }
    }

    /// Insert a char at the cursor.
    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    /// Backspace the char immediately before the cursor.
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let prev = self.input[..self.cursor].chars().last().map(|c| c.len_utf8()).unwrap_or(1);
        let start = self.cursor - prev;
        self.input.drain(start..self.cursor);
        self.cursor = start;
    }

    pub fn delete(&mut self) {
        if self.cursor == self.input.len() {
            return;
        }
        let next = self.input[self.cursor..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        let end = self.cursor + next;
        self.input.drain(self.cursor..end);
    }
}

/// Convert a display column into a byte index for the provided text.
///
/// The `column` value should be relative to the start of the string (excluding any prefixes).
pub fn cursor_index_for_column(text: &str, column: u16) -> usize {
    let mut columns = 0u16;
    for (byte_index, _character) in text.char_indices() {
        if columns >= column {
            return byte_index;
        }
        columns = columns.saturating_add(1);
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_move_insert_backspace() {
        let mut st = TextInputState::new();
        st.set_input("h🙂llo"); // emoji is 4 bytes
        st.set_cursor(1); // between h and 🙂
        st.insert_char('e');
        assert_eq!(st.input(), "he🙂llo");
        st.move_right(); // step over 🙂
        st.backspace(); // delete 🙂
        assert_eq!(st.input(), "hello");
        st.move_left();
        st.backspace();
        assert_eq!(st.input(), "ello");
    }
}
