**Focus Management**
- **Goal:** Let a parent container hand focus to a child; the child cycles its own focus items on Tab/BackTab. When reaching the end (or beginning) of its ring, focus returns to the parent and continues.

**Library**
- **crate:** `rat_focus`
- **Core types:** `Focus`, `FocusBuilder`, `FocusFlag`, `HasFocus`, `Navigation`
- **Helpers:** `impl_has_focus!`, `on_gained!`, `on_lost!`, `match_focus!`, `event::FocusTraversal(&Focus)`

**Top-Level Pattern**
- **Single ring:** Maintain one `Focus` instance for the whole app. Rebuild it whenever widget structure changes or at least once per frame.
- **Storage:** Add `focus: rat_focus::Focus` on `app::App` (or a runtime-local variable). Use `FocusBuilder::rebuild_for(&app_state, Some(old_focus))` so removed widgets get their flags cleared and allocations are reused.
- **Build order:** Parent containers call `builder.widget(&child_state)` during `HasFocus::build` to nest children; the ring flattens but retains container ranges for handoff.

**Child-Manages-Itself Pattern**
- The child owns a container `FocusFlag` and leaf flags for its internal focusables.
- The parent includes the child in its `HasFocus::build` so focus can enter the child.
- While focus is inside the child’s range, `focus.next()`/`focus.prev()` step within the child. When the child reaches its last/first item, the next step naturally lands on the parent’s next/previous item.
- For custom exits (Enter/Esc/etc.), call `focus.expel_focus(&child_container_flag)` to hand focus to the parent’s next item after the child range.

**Palette Example (concrete)**

- `crates/tui/src/ui/components/palette/state.rs:1`
```
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;

pub struct PaletteState {
    // Container focus for the palette as a whole
    container_focus: FocusFlag,
    // Internal items managed by the palette
    input_focus: FocusFlag,
    suggestions_focus: FocusFlag,
    // ... existing fields (input, suggestions, etc.)
}

impl Default for PaletteState {
    fn default() -> Self {
        Self {
            container_focus: FocusFlag::named("palette"),
            input_focus: FocusFlag::named("palette.input"),
            suggestions_focus: FocusFlag::named("palette.suggestions"),
            // ...
        }
    }
}

impl HasFocus for PaletteState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        // Input is navigable normally
        builder.widget_with_navigation(&self.input_focus, Rect::default(), 0, Default::default());
        // Suggestions list can be marked as Leave so Tab can leave forward while open
        // or use Regular and let parent decide.
        builder.widget_with_navigation(&self.suggestions_focus, Rect::default(), 0, rat_focus::Navigation::Leave);
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag { self.container_focus.clone() }
    fn area(&self) -> Rect { Rect::default() }
}
```

- Parent container (e.g., top-level view) includes the palette as a child:
```
impl HasFocus for App {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.widget(&self.palette);     // child container
        builder.widget(&self.logs);        // sibling after palette
        // ... other siblings
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag { /* container flag for App */ }
    fn area(&self) -> Rect { Rect::default() }
}
```

**Event Routing**
- Provide components a way to drive focus travel:
  - Store `app.focus: Focus` (or pass via `event::FocusTraversal<'_>(&Focus)` wrapper from `rat_focus::event`).
  - Rebuild per frame or on structural changes:
```
// at render time (or after layout/state changes)
app.focus = FocusBuilder::rebuild_for(&app, Some(std::mem::take(&mut app.focus)));

// ensure a valid starting focus once
if app.focus.focused().is_none() { app.focus.first(); }
```

**Child Key Handling (Palette)**
- Inside `PaletteComponent::handle_key_events`, drive local cycling first; when hitting ends, hand off to parent.
```
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rat_focus::{event::FocusTraversal, on_gained, match_focus, Navigation};

fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
    let mut effects = Vec::new();

    // Ensure child initializes its internal focus when it gains focus
    on_gained!( app.palette => { app.focus.first_in(&app.palette); } );

    match key.code {
        KeyCode::Tab if key.modifiers.is_empty() => {
            // If suggestions are open and focused, `next()` cycles to input or exits to parent
            app.focus.next();
        }
        KeyCode::BackTab => {
            app.focus.prev();
        }
        KeyCode::Enter => {
            // Optional explicit handoff: leave child to parent’s next sibling
            // when Enter is pressed while child has focus.
            if app.palette_is_done() {
                app.focus.expel_focus(&app.palette.container_focus);
            }
        }
        _ => {
            // Normal editing
        }
    }

    effects
}
```

Notes
- `on_gained!` runs when the palette container gains focus and ensures a valid initial child focus.
- `Focus::next()`/`prev()` respect `Navigation` values; set `Navigation::Leave` on the last child to allow forward exit while keeping backward cycling inside, or use `ReachLeaveFront/Back` for stricter control.
- `Focus::expel_focus(&child_container_flag)` forcibly moves focus to the next item after the child container; if none exists it clears focus.

**Navigation Strategies**
- Regular child cycling: mark all children `Navigation::Regular` and let `next/prev` naturally fall through to siblings after the last/first child.
- One-way leave:
  - First child: `ReachLeaveFront` (can be reached; backward leaves)
  - Last child: `ReachLeaveBack` (can be reached; forward leaves)
- Lock focus temporarily: `Navigation::Lock` prevents `next/prev` transitions until unlocked. Use sparingly (e.g., modal text areas).

**Mouse Focus**
- Provide accurate areas in `HasFocus::area()` (and z-index via `area_z()`) for mouse clicks to move focus via `focus_at(x, y)`.

**Integration Checklist**
- Add `focus: Focus` to `App` and rebuild it each render.
- Implement `HasFocus` for top-level `App` and each focus-owning state.
- Ensure each focusable has a `FocusFlag` and stable debug name.
- On container focus gain, call `focus.first_in(&child)` to initialize child focus.
- In key handlers, call `focus.next()`/`prev()`; use `expel_focus()` for explicit exits.
- Keep build order deterministic to ensure expected traversal.

**Troubleshooting**
- Focus “stuck” in child: check `Navigation` for `Lock` or `Reach` on leaves.
- Focus doesn’t return to parent: ensure the child is a container (used with `builder.start(self)`/`end(tag)`) and parent includes a sibling after the child.
- Unexpected wrap-around: verify the target child’s last item `Navigation` and the parent’s ring order.

