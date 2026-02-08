# TUI Render Performance (As-Built)

## Scope
Implemented patterns for reducing render-path cost in TUI components.

## Implemented Patterns
- Expensive transforms are cached outside hot render loops where possible.
- Large detail text (notably pretty JSON in log details) is cached per selected entry.
- Cached formatted JSON uses `Arc<str>` to avoid repeated large string cloning.
- Table and list render paths separate state mutation from drawing.

## Current Applied Example
- Log details modal caches `serde_json::to_string_pretty` output per selected entry.
- Cache invalidates on selection change.
- Scroll state is maintained independently from formatted payload generation.

## Scrollbar Rule (Implemented)
Scrollable list/table components use offset-domain scrollbar state:
- `max_scroll_offset = total - viewport`
- `ScrollbarState::new(max_scroll_offset)`
- position clamped to `max_scroll_offset`

## Source Alignment
- `crates/tui/src/ui/components/logs/log_details/log_details_component.rs`
- `crates/tui/src/ui/components/common/results_table_view.rs`
- `crates/tui/src/ui/components/logs/logs_component.rs`


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/TABLES.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/LOGGING.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/THEME.md`
