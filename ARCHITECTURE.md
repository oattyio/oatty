# Architecture Overview

- Core Crates: `crates/cli` (binary entry, args dispatch, launches TUI), `crates/tui` (Ratatui UI, focus, autocomplete,
  tables, theme), `crates/registry` (loads command manifest), `crates/registry-gen` (OpenAPI → manifest generator +
  provider inference), `crates/engine` (workflow orchestration, templating, step I/O), `crates/api` (`reqwest` client,
  auth, retries), `crates/util` (logging, redaction, caching, JSON helpers), `crates/mcp` (MCP plugin infrastructure,
  client management, logging).

- Command Spec & Manifest: Commands are identified by `group` + `name` (e.g., `apps info`). Fields are `positional_args`
  or `flags`. The manifest is generated at build-time by `crates/registry-gen` from the API schema and embeds per-field
  `provider` metadata directly in `CommandSpec`. Colon-delimited identifiers (e.g., `apps:list`) are deprecated; the
  engine logs and rejects them in favor of the canonical `<group> <name>` form.

- Output Schema Summaries: Each `CommandSpec` includes an enriched `SchemaProperty` tree that retains
  JSON type, required keys, array item shapes, enumerated literals, optional `format`, and workflow
  tags. The Field Picker and auto-mapping heuristics read this metadata to badge candidates and
  disambiguate provider arguments.

    - Provider shape (embedded):
        - `ValueProvider::Command { command_id: String, binds: Vec<Bind> }`
        - `Bind { provider_key: String, from: String }`
    - Example: `apps info <app>` → positional `app` carries `provider: Command { command_id: "apps:list", binds: [] }`
    - Example with bindings: `addons info <app> <addon>` → positional `addon` carries
      `provider: Command { command_id: "addons list", binds: [{ provider_key: "app", from: "app" }] }`

- Provider Inference (registry-gen): Two-pass inference attaches providers conservatively.
    - Build `<group> <name>` index, detect groups with `list`.
    - Positionals: walk `spec.path` and bind provider from the immediately preceding concrete segment (e.g.,
      `/addons/{addon}/config` → group `addons` → `addons list`).
    - Flags: map flag names to plural groups via a synonyms table + conservative pluralization; bind `<group> list` when
      present.
    - High-reliability bindings:
        - Bind provider path placeholders from earlier consumer positionals (via name synonyms).
        - Bind required provider flags only when they are in a safe set (app/app_id, addon/addon_id, pipeline,
          team/team_name, space/space_id, region, stack) and can be sourced either from earlier positionals or from
          consumer required flags (same/synonym name).
        - If any required provider input cannot be satisfied, no provider is attached for that field.

- **Value Providers:** Pluggable sources for dynamic suggestions:
    - **core:** API-backed (apps, addons, permissions, users).
    - **workflow:** read prior step outputs (e.g., `workflow:from(task, jsonpath)`).
    - **plugins (MCP):** external providers via Model Context Protocol. MCP plugins are configured in
      `~/.config/oatty/mcp.json` and provide tools that can be used as value providers. The `crates/mcp` infrastructure
      manages plugin lifecycle, health monitoring, and bridges MCP tools to the provider system. Providers declare
      inputs (e.g., `partial`, `argOrFlag`), outputs (`label`, `value`, `meta`), TTL, and auth needs. See
      `specs/PLUGINS.md` and `plans/VALUE_PROVIDERS.md`.

- Execution Flow: CLI/TUI loads manifest; suggestion building queries providers asynchronously with caching. Command
  execution uses `exec_remote` (util) with proper Range header handling and logs/pagination parsing. The workflow engine
  supports templating and multi-step runs.

- Value Providers at Runtime:
    - Registry-backed provider (TUI) reads `provider` metadata from the manifest, resolves bound inputs from the user’s
      current input (earlier positionals + provided flags), and fetches via the same HTTP helpers with a short TTL
      cache. When required bound inputs are missing, it returns no suggestions (UI remains predictable).
    - Engine provider fetch resolves provider paths with `build_path` and includes leftover bound inputs as query params
      for GET/DELETE; non-GET requests receive JSON bodies.

- **Workflow Engine:** Runs multi-step workflows, manages dependencies, passes step outputs into later steps/providers,
  and ensures deterministic, replayable runs. See `plans/WORKFLOWS.md`

- **TUI Layer:** Guided/Power modes, autocomplete surfaces provider results, focus management for forms/tables, theming
  from `plans/THEME.md`, accessibility + UX patterns from `plans/FOCUS_MANAGEMENT.md`, general guidelines from
  `plans/UX_GUIDELINES.md`, autocomplete from `plans/AUTOCOMPLETE.md` and workflow.
    - State ownership: top-level components (palette, browser, logs, help, table) keep their state on `app::App` for
      coordination; nested subcomponents (e.g., pagination inside the table) may keep private state and be composed by
      the parent. See AGENTS.md for the component cookbook.
    - Shared view helpers under `ui/components/common/` encapsulate reusable Ratatui widgets (e.g., `ResultsTableView`).
      Controllers implement `Component`, hold no long-lived data, and pass the appropriate slice of `App` state into the
      shared renderers so multiple instances can coexist without duplicating drawing logic.
    - Runtime: The event loop and input routing live in `crates/tui/src/ui/runtime.rs`. It handles terminal
      setup/teardown, emits a constant animation tick (~8 FPS), routes input to focused components, and renders only
      when `App` marks itself dirty. This ensures smooth animations without unnecessary redraws while idle.
    - Mouse input: Components that expose clickable buttons or focusable inputs implement `handle_mouse_events`
      alongside keyboard handlers. Each render pass caches the target rectangles (`state.last_area`,
      `state.per_item_areas`), and a helper (`find_target_index_by_mouse_position`) maps the cursor location from
      `MouseEventKind::Down(MouseButton::Left)` into the matching button. Once identified, the component reuses the
      keyboard path by toggling the same focus flag and calling its `handle_key_events` with an `Enter` key event,
      ensuring mouse and keyboard paths stay in sync without duplicating logic.
    - Message/Effect Architecture: The TUI is TEA-inspired: it keeps a single `App` model, distinguishes between `Msg`
      and `Effect`, and routes side effects through `Cmd`s, while intentionally allowing local-first state mutation and
      a few synchronous effects for ergonomics. See `specs/MSG_EFFECT_ARCHITECTURE.md` for the full description of these
      patterns and their pragmatic deviations.

## Logging during TUI

- To prevent out-of-band terminal output from overlaying the TUI while using the alternate screen, the CLI configures
  tracing to write through a gated stderr writer.
- Implementation: `crates/cli/src/main.rs` defines a static `TUI_ACTIVE: AtomicBool` and a `GatedStderr` writer. While
  `TUI_ACTIVE` is true (set just before launching the TUI), all tracing output to stderr is dropped; it is restored
  immediately after TUI exits.
- MCP plugin logs are collected by the `LogManager` into in-memory ring buffers and shown inside the TUI; they are not
  forwarded to the global tracing subscriber during TUI to avoid overlays.
- In CLI mode (when running commands non-interactively), tracing logs follow the `OATTY_LOG` level and are emitted to
  stderr normally.

## Focus Management

See plans/FOCUS_MANAGEMENT.md for details on the rat-focus model (flags, local focus rings, and traversal rules). It
documents the root ring (palette/logs), browser rings, and the table ↔ pagination navigation flow (Grid ↔ First ↔ Prev ↔
Next ↔ Last buttons).

- Example: `addons info <app> <addon>`
    - Provider: `addons list` exists at `/apps/{app}/addons`.
    - Binding: `{ provider_key: "app", from: "app" }` attaches to the `addon` positional.
    - TUI resolves `app` from the user's input, fetches app-scoped addon names, and suggests values for `addon`.

## MCP Plugin Architecture

The MCP (Model Context Protocol) plugin system extends the CLI with external tools and value providers through a
standardized protocol.

### Core Components (`crates/mcp/`)

- **PluginEngine** (`src/plugin/engine.rs`): Main orchestration layer that manages plugin lifecycle (
  start/stop/restart), coordinates with the client manager, synthesizes registry command specs from MCP tools, and
  maintains plugin registry state.

- **McpClientManager** (`src/client/manager.rs`): Manages MCP client connections, handles transport selection (
  stdio/HTTP), and provides health monitoring for all active plugins.

- **Transport Layer** (`src/client/`):
    - **StdioTransport**: Spawns child processes and communicates via stdin/stdout using the `rmcp` crate's
      `TokioChildProcess` transport.
    - **HttpTransport**: Provides HTTP/SSE connectivity via reqwest-backed clients, optional auth headers/keyring
      lookups, and SSE event handling for remote MCP servers (`SseClientTransport`).

- **Configuration** (`src/config/`): Loads and validates `~/.config/oatty/mcp.json` with support for environment
  variable interpolation (`${env:NAME}`) and secret resolution (`${secret:NAME}` via OS keychain).

- **Logging** (`src/logging/`): Centralized logging system with redaction for sensitive values, audit trails, and ring
  buffer storage for TUI display.

- **Provider Integration** (`src/provider/`): Bridges MCP tools to the existing `ValueProvider` system, allowing MCP
  plugins to provide dynamic suggestions for command arguments.

### Plugin Lifecycle

1. **Configuration**: Plugins defined in `mcp.json` with transport-specific settings (command/args for stdio,
   baseUrl/headers for HTTP).

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
- **TUI Integration**: Plugin status, logs, and management exposed through TUI components (see `specs/PLUGINS.md`).
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
      "ssePath": "events",
      "headers": {
        "Authorization": "Bearer ${secret:EXAMPLE_TOKEN}"
      },
      "auth": {
        "scheme": "basic",
        "username": "${secret:REMOTE_USER}",
        "password": "${secret:REMOTE_PASS}"
      }
    }
  }
}
```
