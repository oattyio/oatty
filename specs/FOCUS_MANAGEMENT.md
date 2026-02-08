# FOCUS_MANAGEMENT.md

As-built specification for keyboard focus management in the TUI.

This document describes currently implemented behavior only.

## Scope

Primary implementation files:
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/app.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/runtime.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/main_component.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/**/state.rs`

## Focus system

- Focus management is built on `rat_focus`.
- `App` owns a single `Focus` ring (`app.focus`).
- Focusable state types implement `HasFocus`.
- Focus targets are represented by `FocusFlag` values created with `FocusFlag::new().with_name(...)`.

## Ring build and rebuild lifecycle

- Initial ring build occurs during app construction via `FocusBuilder::build_for(&app)`.
- During runtime render, the ring is rebuilt before drawing with:
  - `FocusBuilder::rebuild_for(app, Some(old_focus))`
- If no focused element remains after rebuild, `MainView::restore_focus` is invoked.

## Top-level focus composition

`App::build(&self, builder: &mut FocusBuilder)` defines global traversal.

- If a modal is open, the modal subtree is the only focus scope.
- If no modal is open, traversal includes:
  1. nav bar
  2. active route subtree (`Palette`, `Browser`, `Plugins`, `McpHttpServer`, `Workflows*`, `Library`)
  3. logs subtree when logs are visible

## Modal focus behavior

- Opening a modal stores the previously focused widget id in `MainView.transient_focus_id`.
- While modal is open, traversal is limited to modal focusable state.
- On modal close, `restore_focus` attempts to restore prior widget id via `focus.by_widget_id(id)`.
- If restoration is not possible, fallback is `focus.first()`.

## Component-level focus patterns

Implemented patterns across state modules:
- Container + leaf flags (`builder.start` / `builder.end` + `builder.leaf_widget`).
- Nested state delegation using `builder.widget(&child_state)`.
- Dynamic subtree selection based on visible/active mode.
- Direct programmatic focus in component handlers using `app.focus.focus(&flag_or_state)`.

## Keyboard navigation behavior

Common handling across components:
- `Tab` calls `app.focus.next()`.
- `BackTab` calls `app.focus.prev()`.
- Components then interpret keys based on currently active local `FocusFlag`.

Examples include:
- palette
- browser
- logs
- plugins views
- library view
- MCP server view
- file picker modal

## Mouse interaction and focus

- Mouse handlers in components may explicitly set focus (`app.focus.focus(...)`) when clicking interactive regions.
- Most focusable states return `Rect::default()` for `HasFocus::area()` and rely on component-specific hit testing.
- Some components maintain local hit areas/rects and map those regions to focus flags.

## Constraints (current implementation)

- Focus traversal order is entirely defined by `HasFocus::build` ordering.
- Modal routes without focusable children intentionally leave limited/empty modal focus scope until fields are implemented.
- Focus restoration is best-effort by widget id; if unavailable, first focusable element is selected.

## Correctness notes

- This file is as-built. Update it when focus scopes, modal behavior, or traversal ordering semantics change.
- Keep planned focus ergonomics work in planning specs, not here.


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/UX_GUIDELINES.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/MSG_EFFECT_ARCHITECTURE.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/WORKFLOW_TUI.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/LIBRARY.md`
