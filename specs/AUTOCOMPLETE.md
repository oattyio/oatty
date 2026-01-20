# POWER_MODE_AUTOCOMPLETE.md

Design and prototype for a **power-mode command input** with **fuzzy autocomplete**, inspired by *rustyline* but **TUI-native** and **schema-aware**. This document includes:

1) The full UX/tech plan for the power-mode command input (from our discussion).  
2) A concrete **prototype module API** (Rust) with traits, structs, and a test harness sketch.  
3) **ANSI snapshots** of popup rendering states for implementers.

---

## 1) What we’re building

A single-line **command palette** that supports:

- **Fuzzy autocomplete** for commands / subcommands / flags / values  
- **Inline ghost-text** and a **popup suggestion list**  
- **History (persisted between sessions) & reverse-search** (Ctrl-R) à la *rustyline*  
- **Emacs/Vi keybindings**, word/char editing, kill/yank, transpose, etc.  
- **Schema-aware** (OpenAPI + MCP tools) + per-command **value providers**  
- **Low-latency** on thousands of entries inside a TUI frame

---

## 2) UI anatomy (ASCII)

```
┌ Command — Power Mode ───────────────────────────────────────────────────────────────────────────┐
│ :confg st  -a dem<TEXT CURSOR>                                                                  │
│ ghost: :config:set --app demo-app                                                               │
│                                                                                                 │
│  ▸ Suggestions (↑↓ move, Tab accept, Ctrl-Space open/close)                                     │
│    config:get               [CMD]  Get config var                                               │
│  > config:set               [CMD]  Set config var                                               │
│    ensure-config-bundle     [WF ]  Ensure keys; create missing                                  │
│    config                   [CMD]  List config vars                                             │
│                                                                                                 │
│ Hints: Tab complete • Ctrl-R history • Alt-/ cycle source • F1 full UI                          │
└─────────────────────────────────────────────────────────────────────────────────────────────────┘
```

- **Ghost text**: the rest of the likely completion in a dim color; accept by `→`, `Tab`, or `Ctrl-F`.  
- **Popup**: ranked suggestions with badges (CMD / WF / PLUG-IN).

---

## 3) Behavior & keybindings (rustyline-like)

- **Editing (Emacs)**: `Ctrl-A/E` home/end, `Ctrl-K/Y` kill/yank, `Alt-B/F` word back/forward, `Alt-D` kill word, `Ctrl-W` kill backward word.  
- **Vi mode** optional toggle.  
- **Completion**:  
  - `Tab`: accept top or cycle; `Shift-Tab` reverse  
  - `Ctrl-Space`: open/close popup without insert  
  - `→`: accept ghost text by word  
- **History**:  
  - `↑/↓`: browse recently executed commands (including persisted history when available)  
  - `Ctrl-R`: fuzzy reverse search overlay  
  - Entries are sanitized, stored per user, and reused across sessions.  
- **Mode**: `F1` to full guided UI; state preserved.

---

## 4) Parsing model (cursor-aware)

A small lexer yields **tokens** without full shell semantics:

- Handles quotes `"..."` and `'...'`, escaped spaces `\ `, `--` terminator, and `--flag=value` form.  
- Identifies **phase** at cursor:
  - `phase=Command`: completing the command key (e.g., `config:set`)  
  - `phase=FlagName`: completing `--app` / `-a`  
  - `phase=Value`: completing the value for a known flag or positional

No shell expansion or globbing—just robust tokenization for CLI grammar.

---

## 5) Completion sources (priority order)

1. **Command names** from the **Unified Registry** (core OpenAPI commands, workflows, MCP tools).  
2. **Flags / positionals** from the selected command’s schema (required first).  
3. **Values**:
   - **Static** enums/defaults from schema  
   - **Dynamic** providers (e.g., list apps, addons) with caching  
   - **MCP autocomplete** (`autocomplete(tool, field, partial)`) if the plugin supports it  
4. **History** for previously used values per flag/command. Entries persist per user (capped at 500), are filtered by the redaction heuristics, and surface as high-priority suggestions once the command is resolved.

All sources feed a **fuzzy scorer**; we merge, re-rank, and present.

---

## 6) Fuzzy matching & ranking

- Use `SkimMatcherV2` (from `fuzzy-matcher`) or an fzy-like scorer.  
- Score fields with weights:
  - Commands: `name` (1.0), `aliases` (0.9), `tags` (0.6), `summary` (0.4)  
  - Flags: `--long` (1.0), `-s` (0.9), description (0.3)  
  - Values: label (1.0), id (0.8), **recency bonus**  
- Tie-breakers: shorter edit distance; **required** > optional; recent usage boost.  
- Performance: pre-lowercased index, tiny LRU cache keyed by `(phase, token_prefix)`, recompute on idle (30–50ms debounce).

---

## 7) Data contracts (Rust shapes)

```rust
/// Commands in the unified registry (core + workflows + plugins)
#[derive(Clone, Debug)]
pub struct CommandEntry {
    pub key: String,            // "config:set", "apps:create", "sf-pg-tools:pg_promote"
    pub kind: CommandKind,      // Command | Workflow | Plugin
    pub summary: String,
    pub tags: Vec<String>,
    pub flags: Vec<FlagSpec>,
    pub positionals: Vec<PositionalSpec>,
    pub aliases: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
pub enum CommandKind { Command, Workflow, Plugin }

#[derive(Clone, Debug)]
pub struct FlagSpec {
    pub long: String,           // "--app"
    pub short: Option<char>,    // 'a'
    pub required: bool,
    pub multiple: bool,
    pub value_type: ValueType,  // String|Enum|Int|Bool|Json|Url|Email|Duration|Timestamp
    pub enum_vals: Option<Vec<String>>,
    pub provider: Option<ProviderId>, // e.g., "apps:list"
    pub help: String,
    pub negatable: bool,        // suggest --no-foo if true
}

#[derive(Clone, Debug)]
pub struct PositionalSpec {
    pub name: String,           // "app"
    pub required: bool,
    pub value_type: ValueType,
}

#[derive(Clone, Debug)]
pub enum ValueType { String, Enum, Int, Bool, Json, Url, Email, Duration, Timestamp }

#[derive(Clone, Debug)]
pub struct CompletionItem {
    pub display: String,        // shown in popup
    pub insert_text: String,    // inserted text
    pub kind: ItemKind,         // Command|Flag|Value|History
    pub meta: Option<String>,   // e.g., "[CMD] Get config var"
    pub score: i64,
    pub highlight_spans: Vec<(usize, usize)>,
}

#[derive(Clone, Copy, Debug)]
pub enum ItemKind { Command, Flag, Value, History }
```

---

## 8) Flow by phase

1) **Command**: fuzzy across `CommandEntry.key|aliases|tags|summary`. Ghost suggests top match; `Tab` accepts.  
2) **Flag/positional**: show **required** flags first, then optional; for required positional, show an angle-bracket hint `〈app〉`.  
3) **Value**: enums cycle with `→/←`; **providers** fetch async; fallback to history and typed heuristics.

---

## 9) Ghost-text & insertion rules

- Ghost shows only the **remainder** of current token (or the next obvious token).  
- `→` accepts one word (until space or `=`), not the entire suggestion.  
- If value needs quoting, ghost displays quoted suggestion.

---

## 10) History & reverse search

- Session and persistent history store **accepted** lines (successful runs).  
- Per-flag **value history** for common flags (e.g., `--app`).  
- `Ctrl-R` opens a minimal overlay:
  ```
  (reverse-search) api_url: _
  ```
  Fuzzy across commands + flags + values; accept pulls line back into editor.

---

## 11) Safety & validation

- **No secret suggestions**: filter values matching token/password patterns.  
- Mask known secrets in history.  
- **Schema validation** runs on submit; inline error line:
  ```
  ✖ Missing required: --app
  ```

---

## 12) Integration points

- **Unified Registry**: merges core (OpenAPI), workflows, and MCP tools.  
- **Value Providers**:
  - Built-ins: list apps, addons, regions, stacks.  
  - Plugins: MCP `autocomplete(tool, field, partial)` with timeout and cache.

---

## 13) Minimal implementation stack

- **Ratatui**: render line, ghost (dim), popup (List).  
- **Key handling**: crossterm events; editor state machine (Emacs/Vi).  
- **Fuzzy**: `fuzzy-matcher` (skim/fzy).  
- **Async**: `tokio` tasks for providers; result channels.  
- **Cache**: `lru` keyed by `(phase, token_prefix)`.  
- **Lexer**: tolerant tokenizer handling quotes, `--`, `=`.

---

## 14) Edge cases & polish

- After `--`, stop flag completion and treat remainder as positionals.  
- Suggest `--no-foo` if `negatable`.  
- Allow repeated flags where `multiple=true`.  
- Case-insensitive matching; preserve original case on insert.  
- Resize recomputes popup width & keeps cursor stable.

---

## 15) Success criteria

- **< 10ms** per keystroke on 5–10k entries.  
- **Top-1 ghost acceptance ≥ 70%** for common flows.  
- **No panics** on malformed input; degrade gracefully on provider timeouts.

---

## 16) Rollout

1. Commands only → 2. Flags/positionals → 3. Value providers → 4. Reverse history search → 5. Vi mode & advanced ops.

---

# 2) Prototype Module API (Rust)

Below are **traits, structs, and a small orchestrator** to implement the design cleanly.

```rust
// editor/api.rs
use std::time::Duration;
use async_trait::async_trait;

// ===== Lexer & Cursor Model =====

#[derive(Clone, Debug)]
pub struct LexedLine {
    pub tokens: Vec<Token>,
    pub cursor: Cursor,
    pub phase: Phase,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub text: String,
    pub span: (usize, usize), // byte offsets
    pub kind: TokenKind,      // Command | FlagName | Value | Terminator | Whitespace
}

#[derive(Clone, Copy, Debug)]
pub enum TokenKind { Command, FlagName, Value, Terminator, Whitespace }

#[derive(Clone, Copy, Debug)]
pub struct Cursor { pub byte: usize }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase { Command, FlagName, Value }

// Lexer API
pub trait Lexer {
    fn lex(&self, input: &str, cursor_byte: usize) -> LexedLine;
}

// ===== Completion Sources =====

#[async_trait]
pub trait CompletionSource: Send + Sync {
    /// context contains phase, parsed command (if any), the active token and prefix, etc.
    async fn complete(&self, ctx: &CompletionContext) -> anyhow::Result<Vec<CompletionItem>>;
    fn name(&self) -> &'static str;
}

#[derive(Clone, Debug)]
pub struct CompletionContext {
    pub phase: Phase,
    pub prefix: String,
    pub command: Option<CommandEntry>,
    pub active_flag: Option<FlagSpec>,
    pub tokens: Vec<Token>,
}

// ===== Registry & Providers =====

pub trait Registry: Send + Sync {
    fn commands(&self) -> &[CommandEntry];
    fn find_command(&self, key: &str) -> Option<CommandEntry>;
}

#[async_trait]
pub trait ValueProvider: Send + Sync {
    /// Provide value suggestions for a command's field.
    async fn suggest(
        &self,
        command_key: &str,
        field: &str,
        partial: &str
    ) -> anyhow::Result<Vec<CompletionItem>>;
}

// ===== Orchestrator =====

pub struct Completer {
    pub sources: Vec<Box<dyn CompletionSource>>,
    pub timeout: Duration,
}

impl Completer {
    pub async fn complete(&self, ctx: &CompletionContext) -> Vec<CompletionItem> {
        use futures::{stream, StreamExt};
        let futs = self.sources.iter().map(|s| s.complete(ctx));
        stream::iter(futs)
            .buffer_unordered(self.sources.len())
            .timeout(self.timeout)
            .filter_map(|res| async move { res.ok() })
            .flat_map(|items| futures::stream::iter(items))
            .collect::<Vec<_>>()
            .await
    }
}

// ===== Scoring & Ranking =====

pub trait Ranker {
    fn rank(&self, items: Vec<CompletionItem>, limit: usize) -> Vec<CompletionItem>;
}

// ===== Editor State (abbrev) =====

pub struct EditorState {
    pub input: String,
    pub cursor: usize,
    pub ghost: Option<String>,
    pub popup: Vec<CompletionItem>,
}
```

### Built-in completion sources (examples)

```rust
// sources/commands.rs
pub struct CommandSource<R: Registry> { pub reg: R }
#[async_trait]
impl<R: Registry + Send + Sync> CompletionSource for CommandSource<R> {
    fn name(&self) -> &'static str { "commands" }
    async fn complete(&self, ctx: &CompletionContext) -> anyhow::Result<Vec<CompletionItem>> {
        if ctx.phase != Phase::Command { return Ok(vec![]) }
        let pfx = ctx.prefix.to_lowercase();
        let mut out = Vec::new();
        for c in self.reg.commands() {
            // simple prefilter; real impl uses fuzzy
            if c.key.to_lowercase().contains(&pfx) ||
               c.aliases.iter().any(|a| a.contains(&pfx)) {
                out.push(CompletionItem {
                    display: format!("{:<24} [{}] {}", c.key, badge(&c.kind), c.summary),
                    insert_text: c.key.clone(),
                    kind: ItemKind::Command,
                    meta: None,
                    score: 0,
                    highlight_spans: vec![],
                });
            }
        }
        Ok(out)
    }
    fn name(&self) -> &'static str { "CommandSource" }
}

fn badge(k: &CommandKind) -> &'static str {
    match k { CommandKind::Command => "CMD",
             CommandKind::Workflow => "WF ",
             CommandKind::Plugin => "PLG" }
}
```

```rust
// sources/flags.rs
pub struct FlagSource<R: Registry> { pub reg: R }
#[async_trait]
impl<R: Registry + Send + Sync> CompletionSource for FlagSource<R> {
    fn name(&self) -> &'static str { "flags" }
    async fn complete(&self, ctx: &CompletionContext) -> anyhow::Result<Vec<CompletionItem>> {
        if ctx.phase != Phase::FlagName { return Ok(vec![]) }
        let Some(cmd) = &ctx.command else { return Ok(vec![]) };
        let pfx = ctx.prefix.to_lowercase();
        let mut items = Vec::new();
        for f in &cmd.flags {
            let long = f.long.to_lowercase();
            let short = f.short.map(|c| c.to_string()).unwrap_or_default();
            if long.contains(&pfx) || short.starts_with(&pfx) {
                items.push(CompletionItem {
                    display: format!("{:<16}  {}", f.long, f.help),
                    insert_text: f.long.clone(),
                    kind: ItemKind::Flag,
                    meta: None,
                    score: 0,
                    highlight_spans: vec![],
                });
            }
        }
        Ok(items)
    }
}
```

```rust
// sources/values.rs (uses dynamic providers + enums + history)
pub struct ValueSource<R: Registry> {
    pub reg: R,
    pub providers: Vec<Arc<dyn ValueProvider>>,
}

#[async_trait]
impl<R: Registry + Send + Sync> CompletionSource for ValueSource<R> {
    fn name(&self) -> &'static str { "values" }
    async fn complete(&self, ctx: &CompletionContext) -> anyhow::Result<Vec<CompletionItem>> {
        if ctx.phase != Phase::Value { return Ok(vec![]) }
        let Some(flag) = &ctx.active_flag else { return Ok(vec![]) };

        // 1) Enum values
        if let Some(enums) = &flag.enum_vals {
            let pfx = ctx.prefix.to_lowercase();
            let items = enums.iter().filter(|e| e.to_lowercase().contains(&pfx)).map(|e| CompletionItem {
                display: e.clone(),
                insert_text: e.clone(),
                kind: ItemKind::Value,
                meta: Some("enum".into()),
                score: 0,
                highlight_spans: vec![],
            }).collect();
            return Ok(items);
        }

        // 2) Providers (async)
        if let Some(pid) = &flag.provider {
            let mut out = Vec::new();
            for p in &self.providers {
                let mut vals = p.suggest(
                    ctx.command.as_ref().map(|c| c.key.as_str()).unwrap_or(""),
                    &flag.long,
                    &ctx.prefix
                ).await.unwrap_or_default();
                out.append(&mut vals);
            }
            return Ok(out);
        }

        Ok(vec![]) // 3) history source not shown here
    }
}
```

### Ranker example (skim)

```rust
pub struct SkimRanker;
impl Ranker for SkimRanker {
    fn rank(&self, mut items: Vec<CompletionItem>, limit: usize) -> Vec<CompletionItem> {
        // TODO: integrate skim scoring; here we just truncate
        items.truncate(limit);
        items
    }
}
```

### Test harness sketch

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;

    #[test]
    fn completes_command_names() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let reg = mock_registry(); // returns commands: config:get, config:set, apps:create…
            let cmd_src = CommandSource { reg: reg.clone() };
            let ctx = CompletionContext {
                phase: Phase::Command,
                prefix: "confg".into(),
                command: None,
                active_flag: None,
                tokens: vec![],
            };
            let items = cmd_src.complete(&ctx).await.unwrap();
            assert!(items.iter().any(|i| i.insert_text == "config:set"));
        });
    }
}
```

---

## 3) ANSI snapshots (popup rendering states)

Below: **fixed-width snapshots** for golden tests and visual baselines.

### A) Command phase — fuzzy suggestions (top match highlighted)

```
:confg st -a dem▮
ghost: :config:set --app demo-app

▸ Suggestions
  config:get               [CMD]  Get config var
> config:set               [CMD]  Set config var
  ensure-config-bundle     [WF ]  Ensure keys; create missing
  config                   [CMD]  List config vars
```

**Notes**:  
- Cursor `▮` indicates insertion point.  
- Top match is `config:set`; ghost completes `:config:set`.  
- Badges show type.

---

### B) Flag phase — required flags first, then optional

```
:config:set ▮
ghost: :config:set --app

▸ Suggestions
> --app                    (required)  The Oatty app
  --json                                Emit JSON (no table)
  --region                              App region
  -a                                    Short for --app
```

**Notes**:  
- `--app` prioritized as required.  
- Short aliases (`-a`) appear but rank lower than full `--app`.

---

### C) Value phase — enum values and provider values

```
:apps:create --region ▮

▸ Suggestions
> us
  eu
  tokyo
  oregon
```

**Notes**:  
- Static enums from schema.  
- If a **provider** supplies app names, show them with icon/meta:
  - `demo-app     (recent)`
  - `billing-svc  (team: infra)`

---

### D) Reverse history search (Ctrl-R)

```
(reverse-search) api_ ▮
  oatty config:set API_URL=https://ex.com -a demo
> oatty config:get API_URL -a demo
  oatty releases --app demo
```

**Notes**:  
- Accept pulls the selected line into the editor.  
- Fuzzy over commands and flag/value text.

---

## 4) Example Value Provider (apps list)

```rust
pub struct AppsProvider { client: ApiClient, cache: Arc<Mutex<Cache>> }

#[async_trait]
impl ValueProvider for AppsProvider {
    async fn suggest(&self, _cmd: &str, field: &str, partial: &str)
        -> anyhow::Result<Vec<CompletionItem>>
    {
        if field != "--app" && field != "-a" { return Ok(vec![]) }
        // cached fetch
        let list = self.cache.lock().unwrap().get_or_fetch("apps", || async {
            self.client.list_apps().await // Vec<App { name, owner }]
        }).await?;
        let pfx = partial.to_lowercase();
        Ok(list.into_iter()
            .filter(|a| a.name.to_lowercase().contains(&pfx))
            .map(|a| CompletionItem {
                display: format!("{:<24}  {}", a.name, a.owner_email),
                insert_text: a.name,
                kind: ItemKind::Value,
                meta: Some("app".into()),
                score: 0,
                highlight_spans: vec![],
            })
            .collect())
    }
}
```

---

## 5) Concurrency, caching, and timeouts

- **Per-keystroke budget**: keep recompute under ~10ms.  
- **Debounce** provider calls (e.g., 120–200ms idle).  
- **LRU caches**:  
  - Index cache for `(phase, prefix)` → top N items  
  - Provider cache (apps/addons) with TTL (30–120s)  
- **Timeouts**: provider suggest calls time out (e.g., 400ms). Show partial results + spinner glyph `…`.

---

## 6) Error handling & telemetry

- Provider errors don’t bubble to the UI—log to debug channel; show “provider unavailable” badge on value row.  
- Track acceptance actions (ghost accept vs popup select) to measure **Top-1 accuracy** and tune ranking.

---

## 7) Theming & accessibility

- Ghost text uses dim + italics; ensure sufficient contrast.  
- Matched spans in popup **bold/underline** rather than color only.  
- Respect global theme (light/dark) and monochrome fallback.

---


## 8) Persistence & storage

- Command palette and workflow history entries are stored in `~/.config/oatty/history.json`.  
- Entries are scoped by profile (`default_profile` until authentication is wired) and capped
  at **500**; oldest records are pruned automatically.  
- Commands only persist after a successful execution. Cancelled or failed runs leave the previous
  history intact.  
- Every value goes through the shared redaction heuristics before being written to disk.  

## 9) Security

- **Never** store or suggest values that look like secrets (`*_TOKEN`, `*_KEY`, URLs with creds).  
- Redact such tokens from history; replace with `•••••`.

---

## 10) Done definition

- Commands, flags, and enum values completed with fuzzy;  
- Async providers with cache + timeouts;  
- Ghost text, popup, Emacs keys;  
- Reverse history search;  
- Passing golden ANSI snapshots.

## 11) Source Alignment

- **Palette UI** is implemented by `PaletteComponent` (`crates/tui/src/ui/components/palette/palette_component.rs`), which wires key handling, ghost text rendering, and suggestion popups exactly as outlined here.
- **State management** (input buffer, cursor, ghost text, provider loading) lives in `crates/tui/src/ui/components/palette/state.rs`; history persistence flows through `PaletteState::push_history_if_needed` and the shared config directory.
- **Suggestion orchestration** uses `crates/tui/src/ui/components/palette/suggestion_engine.rs`, which matches the lexer/phase model and ranking contract described in sections 4–8.
- **Provider integration** leverages the runtime `ProviderRegistry` from `crates/registry/src/provider.rs`, so palette value suggestions share caching and bindings with the workflow collector.
- **Text editing primitives** reuse `crates/tui/src/ui/components/common/text_input.rs`, ensuring Emacs-style navigation and Vi-mode toggles stay in sync with other inline editors.

---

**Appendix: Minimal Milestones**

- **M1**: Command name fuzzy + ghost + popup  
- **M2**: Flags/positionals ranking + schema validation  
- **M3**: Values (enums + providers), caches  
- **M4**: Ctrl-R history, per-flag value history  
- **M5**: Vi mode, polish, telemetry
