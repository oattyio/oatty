# TUI UX Guidelines (As-Built)

## Scope

This document captures the UX that is implemented today, not aspirational patterns.

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

## Primary Routes

Implemented top-level routes:

- Palette
- Browser
- Workflows
- Workflow Inputs
- Workflow Run
- Plugins
- MCP HTTP Server
- Library

Route wiring is handled by `MainView::set_current_route`.

## Global Interactions

- `Tab`/`BackTab` cycle focus through active focus rings.
- `Ctrl+L` toggles the logs panel.
- `Ctrl+T` opens theme picker when available.
- `Esc` behavior is context-sensitive (clear/close/back depending on active component/modal).

## Search UX

Search in list-driven components follows a shared model:

- Dedicated search field focus
- incremental filtering
- cursor-aware text editing
- clear/reset with `Esc` (component-dependent)

This pattern is present in Browser, Workflows list, and Logs list.

## Logs UX

- Searchable/filterable log list
- keyboard and mouse navigation
- detail modal with table/text rendering
- copy selected entry
- JSON detail pretty rendering with syntax highlight for parsed MCP payload text

## Workflow UX

- Workflow picker with search
- Input collection before execution
- Provider selector modal and manual-entry modal
- Run view with lifecycle status/events

## Source Alignment

- `crates/tui/src/ui/main_component.rs`
- `crates/tui/src/ui/components/palette/palette_component.rs`
- `crates/tui/src/ui/components/browser/browser_component.rs`
- `crates/tui/src/ui/components/workflows/workflows_component.rs`
- `crates/tui/src/ui/components/logs/logs_component.rs`

## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/FOCUS_MANAGEMENT.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/THEME.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/LOGGING.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/WORKFLOW_TUI.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/LIBRARY.md`
