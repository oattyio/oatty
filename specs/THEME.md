# THEME.md

You are implementing the Heroku TUI using opinionated themes. The default is **Dracula**, with optional **Nord** and **Cyberpunk** palettes. Follow these rules exactly to ensure a cohesive, accessible, and professional look across themes.

---

## 1) Palette Assignment (authoritative)

Based on official Dracula (https://spec.draculatheme.com)

**Core**
- BG_MAIN:        `#282a36`  ← app/root background
- BG_PANEL:       `#282a36`  ← secondary panels/cards/inputs
- UI_BORDER:      `#44475a`  ← borders/dividers/scrollbars; also Current Line
- TEXT_MUTED:     `#6272a4`  ← ghost text, hints, placeholders (Comment)

**Foreground**
- TEXT_PRIMARY:   `#f8f8f2`  ← default text (Foreground)
- TEXT_SECONDARY: `#6272a4`  ← titles/headers/labels (Comment)
- TEXT_SELECTED:  `#f8f8f2`  ← highlighted text

**Accents**
- ACCENT_PRIMARY:   `#ff79c6` (Pink)  ← interactive elements/prompts
- ACCENT_SECONDARY: `#8be9fd` (Cyan)  ← focus, progress, keywords
- ACCENT_SUBTLE:    `#6272a4` (Comment) ← subtle accents

**Status (semantic)**
- STATUS_ERROR:   `#ff5555`
- STATUS_WARN:    `#ffb86c`  ← warnings/modified
- STATUS_OK:      `#50fa7b`
- STATUS_INFO:    `#8be9fd`

---

## 2) Global Styling Rules

| Color Name | Hex Code   | Typical TUI Use Case                                    | Example Applications                     | Recommended Widget Use (Tables & Panels)                     |
|------------|------------|---------------------------------------------------------|------------------------------------------|-------------------------------------------------------------|
| Background | #282A36    | Main canvas/background for terminals and TUIs           | Terminal background, htop background     | Table/panel background for consistent dark base             |
| Foreground | #F8F8F2    | Primary text (commands, file names, main content)       | Neovim text, lazygit commit messages     | Table cell text, panel content text for readability         |
| Current Line | #44475A  | Highlighting selected lines or items                    | Neovim cursor line, htop selected process| Selected table row/column or active panel background        |
| Comment    | #6272A4    | Secondary text (logs, comments, non-interactive labels) | Neovim comments, ranger file metadata    | Table headers or panel labels for non-interactive text      |
| Cyan       | #8BE9FD    | Keywords, progress bars, status indicators              | htop CPU bars, Neovim keywords           | Table borders or panel accents for active/focused elements  |
| Green      | #50FA7B    | Success states, additions, active indicators            | lazygit Git additions, htop memory bars  | Table row highlights for positive states (e.g., completed)  |
| Orange     | #FFB86C    | Warnings, modified states, secondary highlights         | lazygit staged changes, Neovim functions | Panel borders for warning states or modified table rows     |
| Pink       | #FF79C6    | Interactive elements (buttons, prompts, selections)     | lazygit branch selection, fzf prompts    | Table selection highlights or panel borders for interactive elements |
| Purple     | #BD93F9    | Navigation cues, constants, special keywords            | ranger directories, Neovim constants     | Table column separators or panel titles for navigation cues |
| Red        | #FF5555    | Errors, deletions, critical alerts                      | htop high CPU alerts, lazygit deletions  | Table row highlights for errors or panel error indicators   |
| Yellow     | #F1FA8C    | Search results, important notifications, active cursors | fzf search matches, Neovim search highlight | Table cell highlights for search results or panel alerts    |


- **Backgrounds**
  - App/root bg = BG_MAIN.
  - Panels (Search, Results, Details, Logs) = BG_PANEL.
  - Borders/dividers/scrollbars = UI_BORDER.

- **Text**
  - Default = TEXT_PRIMARY.
  - Headers/section titles = TEXT_SECONDARY (bold).
  - Muted/ghost/hints/placeholders = TEXT_MUTED (dim).

- **Selection**
  - Selected row: fg = TEXT_SELECTED; bg = UI_BORDER (Current Line).
  - Focus/borders = ACCENT_SECONDARY (cyan); interactive emphasis uses ACCENT_PRIMARY (pink).

- **Highlights**
  - Search/fuzzy spans: Yellow `#f1fa8c` (bold/underline) for high contrast.
  - IDs/Request IDs: ACCENT_PRIMARY (bold) or ACCENT_SUBTLE; ellipsize middle (e.g., `1d2c…9a7b`).
  - Timestamps: ACCENT_SECONDARY (cyan; subtle).

- **Status/Badges**
  - Success ✓ = STATUS_OK.
  - Warning ! = STATUS_WARN (Orange).
  - Error ✖ = STATUS_ERROR.
  - Info ℹ = STATUS_INFO (Cyan).
  - Running/Progress … = STATUS_WARN (e.g., spinner while pending).

- **Secrets**
  - Masked by default with bullets `•••••` in TEXT_MUTED.
  - When revealed, switch to TEXT_PRIMARY but display a warning toast.

---

## 3) Component-Specific Guidance

**Search/Command Input**
- Background: BG_PANEL; border: UI_BORDER.
- Text: TEXT_PRIMARY; caret: TEXT_SELECTED.
- Ghost text: TEXT_MUTED (dim).
 - Focus underline: ACCENT_PRIMARY (1px/line).

**Suggestions Popup**
- Item text: TEXT_PRIMARY.
 - Matched spans: ACCENT_PRIMARY (bold+underline).
- Type badges: `[CMD]` `[WF]` `[PLG]`
  - CMD badge = ACCENT_SECONDARY outline
  - WF badge  = ACCENT_PRIMARY outline
  - PLG badge = ACCENT_PRIMARY outline
- Hover/selected row bg: UI_BORDER; fg: TEXT_SELECTED.

**Tables**
- Header row: TEXT_SECONDARY (bold); apply header bg via Row style (surface_muted) to avoid gaps.
- Body rows: TEXT_PRIMARY; zebra by darkening `surface` and `surface_muted` (no DIM modifiers).
- Truncation ellipsis, no wrap by default.
- Sorting arrow uses ACCENT_SECONDARY.
- State chips use status colors (OK/WARN/ERROR/INFO).
- Hidden-columns chip uses ACCENT_SECONDARY.

**Workflow Steps**
- Icons: ✓ STATUS_OK, ✖ STATUS_ERROR, … STATUS_WARN, ◻ neutral (TEXT_MUTED).
- Expanded preview borders: UI_BORDER; titles TEXT_SECONDARY.
 - Dep/phase labels: ACCENT_SECONDARY.

**Logs**
- Default text: TEXT_PRIMARY.
- Timestamps: ACCENT_SECONDARY (cyan; subtle).
- Request IDs & short SHAs: ACCENT_PRIMARY (bold) or ACCENT_SUBTLE.
 - Status inline tags: use status mapping (OK/WARN/ERROR/INFO).
- Copy/toast messages:
  - Success toast bg: STATUS_OK; fg: BG_MAIN.
  - Failure toast bg: STATUS_ERROR; fg: TEXT_SELECTED.

**Toasts/Modals**
- Modal bg: BG_PANEL; border: UI_BORDER; title: TEXT_SECONDARY.
- Toast bg: darkened BG_PANEL; accent by status color.
- When any modal is open: dim entire view (DIM modifier) and draw a darkened backdrop from BG_MAIN.

---

## 4) Accessibility & Fallbacks

- Never rely on color alone: pair color with **icons/symbols** (`✓ ✖ ! …`) and variations (bold/underline).
- Ensure contrast ratio is comfortable (Dracula uses high contrast—prefer clean emphasis, avoid overuse of DIM).
- Provide a **monochrome mode**: drop color, keep emphasis via bold/underline and symbols.

---

## 5) Do / Don’t

**Do**
- Use accent colors for **non-semantic guidance** (matches, focus, timestamps).
- Use status colors **only** for semantic statuses.
- Keep the palette restrained—Dracula is about high-contrast clarity, not neon everywhere.

**Don’t**
- Mix multiple status colors in the same element.
- Use bright accents for large backgrounds.
- Overuse dim text; reserve for hints/ghost/secondary.

---

## 6) Implementation (Ratatui / Rust)

Define constants once and import everywhere.

```rust
use ratatui::style::Color;

pub mod dracula {
    pub const BG_MAIN:        Color = Color::Rgb(0x28,0x2A,0x36);
    pub const BG_PANEL:       Color = Color::Rgb(0x28,0x2A,0x36);
    pub const UI_BORDER:      Color = Color::Rgb(0x44,0x47,0x5A);
    pub const TEXT_MUTED:     Color = Color::Rgb(0x62,0x72,0xA4);

    pub const TEXT_PRIMARY:   Color = Color::Rgb(0xF8,0xF8,0xF2);
    pub const TEXT_SECONDARY: Color = Color::Rgb(0x62,0x72,0xA4);
    pub const TEXT_SELECTED:  Color = Color::Rgb(0xF8,0xF8,0xF2);

    pub const ACCENT_PRIMARY:   Color = Color::Rgb(0xFF,0x79,0xC6);
    pub const ACCENT_SECONDARY: Color = Color::Rgb(0x8B,0xE9,0xFD);
    pub const ACCENT_SUBTLE:    Color = Color::Rgb(0x62,0x72,0xA4);

    pub const STATUS_ERROR:   Color = Color::Rgb(0xFF,0x55,0x55);
    pub const STATUS_WARN:    Color = Color::Rgb(0xFF,0xB8,0x6C);
    pub const STATUS_OK:      Color = Color::Rgb(0x50,0xFA,0x7B);
    pub const STATUS_INFO:    Color = Color::Rgb(0x8B,0xE9,0xFD);
}
```

Example: **selected row style** in a table
```rust
use ratatui::style::{Style, Modifier};
use crate::ui::theme::dracula;

let selected = Style::default()
    .bg(dracula::UI_BORDER) // Current Line
    .fg(dracula::TEXT_SELECTED)
    .add_modifier(Modifier::BOLD);
```

Example: **fuzzy match highlight**
```rust
let highlight = Style::default()
    .fg(Color::Rgb(0xF1,0xFA,0x8C)) // Yellow for search
    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
```

Example: **status chip**
```rust
fn status_color(s: &str) -> Color {
    match s {
        "succeeded" | "ok" => dracula::STATUS_OK,
        "failed" | "error" => dracula::STATUS_ERROR,
        "pending" | "running" => dracula::STATUS_WARN,
        "warning" | "unstable" => dracula::STATUS_WARN,
        _ => dracula::STATUS_INFO,
    }
}
```

---

## Cyberpunk Theme (experimental)

Set `TUI_THEME=cyberpunk` (or `cyberpunk_hc`) for a neon-forward palette. The theme balances luminous accents with deep purple surfaces and remains legible on dark terminals.

**Core**
- BG_MAIN:        `#0d0221` ← root background
- BG_PANEL:       `#16063b` ← panel cards/inputs
- SURFACE_MUTED:  `#240046` ← secondary fills, zebra striping
- UI_BORDER:      `#41337a` ← borders/dividers/scrollbars
- MODAL_OVERLAY:  `#09011b` ← backdrop behind modals

**Foreground**
- TEXT_PRIMARY:   `#f8eeff`
- TEXT_SECONDARY: `#9a86fd`
- TEXT_MUTED:     `#6a64a4`

**Accents**
- ACCENT_PRIMARY:   `#00f6ff` ← focus glow, prompts
- ACCENT_SECONDARY: `#ff4ecd` ← interactive highlights, borders when focused
- ACCENT_SUBTLE:    `#7cffcb` ← badges, subtle indicators

**Status (semantic)**
- STATUS_INFO:    `#00f6ff`
- STATUS_SUCCESS: `#72f1b8`
- STATUS_WARNING: `#ffd166`
- STATUS_ERROR:   `#ff2965`

**Selection and Focus**
- Selection background: `#2a1a5e`
- Selection text: `#f8eeff`
- Focus border: `#ff4ecd` (standard) / `#00f6ff` (high contrast)

**Guidance**
- Keep large background areas on BG_MAIN/BG_PANEL to avoid eye fatigue.
- Reserve ACCENT_SECONDARY for active focus rings, current navigation targets, and modal headers to create a neon edge without overuse.
- Use ACCENT_PRIMARY for motion cues (spinners, progress) and informational callouts.
- When rendering badges or chips, prefer ACCENT_SUBTLE with bold text to maintain readability.
- High-contrast mode strengthens borders and keeps text at maximum brightness; rely on it when terminals reduce saturation.

Always verify that contrasts meet accessibility requirements by previewing the TUI with `TUI_THEME=cyberpunk` in a dark terminal and checking selection states, zebra striping, and modal overlays.

---

## 7) Verification Checklist (blockers to ship)

- [ ] Selected row uses UI_BORDER/TEXT_SELECTED (not STATUS colors).
- [ ] Fuzzy highlight spans use ACCENT_PRIMARY + bold/underline.
- [ ] Logs: timestamps in ACCENT_SECONDARY (dim), request IDs in ACCENT_SUBTLE or ACCENT_PRIMARY (bold).
- [ ] Status chips map to Dracula status colors consistently across views.
- [ ] Ghost text/hints use TEXT_MUTED (dim), not lowered opacity of primary.
- [ ] Borders/dividers are UI_BORDER, not a random accent or status tone.
- [ ] Monochrome mode renders with bold/underline + symbols correctly.

---

## 8) Example Snapshots (authoritative look)

**Search + Suggestions**
```plaintext
BG: BG_MAIN
Input (BG_PANEL, underline ACCENT_PRIMARY)
Ghost: TEXT_MUTED
Popup items: TEXT_PRIMARY; selected row BG UI_BORDER / FG TEXT_SELECTED
Matched spans: ACCENT_PRIMARY BOLD UNDERLINE
Badges: CMD=ACCENT_SECONDARY, WF=ACCENT_PRIMARY, PLG=ACCENT_PRIMARY
```

**Logs**
```plaintext
Text: TEXT_PRIMARY
Timestamp: ACCENT_SECONDARY (dim)
ID: ACCENT_PRIMARY (bold)
Status badges: OK=STATUS_OK, WARN=STATUS_WARN, ERROR=STATUS_ERROR
Copy toast: bg STATUS_OK/ERROR, fg TEXT_SELECTED
```

**Table**
```plaintext
Header: TEXT_SECONDARY (bold), border UI_BORDER
Body: TEXT_PRIMARY
Selected row: BG UI_BORDER, FG TEXT_SELECTED
Sort arrow: ACCENT_SECONDARY
State chips: Aurora colors
```

---

## 9) Theme Switching (Contributors)

The TUI derives its palette at startup. If `TUI_THEME` is unset, the loader inspects terminal capabilities:
- Truecolor terminals (e.g., iTerm2, Kitty) receive the standard Dracula palette.
- 8-bit/ANSI terminals fall back to the curated `ansi256` theme for consistent contrast.

Contributors can override both the palette and the color mode via environment variables.

Supported `TUI_THEME` values
- `dracula` (default for truecolor terminals)
- `dracula_hc`, `dracula-high-contrast`, `dracula-hc`, `draculahc`
- `nord`
- `nord_hc`, `nord-high-contrast`, `nord-hc`, `nordhc`
- `cyberpunk`
- `cyberpunk_hc`, `cyberpunk-high-contrast`, `cyberpunk-hc`, `cyberpunkhc`
- `ansi256`
- `ansi256_hc`, `ansi256-high-contrast`, `ansi256-hc`, `ansi256hc`

Optional color-mode override (`TUI_COLOR_MODE`)
- `truecolor` / `24bit` — force RGB output even if the terminal misreports.
- `ansi256` / `256` / `8bit` — force the fallback palette for testing.
- `TUI_FORCE_TRUECOLOR=1` is also recognized as a legacy alias.

Examples
```bash
# Run TUI (no args) with Dracula (default)
cargo run -p heroku-cli

# Explicit Dracula
TUI_THEME=dracula cargo run -p heroku-cli

# Dracula High Contrast
TUI_THEME=dracula_hc cargo run -p heroku-cli

# Nord
TUI_THEME=nord cargo run -p heroku-cli

# Nord High Contrast
TUI_THEME=nord_hc cargo run -p heroku-cli

# Cyberpunk fallback test
TUI_THEME=cyberpunk cargo run -p heroku-cli

# Force ANSI palette regardless of terminal
TUI_COLOR_MODE=ansi256 cargo run -p heroku-cli

# Force truecolor palette when a terminal fakes 256-color support
TUI_COLOR_MODE=truecolor cargo run -p heroku-cli
```

Implementation notes
- Loader: `crates/tui/src/ui/theme/mod.rs::load_from_env()` handles env overrides.
- Capability detection: `crates/tui/src/ui/theme/mod.rs::detect_color_capability()` inspects `COLORTERM`, `TERM`, and overrides before selecting `DraculaTheme` or `Ansi256Theme`.
- Theme role usage: prefer semantic roles in `ThemeRoles` over hard-coded colors.
- Hot-switching at runtime is not supported; change `TUI_THEME` and restart.
- Manual QA: validate both truecolor (iTerm2) and ANSI terminals (macOS Terminal) after palette changes.

## 10) Non-Goals (avoid)

- No gradient or multi-colored borders.
- No bright/saturated ANSI defaults; always map to Dracula constants.
- No semantic misuse (e.g., using red for selection).

---

Following these instructions ensures a consistent, accessible **Dracula** experience across the entire TUI, balancing clarity, high-contrast readability, and strong semantic signaling.
