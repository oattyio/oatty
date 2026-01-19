# Oatty CLI — UX Guidelines

## Overview
This document defines **usability and discoverability principles** for the new Oatty CLI TUI.  
It is a design contract for contributors, ensuring that **new features remain consistent, intuitive, and accessible** to both first-time users and experienced power users.

---

## 1. Core UX Principles

### 1.1 Discoverability
- Every command, workflow, and option should be discoverable without prior knowledge.
- Guided UI includes:
  - Inline search across all commands/workflows.
  - Contextual hints at the bottom status bar.
  - Default keybindings displayed when relevant.

### 1.2 Simplicity
- Avoid clutter. Each screen should focus on a **single task** (search, view, run).
- Use **familiar patterns**:
  - Search bar on top.
  - Results list in the center.
  - Details panel optional on the right.
- Defaults should “just work” with minimal flags.

### 1.3 Speed
- Power users can skip UI navigation:
  - Command prompt `:` prefix for instant execution.
  - Arrow-up/down history navigation.
  - Auto-completion for flags and args.

### 1.4 Consistency
- **Workflows behave like commands**:
  - Unified search and run interface.
  - Consistent error display, output formatting, and logging.

---

## 2. Interaction Modes

### 2.1 Guided Mode
- Default mode for new and casual users.
- Step-by-step argument input using form widgets.
- Validation errors shown inline with helpful suggestions.

### 2.2 Power Mode
- Minimal command prompt with history + completion.
- No preview panels.
- One-line status bar feedback:
  - `✔ Success in 138ms`
  - `✖ Missing required param: app`

### 2.3 Mode Switching
- **F1** toggles between Guided and Power modes.
- State preserved when switching (e.g., half-typed command remains).

---

## 3. Output Presentation

### 3.1 Tables
- Array JSON responses → formatted tables.
- Usability:
  - `/` search.
  - `s` sort by column.
  - `c` select/deselect columns.
  - `Enter` expand row.
- Presets saved per user.

### 3.2 JSON
- `--json` flag → raw JSON, no formatting.
- Useful for scripting or piping.

### 3.3 Logs & Errors
- Errors appear in a **dedicated error bar**.
- Logs for workflow/command runs are collapsible.

---

## 4. Workflows UX

### 4.1 Unified Discovery
- Workflows listed alongside commands in search results.
- Workflow tags: `[WF]` vs `[CMD]`.

### 4.2 Expanded Steps View
- Displays workflow tasks with dependencies.
- **Execution**: live progress with ✓/✗ markers.

### 4.3 User Overrides
- Keeps flexibility without editing workflow files.

---

## 5. Shell Handoff UX

- When launching a remote shell (e.g., `psql`):
  - Show modal: “Launching psql… type `exit` to return.”
  - Suspend TUI, handoff to PTY.
  - On exit, show summary log in status bar.
- Ensures robust, familiar terminal experience.

---

## 6. Keybindings (Default)

| Key        | Action                              |
|------------|-------------------------------------|
| `/`        | Search within current list/table    |
| `:`        | Enter Power Mode (command prompt)   |
| `↑ / ↓`    | Navigate history or results         |
| `Enter`    | Execute command / expand row        |
| `s`        | Sort by column                      |
| `c`        | Choose columns                      |
| `F1`       | Toggle Guided / Power mode          |
| `Esc`      | Clear input or close panel          |

---

## 7. Accessibility

- High-contrast default theme.
- Minimal reliance on color alone (badges also show symbols).
- Keyboard-first design, no mouse required.

---

## 8. Security in UX

- Secrets always masked by default (`•••••`).
- Reveal requires explicit action.
- No sensitive values persisted in history.

---

## 9. Guiding Principle
> **“Make the first run effortless, the 100th run lightning-fast.”**  
The interface should be welcoming for beginners, while removing friction for experts.

## Source Alignment

- **Routing + modes**: `crates/tui/src/app.rs` owns the `Route` enum and focus orchestration, so Guided vs. Power mode toggles (`F1`) and `/` search behaviors align with this spec.
- **Power mode implementation**: `crates/tui/src/ui/components/palette/` implements the colon-prefixed palette, history, completion, and validation rules called out in sections 1–3.
- **Guided workflows**: `crates/tui/src/ui/components/workflows/` (list, input collector, run view) delivers the multi-pane experience, inline validation, and status messaging described under Workflow UX.
- **Tables/logs**: `crates/tui/src/ui/components/common/results_table_view.rs` and `components/logs/` provide the search, sort, column picker, and error presentation patterns referenced in sections 3 and 6.
