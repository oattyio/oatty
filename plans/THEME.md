# ASSISTANT_INSTRUCTIONS_NORD_THEME.md

You are implementing the Heroku TUI using the **Nord theme**. Follow these rules exactly to ensure a cohesive, accessible, and professional look.

---

## 1) Palette Assignment (authoritative)

**Polar Night (backgrounds & structure)**
- BG_MAIN:        `#2E3440`  ← use for app background
- BG_PANEL:       `#3B4252`  ← use for secondary panels/cards, inputs
- UI_BORDER:      `#434C5E`  ← use for borders/dividers/scrollbars
- TEXT_MUTED:     `#4C566A`  ← use for ghost text, hints, placeholders

**Snow Storm (foreground text)**
- TEXT_PRIMARY:   `#D8DEE9`  ← default text
- TEXT_SECONDARY: `#E5E9F0`  ← titles, headers, labels
- TEXT_SELECTED:  `#ECEFF4`  ← highlighted text

**Frost (navigation & non-semantic accents)**
- ACCENT_CYAN:    `#8FBCBB`
- ACCENT_TEAL:    `#88C0D0`  ← fuzzy-match highlights, input focus underline
- ACCENT_BLUE:    `#81A1C1`  ← timestamps, secondary accents
- ACCENT_DARK:    `#5E81AC`  ← selected-row bg, IDs

**Aurora (semantic status colors)**
- STATUS_ERROR:   `#BF616A`
- STATUS_WARN:    `#D08770`
- STATUS_PENDING: `#EBCB8B`
- STATUS_OK:      `#A3BE8C`
- STATUS_NOTE:    `#B48EAD`  ← plugin badges or “special”

---

## 2) Global Styling Rules

- **Backgrounds**
  - App/root bg = BG_MAIN.
  - Panels (Search, Results, Details, Logs) = BG_PANEL.
  - Borders/dividers/scrollbars = UI_BORDER.

- **Text**
  - Default = TEXT_PRIMARY.
  - Headers/section titles = TEXT_SECONDARY (bold).
  - Muted/ghost/hints/placeholders = TEXT_MUTED (dim).

- **Selection**
  - Selected row: foreground = TEXT_SELECTED; background = ACCENT_DARK.
  - Focused input underline = ACCENT_TEAL.

- **Highlights**
  - Fuzzy-match spans: ACCENT_TEAL (bold+underline).
  - IDs/Request IDs: ACCENT_DARK (bold), ellipsize middle (e.g., `1d2c…9a7b`).
  - Timestamps: ACCENT_BLUE (dim).

- **Status/Badges**
  - Success ✓ = STATUS_OK.
  - Warning ! = STATUS_PENDING (amber) or STATUS_WARN (orange) for stronger attention.
  - Error ✖ = STATUS_ERROR.
  - Running/Progress … = STATUS_WARN or STATUS_PENDING (spinner).
  - Plugin badge / special = STATUS_NOTE.

- **Secrets**
  - Masked by default with bullets `•••••` in TEXT_MUTED.
  - When revealed, switch to TEXT_PRIMARY but display a warning toast.

---

## 3) Component-Specific Guidance

**Search/Command Input**
- Background: BG_PANEL; border: UI_BORDER.
- Text: TEXT_PRIMARY; caret: TEXT_SELECTED.
- Ghost text: TEXT_MUTED (dim).
- Focus underline: ACCENT_TEAL (1px/line).

**Suggestions Popup**
- Item text: TEXT_PRIMARY.
- Matched spans: ACCENT_TEAL (bold+underline).
- Type badges: `[CMD]` `[WF]` `[PLG]`
  - CMD badge = ACCENT_BLUE outline
  - WF badge  = ACCENT_TEAL outline
  - PLG badge = STATUS_NOTE outline
- Hover/selected row bg: ACCENT_DARK; fg: TEXT_SELECTED.

**Tables**
- Header row: TEXT_SECONDARY (bold) on BG_PANEL; bottom border UI_BORDER.
- Body rows: TEXT_PRIMARY; stripe optional (BG_MAIN ↔ BG_PANEL*0.95).
- Truncation ellipsis, no wrap by default.
- Sorting arrow uses ACCENT_BLUE.
- State chips use Aurora colors (OK/WARN/ERROR/PENDING).
- Hidden-columns chip uses ACCENT_CYAN.

**Workflow Steps**
- Icons: ✓ STATUS_OK, ✖ STATUS_ERROR, … STATUS_PENDING, ◻ neutral (TEXT_MUTED).
- Expanded preview borders: UI_BORDER; titles TEXT_SECONDARY.
- Dep/phase labels: ACCENT_BLUE.

**Logs**
- Default text: TEXT_PRIMARY.
- Timestamps: ACCENT_BLUE (dim).
- Request IDs & short SHAs: ACCENT_DARK (bold).
- Status inline tags: Aurora mapping (see above).
- Copy/toast messages:
  - Success toast bg: STATUS_OK; fg: BG_MAIN.
  - Failure toast bg: STATUS_ERROR; fg: TEXT_SELECTED.

**Toasts/Modals**
- Modal bg: BG_PANEL; border: UI_BORDER; title: TEXT_SECONDARY.
- Toast bg: darkened BG_PANEL; accent by status color.

---

## 4) Accessibility & Fallbacks

- Never rely on color alone: pair color with **icons/symbols** (`✓ ✖ ! …`) and variations (bold/underline).
- Ensure contrast ratio is comfortable (Nord is muted—use **bold/underline** to reinforce).
- Provide a **monochrome mode**: drop color, keep emphasis via bold/underline and symbols.

---

## 5) Do / Don’t

**Do**
- Use Frost colors only for **non-semantic guidance** (matches, focus, timestamps).
- Use Aurora colors **only** for semantic statuses.
- Keep the palette restrained—Nord is about calm clarity.

**Don’t**
- Mix multiple Aurora colors in the same element.
- Use saturated accent colors for large backgrounds.
- Overuse dim text; only for hints/ghost/secondary.

---

## 6) Implementation (Ratatui / Rust)

Define constants once and import everywhere.

```rust
use ratatui::style::Color;

pub mod nord {
    pub const BG_MAIN:        Color = Color::Rgb(0x2E,0x34,0x40);
    pub const BG_PANEL:       Color = Color::Rgb(0x3B,0x42,0x52);
    pub const UI_BORDER:      Color = Color::Rgb(0x43,0x4C,0x5E);
    pub const TEXT_MUTED:     Color = Color::Rgb(0x4C,0x56,0x6A);

    pub const TEXT_PRIMARY:   Color = Color::Rgb(0xD8,0xDE,0xE9);
    pub const TEXT_SECONDARY: Color = Color::Rgb(0xE5,0xE9,0xF0);
    pub const TEXT_SELECTED:  Color = Color::Rgb(0xEC,0xEF,0xF4);

    pub const ACCENT_CYAN:    Color = Color::Rgb(0x8F,0xBC,0xBB);
    pub const ACCENT_TEAL:    Color = Color::Rgb(0x88,0xC0,0xD0);
    pub const ACCENT_BLUE:    Color = Color::Rgb(0x81,0xA1,0xC1);
    pub const ACCENT_DARK:    Color = Color::Rgb(0x5E,0x81,0xAC);

    pub const STATUS_ERROR:   Color = Color::Rgb(0xBF,0x61,0x6A);
    pub const STATUS_WARN:    Color = Color::Rgb(0xD0,0x87,0x70);
    pub const STATUS_PENDING: Color = Color::Rgb(0xEB,0xCB,0x8B);
    pub const STATUS_OK:      Color = Color::Rgb(0xA3,0xBE,0x8C);
    pub const STATUS_NOTE:    Color = Color::Rgb(0xB4,0x8E,0xAD);
}
```

Example: **selected row style** in a table
```rust
use ratatui::style::{Style, Modifier};
use crate::theme::nord;

let selected = Style::default()
    .bg(nord::ACCENT_DARK)
    .fg(nord::TEXT_SELECTED)
    .add_modifier(Modifier::BOLD);
```

Example: **fuzzy match highlight**
```rust
let highlight = Style::default()
    .fg(nord::ACCENT_TEAL)
    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
```

Example: **status chip**
```rust
fn status_color(s: &str) -> Color {
    match s {
        "succeeded" | "ok" => nord::STATUS_OK,
        "failed" | "error" => nord::STATUS_ERROR,
        "pending" | "running" => nord::STATUS_PENDING,
        "warning" | "unstable" => nord::STATUS_WARN,
        _ => nord::ACCENT_BLUE,
    }
}
```

---

## 7) Verification Checklist (blockers to ship)

- [ ] Selected row uses ACCENT_DARK/TEXT_SELECTED (not STATUS colors).
- [ ] Fuzzy highlight spans use ACCENT_TEAL + bold/underline.
- [ ] Logs: timestamps in ACCENT_BLUE (dim), request IDs in ACCENT_DARK (bold).
- [ ] Status chips map to Aurora colors consistently across views.
- [ ] Ghost text/hints use TEXT_MUTED (dim), not lowered opacity of primary.
- [ ] Borders/dividers are UI_BORDER, not a random Frost/Aurora tone.
- [ ] Monochrome mode renders with bold/underline + symbols correctly.

---

## 8) Example Snapshots (authoritative look)

**Search + Suggestions**
```
BG: BG_MAIN
Input (BG_PANEL, underline ACCENT_TEAL)
Ghost: TEXT_MUTED
Popup items: TEXT_PRIMARY; selected row BG ACCENT_DARK / FG TEXT_SELECTED
Matched spans: ACCENT_TEAL BOLD UNDERLINE
Badges: CMD=ACCENT_BLUE, WF=ACCENT_TEAL, PLG=STATUS_NOTE
```

**Logs**
```
Text: TEXT_PRIMARY
Timestamp: ACCENT_BLUE (dim)
ID: ACCENT_DARK (bold)
Status badges: OK=STATUS_OK, WARN=STATUS_PENDING or STATUS_WARN, ERROR=STATUS_ERROR
Copy toast: bg STATUS_OK/ERROR, fg TEXT_SELECTED
```

**Table**
```
Header: TEXT_SECONDARY (bold), border UI_BORDER
Body: TEXT_PRIMARY
Selected row: BG ACCENT_DARK, FG TEXT_SELECTED
Sort arrow: ACCENT_BLUE
State chips: Aurora colors
```

---

## 9) Non-Goals (avoid)

- No gradient or multi-colored borders.
- No bright/saturated ANSI defaults; always map to Nord constants.
- No semantic misuse (e.g., using red for selection).

---

Following these instructions ensures a consistent, accessible **Nord** experience across the entire TUI, balancing clarity, calm aesthetics, and strong semantic signaling.
