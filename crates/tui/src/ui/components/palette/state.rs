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
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::ui::theme::Theme;
use crate::ui::{
    components::palette::suggestion_engine::SuggestionEngine,
    utils::{SpanCollector, truncate_to_width},
};
use chrono::Utc;

use super::suggestion_engine::{parse_user_flags_args, required_flags_remaining};
use crate::ui::components::common::TextInputState;
use crate::ui::theme::theme_helpers::{create_spans_with_match, highlight_segments};
use oatty_engine::provider::{PendingProviderFetch, ValueProvider};
use oatty_registry::{CommandRegistry, find_by_group_and_cmd};
use oatty_types::{CommandExecution, CommandSpec, Effect, ExecOutcome, ItemKind, Modal, SuggestionItem};
use oatty_util::{
    HistoryKey, HistoryScope, HistoryScopeKind, HistoryStore, StoredHistoryValue, has_meaningful_value, lex_shell_like,
    lex_shell_like_ranged, value_contains_secret,
};
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::widgets::{ListItem, ListState};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};
use serde_json::Value;
use tracing::warn;

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
#[derive(Clone)]
pub struct PaletteState {
    /// Focus flag for the input field
    pub f_input: FocusFlag,
    /// list state for the suggestion list
    pub list_state: ListState,
    registry: Arc<Mutex<CommandRegistry>>,
    /// Focus flag for self
    container_focus: FocusFlag,
    /// The current input text
    input: String,
    /// Current cursor position (byte index)
    cursor_position: usize,
    /// Optional ghost text to show as a placeholder
    ghost_text: Option<String>,
    /// Whether the suggestions popup is currently open
    is_suggestions_open: bool,
    /// Whether the current command is destructive (requires confirmation)
    is_destructive: bool,
    /// The index of the current mouse hovered suggestion, if any
    mouse_over_idx: Option<usize>,
    /// List of current suggestions
    suggestions: Vec<SuggestionItem>,
    /// Pre-rendered suggestion list items for efficient display
    rendered_suggestions: Vec<ListItem<'static>>,
    /// Cached width of the suggestions list area, used for truncation calculations.
    suggestions_view_width: Option<u16>,
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
    /// Persistent history store used for palette executions.
    history_store: Arc<dyn HistoryStore>,
    /// Identifier representing the active history profile.
    history_profile_id: String,
    /// Cached persisted commands keyed by canonical command identifier.
    stored_commands: HashMap<String, StoredHistoryValue>,
    /// Pending command identifier awaiting execution completion.
    pending_command_id: Option<String>,
    /// Pending command input captured at dispatch time.
    pending_command_input: Option<String>,
}

impl PaletteState {
    pub fn new(registry: Arc<Mutex<CommandRegistry>>, history_store: Arc<dyn HistoryStore>, history_profile_id: String) -> Self {
        let mut state = Self {
            registry,
            history_store,
            history_profile_id,
            container_focus: FocusFlag::new().with_name("oatty.palette"),
            f_input: FocusFlag::new().with_name("oatty.palette.input"),
            input: String::new(),
            cursor_position: 0,
            ghost_text: None,
            is_suggestions_open: false,
            is_destructive: false,
            mouse_over_idx: None,
            list_state: ListState::default(),
            suggestions: Vec::new(),
            rendered_suggestions: Vec::new(),
            suggestions_view_width: None,
            error_message: None,
            provider_loading: false,
            history: Vec::new(),
            history_index: None,
            draft_input: None,
            cmd_exec_hash: None,
            stored_commands: HashMap::new(),
            pending_command_id: None,
            pending_command_input: None,
        };
        state.load_persisted_history();
        state
    }
}

impl HasFocus for PaletteState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.leaf_widget(&self.f_input);
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
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
        let item = self.suggestions.get(self.list_state.selected()?)?;
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

    /// Updates the cached width of the suggestions popup for truncation logic.
    pub fn update_suggestions_view_width(&mut self, width: u16, theme: &dyn Theme) {
        self.suggestions_view_width = Some(width);
        if !self.suggestions.is_empty() {
            self.refresh_rendered_suggestions(theme);
        }
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
        self.history_index = None;
        self.draft_input = None;
        self.cmd_exec_hash = None;
        self.pending_command_id = None;
        self.pending_command_input = None;
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
        self.is_suggestions_open = !self.suggestions.is_empty();
        self.apply_ghost_text();
    }

    /// Clear all suggestions and close popup
    pub fn reduce_clear_suggestions(&mut self) {
        self.suggestions.clear();
        self.rendered_suggestions.clear();
        self.is_suggestions_open = false;
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
        self.update_is_destructive();
    }

    /// Set the input text
    pub(crate) fn set_input(&mut self, input: String) {
        self.input = input;
        self.update_is_destructive();
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

    /// set the mouse over index
    pub(crate) fn update_mouse_over_idx(&mut self, idx: Option<usize>) {
        self.mouse_over_idx = idx;
    }

    pub(crate) fn is_destructive_command(&self) -> bool {
        self.is_destructive
    }
    fn update_is_destructive(&mut self) {
        if let [group, name, ..] = &lex_shell_like(&self.input)[..] {
            let Ok(lock) = self.registry.try_lock() else { return };
            let Ok(CommandSpec {
                execution: CommandExecution::Http(execution),
                ..
            }) = find_by_group_and_cmd(&lock.commands, group, name)
            else {
                return;
            };
            self.is_destructive = execution.method == "DELETE";
        } else {
            self.is_destructive = false;
        }
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
        self.update_is_destructive();
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

    /// Remove the character immediately after the cursor.
    ///
    /// This method removes the character after the cursor and adjusts
    /// the cursor position accordingly, handling multi-byte UTF-8
    /// characters correctly.
    ///
    /// - No-op if the cursor is at the end of the input.
    /// - Handles multi-byte UTF-8 characters correctly.
    ///
    /// Returns: nothing; mutates `self.input` and `self.cursor`.
    pub fn reduce_delete(&mut self) {
        // Editing cancels history browsing
        if self.history_index.is_some() {
            self.history_index = None;
            self.draft_input = None;
        }
        self.with_text_input(|ti| ti.delete());
    }

    /// Finalize the suggestions list for the UI: rank, truncate, ghost text, and
    /// state flags.
    fn finalize_suggestions(&mut self, items: &mut [SuggestionItem], theme: &dyn Theme) {
        items.sort_by(|a, b| b.score.cmp(&a.score));

        self.suggestions = items.to_vec();
        self.is_suggestions_open = !self.suggestions.is_empty();
        if self.is_suggestions_open {
            let preferred_index = self
                .suggestions
                .iter()
                .enumerate()
                .find(|(_, item)| !matches!(item.meta.as_deref(), Some("history") | Some("loading")))
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            self.list_state.select(Some(preferred_index));
        } else {
            self.list_state.select(None);
        }

        self.refresh_rendered_suggestions(theme);
        self.apply_ghost_text();
    }

    /// Rebuild the rendered suggestions to reflect the latest suggestion data.
    ///
    /// This helper recreates the rendered list items so that highlighting stays in sync
    /// with the current input token and the active theme.
    fn refresh_rendered_suggestions(&mut self, theme: &dyn Theme) {
        let current_token = get_current_token(&self.input, self.cursor_position);
        let needle = current_token.trim();

        let width_hint = self.suggestions_view_width;
        self.rendered_suggestions = self
            .suggestions
            .iter()
            .enumerate()
            .map(|(idx, suggestion_item)| {
                let spans = match suggestion_item.kind {
                    ItemKind::Command | ItemKind::MCP => build_command_spans(suggestion_item, needle, theme, width_hint),
                    ItemKind::Flag => build_flag_spans(suggestion_item, needle, theme, width_hint),
                    ItemKind::Value => build_value_spans(suggestion_item, needle, theme, width_hint),
                    ItemKind::Positional => build_positional_spans(suggestion_item, needle, theme, width_hint),
                };

                let spans = if spans.is_empty() {
                    create_spans_with_match(
                        needle.to_string(),
                        suggestion_item.display.clone(),
                        theme.text_primary_style(),
                        theme.search_highlight_style(),
                    )
                } else {
                    spans
                };

                let mut list_item = ListItem::from(Line::from(spans));
                if self.mouse_over_idx.is_some_and(|hover| hover == idx) {
                    list_item = list_item.style(theme.selection_style().add_modifier(Modifier::BOLD));
                }

                list_item
            })
            .collect();
    }

    pub fn apply_ghost_text(&mut self) {
        if !self.is_suggestions_open {
            self.ghost_text = None;
            return;
        }
        self.ghost_text = self
            .suggestions
            .get(self.list_state.selected().unwrap_or(0))
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
                self.update_is_destructive();
                true
            }
            Some(0) => false,
            Some(i) => {
                let ni = i - 1;
                self.history_index = Some(ni);
                self.input = self.history[ni].clone();
                self.cursor_position = self.input.len();
                self.is_suggestions_open = false;
                self.update_is_destructive();
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
                    self.update_is_destructive();
                    true
                } else {
                    if let Some(draft) = self.draft_input.take() {
                        self.input = draft;
                        self.update_is_destructive();
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
            self.update_is_destructive();
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
        self.update_is_destructive();
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
                self.update_is_destructive();
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
    /// use oatty_tui::ui::components::palette::state::PaletteState;
    /// use Registry;
    ///
    /// let mut st = PaletteState::default();
    /// st.set_input("apps info --app ".into());
    /// st.apply_build_suggestions(&Registry::from_embedded_schema().unwrap(), &[]);
    /// ```
    pub fn apply_build_suggestions(&mut self, providers: &[Arc<dyn ValueProvider>], theme: &dyn Theme) -> Vec<PendingProviderFetch> {
        self.reduce_clear_error();
        let tokens: Vec<String> = lex_shell_like(&self.input);
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
            if items.is_empty() && tokens.len() >= 2 {
                let group = tokens[0].as_str();
                let name = tokens[1].as_str();
                if let Ok(spec) = find_by_group_and_cmd(commands, group, name) {
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
            items
        };

        self.finalize_suggestions(items.as_mut_slice(), theme);
        // Preserve run hint ghost when suggestions are empty
        if self.suggestions.is_empty() && self.ghost_text.is_none() && tokens.len() >= 2 {
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
        pending_fetches
    }

    /// Handle provider fetch failures by clearing loading state and surfacing an error.
    ///
    /// This method removes any transient "loading more…" placeholder, stops the spinner, and
    /// records an error message so the palette communicates that suggestions failed to load.
    ///
    /// # Arguments
    ///
    /// * `log_message` - Text describing the provider fetch failure, forwarded from the logs.
    /// * `theme` - Active theme used to rebuild rendered suggestions after removing placeholders.
    pub(crate) fn handle_provider_fetch_failure(&mut self, log_message: &str, theme: &dyn Theme) {
        let has_loading_placeholder = self.suggestions.iter().any(|item| item.meta.as_deref() == Some("loading"));
        if !self.provider_loading && !has_loading_placeholder {
            return;
        }

        self.provider_loading = false;

        let previous_length = self.suggestions.len();
        self.suggestions.retain(|item| item.meta.as_deref() != Some("loading"));
        let loading_removed = previous_length != self.suggestions.len();

        self.list_state.select_previous();
        if self.suggestions.is_empty() {
            self.rendered_suggestions.clear();
            self.is_suggestions_open = false;
            self.ghost_text = None;
        } else if loading_removed {
            self.refresh_rendered_suggestions(theme);
            self.apply_ghost_text();
        }

        let friendly_message = log_message
            .strip_prefix("Provider fetch failed:")
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(|detail| format!("Provider suggestions failed: {detail}"))
            .unwrap_or_else(|| "Provider suggestions failed.".to_string());
        self.error_message = Some(friendly_message);
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

    fn load_persisted_history(&mut self) {
        let records = match self.history_store.entries_for_scope(HistoryScopeKind::PaletteCommand) {
            Ok(records) => records,
            Err(error) => {
                warn!(error = %error, "failed to load palette history");
                return;
            }
        };

        let mut filtered: Vec<_> = records
            .into_iter()
            .filter(|record| record.key.user_id == self.history_profile_id)
            .filter(|record| matches!(record.key.scope, HistoryScope::PaletteCommand { .. }))
            .collect();

        filtered.sort_by(|a, b| a.value.updated_at.cmp(&b.value.updated_at));

        for record in filtered {
            if let HistoryScope::PaletteCommand { command_id } = record.key.scope
                && let Some(input) = record.value.value.as_str()
                && !input.trim().is_empty()
            {
                self.push_history_if_needed(input);
                self.stored_commands.insert(command_id.clone(), record.value);
            }
        }
    }

    pub(crate) fn record_pending_execution(&mut self, command_id: String, input: String) {
        self.pending_command_id = Some(command_id);
        self.pending_command_input = Some(input.trim().to_string());
    }

    fn persist_command_history(&mut self, command_id: &str, input: &str) {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return;
        }

        let value = Value::String(trimmed.to_string());
        if !has_meaningful_value(&value) || value_contains_secret(&value) {
            return;
        }

        let stored = StoredHistoryValue {
            value: value.clone(),
            updated_at: Utc::now(),
        };
        let key = HistoryKey::palette_command(self.history_profile_id.clone(), command_id.to_string());

        if let Err(error) = self.history_store.insert_value(key, value) {
            warn!(command = %command_id, error = %error, "failed to persist palette history entry");
        }

        self.stored_commands.insert(command_id.to_string(), stored);
    }

    /// Processes general command execution results (non-plugin specific).
    ///
    /// This method handles the standard processing of command results including
    /// logging, table updates, and pagination information.
    ///
    /// # Arguments
    ///
    /// * `execution_outcome` - The result of the command execution
    pub(crate) fn process_general_execution_result(&mut self, execution_outcome: ExecOutcome) -> Vec<Effect> {
        let mut effects = Vec::new();
        let (status, log, is_null, request_id) = match &execution_outcome {
            ExecOutcome::Http {
                status_code,
                log_entry,
                payload,
                request_id,
                ..
            } => (status_code, log_entry, payload.is_null(), request_id),
            ExecOutcome::Mcp {
                log_entry,
                payload,
                request_id,
            } => (&200, log_entry, payload.is_null(), request_id),
            _ => return effects,
        };

        // nothing to do
        if self.cmd_exec_hash.is_none_or(|h| h != *request_id) {
            return effects;
        }

        if *status > 399 {
            self.error_message = Some(format!("Command failed error: status {} - {}", status, log));
            return effects;
        }

        if is_null {
            self.error_message = Some("Command completed successfully but no value was returned".to_string());
            self.reduce_clear_all();
            return effects;
        }

        let history_entry = self.pending_command_input.clone().unwrap_or_else(|| self.input.trim().to_string());
        self.push_history_if_needed(history_entry.as_str());

        if let Some(command_id) = self.pending_command_id.take() {
            let input_to_store = self.pending_command_input.take().unwrap_or(history_entry.clone());
            self.persist_command_history(&command_id, &input_to_store);
        } else {
            self.pending_command_input = None;
        }

        self.reduce_clear_all();
        effects.push(Effect::ShowModal(Modal::Results(Box::new(execution_outcome))));

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

const LIST_HIGHLIGHT_SYMBOL_WIDTH: u16 = 2;
const DEFAULT_COMMAND_SUMMARY_WIDTH: u16 = 72;
const DEFAULT_FLAG_DESCRIPTION_WIDTH: u16 = 80;
const DEFAULT_VALUE_META_WIDTH: u16 = 60;
const DEFAULT_POSITIONAL_HELP_WIDTH: u16 = 80;
const MIN_TRUNCATION_WIDTH: u16 = 4;

fn normalized_width_hint(width_hint: Option<u16>) -> Option<u16> {
    width_hint
        .and_then(|hint| hint.checked_sub(LIST_HIGHLIGHT_SYMBOL_WIDTH))
        .filter(|width| *width > 0)
}

fn build_command_spans(item: &SuggestionItem, needle: &str, theme: &dyn Theme, width_hint: Option<u16>) -> Vec<Span<'static>> {
    let canonical = item.insert_text.trim();
    if canonical.is_empty() {
        return Vec::new();
    }

    let width_hint = normalized_width_hint(width_hint);
    let highlight = theme.search_highlight_style();
    let exec_type = match item.kind {
        ItemKind::Command => "CMD",
        ItemKind::MCP => "MCP",
        _ => "",
    };

    let mut collector = SpanCollector::with_capacity(8);
    collector.push(badge_span(theme, exec_type));
    collector.push(Span::raw(" "));

    let (group, command) = canonical.split_once(' ').unwrap_or(("", canonical));
    if !group.is_empty() {
        collector.extend(highlight_segments(needle, group, theme.syntax_type_style(), highlight));
        if !command.is_empty() {
            collector.push(Span::raw(" "));
        }
    }
    if !command.is_empty() {
        collector.extend(highlight_segments(needle, command, theme.syntax_function_style(), highlight));
    }

    match item.meta.as_deref() {
        Some("history") => {
            collector.push(Span::raw("  "));
            collector.push(Span::styled("(history)", theme.text_muted_style()));
        }
        Some("loading") => {}
        Some(summary) if !summary.trim().is_empty() => {
            collector.push(Span::raw("  "));
            let available = collector.remaining_with_fallback(width_hint, DEFAULT_COMMAND_SUMMARY_WIDTH);
            if available >= MIN_TRUNCATION_WIDTH {
                let truncated = truncate_to_width(summary.trim(), available);
                collector.extend(highlight_segments(
                    needle,
                    truncated.as_str(),
                    theme.text_secondary_style(),
                    highlight,
                ));
            }
        }
        _ => {}
    }

    collector.into_vec()
}

fn build_flag_spans(item: &SuggestionItem, needle: &str, theme: &dyn Theme, width_hint: Option<u16>) -> Vec<Span<'static>> {
    if item.meta.as_deref() == Some("loading") {
        return vec![Span::styled(item.display.clone(), theme.text_muted_style())];
    }

    let highlight = theme.search_highlight_style();
    let width_hint = normalized_width_hint(width_hint);
    let mut collector = SpanCollector::with_capacity(8);
    collector.push(badge_span(theme, "FLAG"));
    collector.push(Span::raw(" "));

    let flag_label = item.insert_text.trim();
    if !flag_label.is_empty() {
        collector.extend(highlight_segments(needle, flag_label, theme.syntax_keyword_style(), highlight));
    }

    if item.display.contains("[required]") {
        collector.push(Span::raw(" "));
        collector.push(Span::styled(
            "(required)",
            theme.syntax_keyword_style().add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(description) = item.meta.as_deref()
        && description != "loading"
        && !description.trim().is_empty()
    {
        let trimmed = description.trim();
        collector.push(Span::raw("  "));
        let available = collector.remaining_with_fallback(width_hint, DEFAULT_FLAG_DESCRIPTION_WIDTH);
        if available >= MIN_TRUNCATION_WIDTH {
            let truncated = truncate_to_width(trimmed, available);
            collector.extend(highlight_segments(
                needle,
                truncated.as_str(),
                theme.text_secondary_style(),
                highlight,
            ));
        }
    }

    collector.into_vec()
}

fn build_value_spans(item: &SuggestionItem, needle: &str, theme: &dyn Theme, width_hint: Option<u16>) -> Vec<Span<'static>> {
    if item.meta.as_deref() == Some("loading") {
        return vec![Span::styled(item.display.clone(), theme.text_muted_style())];
    }

    let highlight = theme.search_highlight_style();
    let width_hint = normalized_width_hint(width_hint);
    let badge_label = match item.meta.as_deref() {
        Some("enum") => "ENUM",
        _ => "VAL",
    };

    let mut collector = SpanCollector::with_capacity(8);
    collector.push(badge_span(theme, badge_label));
    collector.push(Span::raw(" "));

    let value_text = if !item.display.trim().is_empty() {
        item.display.trim().to_string()
    } else {
        item.insert_text.trim().to_string()
    };

    collector.extend(highlight_segments(
        needle,
        value_text.as_str(),
        infer_value_style(theme, value_text.as_str()),
        highlight,
    ));

    if let Some(meta) = item.meta.as_deref()
        && !meta.is_empty()
        && meta != "enum"
        && meta != "loading"
        && !meta.trim().is_empty()
    {
        let trimmed = meta.trim();
        collector.push(Span::raw("  "));
        let available = collector.remaining_with_fallback(width_hint, DEFAULT_VALUE_META_WIDTH);
        if available >= MIN_TRUNCATION_WIDTH {
            let truncated = truncate_to_width(trimmed, available);
            collector.push(Span::styled(truncated, theme.text_muted_style()));
        }
    }

    collector.into_vec()
}

fn build_positional_spans(item: &SuggestionItem, needle: &str, theme: &dyn Theme, width_hint: Option<u16>) -> Vec<Span<'static>> {
    let highlight = theme.search_highlight_style();
    let width_hint = normalized_width_hint(width_hint);
    let mut collector = SpanCollector::with_capacity(8);
    collector.push(badge_span(theme, "ARG"));
    collector.push(Span::raw(" "));

    let label = item.insert_text.trim();
    if !label.is_empty() {
        collector.extend(highlight_segments(needle, label, theme.syntax_type_style(), highlight));
    }

    if let Some(help) = item.meta.as_deref()
        && !help.trim().is_empty()
    {
        collector.push(Span::raw("  "));
        let available = collector.remaining_with_fallback(width_hint, DEFAULT_POSITIONAL_HELP_WIDTH);
        if available >= MIN_TRUNCATION_WIDTH {
            let truncated = truncate_to_width(help.trim(), available);
            collector.extend(highlight_segments(
                needle,
                truncated.as_str(),
                theme.text_secondary_style(),
                highlight,
            ));
        }
    }

    collector.into_vec()
}

fn badge_span(theme: &dyn Theme, label: &str) -> Span<'static> {
    let roles = theme.roles();
    Span::styled(
        format!("[{}]", label),
        Style::default().fg(roles.accent_secondary).add_modifier(Modifier::BOLD),
    )
}

fn infer_value_style(theme: &dyn Theme, value: &str) -> Style {
    if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") || value.eq_ignore_ascii_case("null") {
        theme.syntax_keyword_style()
    } else if value.parse::<f64>().is_ok() {
        theme.syntax_number_style()
    } else {
        theme.syntax_string_style()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{theme::dracula::DraculaTheme, utils::span_display_width};
    use oatty_util::{DEFAULT_HISTORY_PROFILE, InMemoryHistoryStore};

    fn palette_state_with_registry(registry: Arc<Mutex<CommandRegistry>>) -> PaletteState {
        let history_store: Arc<dyn HistoryStore> = Arc::new(InMemoryHistoryStore::new());
        PaletteState::new(registry, history_store, DEFAULT_HISTORY_PROFILE.to_string())
    }

    fn make_palette_state() -> PaletteState {
        let registry = Arc::new(Mutex::new(CommandRegistry::from_config().unwrap()));
        palette_state_with_registry(registry)
    }

    #[test]
    fn persists_command_history_on_success() {
        use serde_json::json;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let registry = Arc::new(Mutex::new(CommandRegistry::from_config().unwrap()));
        let store: Arc<InMemoryHistoryStore> = Arc::new(InMemoryHistoryStore::new());
        let mut palette = PaletteState::new(
            Arc::clone(&registry),
            store.clone() as Arc<dyn HistoryStore>,
            DEFAULT_HISTORY_PROFILE.to_string(),
        );

        let command_input = "apps info --app demo";
        palette.set_input(command_input.to_string());
        palette.set_cursor(command_input.len());

        let mut hasher = DefaultHasher::new();
        hasher.write(command_input.as_bytes());
        let request_hash = hasher.finish();

        let command_id = format!("{}:{}", "apps", "info");
        palette.record_pending_execution(command_id.clone(), command_input.to_string());
        palette.set_cmd_exec_hash(request_hash);

        let outcome = ExecOutcome::Http {
            status_code: 200,
            log_entry: "ok".into(),
            payload: json!({"ok": true}),
            pagination: None,
            request_id: request_hash,
        };
        palette.process_general_execution_result(outcome);

        assert_eq!(palette.history.last().map(|s| s.as_str()), Some(command_input));
        let stored_value = palette.stored_commands.get(&command_id).and_then(|record| record.value.as_str());
        assert_eq!(stored_value, Some(command_input));

        let stored_records = store.entries_for_scope(HistoryScopeKind::PaletteCommand).unwrap();
        assert!(stored_records.iter().any(|record| {
            if let HistoryScope::PaletteCommand { command_id: stored_id } = &record.key.scope {
                stored_id == &command_id && record.value.value.as_str() == Some(command_input)
            } else {
                false
            }
        }));
    }

    #[test]
    fn replace_partial_positional_on_accept() {
        let mut st = make_palette_state();
        st.set_input("apps info hero".into());
        st.set_cursor(st.input().len());
        st.apply_accept_positional_suggestion("sample-prod");
        assert_eq!(st.input(), "apps info sample-prod ");
    }

    #[test]
    fn replace_placeholder_positional_on_accept_value() {
        let mut st = make_palette_state();
        // Placeholder for positional arg
        st.set_input("apps info <app> ".into());
        st.set_cursor(st.input().len());
        // Selecting a provider value (non-flag) should replace placeholder
        st.apply_accept_non_command_suggestion("sample-prod");
        assert_eq!(st.input(), "apps info sample-prod ");
    }

    #[test]
    fn handle_provider_fetch_failure_clears_loading_placeholder_and_sets_error() {
        let registry = Arc::new(Mutex::new(CommandRegistry::from_config().expect("embedded registry")));
        let mut state = palette_state_with_registry(registry);
        state.set_input("apps info --app ".to_string());
        state.provider_loading = true;
        state.is_suggestions_open = true;
        state.list_state.select(Some(1));
        state.suggestions = vec![
            SuggestionItem {
                display: "loading more…".to_string(),
                insert_text: String::new(),
                kind: ItemKind::Value,
                meta: Some("loading".to_string()),
                score: i64::MIN,
            },
            SuggestionItem {
                display: "demo".to_string(),
                insert_text: "demo".to_string(),
                kind: ItemKind::Value,
                meta: None,
                score: 10,
            },
        ];
        let theme = DraculaTheme::new();

        state.handle_provider_fetch_failure("Provider fetch failed: timeout", &theme);

        assert!(!state.provider_loading);
        assert!(state.suggestions.iter().all(|item| item.meta.as_deref() != Some("loading")));
        assert_eq!(state.error_message(), Some(&"Provider suggestions failed: timeout".to_string()));
        assert!(state.is_suggestions_open());
        assert_eq!(state.suggestions_len(), 1);
        assert_eq!(state.rendered_suggestions().len(), 1);
    }

    #[test]
    fn command_spans_respect_available_width_hint() {
        let theme = DraculaTheme::new();
        let suggestion = SuggestionItem {
            display: "apps info".to_string(),
            insert_text: "apps info".to_string(),
            kind: ItemKind::Command,
            meta: Some("This description is intentionally verbose to validate width-based truncation logic.".to_string()),
            score: 100,
        };

        let width_hint = 24;
        let spans = build_command_spans(&suggestion, "", &theme, Some(width_hint));
        let rendered_width: u16 = spans.iter().map(span_display_width).sum();
        let max_width = width_hint.saturating_sub(LIST_HIGHLIGHT_SYMBOL_WIDTH);
        assert!(
            rendered_width <= max_width,
            "rendered width {rendered_width} exceeds available {max_width}"
        );
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

    fn run_palette(input: &str, cursor: usize, ops: &[Op]) -> (String, usize) {
        let reg = Arc::new(Mutex::new(CommandRegistry::from_config().unwrap()));
        let mut st = palette_state_with_registry(reg);
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
