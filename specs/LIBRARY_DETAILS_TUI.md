# LIBRARY_DETAILS_TUI.md

As-built specification for the Library route detail pane and editable catalog metadata.

## Scope

Primary implementation files:
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/library/library_component.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/library/state.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/library/types.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/common/key_value_editor/*`

General Library route behavior (including synchronization) is documented in:
- `/Users/justinwilaby/Development/next-gen-cli/specs/LIBRARY.md`

## Implemented functionality

Library view provides:
- Catalog list selection and enable/disable controls
- Import/remove catalog actions
- Catalog metadata editing (description, base URL collection, headers)
- Focus-managed interaction between list, inputs, buttons, and key/value editor

## Detail pane behavior

When a catalog is selected, detail pane renders current catalog state including:
- title / description
- enabled state
- base URLs and active base URL index
- headers (with redaction by default)
- manifest-derived counts/summary fields when available

When no selection is available, the pane renders an empty-state message.

## Base URL editing

Implemented interactions include:
- Add/remove base URL entries
- Select active base URL index
- Inline input editing for base URL fields
- Validation error display and focus retention on invalid input
- Persistence through registry save effects

## Header editing

Headers use the shared key/value editor:
- Add/remove/edit rows
- show/hide secret values
- dirty tracking
- autosave on focus transitions when dirty

## Focus and navigation

Library state contributes multiple focus flags to the global ring.
Implemented keyboard patterns include:
- `Tab` / `BackTab` focus traversal
- directional navigation for list/table contexts
- direct focus jumps after certain actions (e.g., add row / edit field)

## Correctness notes

- This file is as-built. Keep it aligned with `library_component.rs` and `library/state.rs` behavior.
- Do not document unimplemented alternate UX flows here.


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/LIBRARY.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/INLINE_EDITING_AUTOSAVE_SPEC.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/TABLES.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/THEME.md`
