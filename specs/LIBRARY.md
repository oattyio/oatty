# LIBRARY.md

As-built specification for the Library route and catalog management behavior.

## Scope

Primary implementation files:
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/library/library_component.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/library/state.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/library/types.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/cmd.rs`

## Implemented functionality

Library route provides:
- Catalog list selection
- Catalog enable/disable controls
- Import/remove catalog actions
- Metadata editing (description, base URLs, headers)
- Save/validation flows via command effects

## Synchronization behavior

Library route includes synchronization logic so runtime catalog/workflow mutations can refresh Library projections without restart when the refresh is safe.

Synchronization entry points are tied to effect/command paths that already update registry state (for example, import/remove/save actions), and then rehydrate Library-facing state from the updated registry snapshot.

## State and focus

Library contributes a dedicated focus subtree and maintains route-local edit state, including dirty/editing markers used by autosave and validation flows.

## Correctness notes

- This file is the general Library spec.
- Keep this aligned with `library_component.rs`, `library/state.rs`, and command handlers in `cmd.rs`.
- Route-level behavior and synchronization should be documented here, while detail-pane rendering specifics stay in `LIBRARY_DETAILS_TUI.md`.


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/LIBRARY_DETAILS_TUI.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/OPENAPI_IMPORT.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/INLINE_EDITING_AUTOSAVE_SPEC.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/UX_GUIDELINES.md`
