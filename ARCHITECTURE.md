# Architecture Overview

- Core Crates: `crates/cli` (binary entry, args dispatch, launches TUI), `crates/tui` (Ratatui UI, focus, autocomplete, tables, theme), `crates/registry` (loads command manifest), `crates/registry-gen` (schema → manifest generator + provider inference), `crates/engine` (workflow orchestration, templating, step I/O), `crates/api` (`reqwest` client, auth, retries), `crates/util` (logging, redaction, caching, JSON helpers), `crates/mcp` (MCP plugin infrastructure, client management, logging).

- Command Spec & Manifest: Commands are identified by `group` + `name` (e.g., `apps info`). Fields are `positional_args` or `flags`. The manifest is generated at build-time by `crates/registry-gen` from the API schema and embeds per-field `provider` metadata directly in `CommandSpec`.

  - Provider shape (embedded):
    - `ValueProvider::Command { command_id: String, binds: Vec<Bind> }`
    - `Bind { provider_key: String, from: String }`
  - Example: `apps info <app>` → positional `app` carries `provider: Command { command_id: "apps:list", binds: [] }`
  - Example with bindings: `addons info <app> <addon>` → positional `addon` carries `provider: Command { command_id: "addons:list", binds: [{ provider_key: "app", from: "app" }] }`

- Provider Inference (registry-gen): Two-pass inference attaches providers conservatively.
  - Build `<group>:<name>` index, detect groups with `list`.
  - Positionals: walk `spec.path` and bind provider from the immediately preceding concrete segment (e.g., `/addons/{addon}/config` → group `addons` → `addons:list`).
  - Flags: map flag names to plural groups via a synonyms table + conservative pluralization; bind `<group>:list` when present.
  - High-reliability bindings:
    - Bind provider path placeholders from earlier consumer positionals (via name synonyms).
    - Bind required provider flags only when they are in a safe set (app/app_id, addon/addon_id, pipeline, team/team_name, space/space_id, region, stack) and can be sourced either from earlier positionals or from consumer required flags (same/synonym name).
    - If any required provider input cannot be satisfied, no provider is attached for that field.

- **Value Providers:** Pluggable sources for dynamic suggestions:
  - **core:** API-backed (apps, addons, permissions, users).
  - **workflow:** read prior step outputs (e.g., `workflow:from(task, jsonpath)`).
  - **plugins (MCP):** external providers via Model Context Protocol. MCP plugins are configured in `~/.config/heroku/mcp.json` and provide tools that can be used as value providers. The `crates/mcp` infrastructure manages plugin lifecycle, health monitoring, and bridges MCP tools to the provider system. Providers declare inputs (e.g., `partial`, `argOrFlag`), outputs (`label`, `value`, `meta`), TTL, and auth needs. See `plans/PLUGINS.md` and `plans/VALUE_PROVIDERS.md`.

- Execution Flow: CLI/TUI loads manifest; suggestion building queries providers asynchronously with caching. Command execution uses `exec_remote` (util) with proper Range header handling and logs/pagination parsing. The workflow engine supports templating and multi-step runs.

- Value Providers at Runtime:
  - Registry-backed provider (TUI) reads `provider` metadata from the manifest, resolves bound inputs from the user’s current input (earlier positionals + provided flags), and fetches via the same HTTP helpers with a short TTL cache. When required bound inputs are missing, it returns no suggestions (UI remains predictable).
  - Engine provider fetch resolves provider paths with `build_path` and includes leftover bound inputs as query params for GET/DELETE; non-GET requests receive JSON bodies.

- **Workflow Engine:** Runs multi-step workflows, manages dependencies, passes step outputs into later steps/providers, and ensures deterministic, replayable runs. See `plans/WORKFLOWS.md`

- **TUI Layer:** Guided/Power modes, autocomplete surfaces provider results, focus management for forms/tables, theming from `plans/THEME.md`, accessibility + UX patterns from `plans/FOCUS_MANAGEMENT.md`, general guidelines from `plans/UX_GUIDELINES.md`, autocomplete from `plans/AUTOCOMPLETE.md` and workflow.
  - State ownership: top-level components (palette, browser, logs, help, table) keep their state on `app::App` for coordination; nested subcomponents (e.g., pagination inside the table) may keep private state and be composed by the parent. See AGENTS.md for the component cookbook.
  - Runtime: The event loop and input routing live in `crates/tui/src/ui/runtime.rs`. It handles terminal setup/teardown, emits a constant animation tick (~8 FPS), routes input to focused components, and renders only when `App` marks itself dirty. This ensures smooth animations without unnecessary redraws while idle.

## Focus Management

See plans/FOCUS_MANAGEMENT.md for details on the rat-focus model (flags, local focus rings, and traversal rules). It documents the root ring (palette/logs), browser rings, and the table ↔ pagination navigation flow (Grid ↔ First ↔ Prev ↔ Next ↔ Last buttons).

- API & Security: `reqwest` + TLS; auth via `HEROKU_API_KEY`. Redaction patterns (`token`, `password`, `secret`, etc.) applied to logs. Provider results are cached with a TTL in the TUI.

- Example: `addons info <app> <addon>`
  - Provider: `addons:list` exists at `/apps/{app}/addons`.
  - Binding: `{ provider_key: "app", from: "app" }` attaches to the `addon` positional.
  - TUI resolves `app` from the user's input, fetches app-scoped addon names, and suggests values for `addon`.

## MCP Plugin Architecture

The MCP (Model Context Protocol) plugin system extends the CLI with external tools and value providers through a standardized protocol.

### Core Components (`crates/mcp/`)

- **PluginEngine** (`src/plugin/engine.rs`): Main orchestration layer that manages plugin lifecycle (start/stop/restart), coordinates with the client manager, and maintains plugin registry state.

- **McpClientManager** (`src/client/manager.rs`): Manages MCP client connections, handles transport selection (stdio/HTTP), and provides health monitoring for all active plugins.

- **Transport Layer** (`src/client/`):
  - **StdioTransport**: Spawns child processes and communicates via stdin/stdout using the `rmcp` crate's `TokioChildProcess` transport.
  - **HttpTransport**: Placeholder for HTTP/SSE transport (ready for implementation with actual MCP-over-HTTP protocol).

- **Configuration** (`src/config/`): Loads and validates `~/.config/heroku/mcp.json` with support for environment variable interpolation (`${env:NAME}`) and secret resolution (`${secret:NAME}` via OS keychain).

- **Logging** (`src/logging/`): Centralized logging system with redaction for sensitive values, audit trails, and ring buffer storage for TUI display.

- **Provider Integration** (`src/provider/`): Bridges MCP tools to the existing `ValueProvider` system, allowing MCP plugins to provide dynamic suggestions for command arguments.

### Plugin Lifecycle

1. **Configuration**: Plugins defined in `mcp.json` with transport-specific settings (command/args for stdio, baseUrl/headers for HTTP).

2. **Discovery**: Plugin engine loads configuration and registers available plugins.

3. **Lazy Loading**: Plugins started on-demand when first accessed or auto-started based on configuration.

4. **Health Monitoring**: Continuous health checks with exponential backoff on failures, handshake latency tracking.

5. **Tool Integration**: MCP tools exposed as value providers, integrated with existing suggestion system.

### Security & Isolation

- **Out-of-Process**: Stdio plugins run as separate processes, preventing crashes from affecting the main CLI.
- **Secret Management**: Sensitive values stored in OS keychain, never persisted in configuration files.
- **Redaction**: All sensitive values automatically redacted in logs and UI display.
- **Audit Trails**: Complete audit logging of plugin actions with rotation policies.

### Integration Points

- **Value Providers**: MCP tools can be used as dynamic suggestion sources for command arguments.
- **TUI Integration**: Plugin status, logs, and management exposed through TUI components (see `plans/PLUGINS.md`).
- **Workflow Engine**: MCP tools can be invoked as workflow steps for automation.

### Example Configuration

```json
{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": {
        "GITHUB_TOKEN": "${secret:GITHUB_TOKEN}"
      }
    },
    "remote-example": {
      "baseUrl": "https://mcp.example.com",
      "headers": {
        "Authorization": "Bearer ${secret:EXAMPLE_TOKEN}"
      }
    }
  }
}
```
