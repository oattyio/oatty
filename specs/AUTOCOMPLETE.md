# AUTOCOMPLETE.md

As-built specification for the TUI command palette autocomplete.

This document describes only functionality currently implemented in the codebase.

## Scope

The autocomplete system is implemented in:
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/palette/palette_component.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/palette/state.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/palette/suggestion_engine.rs`

## Implemented behavior

### Input and cursor

- Single-line input with UTF-8 safe cursor movement.
- Character insert, backspace, and delete are supported.
- Left and right arrow move cursor by character.
- Clicking in the input area moves cursor to the clicked column.

### Suggestion popup

- Suggestions are built from command specs in the registry and value providers.
- Popup opens on `Tab` when input is non-empty.
- Popup can also open automatically when suggestions are available.
- Suggestions are ranked and rendered with kind-aware formatting.
- Mouse hover updates the highlighted suggestion.
- Mouse wheel scrolls suggestion list.

### Ghost text

- Ghost text is derived from the currently selected suggestion.
- Ghost text is hidden when popup is closed.
- Moving suggestion selection updates ghost text.

### Suggestion categories

- Command suggestions (`ItemKind::Command`, `ItemKind::MCP`).
- Flag suggestions (`ItemKind::Flag`).
- Positional suggestions (`ItemKind::Positional`).
- Value suggestions (`ItemKind::Value`) from:
  - enum/static options in specs
  - provider-backed dynamic values

### Provider loading and failures

- Provider-backed suggestions can report pending fetches.
- While loading, palette can show a `loading more…` placeholder.
- Provider loading state drives spinner visibility.
- Provider fetch failures clear loading placeholders and set a user-visible error message.

### Acceptance behavior

- `Enter` behavior:
  - If popup is closed: execute current input.
  - If popup is open: accept selected suggestion.
- Acceptance rules:
  - Command/MCP suggestion replaces command portion appropriately.
  - Flag/value suggestions are inserted or replace relevant token contextually.
  - Positional suggestions replace current positional token or append in positional slot.

### History

- Up/down arrows browse command history when popup is closed.
- History browsing stores and restores in-progress draft input.
- Successful command executions are appended to in-memory history.
- History is persisted through `HistoryStore` using palette command scope.
- Persisted history is loaded on palette state initialization.
- In-memory history cap is 200 entries.
- Persistence path filters secret-like values before storage.

### Keybindings (implemented)

- `Tab`: build/open suggestions (or move focus when input is empty).
- `Shift+Tab`: move focus to previous focus target.
- `Enter`: execute or accept suggestion.
- `Esc`: close suggestions; if already closed, clear palette input/state.
- `Up/Down`:
  - navigate suggestions when popup is open
  - browse history when popup is closed
- `Left/Right`: move cursor.
- `Ctrl+H`: open help for current/selected command.
- `Ctrl+F`: open command browser.

## Ranking and matching

- Fuzzy matching uses `oatty_util::fuzzy_score`.
- Suggestions are sorted by score descending.
- Rendering highlights matched segments in display text.

## Execution coupling

- Entering execution computes and stores a request hash.
- Completion handling routes generic execution outcomes through palette state logic.
- Destructive HTTP `DELETE` commands trigger confirmation modal before execution.

## Non-goals / not implemented in current build

The following are not implemented in the current palette behavior:
- Reverse search overlay (`Ctrl-R`)
- Emacs kill/yank word-editing commands
- Vi mode
- Source cycling (`Alt-/`)
- `Ctrl-Space` popup toggle
- Right-arrow word-wise ghost acceptance

## Correctness notes

- This spec is intentionally as-built. If behavior changes, update this file in the same PR.
- Do not document planned features here until implemented.


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/COMMANDS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/COMMAND_SEARCH.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/VALUE_PROVIDERS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/UX_GUIDELINES.md`
