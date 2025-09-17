//! Palette input, parsing, and suggestions for the TUI command line.
//!
//! This module renders the input palette, parses the current line/cursor into
//! a simple state, and produces contextual suggestions (commands, flags,
//! positionals, and values). It follows a linear command structure inspired by
//! the Node REPL implementation:
//!   <group> <sub> <required positionals> <optional positionals>
//!   <required flags> <optional flags>
//!
//! Key behaviors:
//! - Positionals are suggested before flags unless the user explicitly starts a
//!   flag token ("-"/"--").
//! - For non-boolean flags whose value is pending, only values are suggested
//!   (enums and provider values) until the value is complete.
//! - Suggestions never render an empty popup.
use std::{fmt::Debug, sync::Arc};

use super::suggest::{parse_user_flags_args, required_flags_remaining};
use heroku_registry::Registry;
use heroku_types::{CommandSpec, ItemKind, SuggestionItem};
use heroku_util::{fuzzy_score, lex_shell_like, lex_shell_like_ranged};
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;
/// Maximum number of suggestions to display in the popup.
const MAX_SUGGESTIONS: usize = 20;

/// Locate the index of the token under the cursor.
///
/// Uses the shell-like ranged lexer to find the token whose byte range contains
/// `cursor`. If no token contains the cursor, returns the last token index
/// (useful when the cursor is at end-of-line). Returns `None` if there are no
/// tokens.
fn token_index_at_cursor(input: &str, cursor: usize) -> Option<usize> {
    let tokens = lex_shell_like_ranged(input);
    if tokens.is_empty() {
        return None;
    }
    if let Some((idx, _)) = tokens
        .iter()
        .enumerate()
        .find(|(_, t)| t.start <= cursor && cursor <= t.end)
    {
        Some(idx)
    } else {
        Some(tokens.len() - 1)
    }
}

// ItemKind and SuggestionItem moved to types.rs and re-exported via mod.rs

/// State for the command palette input and suggestions.
///
/// This struct manages the current state of the command palette including
/// input text, cursor position, suggestions, and error states.
#[derive(Clone, Debug)]
pub struct PaletteState {
    /// Focus flag for self
    focus: FocusFlag,
    /// Focus flag for the input field
    f_input: FocusFlag,
    /// Every command available
    all_commands: Arc<[CommandSpec]>,
    /// The current input text
    input: String,
    /// Current cursor position (byte index)
    cursor_position: usize,
    /// Optional ghost text to show as placeholder
    ghost_text: Option<String>,
    /// Whether the suggestions popup is currently open
    is_suggestions_open: bool,
    /// Index of the currently selected suggestion
    suggestion_index: usize,
    /// List of current suggestions
    suggestions: Vec<SuggestionItem>,
    /// Pre-rendered suggestion list items for efficient display
    rendered_suggestions: Vec<ratatui::widgets::ListItem<'static>>,
    /// Optional error message to display
    error_message: Option<String>,

    /// Whether provider-backed suggestions are actively loading
    provider_loading: bool,

    /// History of executed palette inputs (most recent last)
    history: Vec<String>,
    /// Current index into history when browsing (0..history.len()-1), None when not browsing
    history_index: Option<usize>,
    /// Draft input captured when entering history browse mode, restored when exiting
    draft_input: Option<String>,
}

impl Default for PaletteState {
    fn default() -> Self {
        Self {
            focus: FocusFlag::named("heroku.palette"),
            f_input: FocusFlag::named("heroku.palette.input"),
            all_commands: Arc::from([]),
            input: String::new(),
            cursor_position: 0,
            ghost_text: None,
            is_suggestions_open: false,
            suggestion_index: 0,
            suggestions: Vec::new(),
            rendered_suggestions: Vec::new(),
            error_message: None,
            provider_loading: false,
            history: Vec::new(),
            history_index: None,
            draft_input: None,
        }
    }
}

impl HasFocus for PaletteState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.leaf_widget(&self.f_input);
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}

impl PaletteState {
    // ===== SELECTORS =====

    /// Check if the palette input is empty (ignoring whitespace)
    pub fn is_input_empty(&self) -> bool {
        self.input.trim().is_empty()
    }

    // Note: prefer is_suggestions_open() over redundant aliases
    /// Check if there's an error message to display
    pub fn has_error(&self) -> bool {
        self.error_message.is_some()
    }

    /// Get the count of current suggestions
    pub fn suggestions_len(&self) -> usize {
        self.suggestions.len()
    }

    /// Get the currently selected suggestion index
    pub fn suggestion_index(&self) -> usize {
        self.suggestion_index
    }

    /// Get the currently selected suggestion item
    pub fn selected_suggestion(&self) -> Option<&SuggestionItem> {
        self.suggestions.get(self.suggestion_index)
    }

    // selected_command(): removed to avoid redundant persisted state

    /// Derive the command spec from the current input tokens ("group sub").
    pub fn selected_command_from_input(&self) -> Option<&CommandSpec> {
        let tokens: Vec<String> = lex_shell_like(&self.input);
        if tokens.len() < 2 {
            return None;
        }
        let group = &tokens[0];
        let name = &tokens[1];
        self.all_commands.iter().find(|c| &c.group == group && &c.name == name)
    }

    /// Derive the command spec from the currently selected suggestion when it
    /// is a command.
    pub fn selected_command_from_suggestion(&self) -> Option<&CommandSpec> {
        let item = self.selected_suggestion()?;
        if !matches!(item.kind, ItemKind::Command) {
            return None;
        }
        let mut parts = item.insert_text.split_whitespace();
        let group = parts.next().unwrap_or("");
        let name = parts.next().unwrap_or("");
        self.all_commands.iter().find(|c| c.group == group && c.name == name)
    }

    /// Selected command for help: prefer highlighted suggestion if open, else
    /// parse input.
    pub fn selected_command(&self) -> Option<&CommandSpec> {
        let selected_command = self.selected_command_from_suggestion();
        if self.is_suggestions_open && selected_command.is_some() {
            return selected_command;
        }
        self.selected_command_from_input()
    }

    /// Check if the cursor is at the end of input
    pub fn is_cursor_at_end(&self) -> bool {
        self.cursor_position >= self.input.len()
    }

    /// Check if the cursor is at the beginning of input
    pub fn is_cursor_at_start(&self) -> bool {
        self.cursor_position == 0
    }

    /// Get the current input text
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Get the current cursor position
    pub fn selected_cursor_position(&self) -> usize {
        self.cursor_position
    }

    /// Get the current ghost text
    pub fn ghost_text(&self) -> Option<&String> {
        self.ghost_text.as_ref()
    }

    /// Get the current error message
    pub fn error_message(&self) -> Option<&String> {
        self.error_message.as_ref()
    }

    /// Whether provider-backed suggestions are currently loading
    pub fn is_provider_loading(&self) -> bool {
        self.provider_loading
    }

    /// Get the current suggestions list
    pub fn suggestions(&self) -> &[SuggestionItem] {
        &self.suggestions
    }

    /// Get the current popup open state
    pub fn is_suggestions_open(&self) -> bool {
        self.is_suggestions_open
    }

    /// Get the current rendered suggestions list
    pub fn rendered_suggestions(&self) -> &[ratatui::widgets::ListItem<'static>] {
        &self.rendered_suggestions
    }

    // ===== REDUCERS =====

    /// Clear all palette state and reset to defaults
    pub fn reduce_clear_all(&mut self) {
        self.input.clear();
        self.cursor_position = 0;
        self.suggestions.clear();
        self.rendered_suggestions.clear();
        self.is_suggestions_open = false;
        self.error_message = None;
        self.ghost_text = None;
        self.suggestion_index = 0;
        // Preserve history; exit browsing mode
        self.history_index = None;
        self.draft_input = None;
    }

    /// Apply an error message to the palette
    pub fn apply_error(&mut self, error: String) {
        self.error_message = Some(error);
    }

    /// Clear any existing error message
    pub fn reduce_clear_error(&mut self) {
        self.error_message = None;
    }

    /// Toggle the suggestions popup visibility
    pub fn toggle_popup(&mut self) {
        self.is_suggestions_open = !self.is_suggestions_open;
    }

    /// Select the next suggestion in the list
    pub fn select_next_suggestion(&mut self) {
        if !self.suggestions.is_empty() {
            self.suggestion_index = (self.suggestion_index + 1) % self.suggestions.len();
            self.apply_ghost_text();
        }
    }

    /// Select the previous suggestion in the list
    pub fn select_prev_suggestion(&mut self) {
        if !self.suggestions.is_empty() {
            self.suggestion_index = if self.suggestion_index == 0 {
                self.suggestions.len() - 1
            } else {
                self.suggestion_index - 1
            };
            self.apply_ghost_text();
        }
    }

    /// Select a specific suggestion by index
    pub fn select_suggestion(&mut self, index: usize) {
        if index < self.suggestions.len() {
            self.suggestion_index = index;
            self.apply_ghost_text();
        }
    }

    /// Apply suggestions and update popup state
    pub fn apply_suggestions(&mut self, suggestions: Vec<SuggestionItem>) {
        self.suggestions = suggestions;
        self.rendered_suggestions.clear();
        self.suggestion_index = self.suggestion_index.min(self.suggestions.len().saturating_sub(1));
        self.is_suggestions_open = !self.suggestions.is_empty();
        self.apply_ghost_text();
    }

    /// Clear all suggestions and close popup
    pub fn reduce_clear_suggestions(&mut self) {
        self.suggestions.clear();
        self.rendered_suggestions.clear();
        self.is_suggestions_open = false;
        self.suggestion_index = 0;
        self.ghost_text = None;
        self.provider_loading = false;
    }

    // ===== PRIVATE SETTERS =====

    pub(crate) fn set_all_commands(&mut self, commands: Arc<[CommandSpec]>) {
        self.all_commands = commands;
    }

    /// Set the input text
    pub(crate) fn set_input(&mut self, input: String) {
        self.input = input;
    }

    /// Set the cursor position
    pub(crate) fn set_cursor(&mut self, cursor: usize) {
        self.cursor_position = cursor;
    }

    /// Set the popup open state
    pub(crate) fn set_is_suggestions_open(&mut self, open: bool) {
        self.is_suggestions_open = open;
    }

    /// Set the selected suggestion index
    pub(crate) fn set_selected(&mut self, selected: usize) {
        self.suggestion_index = selected;
        self.apply_ghost_text();
    }

    /// Insert text at the end of the input with a separating space and advance
    /// the cursor.
    ///
    /// Appends a space before `text` if the current input is non-empty and does
    /// not already end with a space, then appends `text` and a trailing
    /// space. The cursor is moved to the end of the input.
    ///
    /// This helper centralizes the common pattern used when accepting
    /// suggestions so individual handlers remain focused on their control
    /// flow rather than spacing.
    fn insert_with_space(&mut self, text: &str) {
        if !self.input.ends_with(' ') && !self.input.is_empty() {
            self.input.push(' ');
        }
        self.input.push_str(text);
        self.input.push(' ');
        self.cursor_position = self.input.len();
    }

    /// Move the cursor one character to the left.
    ///
    /// This method handles UTF-8 character boundaries correctly,
    /// ensuring the cursor moves by one Unicode character rather than
    /// one byte.
    ///
    /// - No-op if the cursor is already at the start of the input.
    ///
    /// Returns: nothing; updates `self.cursor` in place.
    pub fn reduce_move_cursor_left(&mut self) {
        if self.cursor_position == 0 {
            return;
        }
        let prev_len = self.input[..self.cursor_position]
            .chars()
            .last()
            .map(|c| c.len_utf8())
            .unwrap_or(1);
        self.cursor_position = self.cursor_position.saturating_sub(prev_len);
    }

    /// Move the cursor one character to the right.
    ///
    /// This method handles UTF-8 character boundaries correctly,
    /// ensuring the cursor moves by one Unicode character rather than
    /// one byte.
    ///
    /// - No-op if the cursor is already at the end of the input.
    ///
    /// Returns: nothing; updates `self.cursor` in place.
    pub fn reduce_move_cursor_right(&mut self) {
        if self.cursor_position >= self.input.len() {
            return;
        }
        // Advance by one Unicode scalar starting at current byte offset
        let mut iter = self.input[self.cursor_position..].chars();
        if let Some(next) = iter.next() {
            self.cursor_position = self.cursor_position.saturating_add(next.len_utf8());
        }
    }

    /// Insert a character at the cursor and advance.
    ///
    /// This method inserts a character at the current cursor position
    /// and advances the cursor by the character's UTF-8 length.
    ///
    /// Arguments:
    /// - `c`: The character to insert. UTF-8 length is respected for cursor
    ///   advance.
    ///
    /// Returns: nothing; mutates `self.input` and `self.cursor`.
    pub fn apply_insert_char(&mut self, c: char) {
        // Editing cancels history browsing
        if self.history_index.is_some() {
            self.history_index = None;
            self.draft_input = None;
        }
        self.input.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
    }

    /// Remove the character immediately before the cursor.
    ///
    /// This method removes the character before the cursor and adjusts
    /// the cursor position accordingly, handling multi-byte UTF-8
    /// characters correctly.
    ///
    /// - No-op if the cursor is at the start of the input.
    /// - Handles multi-byte UTF-8 characters correctly.
    ///
    /// Returns: nothing; mutates `self.input` and `self.cursor`.
    pub fn reduce_backspace(&mut self) {
        // Editing cancels history browsing
        if self.history_index.is_some() {
            self.history_index = None;
            self.draft_input = None;
        }
        if self.cursor_position == 0 {
            return;
        }
        let prev = self.input[..self.cursor_position]
            .chars()
            .last()
            .map(|c| c.len_utf8())
            .unwrap_or(1);
        let start = self.cursor_position - prev;
        self.input.drain(start..self.cursor_position);
        self.cursor_position = start;
    }

    /// Finalize suggestion list for the UI: rank, truncate, ghost text, and
    /// state flags.
    fn finalize_suggestions(&mut self, items: &mut Vec<SuggestionItem>, theme: &dyn crate::ui::theme::Theme) {
        items.sort_by(|a, b| b.score.cmp(&a.score));
        if items.len() > MAX_SUGGESTIONS {
            items.truncate(MAX_SUGGESTIONS);
        }
        self.suggestion_index = self.suggestion_index.min(items.len().saturating_sub(1));
        self.suggestions = items.clone();
        self.is_suggestions_open = !self.suggestions.is_empty();

        let current_token = get_current_token(&self.input, self.cursor_position);
        let needle = current_token.trim();

        self.rendered_suggestions = self
            .suggestions
            .iter()
            .map(|s| {
                let display = s.display.clone();
                if needle.is_empty() {
                    return ListItem::new(Line::from(Span::styled(display, theme.text_primary_style())));
                }

                let mut spans: Vec<Span> = Vec::new();
                let hay = display.as_str();
                let mut i = 0usize;
                let needle_lower = needle.to_ascii_lowercase();
                let hay_lower = hay.to_ascii_lowercase();

                // Find and highlight all matches
                while let Some(pos) = hay_lower[i..].find(&needle_lower) {
                    let start = i + pos;

                    // Add text before the match
                    if start > i {
                        spans.push(Span::styled(hay[i..start].to_string(), theme.text_primary_style()));
                    }

                    // Add highlighted match
                    let end = start + needle.len();
                    spans.push(Span::styled(
                        hay[start..end].to_string(),
                        theme.accent_emphasis_style().add_modifier(Modifier::BOLD),
                    ));

                    i = end;
                    if i >= hay.len() {
                        break;
                    }
                }

                // Add remaining text after last match
                if i < hay.len() {
                    spans.push(Span::styled(hay[i..].to_string(), theme.text_primary_style()));
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        self.apply_ghost_text();
    }

    pub fn apply_ghost_text(&mut self) {
        if !self.is_suggestions_open {
            self.ghost_text = None;
            return;
        }
        self.ghost_text = self
            .suggestions
            .get(self.suggestion_index)
            .map(|top| ghost_remainder(&self.input, self.cursor_position, &top.insert_text));
    }

    // ===== HISTORY =====
    /// Append the given input to history, trimming and deduping adjacent entries.
    pub fn push_history_if_needed(&mut self, s: &str) {
        let value = s.trim();
        if value.is_empty() {
            return;
        }
        if self.history.last().map(|h| h.as_str()) == Some(value) {
            return;
        }
        self.history.push(value.to_string());
        const HISTORY_CAP: usize = 200;
        if self.history.len() > HISTORY_CAP {
            let overflow = self.history.len() - HISTORY_CAP;
            self.history.drain(0..overflow);
        }
    }

    /// Move up in history: enter browsing on first Up.
    pub fn history_up(&mut self) -> bool {
        if self.is_suggestions_open {
            return false;
        }
        if self.history.is_empty() {
            return false;
        }
        match self.history_index {
            None => {
                self.draft_input = Some(self.input.clone());
                let idx = self.history.len() - 1;
                self.history_index = Some(idx);
                self.input = self.history[idx].clone();
                self.cursor_position = self.input.len();
                self.is_suggestions_open = false;
                true
            }
            Some(0) => false,
            Some(i) => {
                let ni = i - 1;
                self.history_index = Some(ni);
                self.input = self.history[ni].clone();
                self.cursor_position = self.input.len();
                self.is_suggestions_open = false;
                true
            }
        }
    }

    /// Move down in history; on past-last, restore draft and exit browsing.
    pub fn history_down(&mut self) -> bool {
        if self.is_suggestions_open {
            return false;
        }
        match self.history_index {
            None => false,
            Some(i) => {
                if i + 1 < self.history.len() {
                    let ni = i + 1;
                    self.history_index = Some(ni);
                    self.input = self.history[ni].clone();
                    self.cursor_position = self.input.len();
                    self.is_suggestions_open = false;
                    true
                } else {
                    if let Some(draft) = self.draft_input.take() {
                        self.input = draft;
                        self.cursor_position = self.input.len();
                    }
                    self.history_index = None;
                    self.is_suggestions_open = false;
                    true
                }
            }
        }
    }

    /// Accept a positional suggestion/value: fill the next positional slot
    /// after "group sub". If the last existing positional is a placeholder
    /// like "<app>", replace it; otherwise append before any flags.
    pub fn apply_accept_positional_suggestion(&mut self, value: &str) {
        let tokens_r = lex_shell_like_ranged(&self.input);
        if tokens_r.len() < 2 {
            // No command yet; just append with proper spacing
            self.insert_with_space(value);
            return;
        }
        // Identify first flag position after command tokens
        let mut first_flag_idx = tokens_r.len();
        for (i, t) in tokens_r.iter().enumerate().skip(2) {
            if t.text.starts_with("--") {
                first_flag_idx = i;
                break;
            }
        }
        // Determine if the cursor is currently within a positional token
        let token_index = token_index_at_cursor(&self.input, self.cursor_position).unwrap_or(tokens_r.len() - 1);
        let editing_positional = token_index >= 2 && token_index < first_flag_idx;
        if editing_positional {
            // Replace the positional token under the cursor with the selected value
            let start = tokens_r[token_index].start;
            let end = tokens_r[token_index].end;
            self.input.replace_range(start..end, value);
            let mut new_cursor = start + value.len();
            // Ensure a space after the replaced token
            if self.input.len() == new_cursor {
                self.input.push(' ');
                new_cursor += 1;
            } else if !self.input[new_cursor..].starts_with(' ') {
                self.input.insert(new_cursor, ' ');
                new_cursor += 1;
            }
            self.cursor_position = new_cursor;
            return;
        }
        // Otherwise, append as the next positional value before any flags
        let tokens: Vec<&str> = self.input.split_whitespace().collect();
        let mut first_flag_idx2 = tokens.len();
        for (i, t) in tokens.iter().enumerate().skip(2) {
            if t.starts_with("--") {
                first_flag_idx2 = i;
                break;
            }
        }
        let mut out: Vec<String> = Vec::new();
        out.push(tokens[0].to_string());
        if tokens.len() > 1 {
            out.push(tokens[1].to_string());
        }
        for t in tokens[2..first_flag_idx2].iter() {
            out.push((*t).to_string());
        }
        out.push(value.to_string());
        for t in tokens.iter().skip(first_flag_idx2) {
            out.push((*t).to_string());
        }
        self.input = out.join(" ") + " ";
        self.cursor_position = self.input.len();
    }

    /// Accept a command suggestion by replacing the input with the execution
    /// form (e.g., "group sub") followed by a trailing space, and moving
    /// the cursor to the end.
    ///
    /// This does not modify popup state or suggestions list; callers remain in
    /// control of those aspects of the interaction.
    ///
    /// Arguments:
    /// - `p`: The palette state to update.
    /// - `exec`: The command execution text (typically "group sub").
    pub fn apply_accept_command_suggestion(&mut self, exec: &str) {
        self.input.clear();
        self.insert_with_space(exec);
    }

    // Renders the palette UI components.
    //
    // This function used to render the complete command palette including the input
    // line, optional ghost text, error messages, and the suggestions popup.
    // Rendering responsibility has been migrated to PaletteComponent::render(),
    // and this comment remains for historical context for future refactors.

    /// Accept a non-command suggestion (flag/value) without clobbering the
    /// resolved command (group sub).
    ///
    /// Rules:
    /// - If cursor is at a new token position (ends with space), insert
    ///   suggestion + trailing space.
    /// - If current token starts with '-' or previous token is a flag expecting
    ///   a value or the current token is a partial flag starter ('-' or '--') → replace token.
    /// - Otherwise (we're on the command tokens or a positional token) → append
    ///   suggestion separated by space.
    pub fn apply_accept_non_command_suggestion(&mut self, text: &str) {
        let at_new_token = self.input.ends_with(' ');
        let tokens = lex_shell_like_ranged(&self.input);

        // New token position or empty input: replace a trailing positional placeholder
        // if present; otherwise insert suggestion. Also clean up stray '-'/'--'.
        if at_new_token || tokens.is_empty() {
            // Precompute cleanup range and optional placeholder range before mutating input
            let remove_from: Option<usize> = tokens
                .last()
                .and_then(|t| (t.text == "-" || t.text == "--").then_some(t.start));
            let placeholder_range: Option<(usize, usize)> = if tokens.len() >= 3 {
                let mut first_flag_idx = tokens.len();
                for (i, t) in tokens.iter().enumerate().skip(2) {
                    if t.text.starts_with("--") {
                        first_flag_idx = i;
                        break;
                    }
                }
                if first_flag_idx > 2 {
                    let last_positional_idx = first_flag_idx - 1;
                    let last_tok = &tokens[last_positional_idx];
                    let is_placeholder = last_tok.text.starts_with('<') && last_tok.text.ends_with('>');
                    if is_placeholder {
                        Some((last_tok.start, last_tok.end))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(start) = remove_from {
                self.input.replace_range(start..self.input.len(), "");
                self.cursor_position = self.input.len();
            }
            if let Some((start, end)) = placeholder_range {
                self.input.replace_range(start..end, text);
                // Ensure exactly one space after replaced token
                let mut new_cursor = start + text.len();
                if self.input.len() == new_cursor {
                    self.input.push(' ');
                    new_cursor += 1;
                } else if !self.input[new_cursor..].starts_with(' ') {
                    self.input.insert(new_cursor, ' ');
                    new_cursor += 1;
                }
                self.cursor_position = new_cursor;
                return;
            }

            self.insert_with_space(text);
            return;
        }

        // Identify the token under the cursor and its predecessor (if any)
        let token_index = token_index_at_cursor(&self.input, self.cursor_position).unwrap_or(tokens.len() - 1);
        let (start, end) = (tokens[token_index].start, tokens[token_index].end);
        let current_token = self.input[start..end].to_string();
        let prev_token: Option<String> = token_index
            .checked_sub(1)
            .map(|i| (tokens[i].start, tokens[i].end))
            .map(|(s, e)| self.input[s..e].to_string());

        let prev_is_flag = prev_token.map(|t| t.starts_with("--")).unwrap_or(false);
        let inserting_is_flag = text.starts_with("--");

        // If previous token is a flag and user picked another flag, append instead of
        // replacing the value.
        if prev_is_flag && !current_token.starts_with('-') && inserting_is_flag {
            self.cursor_position = self.input.len();
            self.insert_with_space(text);
            return;
        }

        // Replace flag token or its value, or replace positional under edit; otherwise append.
        // Replace when current token is a flag OR when editing a positional token.
        let mut replace_range: Option<(usize, usize)> = None;
        if current_token.starts_with("--") || prev_is_flag {
            replace_range = Some((start, end));
        } else {
            // Determine if the token under the cursor is a positional (between command and first flag)
            let mut first_flag_idx = tokens.len();
            for (i, t) in tokens.iter().enumerate().skip(2) {
                if t.text.starts_with("--") {
                    first_flag_idx = i;
                    break;
                }
            }
            if token_index >= 2 && token_index < first_flag_idx {
                replace_range = Some((start, end));
            }
        }

        if let Some((rs, re)) = replace_range {
            self.input.replace_range(rs..re, text);
            self.cursor_position = rs + text.len();
            if !self.input.ends_with(' ') {
                self.input.push(' ');
                self.cursor_position += 1;
            }
        } else {
            self.cursor_position = self.input.len();
            self.insert_with_space(text);
        }
    }

    /// Build suggestions based on input, registry, and value providers.
    ///
    /// Precedence:
    /// 1. Required positionals (unless user started a flag token)
    /// 2. Optional positionals
    /// 3. Required flags
    /// 4. Optional flags
    /// 5. End-of-line hint for starting flags
    ///
    /// If a non-boolean flag value is pending and incomplete, only values
    /// (enums and provider-derived) are suggested for that flag.
    ///
    /// Arguments:
    /// - `st`: Mutable palette state; suggestions and ghost text are written
    ///   here.
    /// - `reg`: Command registry providing command/flag/positional specs.
    /// - `providers`: Value providers consulted for flags and positional
    ///   arguments.
    ///
    /// Returns: nothing; updates `st.suggestions`, `st.ghost`, and related
    /// fields.
    ///
    /// Example:
    ///
    /// ```rust,ignore
    /// use heroku_tui::ui::components::palette::state::PaletteState;
    /// use Registry;
    ///
    /// let mut st = PaletteState::default();
    /// st.set_input("apps info --app ".into());
    /// st.apply_build_suggestions(&Registry::from_embedded_schema().unwrap(), &[]);
    /// assert!(!st.selected_suggestions().is_empty());
    /// ```
    pub fn apply_build_suggestions(
        &mut self,
        reg: &Registry,
        providers: &[Box<dyn ValueProvider>],
        theme: &dyn crate::ui::theme::Theme,
    ) {
        let result = super::suggest::SuggestionEngine::build(reg, providers, &self.input);
        let mut items = result.items;
        self.provider_loading = result.provider_loading;

        // When provider-backed suggestions are still loading and we have nothing to show yet,
        // present a lightweight placeholder so the popup can open immediately.
        if self.provider_loading {
            items.push(SuggestionItem {
                display: "loading more…".to_string(),
                insert_text: String::new(),
                kind: ItemKind::Value,
                meta: Some("loading".to_string()),
                score: i64::MIN, // ensure it sorts to the bottom if mixed
            });
        }

        // Offer end-of-line flag hint if still empty
        if items.is_empty() {
            let tokens: Vec<String> = lex_shell_like(&self.input);
            if tokens.len() >= 2 {
                let group = tokens[0].clone();
                let name = tokens[1].clone();
                if let Some(spec) = reg.commands.iter().find(|c| c.group == group && c.name == name) {
                    let parts: &[String] = if tokens.len() >= 2 { &tokens[2..] } else { &tokens[0..0] };
                    let (user_flags, user_args, _flag_values) = parse_user_flags_args(spec, parts);
                    if let Some(hint) = self.eol_flag_hint(spec, &user_flags) {
                        items.push(hint);
                    } else {
                        // If command is complete (all positionals filled, no required flags), show run hint
                        let positionals_complete = user_args.len() >= spec.positional_args.len();
                        let required_remaining = required_flags_remaining(spec, &user_flags);
                        if positionals_complete && !required_remaining {
                            self.ghost_text = Some(" press Enter to run".to_string());
                        }
                    }
                }
            }
        }

        self.finalize_suggestions(&mut items, theme);
        // Preserve run hint ghost when suggestions are empty
        if self.suggestions.is_empty() && self.ghost_text.is_none() {
            let tokens: Vec<String> = lex_shell_like(&self.input);
            if tokens.len() >= 2 {
                let group = tokens[0].clone();
                let name = tokens[1].clone();
                if let Some(spec) = reg.commands.iter().find(|c| c.group == group && c.name == name) {
                    let parts: &[String] = if tokens.len() >= 2 { &tokens[2..] } else { &tokens[0..0] };
                    let (user_flags, user_args, _flag_values) = parse_user_flags_args(spec, parts);
                    let positionals_complete = user_args.len() >= spec.positional_args.len();
                    let required_remaining = required_flags_remaining(spec, &user_flags);
                    if positionals_complete && !required_remaining {
                        self.ghost_text = Some(" press Enter to run".to_string());
                    }
                }
            }
        }
    }

    /// Suggest an end-of-line hint for starting flags when any remain.
    fn eol_flag_hint(&mut self, spec: &CommandSpec, user_flags: &[String]) -> Option<SuggestionItem> {
        let total_flags = spec.flags.len();
        let used = user_flags.len();
        if used < total_flags {
            let hint = if self.input.ends_with(' ') { "--" } else { " --" };
            return Some(SuggestionItem {
                display: hint.to_string(),
                insert_text: hint.trim().to_string(),
                kind: ItemKind::Flag,
                meta: None,
                score: 0,
            });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_partial_positional_on_accept() {
        let mut st = PaletteState::default();
        st.set_input("apps info hero".into());
        st.set_cursor(st.input().len());
        st.apply_accept_positional_suggestion("heroku-prod");
        assert_eq!(st.input(), "apps info heroku-prod ");
    }

    #[test]
    fn replace_placeholder_positional_on_accept_value() {
        let mut st = PaletteState::default();
        // Placeholder for positional arg
        st.set_input("apps info <app> ".into());
        st.set_cursor(st.input().len());
        // Selecting a provider value (non-flag) should replace placeholder
        st.apply_accept_non_command_suggestion("heroku-prod");
        assert_eq!(st.input(), "apps info heroku-prod ");
    }
}

/// Trait for providing dynamic values for command suggestions.
///
/// This trait allows external systems to provide dynamic values
/// for command parameters, such as app names, region names, etc.
pub trait ValueProvider: Send + Sync + Debug {
    /// Suggests values for the given command and field combination.
    ///
    /// # Arguments
    ///
    /// * `command_key` - The command key (e.g., "apps:info")
    /// * `field` - The field name (e.g., "app")
    /// * `partial` - The partial input to match against
    ///
    /// # Returns
    ///
    /// Vector of suggestion items that match the partial input.
    fn suggest(
        &self,
        command_key: &str,
        field: &str,
        partial: &str,
        inputs: &std::collections::HashMap<String, String>,
    ) -> Vec<SuggestionItem>;
}

/// A simple value provider that returns static values.
///
/// This provider returns a predefined list of values for specific
/// command and field combinations.
#[derive(Debug)]
#[allow(dead_code)]
pub struct StaticValuesProvider {
    /// The command key this provider matches
    pub command_key: String,
    /// The field name this provider provides values for
    pub field: String,
    /// The static values to suggest
    pub values: Vec<String>,
}

impl ValueProvider for StaticValuesProvider {
    /// Suggest values that fuzzy-match `partial` for the configured (command,
    /// field).
    fn suggest(
        &self,
        command_key: &str,
        field: &str,
        partial: &str,
        _inputs: &std::collections::HashMap<String, String>,
    ) -> Vec<SuggestionItem> {
        if command_key != self.command_key || field != self.field {
            return vec![];
        }
        let mut out = Vec::new();
        for v in &self.values {
            if let Some(score) = fuzzy_score(v, partial) {
                out.push(SuggestionItem {
                    display: v.clone(),
                    insert_text: v.clone(),
                    kind: ItemKind::Value,
                    meta: Some("provider".into()),
                    score,
                });
            }
        }
        out
    }
}

// Determine if the first two tokens resolve to a known command.
//
// A command is considered resolved when at least two tokens exist and they
// match a `(group, name)` pair in the registry.
// is_command_resolved is implemented in suggest.rs

// Compute the prefix used to rank command suggestions.
//
// When two or more tokens exist, uses "group sub"; otherwise uses the first
// token or empty string.
// compute_command_prefix is implemented in suggest.rs

// Build command suggestions in execution form ("group sub").
//
// Uses `fuzzy_score` against the computed prefix to rank candidates and embeds
// the command summary in the display text.
// suggest_commands is implemented in suggest.rs

// Parse user-provided flags and positional arguments from the portion of
// tokens after the resolved (group, sub) command.
//
// long flags are collected without the leading dashes; values immediately
// following non-boolean flags are consumed. Returns `(user_flags, user_args)`.
// parse_user_flags_args is implemented in suggest.rs

// Find the last pending non-boolean flag that expects a value.
//
// Scans tokens from the end to find the most recent flag and checks whether
// its value has been supplied. If a value is already complete (per
// `is_flag_value_complete`), returns `None`.
// find_pending_flag is implemented in suggest.rs

// Derive the value fragment currently being typed for the last flag.
//
// If the last token is a flag containing an equals sign (e.g., `--app=pa`),
// returns the suffix after `=`; otherwise returns the last token itself (or an
// empty string when no tokens exist in `parts`).
// flag_value_partial is implemented in suggest.rs

// Suggest values for a specific non-boolean flag, combining enum values with
// provider-derived suggestions.
// suggest_values_for_flag is implemented in suggest.rs

// Suggest positional values for the next expected positional parameter using
// providers; when no provider values are available, suggest a placeholder
// formatted as `<name>`.
// suggest_positionals is implemented in suggest.rs

// Whether any required flags are not yet supplied by the user.
// required_flags_remaining is implemented in suggest.rs

// Determine whether the last flag's value is complete according to REPL rules.
//
// Rules:
// - If the last token is `-` or `--`, it is not complete.
// - If no flag token is found when scanning backward, it is complete.
// - If the last token is the flag itself (no value yet), it is not complete.
// - If the last token is the value immediately after the flag, it is complete
//   only if the input ends in whitespace (typing may continue otherwise).
//
// Arguments:
// - `input`: The full input line.
//
// Returns: `true` if the last flag value is considered complete.
//
// Example:
//
// ```rust,ignore
// use heroku_tui::ui::components::palette::state::is_flag_value_complete;
//
// assert!(!is_flag_value_complete("--app"));
// assert!(!is_flag_value_complete("--app my"));
// assert!(is_flag_value_complete("--app my "));
// ```
fn get_current_token(input: &str, cursor_position: usize) -> String {
    let tokens = lex_shell_like_ranged(input);
    let token = tokens
        .iter()
        .find(|t| t.start <= cursor_position && cursor_position <= t.end)
        .or_else(|| tokens.last());

    token.map(|t| t.text.to_string()).unwrap_or_default()
}

// is_flag_value_complete is implemented in suggest.rs and re-exported above

// Collect candidate flag suggestions for a command specification.
//
// Generates suggestions for either required or optional flags that have not
// yet been provided by the user. When `current` starts with a dash, only flags
// whose long form starts with `current` are included (prefix filtering).
//
// Arguments:
// - `spec`: The command specification whose flags are considered.
// - `user_flags`: Long flag names already present in the input (without `--`).
// - `current`: The current token text (used for prefix filtering when typing a
//   flag).
// - `required_only`: When `true`, include only required flags; when `false`,
//   only optional flags.
// collect_flag_candidates is implemented in suggest.rs

// Compute the remainder of the current token toward a target insert text toward end.
//
// If the token under the cursor is a prefix of `insert`, returns the suffix
// that would be inserted to complete it. Used to render subtle ghost text to
// the right of the cursor previewing acceptance of the top suggestion.
//
// Arguments:
// - `input`: Full input line.
// - `cursor`: Cursor position (byte index) into `input`.
// - `insert`: The prospective full text to insert for the current token.
//
// Returns: The suffix of `insert` beyond the current token, or empty string.
//
// Example:
//
// ```rust,ignore
// use heroku_tui::ui::components::palette::state::ghost_remainder;
//
// assert_eq!(ghost_remainder("ap", 2, "apps"), "ps");
// assert_eq!(ghost_remainder("foo", 3, "bar"), "");
// ```
pub fn ghost_remainder(input: &str, cursor: usize, insert: &str) -> String {
    let tokens = lex_shell_like_ranged(input);
    // Find the token that contains the cursor, otherwise take the last token
    let last_tok = tokens
        .iter()
        .find(|t| t.start <= cursor && cursor <= t.end)
        .or_else(|| tokens.last());
    let token_text = match last_tok {
        Some(t) => t.text,
        None => "",
    };
    if let Some(rest) = insert.strip_prefix(token_text) {
        rest.to_string()
    } else {
        String::new()
    }
}
