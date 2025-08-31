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
//! - Positionals are suggested before flags unless the user explicitly starts
//!   a flag token ("-"/"--").
//! - For non-boolean flags whose value is pending, only values are suggested
//!   (enums and provider values) until the value is complete.
//! - Suggestions never render an empty popup.
//!
use std::{fmt::Debug, sync::Arc};

use heroku_registry::Registry;
use heroku_types::CommandSpec;
use heroku_util::{fuzzy_score, lex_shell_like, lex_shell_like_ranged};

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

/// Represents the type of suggestion item.
///
/// This enum categorizes different types of suggestions that can be
/// provided to the user in the command palette.
#[derive(Clone, Debug)]
pub enum ItemKind {
    /// A command name (e.g., "apps:list")
    Command,
    /// A flag or option (e.g., "--app", "--region")
    Flag,
    /// A value for a flag or positional argument
    Value,
    /// A positional argument (e.g., app name, dyno name)
    Positional,
}

/// Represents a single suggestion item in the palette.
///
/// This struct contains all the information needed to display and
/// insert a suggestion in the command palette.
#[derive(Clone, Debug)]
pub struct SuggestionItem {
    /// The text to display in the suggestion list
    pub display: String,
    /// The text to insert when the suggestion is selected
    pub insert_text: String,
    /// The type of suggestion (command, flag, value, etc.)
    pub kind: ItemKind,
    /// Optional metadata to display (e.g., flag description)
    pub meta: Option<String>,
    /// Score for ranking suggestions (higher is better)
    pub score: i64,
}

/// State for the command palette input and suggestions.
///
/// This struct manages the current state of the command palette including
/// input text, cursor position, suggestions, and error states.
#[derive(Clone, Debug, Default)]
pub struct PaletteState {
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
    /// Optional error message to display
    error_message: Option<String>,
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

    /// Derive the command spec from the currently selected suggestion when it is a command.
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

    /// Selected command for help: prefer highlighted suggestion if open, else parse input.
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

    /// Get the current suggestions list
    pub fn suggestions(&self) -> &[SuggestionItem] {
        &self.suggestions
    }

    /// Get the current popup open state
    pub fn is_suggestions_open(&self) -> bool {
        self.is_suggestions_open
    }

    // ===== REDUCERS =====

    /// Clear all palette state and reset to defaults
    pub fn reduce_clear_all(&mut self) {
        self.input.clear();
        self.cursor_position = 0;
        self.suggestions.clear();
        self.is_suggestions_open = false;
        self.error_message = None;
        self.ghost_text = None;
        self.suggestion_index = 0;
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
        self.suggestion_index = self.suggestion_index.min(self.suggestions.len().saturating_sub(1));
        self.is_suggestions_open = !self.suggestions.is_empty();
        self.apply_ghost_text();
    }

    /// Clear all suggestions and close popup
    pub fn reduce_clear_suggestions(&mut self) {
        self.suggestions.clear();
        self.is_suggestions_open = false;
        self.suggestion_index = 0;
        self.ghost_text = None;
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

    /// Insert text at the end of the input with a separating space and advance the cursor.
    ///
    /// Appends a space before `text` if the current input is non-empty and does not
    /// already end with a space, then appends `text` and a trailing space. The cursor
    /// is moved to the end of the input.
    ///
    /// This helper centralizes the common pattern used when accepting suggestions so
    /// individual handlers remain focused on their control flow rather than spacing.
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
    /// - `c`: The character to insert. UTF-8 length is respected for cursor advance.
    ///
    /// Returns: nothing; mutates `self.input` and `self.cursor`.
    pub fn apply_insert_char(&mut self, c: char) {
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

    /// Finalize suggestion list for the UI: rank, truncate, ghost text, and state flags.
    fn finalize_suggestions(&mut self, items: &mut Vec<SuggestionItem>) {
        items.sort_by(|a, b| b.score.cmp(&a.score));
        if items.len() > MAX_SUGGESTIONS {
            items.truncate(MAX_SUGGESTIONS);
        }
        self.suggestion_index = self.suggestion_index.min(self.suggestions.len().saturating_sub(1));
        self.suggestions = items.clone();
        self.is_suggestions_open = !self.suggestions.is_empty();
        self.apply_ghost_text();
    }

    pub fn apply_ghost_text(&mut self) {
        self.ghost_text = self
            .suggestions
            .get(self.suggestion_index)
            .map(|top| ghost_remainder(&self.input, self.cursor_position, &top.insert_text));
    }
    /// Accept a positional suggestion/value: fill the next positional slot after "group sub".
    /// If the last existing positional is a placeholder like "<app>", replace it; otherwise append before any flags.
    pub fn apply_accept_positional_suggestion(&mut self, value: &str) {
        let tokens: Vec<&str> = self.input.split_whitespace().collect();
        if tokens.len() < 2 {
            // No command yet; just append with proper spacing
            self.insert_with_space(value);
            return;
        }
        // Identify first flag position after command tokens
        let mut first_flag_idx = tokens.len();
        for (i, t) in tokens.iter().enumerate().skip(2) {
            if t.starts_with("--") {
                first_flag_idx = i;
                break;
            }
        }
        // Existing positionals are tokens[2..first_flag_idx]
        let mut out: Vec<String> = Vec::new();
        out.push(tokens[0].to_string());
        out.push(tokens[1].to_string());
        // Copy existing positionals as-is, then append new positional value
        for t in tokens[2..first_flag_idx].iter() {
            out.push((*t).to_string());
        }
        out.push(value.to_string());
        // Append the rest (flags and any trailing tokens) in original order
        for t in tokens.iter().skip(first_flag_idx) {
            out.push((*t).to_string());
        }
        self.input = out.join(" ") + " ";
        self.cursor_position = self.input.len();
    }
    /// Accept a command suggestion by replacing the input with the execution form
    /// (e.g., "group sub") followed by a trailing space, and moving the cursor to
    /// the end.
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
    // This function used to render the complete command palette including the input line,
    // optional ghost text, error messages, and the suggestions popup. Rendering responsibility
    // has been migrated to PaletteComponent::render(), and this legacy documentation remains
    // here only as historical context for future refactors.

    /// Accept a non-command suggestion (flag/value) without clobbering the resolved command (group sub).
    ///
    /// Rules:
    /// - If cursor is at a new token position (ends with space), insert suggestion + trailing space.
    /// - If current token starts with '-' or previous token is a flag expecting a value → replace token.
    /// - Otherwise (we're on the command tokens or a positional token) → append suggestion separated by space.
    pub fn apply_accept_non_command_suggestion(&mut self, text: &str) {
        let at_new_token = self.input.ends_with(' ');
        let toks = lex_shell_like_ranged(&self.input);

        // New token position or empty input: just insert suggestion, but clean up stray '-'/'--'.
        if at_new_token || toks.is_empty() {
            // Avoid borrowing across mutation by computing range first
            let remove_from: Option<usize> = toks
                .last()
                .and_then(|t| (t.text == "-" || t.text == "--").then_some(t.start));
            if let Some(start) = remove_from {
                self.input.replace_range(start..self.input.len(), "");
                self.cursor_position = self.input.len();
            }
            self.insert_with_space(text);
            return;
        }

        // Identify the token under the cursor and its predecessor (if any)
        let token_index = token_index_at_cursor(&self.input, self.cursor_position).unwrap_or(toks.len() - 1);
        let (start, end) = (toks[token_index].start, toks[token_index].end);
        let current_token = self.input[start..end].to_string();
        let prev_token: Option<String> = token_index
            .checked_sub(1)
            .map(|i| (toks[i].start, toks[i].end))
            .map(|(s, e)| self.input[s..e].to_string());

        let prev_is_flag = prev_token.map(|t| t.starts_with("--")).unwrap_or(false);
        let inserting_is_flag = text.starts_with("--");

        // If previous token is a flag and user picked another flag, append instead of replacing the value.
        if prev_is_flag && !current_token.starts_with('-') && inserting_is_flag {
            self.cursor_position = self.input.len();
            self.insert_with_space(text);
            return;
        }

        // Replace flag token or its value, otherwise append to the end as a new token
        if current_token.starts_with("--") || prev_is_flag {
            self.input.replace_range(start..end, text);
            self.cursor_position = start + text.len();
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
    /// If a non-boolean flag value is pending and incomplete, only values (enums
    /// and provider-derived) are suggested for that flag.
    ///
    /// Arguments:
    /// - `st`: Mutable palette state; suggestions and ghost text are written here.
    /// - `reg`: Command registry providing command/flag/positional specs.
    /// - `providers`: Value providers consulted for flags and positional arguments.
    ///
    /// Returns: nothing; updates `st.suggestions`, `st.ghost`, and related fields.
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
    pub fn apply_build_suggestions(&mut self, reg: &Registry, providers: &[Box<dyn ValueProvider>]) {
        let input = &self.input;
        let tokens: Vec<String> = lex_shell_like(input);

        // No command yet (need group + sub) or unresolved -> suggest commands in execution format: "group sub"
        if !is_command_resolved(reg, &tokens) {
            let mut items = suggest_commands(reg, &compute_command_prefix(&tokens));
            self.finalize_suggestions(&mut items);
            return;
        }

        // Resolve command key from first two tokens: "group sub"
        let group = tokens.first().unwrap_or(&String::new()).to_owned();
        let name = tokens.get(1).unwrap_or(&String::new()).to_owned();
        let spec = match reg.commands.iter().find(|c| c.group == group && c.name == name) {
            Some(s) => s.clone(),
            None => {
                self.suggestions.clear();
                self.is_suggestions_open = false;
                return;
            }
        };

        // Build user flags and args from parts
        let parts: &[String] = if tokens.len() >= 2 { &tokens[2..] } else { &tokens[0..0] };
        let (user_flags, user_args) = parse_user_flags_args(&spec, parts);
        let current = parts.last().map(|s| s.as_str()).unwrap_or("");

        // Determine if expecting a flag value (last used flag without value)
        let pending_flag = find_pending_flag(&spec, parts, input);

        // Determine if current editing token looks like a flag
        let current_is_flag = current.starts_with('-');

        // 1) If a non-boolean flag value is pending and not complete, only suggest values for it
        if let Some(flag_name) = pending_flag.clone() {
            let value_partial = flag_value_partial(parts);
            let mut items = suggest_values_for_flag(&spec, &flag_name, &value_partial, providers);
            self.finalize_suggestions(&mut items);
        } else {
            // 2) Next expected item: positional arguments first
            let mut items: Vec<SuggestionItem> = if user_args.len() < spec.positional_args.len() && !current_is_flag {
                suggest_positionals(&spec, user_args.len(), current, providers)
            } else {
                Vec::new()
            };

            // 3) If no positional needed (or user explicitly typed a flag), suggest required flags
            if items.is_empty() {
                let required_remaining = required_flags_remaining(&spec, &user_flags);
                if required_remaining || current_is_flag {
                    items.extend(collect_flag_candidates(&spec, &user_flags, current, true));
                }
            }

            // 4) Optional flags when required are satisfied
            if items.is_empty() {
                items.extend(collect_flag_candidates(&spec, &user_flags, current, false));
            }

            // 5) If still empty and there are remaining positionals, offer placeholder for the next one
            if items.is_empty() && user_args.len() < spec.positional_args.len() {
                let pa = &spec.positional_args[user_args.len()];
                items.push(SuggestionItem {
                    display: format!(
                        "<{:<15}> [ARG] {}",
                        pa.name,
                        pa.help.as_deref().unwrap_or(&pa.name)
                    ),
                    insert_text: current.to_string(),
                    kind: ItemKind::Positional,
                    meta: pa.help.clone(),
                    score: 0,
                });
            }

            // 6) End of line hint for starting flags if any remain
            if items.is_empty()
                && let Some(hint) = self.eol_flag_hint(&spec, &user_flags)
            {
                items.push(hint);
            }

            self.finalize_suggestions(&mut items);
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
    fn suggest(&self, command_key: &str, field: &str, partial: &str) -> Vec<SuggestionItem>;
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
    /// Suggest values that fuzzy-match `partial` for the configured (command, field).
    fn suggest(&self, command_key: &str, field: &str, partial: &str) -> Vec<SuggestionItem> {
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

/// Determine if the first two tokens resolve to a known command.
///
/// A command is considered resolved when at least two tokens exist and they
/// match a `(group, name)` pair in the registry.
fn is_command_resolved(reg: &Registry, tokens: &[String]) -> bool {
    if tokens.len() < 2 {
        return false;
    }
    let (group, name) = (&tokens[0], &tokens[1]);
    reg.commands.iter().any(|c| &c.group == group && &c.name == name)
}

/// Compute the prefix used to rank command suggestions.
///
/// When two or more tokens exist, uses "group sub"; otherwise uses the first
/// token or empty string.
fn compute_command_prefix(tokens: &[String]) -> String {
    if tokens.len() >= 2 {
        format!("{} {}", tokens[0], tokens[1])
    } else {
        tokens.first().map(|s| s.as_str()).unwrap_or("").to_string()
    }
}

/// Build command suggestions in execution form ("group sub").
///
/// Uses `fuzzy_score` against the computed prefix to rank candidates and embeds
/// the command summary in the display text.
fn suggest_commands(reg: &Registry, prefix: &str) -> Vec<SuggestionItem> {
    let mut items = Vec::new();
    if prefix.is_empty() {
        return items;
    }

    for command in &*reg.commands {
        let group = &command.group;
        let name = &command.name;
        let exec = if name.is_empty() {
            group.to_string()
        } else {
            format!("{} {}", group, name)
        };
        if let Some(s) = fuzzy_score(&exec, prefix) {
            items.push(SuggestionItem {
                display: format!("{:<28} [CMD] {}", exec, command.summary),
                insert_text: exec,
                kind: ItemKind::Command,
                meta: None,
                score: s,
            });
        }
    }
    items
}

/// Parse user-provided flags and positional arguments from the portion of tokens
/// after the resolved (group, sub) command.
///
/// long flags are collected without the leading dashes; values immediately
/// following non-boolean flags are consumed. Returns `(user_flags, user_args)`.
fn parse_user_flags_args(spec: &CommandSpec, parts: &[String]) -> (Vec<String>, Vec<String>) {
    let mut user_flags: Vec<String> = Vec::new();
    let mut user_args: Vec<String> = Vec::new();
    let mut i = 0;
    while i < parts.len() {
        let t = parts[i].as_str();
        if t.starts_with("--") {
            let name = t.trim_start_matches('-');
            user_flags.push(name.to_string());
            if let Some(f) = spec.flags.iter().find(|f| f.name == name)
                && f.r#type != "boolean"
                && i + 1 < parts.len()
                && !parts[i + 1].starts_with('-')
            {
                i += 1; // consume value
            }
        } else if t.contains('=') && t.starts_with("--") {
            let name = t.split('=').next().unwrap_or("").trim_start_matches('-');
            user_flags.push(name.to_string());
        } else {
            user_args.push(t.to_string());
        }
        i += 1;
    }
    (user_flags, user_args)
}

/// Find the last pending non-boolean flag that expects a value.
///
/// Scans tokens from the end to find the most recent flag and checks whether
/// its value has been supplied. If a value is already complete (per
/// `is_flag_value_complete`), returns `None`.
fn find_pending_flag(spec: &CommandSpec, parts: &[String], input: &str) -> Option<String> {
    let mut j = (parts.len() as isize) - 1;
    while j >= 0 {
        let t = parts[j as usize].as_str();
        if t.starts_with("--") {
            let name = t.trim_start_matches('-');
            if let Some(f) = spec.flags.iter().find(|f| f.name == name)
                && f.r#type != "boolean"
            {
                // if the token after this flag is not a value, we are pending
                if ((j as usize) == parts.len() - 1 || parts[(j as usize) + 1].starts_with('-'))
                    && !is_flag_value_complete(input)
                {
                    return Some(name.to_string());
                }
            }
            break;
        }
        j -= 1;
    }
    None
}

/// Derive the value fragment currently being typed for the last flag.
///
/// If the last token is a flag containing an equals sign (e.g., `--app=pa`),
/// returns the suffix after `=`; otherwise returns the last token itself (or an
/// empty string when no tokens exist in `parts`).
fn flag_value_partial(parts: &[String]) -> String {
    if let Some(last) = parts.last() {
        let s = last.as_str();
        if s.starts_with("--") {
            if let Some(eq) = s.find('=') {
                return s[eq + 1..].to_string();
            }
            return String::new();
        }
        return s.to_string();
    }
    String::new()
}

/// Suggest values for a specific non-boolean flag, combining enum values with
/// provider-derived suggestions.
fn suggest_values_for_flag(
    spec: &CommandSpec,
    flag_name: &str,
    partial: &str,
    providers: &[Box<dyn ValueProvider>],
) -> Vec<SuggestionItem> {
    let mut items: Vec<SuggestionItem> = Vec::new();
    if let Some(f) = spec.flags.iter().find(|f| f.name == flag_name) {
        for value in &f.enum_values {
            if let Some(s) = fuzzy_score(value, partial) {
                items.push(SuggestionItem {
                    display: value.clone(),
                    insert_text: value.clone(),
                    kind: ItemKind::Value,
                    meta: Some("enum".into()),
                    score: s,
                });
            }
        }
    }
    for p in providers {
        let mut vals = p.suggest(&spec.name, flag_name, partial);
        items.append(&mut vals);
    }
    items
}

/// Suggest positional values for the next expected positional parameter using
/// providers; when no provider values are available, suggest a placeholder
/// formatted as `<name>`.
fn suggest_positionals(
    spec: &CommandSpec,
    arg_count: usize,
    current: &str,
    providers: &[Box<dyn ValueProvider>],
) -> Vec<SuggestionItem> {
    let mut items: Vec<SuggestionItem> = Vec::new();
    if let Some(pa) = spec.positional_args.get(arg_count) {
        for p in providers {
            let mut vals = p.suggest(&spec.name, &pa.name, current);
            items.append(&mut vals);
        }
        if items.is_empty() {
            items.push(SuggestionItem {
                display: format!(
                    "<{}> [ARG] {}",
                    pa.name,
                    pa.help.as_deref().unwrap_or(&pa.name)
                ),
                insert_text: format!("<{}>", pa.name),
                kind: ItemKind::Positional,
                meta: pa.help.clone(),
                score: 0,
            });
        }
    }
    items
}

/// Whether any required flags are not yet supplied by the user.
fn required_flags_remaining(spec: &CommandSpec, user_flags: &[String]) -> bool {
    spec.flags
        .iter()
        .any(|f| f.required && !user_flags.iter().any(|u| u == &f.name))
}

/// Determine whether the last flag's value is complete according to REPL rules.
///
/// Rules:
/// - If the last token is `-` or `--`, it is not complete.
/// - If no flag token is found when scanning backward, it is complete.
/// - If the last token is the flag itself (no value yet), it is not complete.
/// - If the last token is the value immediately after the flag, it is complete
///   only if the input ends in whitespace (typing may continue otherwise).
///
/// Arguments:
/// - `input`: The full input line.
///
/// Returns: `true` if the last flag value is considered complete.
///
/// Example:
///
/// ```rust,ignore
/// use heroku_tui::ui::components::palette::state::is_flag_value_complete;
///
/// assert!(!is_flag_value_complete("--app"));
/// assert!(!is_flag_value_complete("--app my"));
/// assert!(is_flag_value_complete("--app my "));
/// ```
pub fn is_flag_value_complete(input: &str) -> bool {
    // Preserve EOL whitespace semantics; only trim for tokenization via ranged lexer
    let tokens_r = lex_shell_like_ranged(input);
    let tokens: Vec<&str> = tokens_r.iter().map(|t| t.text).collect();
    let len = tokens.len();
    if len == 0 {
        return false;
    }
    let last = tokens[len - 1];
    if last == "-" || last == "--" {
        return false;
    }
    // find last flag index
    let mut last_flag_idx: isize = -1;
    for i in (0..len).rev() {
        if tokens[i].starts_with('-') {
            last_flag_idx = i as isize;
            break;
        }
    }
    if last_flag_idx == -1 {
        return true;
    }
    if last_flag_idx as usize == len - 1 {
        return false;
    }
    if last_flag_idx as usize == len - 2 {
        return input.ends_with(' ') || input.ends_with('\t') || input.ends_with('\n') || input.ends_with('\r');
    }
    true
}

/// Collect candidate flag suggestions for a command specification.
///
/// Generates suggestions for either required or optional flags that have not yet
/// been provided by the user. When `current` starts with a dash, only flags whose
/// long form starts with `current` are included (prefix filtering).
///
/// Arguments:
/// - `spec`: The command specification whose flags are considered.
/// - `user_flags`: Long flag names already present in the input (without `--`).
/// - `current`: The current token text (used for prefix filtering when typing a flag).
/// - `required_only`: When `true`, include only required flags; when `false`, only optional flags.
fn collect_flag_candidates(
    spec: &CommandSpec,
    user_flags: &[String],
    current: &str,
    required_only: bool,
) -> Vec<SuggestionItem> {
    let mut out: Vec<SuggestionItem> = Vec::new();
    for f in &spec.flags {
        if required_only && !f.required {
            continue;
        }
        if !required_only && f.required {
            continue;
        }
        if user_flags.iter().any(|u| u == &f.name) {
            continue;
        }
        let long = format!(
            "--{:<15} [FLAG] {}",
            f.name,
            f.description.as_ref().unwrap_or(&"".to_string())
        );
        let include = if current.starts_with('-') {
            long.starts_with(current)
        } else {
            true
        };
        if include {
            out.push(SuggestionItem {
                display: format!("{:<22}{}", long, if f.required { "  [required]" } else { "" }),
                insert_text: format!("--{}", f.name),
                kind: ItemKind::Flag,
                meta: f.description.clone(),
                score: 0,
            });
        }
    }
    out
}

/// Compute the remainder of the current token toward a target insert text.
///
/// If the token under the cursor is a prefix of `insert`, returns the suffix
/// that would be inserted to complete it. Used to render subtle ghost text to
/// the right of the cursor previewing acceptance of the top suggestion.
///
/// Arguments:
/// - `input`: Full input line.
/// - `cursor`: Cursor position (byte index) into `input`.
/// - `insert`: The prospective full text to insert for the current token.
///
/// Returns: The suffix of `insert` beyond the current token, or empty string.
///
/// Example:
///
/// ```rust,ignore
/// use heroku_tui::ui::components::palette::state::ghost_remainder;
///
/// assert_eq!(ghost_remainder("ap", 2, "apps"), "ps");
/// assert_eq!(ghost_remainder("foo", 3, "bar"), "");
/// ```
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
