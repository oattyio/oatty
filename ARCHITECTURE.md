# Architecture Overview

## Workspace crates

- `crates/cli`: binary entrypoint, CLI routing, TUI startup.
- `crates/tui`: Ratatui UI, focus/state management, library/log/workflow views.
- `crates/registry`: runtime registry config + catalog loading.
- `crates/registry-gen`: OpenAPI -> registry manifest generation.
- `crates/engine`: workflow execution engine.
- `crates/api`: HTTP execution helpers.
- `crates/mcp`: MCP server + plugin runtime + tool routing.
- `crates/util`: shared utilities (redaction, schema helpers, formatting).
- `crates/types`: shared data structures used across crates.

## Runtime data model

- **Catalogs**:
  - Registry config: `~/.config/oatty/registry.json` (or `REGISTRY_CONFIG_PATH`)
  - Catalog files: `~/.config/oatty/catalogs/*.bin` (or `REGISTRY_CATALOGS_PATH`)
- **Workflows**:
  - Runtime manifests: `~/.config/oatty/workflows` (or `REGISTRY_WORKFLOWS_PATH`)
  - Loaded from filesystem at runtime (not bundled at build time)
- **MCP config**:
  - `~/.config/oatty/mcp.json` (or `MCP_CONFIG_PATH`)

## Command and execution flow

1. Registry loads configured catalog manifests from disk.
2. CLI/TUI builds command UX from `CommandSpec` entries.
3. User executes a command:
   - HTTP commands route through API helpers.
   - MCP commands route through plugin/MCP runtime.
4. Output is redacted and rendered in CLI/TUI views.

## Workflow flow

1. Workflow manifests are discovered from runtime workflow storage.
2. Engine resolves inputs and provider bindings.
3. Workflow runner executes ordered steps and emits events/results.
4. MCP tools expose workflow lifecycle operations (`list/get/save/run/cancel/...`) over the same runtime storage.

## TUI architecture

- Single app model (`App`) with component-specific state.
- Top-level components own state in `App`; shared renderers live under `ui/components/common`.
- Focus is managed with `rat_focus::FocusFlag` and focus rings.
- Theme roles/styles are centralized in `ui/theme`.
- Logs are shown in-app and persisted for diagnostics.

## MCP architecture

- Plugin engine manages transport lifecycle (stdio + HTTP/SSE).
- MCP tools are surfaced into command search and execution paths.
- Workflow authoring/runtime tools are served from `crates/mcp/src/server/workflow`.
- Catalog import tools are served from `crates/mcp/src/server/catalog`.

## Related specs

- `specs/LIBRARY.md`
- `specs/COMMAND_SEARCH.md`
- `specs/LOGGING.md`
- `specs/OPENAPI_IMPORT.md`
- `specs/MCP_WORKFLOWS.md`
- `specs/MCP_CATALOG_TOOLS.md`
- `specs/PLUGINS.md`
- `specs/THEME.md`
