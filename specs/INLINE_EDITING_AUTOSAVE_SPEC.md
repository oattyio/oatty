# INLINE_EDITING_AUTOSAVE_SPEC.md

As-built specification for inline editing and autosave behavior in current TUI components.

## Scope

Primary implementation files:
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/common/key_value_editor/state.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/common/key_value_editor/key_value_view.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/library/library_component.rs`

## Implemented model

The key/value editor is a row-based inline editor with two field focuses:
- key column
- value column

State tracks:
- selected row
- active field
- `is_dirty`
- `show_secrets`

Edits mutate row values immediately in local state.

## Key interactions (key/value editor)

Implemented behavior includes:
- Row navigation: `Up`, `Down`, `Home`, `End`
- Field-level editing: character input, `Backspace`, `Delete`, `Left`, `Right`
- Add row: `Ctrl+A` (after focused-row validation)
- Delete row: `Ctrl+D`
- Toggle secrets visibility: `Space` or `Enter` when secrets toggle is focused
- Focus traversal: `Tab`, `BackTab` via global focus ring
- Mouse row/column selection and add/remove/toggle controls

## Validation and commit semantics

- Focused row can be validated via state helpers (`validate_focused_row`, `validate_row`).
- Key text is trimmed and validated at commit boundaries defined by host components.
- Invalid rows surface inline error indicators in the key/value view.
- Editor keeps user input on validation error.

## Autosave behavior in Library

Library component integrates autosave for catalog edits:
- Header key/value changes are tracked as dirty.
- Losing key/value editor focus triggers autosave effect generation when dirty.
- Lost focus on active inline input fields similarly triggers save checks.
- Save success/failure is surfaced through library/log messaging.

## Non-implemented / not universal

The following are not global cross-app guarantees:
- A single shared "edit mode" abstraction for all components
- Uniform Enter=commit/Esc=cancel semantics across every editable widget
- Per-keystroke persistence to disk

Current behavior is component-specific, with the key/value editor and library panel being the primary inline-edit autosave implementation.

## Correctness notes

- This file is as-built. Update it when key/value editing semantics or autosave triggers change.
- Planned UX harmonization across components should live in planning specs, not here.


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/LIBRARY.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/LIBRARY_DETAILS_TUI.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/WORKFLOW_TUI.md`
