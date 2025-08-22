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
use crate::theme;
use ratatui::{
    prelude::*,
    text::{Line, Span},
    widgets::*,
};

const MAX_SUGGESTIONS: usize = 20;

#[derive(Clone, Debug)]
pub enum ItemKind {
    Command,
    Flag,
    Value,
    Positional,
}

#[derive(Clone, Debug)]
pub struct SuggestionItem {
    pub display: String,
    pub insert_text: String,
    pub kind: ItemKind,
    pub meta: Option<String>,
    pub score: i64,
}

#[derive(Clone, Debug, Default)]
pub struct PaletteState {
    pub input: String,
    pub cursor: usize, // byte index
    pub ghost: Option<String>,
    pub popup_open: bool,
    pub selected: usize,
    pub suggestions: Vec<SuggestionItem>,
    pub error: Option<String>,
}

impl PaletteState {
    /// Move the cursor one character to the left.
    ///
    /// - No-op if the cursor is already at the start of the input.
    ///
    /// Returns: nothing; updates `self.cursor` in place.
    pub fn move_cursor_left(&mut self) {
        if self.cursor == 0 { return; }
        let prev_len = self.input[..self.cursor]
            .chars()
            .last()
            .map(|c| c.len_utf8())
            .unwrap_or(1);
        self.cursor = self.cursor.saturating_sub(prev_len);
    }
    /// Move the cursor one character to the right.
    ///
    /// - No-op if the cursor is already at the end of the input.
    ///
    /// Returns: nothing; updates `self.cursor` in place.
    pub fn move_cursor_right(&mut self) {
        if self.cursor >= self.input.len() { return; }
        // Advance by one Unicode scalar starting at current byte offset
        let mut iter = self.input[self.cursor..].chars();
        if let Some(next) = iter.next() {
            self.cursor = self.cursor.saturating_add(next.len_utf8());
        }
    }
    /// Insert a character at the cursor and advance.
    ///
    /// Arguments:
    /// - `c`: The character to insert. UTF-8 length is respected for cursor advance.
    ///
    /// Returns: nothing; mutates `self.input` and `self.cursor`.
    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }
    /// Remove the character immediately before the cursor.
    ///
    /// - No-op if the cursor is at the start of the input.
    /// - Handles multi-byte UTF-8 characters correctly.
    ///
    /// Returns: nothing; mutates `self.input` and `self.cursor`.
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let prev = self.input[..self.cursor]
            .chars()
            .last()
            .map(|c| c.len_utf8())
            .unwrap_or(1);
        let start = self.cursor - prev;
        self.input.drain(start..self.cursor);
        self.cursor = start;
    }
}

/// Render the palette UI components.
///
/// Renders the input line, optional ghost text, an error line, and the
/// suggestions popup. Cursor placement accounts for character width, not bytes.
/// The popup is hidden if a modal is open or there are no suggestions.
///
/// Arguments:
/// - `f`: Frame to render to.
/// - `area`: Area allocated for the palette.
/// - `app`: Application state providing palette, theme, and modal flags.
pub fn render_palette(f: &mut Frame, area: Rect, app: &crate::app::App) {
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(theme::border_style(true))
        .border_type(ratatui::widgets::block::BorderType::Thick);
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);
    let splits = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);

    // Input line with ghost text; dim when a modal is open. Show throbber if executing.
    let dimmed = app.show_builder || app.show_help;
    let base_style = if dimmed {
        theme::text_muted()
    } else {
        theme::text_style()
    };
    let mut spans: Vec<Span> = Vec::new();
    if app.executing {
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let sym = frames[app.throbber_idx % frames.len()];
        spans.push(Span::styled(
            format!("{} ", sym),
            theme::title_style().fg(theme::ACCENT),
        ));
    }
    spans.push(Span::styled(app.palette.input.as_str(), base_style));
    if let Some(ghost) = &app.palette.ghost {
        if !ghost.is_empty() {
            spans.push(Span::styled(ghost.as_str(), theme::text_muted()));
        }
    }
    let p = Paragraph::new(Line::from(spans)).block(Block::default());
    f.render_widget(p, splits[0]);
    // Cursor placement (count characters, not bytes); hide when a modal is open
    if !dimmed {
        let col = app
            .palette
            .input
            .get(..app.palette.cursor)
            .map(|s| s.chars().count() as u16)
            .unwrap_or(0);
        let x = splits[0].x.saturating_add(col);
        let y = splits[0].y;
        f.set_cursor_position((x, y));
    }

    // Error line below input when present
    if let Some(err) = &app.palette.error {
        let line = Line::from(vec![
            Span::styled("✖ ", Style::default().fg(theme::WARN)),
            Span::styled(err.as_str(), theme::text_style()),
        ]);
        f.render_widget(Paragraph::new(line), splits[1]);
    }

    // Popup suggestions (separate popup under the input; no overlap with input text). Hidden if error is present.
    if app.palette.error.is_none()
        && app.palette.popup_open
        && !app.show_builder
        && !app.show_help
        && !app.palette.suggestions.is_empty()
    {
        let items_all: Vec<ListItem> = app
            .palette
            .suggestions
            .iter()
            .map(|s| ListItem::new(s.display.clone()).style(theme::text_style()))
            .collect();
        let max_rows = 10usize;
        let rows = items_all.len().min(max_rows);
        if rows == 0 {
            // Do not render an empty popup list
            return;
        }
        // Compute a compact popup area just below the input line
        let popup_area = Rect {
            x: splits[0].x,
            y: splits[0].y.saturating_add(1),
            width: inner.width,
            height: rows as u16 + 2, // borders
        };
        f.render_widget(Clear, popup_area);
        let list = List::new(items_all.into_iter().take(rows).collect::<Vec<_>>())
            .block(
                Block::default()
                    .borders(Borders::LEFT)
                    .border_style(theme::border_style(false)),
            )
            .highlight_style(theme::list_highlight_style())
            .highlight_symbol("▸ ");
        let mut list_state = ratatui::widgets::ListState::default();
        let sel = if app.palette.suggestions.is_empty() {
            None
        } else {
            Some(app.palette.selected.min(app.palette.suggestions.len() - 1))
        };
        list_state.select(sel);
        f.render_stateful_widget(list, popup_area, &mut list_state);
    }
}

/// Simple subsequence fuzzy matcher with a naive scoring heuristic.
///
/// Returns `Some(score)` if all characters in `needle` appear in order within
/// `hay`, otherwise returns `None`. Higher scores indicate better matches. The
/// scoring favors consecutive matches, prefix matches, and shorter candidates.
///
/// Arguments:
/// - `hay`: The candidate string to search within.
/// - `needle`: The query to match as a subsequence.
///
/// Returns: `Option<i64>` where `Some(score)` indicates a match.
///
/// Example:
///
/// ```rust
/// assert!(fuzzy_score("applications", "app").unwrap() > 0);
/// assert!(fuzzy_score("applications", "axp").is_none());
/// ```
pub fn fuzzy_score(hay: &str, needle: &str) -> Option<i64> {
    if needle.is_empty() { return Some(0); }
    let mut h_lower = String::with_capacity(hay.len());
    for ch in hay.chars() { h_lower.extend(ch.to_lowercase()); }
    let mut n_lower = String::with_capacity(needle.len());
    for ch in needle.chars() { n_lower.extend(ch.to_lowercase()); }

    let h = h_lower.as_str();
    let n = n_lower.as_str();

    let mut hi = 0usize;
    let mut score: i64 = 0;
    let mut consec = 0i64;
    let mut first_match_idx: Option<usize> = None;
    for ch in n.chars() {
        if let Some(pos) = h[hi..].find(ch) {
            let abs = hi + pos;
            if first_match_idx.is_none() { first_match_idx = Some(abs); }
            hi = abs + ch.len_utf8();
            consec += 1;
            score += 6 * consec; // stronger reward for consecutive matches
        } else {
            return None;
        }
    }
    // Boost for prefix match
    if h.starts_with(n) { score += 30; }
    // Earlier start is better
    if let Some(start) = first_match_idx { score += i64::max(0, 20 - start as i64); }
    // Prefer shorter candidates when all else equal
    score -= hay.len() as i64 / 8;
    Some(score)
}
/// Tokenize input using a simple, shell-like lexer.
///
/// Supports single and double quotes and backslash escapes. Used by the
/// suggestion engine to derive tokens and assess completeness of flag values.
///
/// Arguments:
/// - `input`: The raw input line.
///
/// Returns: A vector of tokens preserving quoted segments.
///
/// Example:
///
/// ```rust
/// let toks = lex_shell_like("cmd --flag 'some value'");
/// assert_eq!(toks, vec!["cmd", "--flag", "'some value'"].iter().map(|s| s.to_string()).collect::<Vec<_>>());
/// ```
fn lex_shell_like(input: &str) -> Vec<String> {
    lex_shell_like_ranged(input).into_iter().map(|t| t.text.to_string()).collect()
}
/// Token with original byte positions.
struct LexTok<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

/// Tokenize input returning borrowed slices and byte ranges.
fn lex_shell_like_ranged(input: &str) -> Vec<LexTok<'_>> {
    let mut out: Vec<LexTok<'_>> = Vec::new();
    let mut i = 0usize;
    let bytes = input.as_bytes();
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let start = i;
        let mut in_sq = false;
        let mut in_dq = false;
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if b == b'\'' && !in_dq {
                in_sq = !in_sq;
                i += 1;
                continue;
            }
            if b == b'"' && !in_sq {
                in_dq = !in_dq;
                i += 1;
                continue;
            }
            if !in_sq && !in_dq && b.is_ascii_whitespace() {
                break;
            }
            i += 1;
        }
        out.push(LexTok { text: &input[start..i], start, end: i });
    }
    out
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
/// ```rust
/// assert!(!is_flag_value_complete("--app"));
/// assert!(!is_flag_value_complete("--app my"));
/// assert!(is_flag_value_complete("--app my "));
/// ```
fn is_flag_value_complete(input: &str) -> bool {
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
        return input.ends_with(' ')
            || input.ends_with('\t')
            || input.ends_with('\n')
            || input.ends_with('\r');
    }
    true
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
/// ```rust
/// let mut st = PaletteState::default();
/// st.input = "apps info --app ".into();
/// build_suggestions(&mut st, &reg, &providers);
/// assert!(!st.suggestions.is_empty());
/// ```
pub fn build_suggestions(
    st: &mut PaletteState,
    reg: &heroku_registry::Registry,
    providers: &[Box<dyn ValueProvider>],
) {
    let input = &st.input;
    let tokens: Vec<String> = lex_shell_like(input);

    let mut items: Vec<SuggestionItem> = Vec::new();
    // No command yet (need group + sub) or unresolved → suggest commands in execution format: "group sub"
    let resolved_key = if tokens.len() >= 2 {
        Some(format!("{}:{}", tokens[0], tokens[1]))
    } else {
        None
    };
    let resolved = resolved_key
        .as_ref()
        .and_then(|k| reg.commands.iter().find(|c| &c.name == k));
    if resolved.is_none() {
        let prefix = if tokens.len() >= 2 {
            format!("{} {}", tokens[0], tokens[1])
        } else {
            tokens.get(0).map(|s| s.as_str()).unwrap_or("").to_string()
        };
        for c in &reg.commands {
            let mut split = c.name.splitn(2, ':');
            let group = split.next().unwrap_or("");
            let rest = split.next().unwrap_or("");
            let exec = if rest.is_empty() {
                group.to_string()
            } else {
                format!("{} {}", group, rest)
            };
            if let Some(s) = fuzzy_score(&exec, &prefix) {
                items.push(SuggestionItem {
                    display: format!("{:<28} [CMD] {}", exec, c.summary),
                    insert_text: exec,
                    kind: ItemKind::Command,
                    meta: None,
                    score: s,
                });
            }
        }
        items.sort_by(|a, b| b.score.cmp(&a.score));
        if items.len() > MAX_SUGGESTIONS {
            items.truncate(MAX_SUGGESTIONS);
        }
        st.ghost = items
            .get(0)
            .map(|top| ghost_remainder(&st.input, st.cursor, &top.insert_text));
        st.suggestions = items;
        st.selected = st.selected.min(st.suggestions.len().saturating_sub(1));
        st.popup_open = !st.suggestions.is_empty();
        return;
    }

    // Resolve command key from first two tokens: "group sub"
    let cmd_key = format!(
        "{}:{}",
        tokens[0],
        tokens.get(1).map(|s| s.as_str()).unwrap_or("")
    );
    let spec = match reg.commands.iter().find(|c| c.name == cmd_key) {
        Some(s) => s.clone(),
        None => {
            st.suggestions.clear();
            st.popup_open = false;
            return;
        }
    };

    // Build user flags and args from parts
    let mut user_flags: Vec<String> = Vec::new();
    let mut user_args: Vec<String> = Vec::new();
    // parts after command = tokens after first two tokens (group + sub)
    let parts: &[String] = if tokens.len() >= 2 { &tokens[2..] } else { &tokens[0..0] };
    let mut i = 0;
    while i < parts.len() {
        let t = parts[i].as_str();
        if t.starts_with("--") {
            let name = t.trim_start_matches('-');
            user_flags.push(name.to_string());
            // skip value if present (non-dash and flag not boolean)
            if let Some(f) = spec.flags.iter().find(|f| f.name == name) {
                if f.r#type != "boolean" {
                    if i + 1 < parts.len() && !parts[i + 1].starts_with('-') {
                        i += 1; // consume value
                    }
                }
            }
        } else if t.contains('=') && t.starts_with("--") {
            let name = t.split('=').next().unwrap_or("").trim_start_matches('-');
            user_flags.push(name.to_string());
        } else {
            user_args.push(t.to_string());
        }
        i += 1;
    }
    let current = parts.last().map(|s| s.as_str()).unwrap_or("");

    // Helper: collect flag candidates without borrowing `items` across calls
    fn collect_flag_candidates(
        spec: &heroku_registry::CommandSpec,
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
            let long = format!("--{}", f.name);
            let include = if current.starts_with('-') {
                long.starts_with(current)
            } else {
                true
            };
            if include {
                out.push(SuggestionItem {
                    display: format!(
                        "{:<22}{}",
                        long,
                        if f.required { "  [required]" } else { "" }
                    ),
                    insert_text: long,
                    kind: ItemKind::Flag,
                    meta: f.description.clone(),
                    score: 0,
                });
            }
        }
        out
    }

    // Determine if expecting a flag value (last used flag without value)
    let mut pending_flag: Option<String> = None;
    // Scan from end to find last flag and check whether a value followed
    let mut j = (parts.len() as isize) - 1;
    while j >= 0 {
        let t = parts[j as usize].as_str();
        if t.starts_with("--") {
            let name = t.trim_start_matches('-');
            if let Some(f) = spec.flags.iter().find(|f| f.name == name) {
                if f.r#type != "boolean" {
                    // if the token after this flag is not a value, we are pending
                    if (j as usize) == parts.len() - 1 || parts[(j as usize) + 1].starts_with('-') {
                        pending_flag = Some(name.to_string());
                    }
                }
            }
            break;
        }
        j -= 1;
    }

    // If the last flag value is already complete (per repl.js rules), do not offer value suggestions
    if pending_flag.is_some() && is_flag_value_complete(input) {
        pending_flag = None;
    }

    // Determine if current editing token looks like a flag
    let current_is_flag = current.starts_with('-');

    // 1) If a non-boolean flag value is pending and not complete, only suggest values for it
    if let Some(flag_name) = pending_flag.clone() {
        // Determine partial value being typed for this flag
        let value_partial: &str = if let Some(last) = parts.last() {
            let s = last.as_str();
            if s.starts_with("--") {
                if let Some(eq) = s.find('=') { &s[eq + 1..] } else { "" }
            } else { s }
        } else { "" };
        // Suggest values for this flag: enums + providers
        if let Some(f) = spec.flags.iter().find(|f| f.name == flag_name) {
            for v in &f.enum_values {
                if let Some(s) = fuzzy_score(v, value_partial) {
                    items.push(SuggestionItem { display: v.clone(), insert_text: v.clone(), kind: ItemKind::Value, meta: Some("enum".into()), score: s });
                }
            }
        }
        for p in providers { let mut vals = p.suggest(&spec.name, &flag_name, value_partial); items.append(&mut vals); }
        // Do not suggest anything else until value is provided
        // Store and return at end
    } else {
        // 2) Next expected item: positional arguments first
        if user_args.len() < spec.positional_args.len() && !current_is_flag {
            let pos_name = &spec.positional_args[user_args.len()];
            for p in providers { let mut vals = p.suggest(&spec.name, pos_name, current); items.append(&mut vals); }
            if items.is_empty() {
                items.push(SuggestionItem { display: format!("<{}>", pos_name), insert_text: format!("<{}>", pos_name), kind: ItemKind::Positional, meta: spec.positional_help.get(pos_name).cloned(), score: 0 });
            }
        }

        // 3) If no positional needed (or user explicitly typed a flag), suggest required flags
        if items.is_empty() {
            let required_remaining = spec.flags.iter().any(|f| f.required && !user_flags.iter().any(|u| u == &f.name));
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
            let pos_name = &spec.positional_args[user_args.len()];
            items.push(SuggestionItem { display: format!("<{}>", pos_name), insert_text: current.to_string(), kind: ItemKind::Positional, meta: spec.positional_help.get(pos_name).cloned(), score: 0 });
        }

        // 6) End of line hint for starting flags if any remain
        if items.is_empty() {
            let total_flags = spec.flags.len();
            let used = user_flags.len();
            if used < total_flags {
                let hint = if st.input.ends_with(' ') { "--" } else { " --" };
                items.push(SuggestionItem { display: hint.to_string(), insert_text: hint.trim().to_string(), kind: ItemKind::Flag, meta: None, score: 0 });
            }
        }
    }

    // Rank and store
    items.sort_by(|a, b| b.score.cmp(&a.score));
    if items.len() > MAX_SUGGESTIONS {
        items.truncate(MAX_SUGGESTIONS);
    }
    st.ghost = items
        .get(0)
        .map(|top| ghost_remainder(&st.input, st.cursor, &top.insert_text));
    st.suggestions = items;
    st.selected = st.selected.min(st.suggestions.len().saturating_sub(1));
    st.popup_open = !st.suggestions.is_empty();
}

// ===== Provider hook (sync for now) =====
/// Value suggestion provider trait for flags and positional arguments.
pub trait ValueProvider: Send + Sync {
    /// command_key: e.g., "apps:info"; field: flag name without dashes (e.g., "app") or positional name
    fn suggest(&self, command_key: &str, field: &str, partial: &str) -> Vec<SuggestionItem>;
}

/// A simple static provider useful for demos or tests.
pub struct StaticValuesProvider {
    pub command_key: String,
    pub field: String,
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
/// ```rust
/// assert_eq!(ghost_remainder("ap", 2, "apps"), "ps");
/// assert_eq!(ghost_remainder("foo", 3, "bar"), "");
/// ```
fn ghost_remainder(input: &str, cursor: usize, insert: &str) -> String {
    let toks = lex_shell_like_ranged(input);
    // Find the token that contains the cursor, otherwise take the last token
    let last_tok = toks
        .iter()
        .find(|t| t.start <= cursor && cursor <= t.end)
        .or_else(|| toks.last());
    let token_text = match last_tok { Some(t) => t.text, None => "" };
    if insert.starts_with(token_text) {
        insert[token_text.len()..].to_string()
    } else {
        String::new()
    }
}
