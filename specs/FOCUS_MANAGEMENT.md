**Focus Management**
- The TUI coordinates keyboard and mouse focus with the [`rat_focus`](https://crates.io/crates/rat-focus) crate. Every focusable state implements `HasFocus`, and the application owns a single [`Focus`](https://docs.rs/rat-focus/latest/rat_focus/struct.Focus.html) ring that is rebuilt just before rendering.

**Core Building Blocks**
- `Focus` lives on `App` (`crates/tui/src/app.rs`) and is rebuilt on each frame via `FocusBuilder::rebuild_for` in `crates/tui/src/ui/runtime.rs`.
- `FocusFlag::named` gives each container or leaf a stable identifier; focus flags are stored alongside component state (for example `PaletteState::new` in `crates/tui/src/ui/components/palette/state.rs`).
- `HasFocus::build` composes the traversal order. Use `builder.start(self)` / `builder.end(tag)` for containers, `builder.widget(child_state)` for nested containers, and `builder.leaf_widget(flag)` for direct leaf flags.

**Application-Level Lifecycle**
- When routes or modals change, `App` rebuilds the ring with `FocusBuilder::build_for(self)` and focuses the relevant widget (`crates/tui/src/app.rs`).
- Opening a modal stores the currently focused widget identifier so it can be restored when the modal closes (`crates/tui/src/app.rs`). `App::restore_focus` re-applies that identifier or falls back to `focus.first()` after each render.
- During rendering the runtime performs:
  ```rust
  let old_focus = std::mem::take(&mut application.focus);
  application.focus = FocusBuilder::rebuild_for(application, Some(old_focus));
  if application.focus.focused().is_none() {
      application.restore_focus();
  }
  ```
  (`crates/tui/src/ui/runtime.rs`).

**Implementing `HasFocus`**
- **Top-level containers:** `App<'_>` checks modal state and only exposes the active subtree (`crates/tui/src/app.rs`). When no modal is open it adds the navigation bar plus the current main view and logs to the ring in the order users should traverse them.
- **Simple leaf containers:** Components with a single focusable element register one `FocusFlag`. `PaletteState` builds a container and registers the input flag with `builder.leaf_widget(&self.f_input)` (`crates/tui/src/ui/components/palette/state.rs`). `BrowserState` registers both the search input and the command list in order (`crates/tui/src/ui/components/browser/state.rs`).
- **Collections of focus flags:** `VerticalNavBarState` maintains a vector of item flags and loops through them inside `HasFocus::build` (`crates/tui/src/ui/components/nav_bar/state.rs`). This pattern keeps traversal in sync with the rendered items and enables programmatic selection to toggle the active flag.
- **Nested components:** Parent states call `builder.widget(&child_state)` for nested containers, allowing the builder to flatten the tree while preserving container ranges. For example `TableState` nests the pagination component after the main grid (`crates/tui/src/ui/components/table/state.rs`).
- **Visibility gating:** Components can opt out of the ring by returning early. The pagination controls do nothing when they are hidden so Tab skips them (`crates/tui/src/ui/components/pagination/state.rs`).
- **Dynamic delegation:** `WorkflowState` delegates `HasFocus` to either the list view or the active input view, mirroring what is currently rendered (`crates/tui/src/ui/components/workflows/state.rs`). This keeps focus aligned with mode changes without rebuilding intermediate flags manually.

**Driving Focus in Event Handlers**
- Keyboard handlers advance the ring with `app.focus.next()` and `app.focus.prev()`. Examples include the browser (`crates/tui/src/ui/components/browser/browser_component.rs`), palette (`crates/tui/src/ui/components/palette/palette_component.rs`), and logs (`crates/tui/src/ui/components/logs/logs_component.rs`).
- Components typically inspect their own flags (`FocusFlag::get()`) to decide how to interpret the current key event before advancing the global ring (see the browser search handling in `crates/tui/src/ui/components/browser/browser_component.rs`).
- When a component changes visibility or replaces its internal layout it should call `app.focus.focus(&state)` or `app.focus.first_in(&state)` to ensure the new subtree has an active leaf (for example palette initialization in `crates/tui/src/app.rs`).

**Mouse and Layout Integration**
- `HasFocus::area` returns the last rendered rectangle for hit-testing. Most states expose `Rect::default()` because they do not support mouse focus yet, but widgets such as the navigation bar capture their last layout (`crates/tui/src/ui/components/nav_bar/state.rs`) so mouse clicks can move focus correctly.
- Components that support mouse focus should update their stored `Rect` during rendering so `Focus::focus_at(x, y)` can succeed.

**Practical Checklist**
- Create container and leaf `FocusFlag`s with descriptive names.
- Implement `HasFocus` on every state that owns focusable elements; compose children with `builder.widget` and `builder.leaf_widget`.
- Rebuild the focus ring whenever layout changes (`FocusBuilder::rebuild_for`) and restore a valid starting leaf (`App::restore_focus`).
- Use `app.focus.next()` / `app.focus.prev()` for Tab and BackTab. Only use advanced APIs such as `expel_focus` when a component needs to break out of its container explicitly (not currently required in the implemented components).
- Keep traversal order consistent with the rendered layout so assistive tooling and keyboard users experience predictable navigation.
