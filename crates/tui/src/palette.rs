use crate::theme;
use ratatui::{prelude::*, widgets::*};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase { Command, FlagName, Value }

#[derive(Clone, Debug)]
pub enum ItemKind { Command, Flag, Value, Positional }

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

pub struct ParseCtx {
    pub phase: Phase,
    pub prefix: String,
    pub cmd_key: Option<String>,
    pub active_flag: Option<String>,
    pub active_positional: Option<String>,
}

impl PaletteState {
    pub fn move_cursor_left(&mut self) { if self.cursor > 0 { self.cursor -= 1; } }
    pub fn move_cursor_right(&mut self) { if self.cursor < self.input.len() { self.cursor += 1; } }
    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }
    pub fn backspace(&mut self) {
        if self.cursor == 0 { return; }
        let prev = self.input[..self.cursor].chars().last().map(|c| c.len_utf8()).unwrap_or(1);
        let start = self.cursor - prev;
        self.input.drain(start..self.cursor);
        self.cursor = start;
    }
}

pub fn parse_line(input: &str, cursor: usize, reg: &heroku_registry::Registry) -> ParseCtx {
    // Minimal lexer: split by whitespace, handle --flag=value, track token under cursor
    #[derive(Clone)]
    struct Tok { text: String, start: usize, end: usize }
    let mut toks: Vec<Tok> = Vec::new();
    let mut i = 0; let bytes = input.as_bytes();
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1; }
        if i >= bytes.len() { break; }
        let start = i;
        let mut in_sq = false; let mut in_dq = false;
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'\\' && i + 1 < bytes.len() { i += 2; continue; }
            if b == b'\'' && !in_dq { in_sq = !in_sq; i += 1; continue; }
            if b == b'"' && !in_sq { in_dq = !in_dq; i += 1; continue; }
            if !in_sq && !in_dq && b.is_ascii_whitespace() { break; }
            i += 1;
        }
        let end = i;
        toks.push(Tok { text: input[start..end].to_string(), start, end });
    }
    let mut phase = Phase::Command;
    let mut prefix = String::new();
    let mut cmd_key: Option<String> = None;
    let mut active_flag: Option<String> = None;
    let mut active_positional: Option<String> = None;

    // Identify token under cursor
    let under = toks.iter().find(|t| t.start <= cursor && cursor <= t.end);
    if toks.is_empty() || under.is_none() {
        return ParseCtx { phase, prefix, cmd_key, active_flag, active_positional };
    }
    let under = under.unwrap().clone();
    let idx = toks.iter().position(|t| t.start == under.start).unwrap_or(0);
    // Determine command key if first token matches exactly a known command
    if !toks.is_empty() {
        let cand = toks[0].text.clone();
        if reg.commands.iter().any(|c| c.name == cand) {
            cmd_key = Some(cand);
        }
    }

    // Determine phase and prefix
    if idx == 0 {
        phase = Phase::Command;
        prefix = under.text.clone();
    } else {
        // Look for flag=value
        if under.text.starts_with("--") {
            if under.text.contains('=') {
                phase = Phase::Value;
                let parts: Vec<&str> = under.text.splitn(2, '=').collect();
                active_flag = Some(parts[0].to_string());
                prefix = parts.get(1).unwrap_or(&"").to_string();
            } else {
                phase = Phase::FlagName;
                prefix = under.text.trim_start_matches('-').to_string();
            }
        } else {
            // If previous token is a flag expecting a value, treat as value phase
            if idx > 0 && toks[idx-1].text.starts_with("--") && !toks[idx-1].text.contains('=') {
                phase = Phase::Value;
                active_flag = Some(toks[idx-1].text.clone());
                prefix = under.text.clone();
            } else {
                // If first token is a known command and this token appears to be a positional,
                // enter Value phase so we DO NOT show command/flag suggestions.
                if let Some(cmd_key) = &cmd_key {
                    if let Some(cmd) = reg.commands.iter().find(|c| &c.name == cmd_key) {
                        // Count non-flag tokens between token 1..=idx to get positional index
                        let mut pos_count = 0usize;
                        for j in 1..=idx {
                            let t = &toks[j];
                            if !t.text.starts_with("--") && !(j > 0 && toks[j-1].text.starts_with("--") && !toks[j-1].text.contains('=')) {
                                pos_count += 1;
                            }
                        }
                        if pos_count > 0 && pos_count <= cmd.positional_args.len() {
                            phase = Phase::Value; // positional value entry
                            prefix = under.text.clone();
                            active_positional = Some(cmd.positional_args[pos_count - 1].clone());
                        } else {
                            phase = Phase::FlagName;
                            prefix = under.text.clone();
                        }
                    } else {
                        phase = Phase::FlagName;
                        prefix = under.text.clone();
                    }
                } else {
                    // No command recognized yet
                    phase = Phase::FlagName;
                    prefix = under.text.clone();
                }
            }
        }
    }

    ParseCtx { phase, prefix, cmd_key, active_flag, active_positional }
}

pub fn render_palette(f: &mut Frame, area: Rect, app: &crate::app::App) {
    use ratatui::text::{Line, Span};
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(theme::border_style(true))
        .border_type(ratatui::widgets::block::BorderType::Thick);
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);
    let splits = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    // Input line with ghost text; dim when a modal is open. Show throbber if executing.
    let dimmed = app.show_builder || app.show_help;
    let base_style = if dimmed { theme::text_muted() } else { theme::text_style() };
    let mut spans: Vec<Span> = Vec::new();
    if app.executing {
        let frames = ["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"];
        let sym = frames[app.throbber_idx % frames.len()];
        spans.push(Span::styled(format!("{} ", sym), theme::title_style().fg(theme::ACCENT)));
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
        f.set_cursor(x, y);
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
    if app.palette.error.is_none() && app.palette.popup_open && !app.show_builder && !app.show_help {
        let items_all: Vec<ListItem> = app
            .palette
            .suggestions
            .iter()
            .map(|s| ListItem::new(s.display.clone()).style(theme::text_style()))
            .collect();
        let max_rows = 8usize;
        let rows = items_all.len().min(max_rows);
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
        let sel = if app.palette.suggestions.is_empty() { None } else { Some(app.palette.selected.min(app.palette.suggestions.len()-1)) };
        list_state.select(sel);
        f.render_stateful_widget(list, popup_area, &mut list_state);
    }
}

// Simple subsequence fuzzy matcher with a naive score
pub fn fuzzy_score(hay: &str, needle: &str) -> Option<i64> {
    if needle.is_empty() { return Some(0); }
    let h = hay.to_lowercase();
    let n = needle.to_lowercase();
    let mut hi = 0usize; let mut score: i64 = 0; let mut consec = 0i64;
    for ch in n.chars() {
        if let Some(pos) = h[hi..].find(ch) {
            hi += pos + ch.len_utf8();
            consec += 1; score += 5 * consec; // reward consecutive matches
        } else { return None; }
    }
    // bonus for prefix and shorter
    if h.starts_with(&n) { score += 20; }
    score -= hay.len() as i64 / 10;
    Some(score)
}

pub fn build_suggestions(
    st: &mut PaletteState,
    reg: &heroku_registry::Registry,
    providers: &[Box<dyn ValueProvider>],
) {
    // Tokenize the input up to cursor
    let input = &st.input;
    let tokens: Vec<&str> = input.split_whitespace().collect();

    let mut items: Vec<SuggestionItem> = Vec::new();
    // No command yet (need group + sub) or unresolved → suggest commands in execution format: "group sub"
    let resolved_key = if tokens.len() >= 2 { Some(format!("{}:{}", tokens[0], tokens[1])) } else { None };
    let resolved = resolved_key.as_ref().and_then(|k| reg.commands.iter().find(|c| &c.name == k));
    if resolved.is_none() {
        let prefix = if tokens.len() >= 2 { format!("{} {}", tokens[0], tokens[1]) } else { tokens.get(0).copied().unwrap_or("").to_string() };
        for c in &reg.commands {
            let mut split = c.name.splitn(2, ':');
            let group = split.next().unwrap_or("");
            let rest = split.next().unwrap_or("");
            let exec = if rest.is_empty() { group.to_string() } else { format!("{} {}", group, rest) };
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
        if items.len() > 20 { items.truncate(20); }
        st.ghost = items.get(0).map(|top| ghost_remainder(&st.input, st.cursor, &top.insert_text));
        st.suggestions = items;
        st.selected = st.selected.min(st.suggestions.len().saturating_sub(1));
        st.popup_open = !st.suggestions.is_empty();
        return;
    }

    // Resolve command key from first two tokens: "group sub"
    let cmd_key = format!("{}:{}", tokens[0], tokens.get(1).copied().unwrap_or(""));
    let spec = match reg.commands.iter().find(|c| c.name == cmd_key) { Some(s) => s.clone(), None => { st.suggestions.clear(); st.popup_open = false; return; } };

    // Build user flags and args from parts
    let mut user_flags: Vec<String> = Vec::new();
    let mut user_args: Vec<String> = Vec::new();
    // parts after command = tokens after first two tokens (group + sub)
    let parts: &[&str] = if tokens.len() >= 2 { &tokens[2..] } else { &tokens[0..0] };
    let mut i = 0;
    while i < parts.len() {
        let t = parts[i];
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
    let current = parts.last().copied().unwrap_or("");

    // Helper: collect flag candidates without borrowing `items` across calls
    fn collect_flag_candidates(
        spec: &heroku_registry::CommandSpec,
        user_flags: &[String],
        current: &str,
        required_only: bool,
    ) -> Vec<SuggestionItem> {
        let mut out: Vec<SuggestionItem> = Vec::new();
        for f in &spec.flags {
            if required_only && !f.required { continue; }
            if !required_only && f.required { continue; }
            if user_flags.iter().any(|u| u == &f.name) { continue; }
            let long = format!("--{}", f.name);
            let include = if current.starts_with('-') { long.starts_with(current) } else { true };
            if include {
                out.push(SuggestionItem {
                    display: format!("{:<22}{}", long, if f.required { "  [required]" } else { "" }),
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
        let t = parts[j as usize];
        if t.starts_with("--") {
            let name = t.trim_start_matches('-');
            if let Some(f) = spec.flags.iter().find(|f| f.name == name) {
                if f.r#type != "boolean" {
                    // if the token after this flag is not a value, we are pending
                    if (j as usize) == parts.len() - 1 || parts[(j as usize)+1].starts_with('-') {
                        pending_flag = Some(name.to_string());
                    }
                }
            }
            break;
        }
        j -= 1;
    }

    // 1) Required flags (names) or their values
    if let Some(flag_name) = pending_flag.clone() {
        // Suggest values for this flag: enums + providers
        if let Some(f) = spec.flags.iter().find(|f| f.name == flag_name) {
            for v in &f.enum_values {
                if let Some(s) = fuzzy_score(v, current) {
                    items.push(SuggestionItem { display: v.clone(), insert_text: v.clone(), kind: ItemKind::Value, meta: Some("enum".into()), score: s });
                }
            }
        }
        for p in providers {
            let mut vals = p.suggest(&spec.name, &flag_name, current);
            items.append(&mut vals);
        }
    } else {
        // Not entering a flag value; show next required flags
        let required_needed: Vec<_> = spec.flags.iter().filter(|f| f.required && !user_flags.iter().any(|u| u == &f.name)).collect();
        if !required_needed.is_empty() { items.extend(collect_flag_candidates(&spec, &user_flags, current, true)); }
    }

    // If no items yet, 2) Required args (positionals)
    if items.is_empty() {
        if user_args.len() < spec.positional_args.len() {
            let pos_name = &spec.positional_args[user_args.len()];
            // Provider suggestions
            for p in providers {
                let mut vals = p.suggest(&spec.name, pos_name, current);
                items.append(&mut vals);
            }
            if items.is_empty() {
                // Show placeholder
                items.push(SuggestionItem { display: format!("<{}>", pos_name), insert_text: current.to_string(), kind: ItemKind::Positional, meta: spec.positional_help.get(pos_name).cloned(), score: 0 });
            }
        }
    }

    // If still empty, 3) Optional flags
    if items.is_empty() { items.extend(collect_flag_candidates(&spec, &user_flags, current, false)); }

    // If still empty, 4) Optional args — placeholder only
    if items.is_empty() {
        if user_args.len() >= spec.positional_args.len() {
            // nothing else
        } else {
            let pos_name = &spec.positional_args[user_args.len()];
            items.push(SuggestionItem { display: format!("<{}>", pos_name), insert_text: current.to_string(), kind: ItemKind::Positional, meta: spec.positional_help.get(pos_name).cloned(), score: 0 });
        }
    }

    // 5) End of line flag hint
    if items.is_empty() {
        let total_flags = spec.flags.len();
        let used = user_flags.len();
        if used < total_flags {
            // Offer a leading dashes hint
            let hint = if st.input.ends_with(' ') { "--" } else { " --" };
            items.push(SuggestionItem { display: hint.to_string(), insert_text: hint.trim().to_string(), kind: ItemKind::Flag, meta: None, score: 0 });
        }
    }

    // Rank and store
    items.sort_by(|a, b| b.score.cmp(&a.score));
    if items.len() > 20 { items.truncate(20); }
    st.ghost = items.get(0).map(|top| ghost_remainder(&st.input, st.cursor, &top.insert_text));
    st.suggestions = items;
    st.selected = st.selected.min(st.suggestions.len().saturating_sub(1));
    st.popup_open = !st.suggestions.is_empty();
}

// ===== Provider hook (sync for now) =====
pub trait ValueProvider: Send + Sync {
    /// command_key: e.g., "apps:info"; field: flag name without dashes (e.g., "app") or positional name
    fn suggest(&self, command_key: &str, field: &str, partial: &str) -> Vec<SuggestionItem>;
}

/// Simple static provider useful for demos or tests.
pub struct StaticValuesProvider {
    pub command_key: String,
    pub field: String,
    pub values: Vec<String>,
}

impl ValueProvider for StaticValuesProvider {
    fn suggest(&self, command_key: &str, field: &str, partial: &str) -> Vec<SuggestionItem> {
        if command_key != self.command_key || field != self.field { return vec![]; }
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

fn ghost_remainder(input: &str, cursor: usize, insert: &str) -> String {
    // Show remainder of the current token towards insert text when it shares a prefix
    // Simple: if at Command phase and insert starts with current token, append the remainder as ghost
    let cur = &input[..cursor];
    let last = cur.split_whitespace().last().unwrap_or("");
    if insert.starts_with(last) { insert[last.len()..].to_string() } else { String::new() }
}
