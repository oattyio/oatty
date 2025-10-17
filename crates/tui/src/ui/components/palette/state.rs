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
use std::sync::{Arc, Mutex};

use crate::ui::components::palette::suggestion_engine::SuggestionEngine;
use crate::ui::theme::Theme;

use super::suggestion_engine::{parse_user_flags_args, required_flags_remaining};
use crate::ui::components::common::TextInputState;
use crate::ui::theme::theme_helpers::create_spans_with_match;
use heroku_engine::provider::{PendingProviderFetch, ValueProvider};
use heroku_registry::{CommandRegistry, find_by_group_and_cmd};
use heroku_types::{CommandSpec, Effect, ExecOutcome, ItemKind, Modal, SuggestionItem};
use heroku_util::{lex_shell_like, lex_shell_like_ranged};
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::ListItem;

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
    if let Some((idx, _)) = tokens.iter().enumerate().find(|(_, t)| t.start <= cursor && cursor <= t.end) {
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
    registry: Arc<Mutex<CommandRegistry>>,
    /// Focus flag for self
    focus: FocusFlag,
    /// Focus flag for the input field
    f_input: FocusFlag,
    /// The current input text
    input: String,
    /// Current cursor position (byte index)
    cursor_position: usize,
    /// Optional ghost text to show as a placeholder
    ghost_text: Option<String>,
    /// Whether the suggestions popup is currently open
    is_suggestions_open: bool,
    /// Index of the currently selected suggestion
    suggestion_index: usize,
    /// List of current suggestions
    suggestions: Vec<SuggestionItem>,
    /// Pre-rendered suggestion list items for efficient display
    rendered_suggestions: Vec<ListItem<'static>>,
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
    /// The hash of the command being waited on by the palette
    cmd_exec_hash: Option<u64>,
}

impl PaletteState {
    pub fn new(registry: Arc<Mutex<CommandRegistry>>) -> Self {
        Self {
            registry,
            focus: FocusFlag::named("heroku.palette"),
            f_input: FocusFlag::named("heroku.palette.input"),
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
            cmd_exec_hash: None,
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

    /// Get the count of current suggestions
    pub fn suggestions_len(&self) -> usize {
        self.suggestions.len()
    }

    /// Get the currently selected suggestion index
    pub fn suggestion_index(&self) -> usize {
        self.suggestion_index
    }

    pub fn cmd_exec_hash(&self) -> Option<u64> {
        self.cmd_exec_hash
    }

    /// Derive the command spec from the current input tokens ("group sub").
    fn selected_command_from_input(&self, commands: &[CommandSpec]) -> Option<CommandSpec> {
        let tokens: Vec<String> = lex_shell_like(&self.input);
        if tokens.len() < 2 {
            return None;
        }
        let group = &tokens[0];
        let name = &tokens[1];
        find_by_group_and_cmd(commands, group.as_str(), name.as_str()).ok()
    }

    /// Derive the command spec from the currently selected suggestion when it
    /// is a command.
    fn selected_command_from_suggestion(&self, commands: &[CommandSpec]) -> Option<CommandSpec> {
        let item = self.suggestions.get(self.suggestion_index)?;
        if !matches!(item.kind, ItemKind::Command) {
            return None;
        }
        let mut parts = item.insert_text.split_whitespace();
        let group = parts.next().unwrap_or("");
        let name = parts.next().unwrap_or("");
        find_by_group_and_cmd(commands, group, name).ok()
    }

    /// Selected command for help: prefer highlighted suggestion if open, else
    /// parse input.
    pub fn selected_command(&self) -> Option<CommandSpec> {
        let lock = self.registry.lock().ok()?;
        let commands = &lock.commands;
        let selected_command = self.selected_command_from_suggestion(commands);
        if self.is_suggestions_open && selected_command.is_some() {
            return selected_command;
        }
        self.selected_command_from_input(commands)
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
    pub fn rendered_suggestions(&self) -> &[ListItem<'static>] {
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

    /// Adapter to delegate text/cursor editing to common::TextInputState
    fn with_text_input<F: FnOnce(&mut TextInputState)>(&mut self, f: F) {
        let mut ti = TextInputState::new();
        ti.set_input(self.input.clone());
        ti.set_cursor(self.cursor_position);
        f(&mut ti);
        self.input = ti.input().to_string();
        self.cursor_position = ti.cursor();
    }

    /// Set the input text
    pub(crate) fn set_input(&mut self, input: String) {
        self.input = input;
    }

    pub(crate) fn set_cmd_exec_hash(&mut self, hash: u64) {
        self.cmd_exec_hash = Some(hash)
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
        self.with_text_input(|ti| ti.move_left());
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
        self.with_text_input(|ti| ti.move_right());
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
        self.with_text_input(|ti| ti.insert_char(c));
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
        self.with_text_input(|ti| ti.backspace());
    }

    /// Finalize suggestion list for the UI: rank, truncate, ghost text, and
    /// state flags.
    fn finalize_suggestions(&mut self, items: &mut [SuggestionItem], theme: &dyn crate::ui::theme::Theme) {
        items.sort_by(|a, b| b.score.cmp(&a.score));

        self.suggestion_index = self.suggestion_index.min(items.len().saturating_sub(1));
        self.suggestions = items.to_vec();
        self.is_suggestions_open = !self.suggestions.is_empty();

        let current_token = get_current_token(&self.input, self.cursor_position);
        let needle = current_token.trim();

        self.rendered_suggestions = self
            .suggestions
            .iter()
            .map(|suggestion_item| {
                let display = suggestion_item.display.clone();
                let spans = create_spans_with_match(
                    needle.to_string(),
                    display,
                    theme.text_primary_style(),
                    theme.accent_emphasis_style(),
                );
                ListItem::from(Line::from(spans))
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
    pub fn push_history_if_needed(&mut self, entry: &str) {
        let value = entry.trim();
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
            let remove_from: Option<usize> = tokens.last().and_then(|t| (t.text == "-" || t.text == "--").then_some(t.start));
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
    /// ```
    pub fn apply_build_suggestions(&mut self, providers: &[Arc<dyn ValueProvider>], theme: &dyn Theme) -> Vec<PendingProviderFetch> {
        let mut pending_fetches = Vec::new();
        let mut items = {
            let Some(lock) = self.registry.lock().ok() else {
                return pending_fetches;
            };
            let commands = &lock.commands;

            let result = SuggestionEngine::build(commands, providers, &self.input);

            let mut items = result.items;
            pending_fetches = result.pending_fetches;
            self.provider_loading = result.provider_loading || !pending_fetches.is_empty();

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
                    if let Ok(spec) = find_by_group_and_cmd(commands, group.as_str(), name.as_str()) {
                        let parts: &[String] = if tokens.len() >= 2 { &tokens[2..] } else { &tokens[0..0] };
                        let (user_flags, user_args, _flag_values) = parse_user_flags_args(&spec, parts);
                        if let Some(hint) = self.eol_flag_hint(&spec, &user_flags) {
                            items.push(hint);
                        } else {
                            // If command is complete (all positionals filled, no required flags), show run hint
                            let positionals_complete = user_args.len() >= spec.positional_args.len();
                            let required_remaining = required_flags_remaining(&spec, &user_flags);
                            if positionals_complete && !required_remaining {
                                self.ghost_text = Some(" press Enter to run".to_string());
                            }
                        };
                    };
                }
            }
            items
        };

        self.finalize_suggestions(items.as_mut_slice(), theme);
        // Preserve run hint ghost when suggestions are empty
        if self.suggestions.is_empty() && self.ghost_text.is_none() {
            let tokens: Vec<String> = lex_shell_like(&self.input);
            if tokens.len() >= 2 {
                let group = tokens[0].clone();
                let name = tokens[1].clone();
                let Some(spec) = self
                    .registry
                    .lock()
                    .ok()
                    .and_then(|lock| find_by_group_and_cmd(&lock.commands, group.as_str(), name.as_str()).ok())
                else {
                    return pending_fetches;
                };

                let parts: &[String] = if tokens.len() >= 2 { &tokens[2..] } else { &tokens[0..0] };
                let (user_flags, user_args, _flag_values) = parse_user_flags_args(&spec, parts);
                let positionals_complete = user_args.len() >= spec.positional_args.len();
                let required_remaining = required_flags_remaining(&spec, &user_flags);
                if positionals_complete && !required_remaining {
                    self.ghost_text = Some(" press Enter to run".to_string());
                }
            }
        }
        pending_fetches
    }

    /// Suggest an end-of-line hint for starting flags when any remain.
    fn eol_flag_hint(&self, spec: &CommandSpec, user_flags: &[String]) -> Option<SuggestionItem> {
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

    /// Processes general command execution results (non-plugin specific).
    ///
    /// This method handles the standard processing of command results including
    /// logging, table updates, and pagination information.
    ///
    /// # Arguments
    ///
    /// * `execution_outcome` - The result of the command execution
    pub(crate) fn process_general_execution_result(&mut self, execution_outcome: &Box<ExecOutcome>) -> Vec<Effect> {
        let mut effects = Vec::new();
        let (value, request_id) = match execution_outcome.as_ref() {
            ExecOutcome::Http(_, _, value, _, request_id) => (value.clone(), request_id.clone()),
            ExecOutcome::Mcp(_, value, request_id) => (value.clone(), request_id.clone()),
            _ => return effects,
        };

        // nothing to do
        if !self.cmd_exec_hash.is_some_and(|h| h == request_id) || value.is_null() {
            return effects;
        }

        let input = self.input.to_string();
        self.push_history_if_needed(input.trim());
        self.reduce_clear_all();
        effects.push(Effect::ShowModal(Modal::Results(execution_outcome.clone())));

        effects
    }
}

/// Retrieves the current token at or near a specific cursor position in the input string.
///
/// This function processes the input string using a lexer function `lex_shell_like_ranged`
/// to tokenize it into a series of tokens. It then identifies the token that contains
/// the given cursor position (if any). If no such token is found, it defaults to
/// returning the last token in the list. If the list of tokens is empty, it returns an
/// empty string.
///
/// # Arguments
///
/// * `input` - A reference to the input string which will be tokenized.
/// * `cursor_position` - A `usize` representing the cursor's position in the input string.
///
/// # Returns
///
/// A `String` representation of the current token at the cursor's position. If no token
/// can be identified at the cursor's position, the function returns an empty string.
///
/// # Example
///
/// ```rust
/// let input = "echo hello world";
/// let cursor_position = 6;
/// let token = get_current_token(input, cursor_position);
/// assert_eq!(token, "hello");
/// ```
///
/// # Notes
///
/// The function relies on `lex_shell_like_ranged`, which is assumed to return a list of
/// tokens where each token is a structure or object that includes the fields:
/// - `start`: The starting index of the token in the input string.
/// - `end`: The ending index of the token in the input string.
/// - `text`: The actual text of the token.
///
/// The range `[start, end]` is inclusive, meaning the token at `cursor_position` is
/// determined if the position satisfies `start <= cursor_position <= end`.
///
/// If the cursor does not match any token but there are tokens available, the final
/// token in the list will be returned.
fn get_current_token(input: &str, cursor_position: usize) -> String {
    let tokens = lex_shell_like_ranged(input);
    let token = tokens
        .iter()
        .find(|t| t.start <= cursor_position && cursor_position <= t.end)
        .or_else(|| tokens.last());

    token.map(|t| t.text.to_string()).unwrap_or_default()
}

/// Computes the remaining portion of the `insert` string after stripping the prefix
/// that matches the token containing the cursor position within the `input` string,
/// or the last token if no token contains the cursor.
///
/// # Arguments
///
/// * `input` - A string slice representing the input text to be tokenized.
/// * `cursor` - A `usize` representing the position of the cursor within the `input`.
/// * `insert` - A string slice representing the text to be analyzed for the remainder.
///
/// # Returns
///
/// Returns a `String` containing the portion of `insert` after removing the
/// token-matching prefix. If no matching token is found, or the prefix does not match,
/// it returns an empty `String`.
///
/// # Behavior
///
/// The function works as follows:
/// 1. Tokenizes the input string into shell-like tokens using `lex_shell_like_ranged`.
/// 2. Determines the token that contains the cursor position, or defaults to the last token.
/// 3. Extracts the matching text of the identified token.
/// 4. Checks whether `insert` starts with the token text. If so, it strips the token text
///    from `insert` and returns the remainder. Otherwise, it returns an empty string.
///
/// # Example
///
/// ```
/// let input = "echo hello world";
/// let cursor = 5; // Cursor is within "hello".
/// let insert = "hello world";
/// let result = ghost_remainder(input, cursor, insert);
/// assert_eq!(result, " world");
/// ```
///
/// If the `insert` does not start with the token text:
///
/// ```
/// let input = "echo hello";
/// let cursor = 0; // Cursor is outside of any token (matches "echo").
/// let insert = "world";
/// let result = ghost_remainder(input, cursor, insert);
/// assert_eq!(result, ""); // No match, so empty string is returned.
/// ```
///
/// # Note
///
/// The function depends on the `lex_shell_like_ranged` external function for tokenizing
/// the `input` string, which must return a collection of tokens, each with fields
/// `start`, `end`, and `text`. The behavior of this function heavily depends
/// on the implementation of the tokenizer.
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_partial_positional_on_accept() {
        let mut st = PaletteState::new(Arc::new(Mutex::new(CommandRegistry::from_embedded_schema().unwrap())));
        st.set_input("apps info hero".into());
        st.set_cursor(st.input().len());
        st.apply_accept_positional_suggestion("heroku-prod");
        assert_eq!(st.input(), "apps info heroku-prod ");
    }

    #[test]
    fn replace_placeholder_positional_on_accept_value() {
        let mut st = PaletteState::new(Arc::new(Mutex::new(CommandRegistry::from_embedded_schema().unwrap())));
        // Placeholder for positional arg
        st.set_input("apps info <app> ".into());
        st.set_cursor(st.input().len());
        // Selecting a provider value (non-flag) should replace placeholder
        st.apply_accept_non_command_suggestion("heroku-prod");
        assert_eq!(st.input(), "apps info heroku-prod ");
    }

    // --- Parity tests with common::TextInputState to quantify integration risk ---
    use crate::ui::components::common::TextInputState;

    #[derive(Clone, Copy, Debug)]
    enum Op {
        Left,
        Right,
        Ins(char),
        Back,
    }

    fn run_palette(mut input: &str, cursor: usize, ops: &[Op]) -> (String, usize) {
        let reg = Arc::new(Mutex::new(CommandRegistry::from_embedded_schema().unwrap()));
        let mut st = PaletteState::new(reg);
        st.set_input(input.to_string());
        st.set_cursor(cursor);
        for op in ops {
            match *op {
                Op::Left => st.reduce_move_cursor_left(),
                Op::Right => st.reduce_move_cursor_right(),
                Op::Ins(c) => st.apply_insert_char(c),
                Op::Back => st.reduce_backspace(),
            }
        }
        (st.input().to_string(), st.selected_cursor_position())
    }

    fn run_text_input(input: &str, cursor: usize, ops: &[Op]) -> (String, usize) {
        let mut st = TextInputState::new();
        st.set_input(input);
        st.set_cursor(cursor);
        for op in ops {
            match *op {
                Op::Left => st.move_left(),
                Op::Right => st.move_right(),
                Op::Ins(c) => st.insert_char(c),
                Op::Back => st.backspace(),
            }
        }
        (st.input().to_string(), st.cursor())
    }

    fn assert_parity(input: &str, cursor: usize, ops: &[Op]) {
        let a = run_palette(input, cursor, ops);
        let b = run_text_input(input, cursor, ops);
        assert_eq!(a, b, "Parity mismatch for input='{input}', cursor={cursor}, ops={ops:?}");
    }

    #[test]
    fn palette_text_input_parity_ascii() {
        let ops = [
            Op::Ins('h'),
            Op::Ins('e'),
            Op::Ins('l'),
            Op::Ins('l'),
            Op::Ins('o'),
            Op::Left,
            Op::Back,
            Op::Ins('y'),
        ];
        assert_parity("", 0, &ops);
    }

    #[test]
    fn palette_text_input_parity_utf8_emoji() {
        // Start with multi-byte character in the middle
        let input = "h🙂llo"; // emoji is 4 bytes
        // place cursor after 'h'
        let cursor = 1;
        let ops = [Op::Ins('e'), Op::Right, Op::Back, Op::Left, Op::Back];
        assert_parity(input, cursor, &ops);
    }

    #[test]
    fn palette_text_input_parity_boundaries() {
        // Deleting at start is a no-op; moving past end is a no-op
        let input = "abc";
        let cursor = 0;
        let ops = [Op::Back, Op::Left, Op::Ins('x'), Op::Left, Op::Left, Op::Left, Op::Left, Op::Back];
        assert_parity(input, cursor, &ops);
    }

    #[test]
    fn palette_text_input_parity_mixed_sequence() {
        let input = "héllo"; // accented e is 2 bytes
        // put cursor between h and é
        let cursor = 1;
        let ops = [
            Op::Right, // over é correctly (2 bytes)
            Op::Ins('-'),
            Op::Left,
            Op::Back,
            Op::Ins('X'),
            Op::Right,
            Op::Right,
            Op::Ins('!'),
        ];
        assert_parity(input, cursor, &ops);
    }
}
