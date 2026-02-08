# TUI Render Performance Patterns

This document captures a reusable performance pattern for Ratatui views that render large text payloads (for example, pretty-printed JSON in modals).

## Problem Pattern

Symptoms in a render loop:

- Repeated heavy transforms every frame (`serde_json::to_string_pretty`, expensive parsing, reformatting).
- Repeated large string clones used to build `Line`/`Span`.
- Scroll jitter or lag when opening detail modals with large payloads.

## Recommended Pattern: Render Cache + Borrowed Rendering

### 1) Cache expensive derived text by stable view identity

Use a cache keyed by a stable identity for the displayed payload (for example, selected row index, record id, hash).

Example shape:

```rust
struct CachedPrettyPayload {
    selected_index: usize,
    formatted: Arc<str>,
}
```

`Arc<str>` keeps per-frame reads cheap (`Arc::clone`) and avoids large `String` clones.

### 2) Invalidate only when identity changes

Clear cache when:

- selected item changes
- source payload changes for the same selected item
- view is reset/closed (optional if selection identity always changes)

Do **not** invalidate on every render.

### 3) Keep render path borrow-friendly

- Use cached text as `&str` when constructing `Line`/`Span` so Ratatui can use `Cow` effectively.
- Avoid allocating in hot rendering paths unless the UI state changed.

### 4) Keep scroll state independent from text formatting

Track scroll metrics (`offset`, `content_height`, `viewport_height`) separately from cache contents.
Update metrics each render, but keep expensive text generation outside the hot path.

## Migration Checklist for Existing Views

For each modal/view that renders large text:

1. Identify expensive per-frame transform(s).
2. Introduce a small component-local cache keyed by stable identity.
3. Invalidate cache only on identity/source changes.
4. Switch cached payload type from `String` to `Arc<str>` when possible.
5. Ensure render helpers accept `&str` and avoid cloning large buffers.
6. Verify scroll behavior remains correct with cache hits/misses.
7. Add/update focused tests if logic branches depend on view mode or selected entry changes.

## Candidate Views to Migrate

- Log/detail modals with pretty-printed JSON or large text blobs.
- Plugin/log panels that reformat payloads in render.
- Any detail pane that repeatedly builds long wrapped text from structured data.

## Notes

- Caching should be local to a component unless multiple components truly share identical derived payloads.
- Prefer simple cache keys first (selection index or id). Add payload hashing only when needed.
- If syntax highlighting becomes dominant after formatting is cached, apply the same pattern to highlighted lines (`Vec<Line<'static>>` or another owned representation) with the same invalidation rules.

## Scrollbar State Contract (Do Not Regress)

This is a repeated source of bugs in list/table views.

### Correct model

For a vertically scrollable list:

- `viewport_height = visible_rows.max(1)`
- `max_scroll_offset = total_rows.saturating_sub(viewport_height)`
- `position = current_offset.min(max_scroll_offset)`
- `ScrollbarState::new(max_scroll_offset)`
- `.viewport_content_length(viewport_height)`
- `.position(position)`

### Incorrect model (common mistake)

Do **not** initialize the scrollbar with total row count:

- `ScrollbarState::new(total_rows)` (wrong range)

This causes thumb position drift and incorrect proportional movement because the
state range no longer represents the legal offset domain.

### Rule of thumb

- `ListState/TableState.offset()` is an offset-domain value, so scrollbar state
  must also be offset-domain (`0..=max_scroll_offset`), not content-domain.
