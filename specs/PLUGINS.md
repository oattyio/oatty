# MCP Plugin Architecture Specification

This document describes the current Model Context Protocol (MCP) plugin system that powers the
Oatty CLI and Terminal UI (TUI). It covers the runtime architecture, configuration contract,
transport support, lifecycle management, logging pipeline, provider integration, and user
experience. The specification reflects the multi-crate implementation that ships today, spanning
`crates/mcp`, `crates/registry`, `crates/tui`, and `crates/util`.

## Current Capabilities

- **Configuration:** Uses `~/.config/oatty/mcp.json` (override via `MCP_CONFIG_PATH`) with
  camelCase fields under `mcpServers`. Supports `${env:}` and `${secret:}` interpolation during
  load, and preserves redactions on write.
- **Transports:** Fully supports stdio plugins (spawned child process) and HTTP/SSE transports via
  `rmcp` with reqwest clients. SSE endpoints default to `sse` and can be overridden with
  `ssePath`.
- **Lifecycle:** `LifecycleManager` tracks start/stop state, startup/shutdown timeouts, restarts,
  health, and handshake latency. Autostart runs for all enabled servers at engine boot.
- **Command Registry:** Discovered MCP tools are converted into synthetic `CommandSpec` entries,
  allowing the CLI command registry to expose plugin commands alongside native commands.
- **Provider Bridge:** An `McpProviderRegistry` allows MCP tools to be surfaced as value providers
  inside the workflow engine, enabling plugins to feed downstream automation.
- **Logging & Audit:** Each plugin maintains a ring-buffer of logs with redaction helpers. All
  lifecycle and invocation events are appended to a JSON Lines audit log (`mcp-audit.jsonl`) with
  10MB/7-day rotation.
- **Security & Secrets:** Sensitive values are redacted in UI, logs, and audit trails. `${secret:}`
  entries resolve through `keyring-rs`, and OAuth tokens can be pulled from the OS keychain for
  remote transports.
- **Live Reload:** The TUI watches `~/.config/oatty/mcp.json` for changes. When a valid write is
  detected, the MCP engine reloads the configuration, rewrites the registry, and restarts any
  plugins that were running before the edit so the UI reflects manual changes without re-launching
  the CLI.
- **TUI Experience:** Ratatui widgets provide plugin list, detail, logs, environment editor,
  secrets editor, and add/edit workflows with keyboard-first navigation and responsive layouts.

## Architecture

### Configuration Layer (`crates/mcp::config`)

1. `load_config()` resolves the active file, interpolates environment/secret placeholders, and
   validates the structure.
2. Validation enforces server names (`^[a-z0-9._-]+$`), environment key casing, and required
   transport fields (`command` for stdio, `baseUrl` for HTTP/SSE). Unsupported schemes or malformed
   headers raise structured errors.
3. Configuration writes (`save_config`) pretty-print the JSON and ensure directories exist with
   restrictive permissions when possible. When editing an existing plugin via the UI, renaming the
   server updates both the key and the value so the old entry is removed before the new one is
   inserted, preventing duplicate records.
4. A file watcher (driven from the TUI runtime) triggers `load_config_from_path` and
   `PluginEngine::update_config` whenever `mcp.json` changes on disk. Invalid JSON is logged and
   ignored until the file parses successfully.

### Engine Layer (`crates/mcp::plugin`)

1. `PluginEngine` orchestrates the runtime using:
   - `McpClientManager` (connection pool, autostart, broadcast events),
   - `LifecycleManager` (timeouts, restart policies, health tracking),
   - `PluginRegistry` (UI-facing plugin metadata with status/health/tool counts),
   - `LogManager` (ring buffers, audit logger),
   - `CommandRegistry` injection (register/unregister synthetic command specs).
2. A background status listener subscribes to client events. Tool updates refresh caches and
   rebuild synthetic command specs grouped by plugin/tool naming conventions.
3. Configuration reloads stop every registered plugin, flush caches, and rebuild the registry from
   the new file. Afterward, only the plugins that were previously running (and remain enabled in the
   updated config) are restarted, minimizing churn while ensuring changes take effect uniformly.
3. Tool invocations return `ExecOutcome::Mcp`, including pretty-printed JSON payloads. Failures are
   audited and surfaced in plugin logs.

### Provider Integration (`crates/mcp::provider`)

1. `McpProviderRegistry` exposes MCP tools as value providers via adapters. Registered providers are
   keyed by `plugin:tool` and hold a handle to the shared `PluginEngine`.
2. Providers implement asynchronous `fetch_values` and availability checks, enabling workflows to
   reuse MCP tools for data fetching.
3. Discovery hooks are available for future work to auto-register providers when plugins expose
   declarative contracts.

### Logging and Audit (`crates/mcp::logging`)

1. `LogManager` stores recent log entries per plugin (default capacity: 1000). Logs can be exported
   with or without redaction.
2. `AuditLogger` writes JSONL records with rotation after 10MB or seven days, ensuring restrictive
   permissions. Entries capture action (`Start`, `ToolInvoke`, etc.), result, and metadata.
3. All text output goes through `LogFormatter`, which applies `oatty_util::redact_sensitive_with`
   to scrub tokens before rendering in the UI or terminal.

## Configuration Reference

### File Location

- Default: `~/.config/oatty/mcp.json`.
- Override: `MCP_CONFIG_PATH` environment variable (supports `~` expansion).
- No fallback to `~/.cursor/mcp.json`; configuration remains local to the CLI.

### Schema Overview

```json
{
  "mcpServers": {
    "plugin-name": {
      "command": "node",
      "args": ["-e", "require('@mcp/server').start()"],
      "env": { "OATTY_API_TOKEN": "${env:OATTY_API_TOKEN}" },
      "cwd": "/optional/path",
      "disabled": false,
      "tags": ["code", "gh"]
    },
    "remote-example": {
      "baseUrl": "https://mcp.example.com",
      "ssePath": "events",
      "headers": { "Authorization": "Bearer ${secret:EXAMPLE_TOKEN}" },
      "auth": {
        "scheme": "basic",
        "username": "${secret:REMOTE_USER}",
        "password": "${secret:REMOTE_PASS}",
        "interactive": true
      }
    }
  }
}
```

### Supported Fields

- **Common:** `disabled`, `tags`.
- **Stdio:** `command` (required), `args`, `env`, `cwd`.
- **HTTP/SSE:** `baseUrl` (required), `ssePath`, `headers`, `auth` (Basic or OAuth-style). OAuth
  tokens fall back to `${secret:}` but can be looked up in the OS keyring via the composed
  `baseUrl` identifier.
- **Interpolation:**
  - `${env:NAME}` – loads from process environment at runtime.
  - `${secret:NAME}` – resolves via OS keychain; never persisted.

### Validation Rules

- Server names: lowercase alphanumeric plus `.`, `_`, `-`.
- Environment keys: uppercase snake case.
- HTTP URLs: `http` or `https` only. SSE endpoints are normalized to avoid duplicate slashes.
- Headers: must be non-empty and free of control characters.

## TUI Experience

- **Global Shell:** Title bar with plugin counts, main body split among search/table/details views,
  footer hint bar using Nord-inspired theming.

  ```text
  ┌══════════════════ Plugins — MCP ══════════════════┐
  │ ▸ Search ▏github plugins           Plugins: 4 ✓   │
  ├───────────────────────────────────────────────────┤
  │ Name          Status   Transport      Tags        │
  │ github        ✓        stdio          code,gh     │
  │ remote-api    !        http/sse       api,prod    │
  │ vector-store  ✗        stdio          ml,search   │
  │ …                                                 │
  ├───────────────────────────────────────────────────┤
  │ Hints: Tab cycle • Ctrl-A add • Enter details     │
  └───────────────────────────────────────────────────┘
  ```

- **Search & Table:** `PluginsSearchComponent` filters by name and tags, while
  `PluginsTableComponent` shows status (`✓ Running`, `✗ Stopped`, `! Error`), transport, auth
  summary, and tags. Keyboard navigation supports `j/k`, `↑/↓`, and global focus cycling.

  ```text
  ┌─ Search ───────────────────────────────────────────┐
  │ / search ▏vector                                   │
  └────────────────────────────────────────────────────┘
  ┌─ Plugins Table ─────────────────────────────────────┐
  │ Name          Status   Transport   Auth    Tags     │
  │ vector-store  !        http/sse    token   ml,search│
  │ github        ✓        stdio       keyring code,gh  │
  │ pg-remote     ✗        http/sse    basic   db       │
  └─────────────────────────────────────────────────────┘
  ```

- **Details View:** `PluginsDetailsComponent` now renders a single, stacked details view. The left pane stacks Overview, Health, Env, and Logs separated by horizontal rules; the right pane shows Tools inside a block with a full-height left border. Logs are scrollable with ↑/↓ and display a vertical scrollbar. Quick actions are inline in Overview: [R] Restart, [S] Start, [T] Stop, and [Ctrl-R] Refresh.

  ```text
  ┌────────────────── Plugin Details — github ──────────────────┐
  │ Command   npx -y @mcp/server-github            │ Tools      │
  │ Transport stdio (local)                        │ list_repos │
  │ Tags      code, gh                             │ create_pr  │
  │ Actions   [R] Restart  [S] Start  [T] Stop     │ …          │
  │           [Ctrl-R] Refresh                     │            │
  ├────────────────────────────────────────────────┼────────────┤
  │ Health: ✓ Healthy      Handshake: 180ms        │            │
  │ Last start: 12:41:03   Restarts: -             │            │
  ├────────────────────────────────────────────────┤            │
  │ Env (masked)                                   │            │
  │   GITHUB_TOKEN  •••••••••••  secret ✓          │            │
  │   USER_AGENT    oatty-cli     file   ✓         │            │
  ├────────────────────────────────────────────────┤            │
  │ Logs (recent)                                  │            │
  │   [12:41:03] info handshake ok (180ms)         │            │
  │   ↑/↓ scroll • visible scrollbar ▮             │            │
  └────────────────────────────────────────────────┴────────────┘
  ```

- **Logs Drawer:** `PluginsLogsComponent` displays live ring-buffer entries with follow mode,
  search, OSC52 copy helpers, and export options.

  ```text
  ┌ Logs — github        [f] follow  [/] search  [Y] all ┐
  │ [12:41:03] info handshake ok (180ms)                 │
  │ [12:42:10] warn restart requested                    │
  │ [12:42:15] info tool list_repos succeeded            │
  ├──────────────────────────────────────────────────────┤
  │ ↑↓ scroll • y copy line • Esc close                  │
  └──────────────────────────────────────────────────────┘
  ```

- **Add/Edit Workflow:** `PluginsEditComponent` presents radio-select transport, validates
  configuration (including handshake checks), previews registry impact, and writes through the
  config module.

  ```text
  ┌ Plugins — MCP ─────────────────────────────────────────────┐
  │ / search ▏                                                 │
  ├────────────────────────────────────────────────────────────┤
  │ ┌ Add Plugin ────────────────────────────────────────────┐ │
  │ │ Name: github-local                                    │ │
  │ │ Transport: (✓) Local   ( ) Remote                     │ │
  │ │ Command: npx                                          │ │
  │ │ Args: -y @mcp/server-github                           │ │
  │ │ Tags: code,gh                                         │ │
  │ │                                                       │ │
  │ │ [Ctrl-V] validate   [Ctrl-A] apply   [Esc] cancel     │ │
  │ └───────────────────────────────────────────────────────┘ │
  │ ┌ Plugins Table ─────────────────────────────────────────┐ │
  │ │ Name        Status   Transport      Tags               │ │
  │ │ github      ✓        stdio          code,gh            │ │
  │ │ remote-api  ✗        http/sse       api                │ │
  │ └────────────────────────────────────────────────────────┘ │
  └────────────────────────────────────────────────────────────┘
  ```

- **Overlays:** Components use centered modal rectangles with `widgets::Clear`, respecting focus
  rings and global Escape handling.

## Workflows

### Plugin Lifecycle

1. Engine boot autostarts enabled plugins asynchronously. Each start attempt is audited.
2. `LifecycleManager` enforces startup/shutdown timeouts (30s/10s default) and exponential backoff
   restarts with a maximum of three attempts.
3. Manual actions (start, stop, restart) are delegated through the component tree to
   `PluginEngine::start_plugin` / `stop_plugin`, ensuring registry and command catalogs stay in
   sync.

### Installation & Editing

1. Adding a plugin collects fields via the TUI overlay, validates the name and transport-specific
   requirements, and saves the config.
2. After save, the engine reloads configuration, registers lifecycle tracking, and triggers
   autostart if the plugin is enabled.
3. Editing secrets/env values respects masking and writes through the config module without leaking
   sensitive data in logs.

### Tool Invocation

1. When the CLI executes a synthetic command produced from MCP tools, it calls back into
   `PluginEngine::invoke_tool`.
2. The engine forwards the invocation to the corresponding `McpClient`, captures the JSON payload,
   and returns an `ExecOutcome::Mcp` with both textual and structured results.
3. Errors propagate with detailed audit records and `PluginStatus::Error` transitions in the
   registry.

### Logging & Observation

1. Stdio plugins route stderr into the log ring buffer using spawned asynchronous readers.
2. HTTP/SSE transports reuse the rmcp client for diagnostic logging and expose health probes via
   `McpClient::health_check`.
3. Log exports can be redacted (default) or raw and are written asynchronously to user-provided
   paths.

## Security and Compliance

- Secrets never persist in plaintext within config files, logs, or audit entries.
- Audit logs enforce restrictive permissions (0600 on Unix) and redact sensitive metadata.
- OAuth token lookups rely on the OS keyring; missing entries fall back to config-provided tokens.
- All remote transports validate scheme and rely on TLS (for `https`) via reqwest defaults.

## Future Enhancements

- Automated provider discovery that registers value providers from tool metadata without manual
  configuration.
- Expanded auth schemes (e.g., OAuth device flow) surfaced through the `auth` block.
- Additional health signals (e.g., uptime, request error rates) exposed in the details panel and
  provider APIs.

This specification should be kept in sync with crate-level documentation and any architectural
decisions captured under `plans/` to ensure contributors have an accurate map of the MCP plugin
system.
