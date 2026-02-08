# LOGGING.md

As-built specification for logging behavior in the TUI and MCP subsystems.

## Scope

Primary implementation files:
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/logs/logs_component.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/logs/state.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/logs/log_details/log_details_component.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/log_persistence.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/mcp/src/logging/mod.rs`

## TUI logs (in-memory + persisted)

Implemented behavior:
- Logs are stored as structured `LogEntry` values in memory.
- Logs view supports filtering/search, selection, detail modal rendering, copy, and scrolling.
- JSON-capable entries can be rendered in table or text detail views.
- Detail view pretty JSON formatting is cached per selected row to avoid repeated heavy formatting.
- A background persistent writer stores logs as JSONL on disk with redaction and rotation.

Persistence details:
- Default path resolves under the registry config directory (`logs/tui.jsonl`), override via `OATTY_TUI_LOG_PATH`.
- Rotation controls:
  - `OATTY_TUI_LOG_MAX_BYTES`
  - `OATTY_TUI_LOG_MAX_FILES`

## MCP plugin logs

Implemented behavior:
- Per-plugin in-memory ring buffers in `LogManager`.
- Redacted formatting for export/display paths.
- Audit stream persisted to `mcp-audit.jsonl`.

## Redaction

Sensitive data is redacted in both TUI persisted logs and MCP logging/export pathways.

## Correctness notes

- This is as-built behavior only.
- Keep aligned with `logs/state.rs`, `log_details_component.rs`, `log_persistence.rs`, and `mcp/logging/*`.


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/TUI_RENDER_PERFORMANCE.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/TABLES.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/PLUGINS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/UX_GUIDELINES.md`
