# Table Rendering (As-Built)

## Scope
This spec documents table behavior currently implemented in:
- `crates/tui/src/ui/components/common/results_table_view.rs`
- `crates/tui/src/ui/components/results/state.rs`
- table-using views (results modal, logs detail, workflow selector)

## Rendering Model
- Structured result payloads are normalized before rendering.
- When array/object data is table-compatible, `ResultsTableView::render_results` renders:
  - Header row
  - Body rows
  - Vertical scrollbar (when needed)
- When table rendering is not possible:
  - Object payloads fall back to key/value list rendering
  - Primitive payloads fall back to paragraph text rendering

## Column and Row Behavior
- Columns are inferred from JSON payload shape.
- Column widths are derived from measured content length with guards.
- Row styling uses theme-based zebra striping.
- Status-like values use semantic color styling.

## Interaction
- Keyboard navigation routes through shared table navigation helpers.
- Mouse hover/selection is supported in table-using components.
- Selected row state is maintained in `TableState`.

## Scrollbar Contract
- Scrollbar range is offset-domain based:
  - `max_scroll_offset = total_rows - viewport_rows`
  - `ScrollbarState::new(max_scroll_offset)`
  - `position = offset.min(max_scroll_offset)`
- This matches the shared implementation and avoids thumb drift.

## Source Alignment
- `crates/tui/src/ui/components/common/results_table_view.rs`
- `crates/tui/src/ui/components/results/state.rs`
- `crates/tui/src/ui/components/logs/log_details/log_details_component.rs`
- `crates/tui/src/ui/components/workflows/collector/collector_component.rs`


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/TUI_RENDER_PERFORMANCE.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/THEME.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/LOGGING.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/WORKFLOW_TUI.md`
