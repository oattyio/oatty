# Focus Management Architecture

This document explains how focus is implemented across the TUI using rat-focus, the migration choices we made, and how to extend or modify focus behavior for new and existing components.

## Overview

- Library: [rat-focus 1.0.2](https://docs.rs/rat-focus/1.0.2/rat_focus/)
- Core types we use:
  - `FocusFlag`: a per-widget focus flag (lives on the widget state)
  - `HasFocus`: trait a state struct implements (or small wrapper implements) so it can participate in a focus tree
  - `FocusBuilder`: constructs a focus tree (ring) of widgets/containers
  - `Focus::next()/prev()`: cycles focus forward/backward
- Styling follows a consistent pattern: compute a `focused: bool` from a `FocusFlag` and pass it to theme helpers (e.g., `theme.border_style(focused)`).

## Current State (Post-Migration)

We migrated off custom focus enums and adopted rat-focus flags across the root screen and modal components.

- Legacy removed:
  - `MainFocus` (root focus enum)
  - `heroku_types::Focus` (builder panel enum)
  - `TableFocus` and `PaginationFocus` (table/pagination enums)
- rat-focus in use:
  - `PaletteState`: `focus: FocusFlag` (leaf)
  - `LogsState`: `focus: FocusFlag` (leaf)
  - `BuilderState`: `search_f`, `commands_f`, `inputs_f` flags + a local focus ring
  - `TableState`: `grid_f` flag
  - `PaginationState`: `range_field_f`, `range_start_f`, `range_end_f`, `nav_f` flags + a local focus ring

The app builds small focus rings (via `FocusBuilder`) at the places where traversal is needed (root, builder, table/pagination). This keeps code localized and avoids a single global focus graph.

## Files and Responsibilities

- Root wiring
  - `crates/tui/src/app.rs`
    - Initializes a root ring (palette, logs) and focuses the palette at startup.
  - `crates/tui/src/lib.rs`
    - Global key loop. Tab/Shift-Tab builds a small ring over (palette, logs) and calls `next()/prev()`.
    - Short-circuits Palette Tab when suggestions are open (i.e., don’t traverse).

- Palette
  - `crates/tui/src/ui/components/palette/state.rs`
    - `PaletteState { focus: FocusFlag, … }`
    - Implements `HasFocus` so it can participate in the root ring.
  - `crates/tui/src/ui/components/palette/palette.rs`
    - Rendering remains the same; focus-driven styling derives from `palette_state.focus.get()` when needed.

- Logs
  - `crates/tui/src/ui/components/logs/state.rs`
    - `LogsState { focus: FocusFlag, … }`, implements `HasFocus`.
  - `crates/tui/src/ui/components/logs/logs.rs`
    - Borders/cursor/hints derive `focused` from `app.logs.focus.get()`.

- Builder
  - `crates/tui/src/ui/components/builder/state.rs`
    - Flags: `search_f`, `commands_f`, `inputs_f`.
    - `focus_ring() -> Focus`: `FocusBuilder` over the three flags.
  - `crates/tui/src/ui/components/builder/builder.rs`
    - Tab/Shift-Tab: `focus_ring().next()/prev()`.
    - Which panel handles keys: check `search_f.get()`, `commands_f.get()`, `inputs_f.get()`.
    - Styling and cursor placement: same checks.
    - Enter sets `inputs_f = true` (moves focus to Inputs).

- Table + Pagination
  - `crates/tui/src/ui/components/table/state.rs`
    - Flag: `grid_f` (table grid focus).
  - `crates/tui/src/ui/components/pagination/state.rs`
    - Flags: `range_field_f`, `range_start_f`, `range_end_f`, `nav_f`.
  - `crates/tui/src/ui/components/table/table.rs`
    - Tab/Shift-Tab builds a ring over: `grid_f`, then pagination flags.
    - If any pagination flag is focused, key handling delegates to `PaginationComponent`.
    - Styling uses `grid_f.get()`.
  - `crates/tui/src/ui/components/pagination/pagination.rs`
    - Tab/Shift-Tab builds a ring over its four flags and calls `next()/prev()`.
    - Styling and input focus checks read the relevant flags.

## Traversal & Event Handling

- Root traversal (palette/logs)
  - Build a `Focus` with `(palette, logs)`.
  - On Tab: `focus.next()`. On Shift-Tab: `focus.prev()`.
  - When Palette suggestions are open, the Tab is consumed by the palette (no traversal).

- Builder traversal (search → commands → inputs)
  - Build a `Focus` with `(search_f, commands_f, inputs_f)`.
  - On Tab/Shift-Tab: `focus.next()/prev()`.
  - Enter sets `inputs_f = true` to jump into Inputs.

- Table traversal (grid ↔ pagination children)
  - Build a `Focus` with `(grid_f, range_field_f, range_start_f, range_end_f, nav_f)`.
  - On Tab/Shift-Tab: `next()/prev()`.
  - If any pagination flag is focused, route keys to `PaginationComponent`.

## Focus-Driven Styling

- Compute a local `focused: bool` using `.get()` on the relevant `FocusFlag`:
  - Logs panel: `app.logs.focus.get()`
  - Builder panels: `app.builder.search_f.get()`, etc.
  - Table grid: `app.table.grid_f.get()`
  - Pagination sub-panels: `state.range_start_f.get()`, etc.
- Pass `focused` into theme helpers like `theme.border_style(focused)` and conditional rendering (e.g., cursor placement, highlight symbols).

## Implementation Notes and Rationale

- We removed enum-based focus to avoid scattered state machines across modules and unify traversal with rat-focus rings.
- We keep rings local (built on demand) instead of a single shared focus object so components can evolve independently and without tight coupling.
- For now, we update some focus flags directly (e.g., moving Builder focus to Inputs on Enter). If you need more formal semantics, you can use `Focus::focus(&widget)` or the `on_gained!/on_lost!` macros to coordinate enter/leave behavior.
- Mouse: `HasFocus::area()` returns a `Rect` for hit-testing if you want mouse-driven focus; currently we use `Rect::default()`. Add real areas when enabling mouse focus.

## Adding Focus to a New Panel/Component

1) Add a `FocusFlag` to the component’s state:

```rust
pub struct MyPanelState {
    pub focus: FocusFlag,
    // …
}
```

2) Use it in rendering:

```rust
let focused = state.focus.get();
let block = th::block(theme, Some("My Panel"), focused);
```

3) Include it in a ring:

```rust
let mut b = FocusBuilder::new(None);
b.widget(&PanelLeaf(state_a.focus.clone()));
b.widget(&PanelLeaf(state_b.focus.clone()));
let ring = b.build();
let _ = ring.next();
```

4) Route keys based on which flag is focused and use Tab/Shift-Tab to move focus.

## Testing & Validation

- Build: `cargo build --workspace`
- Unit tests: `cargo test --workspace`
- Lints/format: `cargo clippy --workspace -- -D warnings` and `cargo fmt --all`
- Manual checks:
  - Root: Tab/Shift-Tab moves focus Palette ↔ Logs (Palette Tab still accepts suggestions).
  - Builder: Tab cycles Search → Commands → Inputs; Enter jumps to Inputs.
  - Table: Tab cycles Grid ↔ RangeField → Start → End → Nav; when any pagination control is focused, arrow/typing control those inputs.

## Future Enhancements

- Consider caching and reusing `Focus` objects per component instead of rebuilding on each key press (micro-optimization; current approach is simple and safe).
- Use `on_gained!`/`on_lost!` macros for validation or initializations when focus changes (e.g., select first command on entering Commands panel).
- Add real `area()` rectangles for mouse focus; use `area_z()` to define stacking where needed.
- Remove the remaining scaffolding in `ui::focus` (string IDs and local `FocusStore`) if no longer needed.

This architecture gives us a predictable, testable focus model with rat-focus, removes custom enums, and keeps component responsibilities local and easy to extend.
