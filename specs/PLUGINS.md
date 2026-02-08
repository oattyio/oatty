# MCP Plugins (As-Built)

## Scope
This document describes the MCP plugin behavior currently implemented across:
- `crates/mcp`
- `crates/tui/src/ui/components/plugins`
- `crates/tui/src/cmd.rs`

## Runtime Configuration
- Config file path is resolved by `oatty_mcp::config::default_config_path()`.
- `MCP_CONFIG_PATH` overrides the default path when set.
- Supported plugin transports are:
  - Stdio (`command`, `args`, optional `env`, optional `cwd`)
  - HTTP/SSE (`base_url`, optional `sse_path`, optional headers/auth)
- Configuration is validated on load and save.

## Engine Behavior
- `PluginEngine` manages plugin lifecycle (start/stop/restart), health/state updates, tool discovery, and per-plugin logs.
- Tool updates from a running plugin are converted to synthetic `CommandSpec` entries and injected into the shared command registry.
- Synthetic MCP commands are removed/replaced when plugin tool sets change.
- Plugin lifecycle/status updates are propagated through the client gateway event stream.

## Logging
- Per-plugin in-memory ring buffers are maintained by `LogManager`.
- Audit entries are written to `mcp-audit.jsonl`.
- Log text is redacted before formatting/export.

## TUI Behavior
- Plugins route provides:
  - Filterable plugin list
  - Plugin add/edit modal (local/remote transport)
  - Plugin detail modal (overview, health, env, tools, logs)
- Supported plugin actions in the TUI:
  - Start, stop, restart, refresh
  - Validate/save plugin config changes
  - Export plugin logs

## Current Constraints
- No automatic filesystem watch/reload loop for `mcp.json` is currently wired in the TUI runtime.
- Config changes are applied through explicit save/update flows.

## Source Alignment
- `crates/mcp/src/config/mod.rs`
- `crates/mcp/src/plugin/engine.rs`
- `crates/mcp/src/logging/mod.rs`
- `crates/tui/src/ui/components/plugins/plugins_component.rs`
- `crates/tui/src/ui/components/plugins/details_component.rs`
- `crates/tui/src/ui/components/plugins/plugin_editor/plugin_editor_component.rs`
- `crates/tui/src/cmd.rs`


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/LOGGING.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/COMMANDS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/MCP_WORKFLOWS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/UX_GUIDELINES.md`
