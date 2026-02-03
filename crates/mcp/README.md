# oatty-mcp

Model Context Protocol (MCP) plugin infrastructure for the Oatty CLI.

This crate provides the core building blocks to discover, configure, start/stop, and interact with MCP-enabled plugins over stdio or HTTP/SSE transports. It includes a plugin engine, client/transport management, provider integration, configuration loading/validation, health monitoring, and logging/auditing with redaction.

## Features

- Plugin engine orchestrating lifecycle, registry, health, and logs
- Stdio and HTTP/SSE transports built on `rmcp`
- Managed clients with connection status, health checks, and error tracking
- Synchronous config loader for `~/.config/oatty/mcp.json` with interpolation and validation
- Provider bridge to expose MCP tools to the broader engine provider system
- Structured logging, redaction, ring buffers, and audit log to file

## Crate layout

- `lib.rs` — public API and re-exports
- `client/` — transports, client wrapper, health monitor, client manager
  - `stdio.rs` — spawn MCP servers as local processes (Tokio child)
  - `http.rs` — HTTP/SSE transport implementation with reqwest client + SSE listener
  - `manager.rs` — start/stop plugins, manage `McpClient`s
  - `health.rs` — periodic health checks and aggregation
- `config/` — config models, interpolation, validation, file I/O
- `plugin/` — `PluginEngine`, registry, and lifecycle coordination
- `provider/` — `McpProvider` and `McpProviderAdapter` to map MCP tools to providers
- `logging/` — log formatting/redaction, ring buffers, and audit logger
- `types/` — shared types and strongly-typed error/status models

## Concepts and data flow

1. Configuration is loaded synchronously via `config::load_config()` from `MCP_CONFIG_PATH` or `~/.config/oatty/mcp.json`.
   - `${env:FOO}` and `${secret:NAME}` are resolved via process env and OS keychain (`keyring-rs`).
   - Config is validated (naming, transport presence, headers/env constraints).
2. `PluginEngine::new(config)` wires together:
   - `McpClientManager` (transports + connections)
   - `PluginRegistry` (metadata, tags, status)
   - `LifecycleManager` (start/stop/restart with timeouts/backoff)
   - `LogManager` (log buffers + audit)
3. `PluginEngine::start()` bootstraps the client manager and registers configured plugins.
4. Starting a plugin creates a transport (`stdio` | `http`), establishes an MCP `service::Peer<RoleClient>`, and runs periodic health checks.
5. The provider layer (`provider::McpProvider`) wraps a plugin tool, introspects its metadata (`list_tools`), builds a provider contract, and executes tool calls via `peer.call_tool`.

## Configuration

Default path: `~/.config/oatty/mcp.json` (override with `MCP_CONFIG_PATH`). Example:

```json
{
  "mcpServers": {
    "server-name": {
      "command": "node",
      "args": ["-e", "require('@mcp/server').start()"],
      "env": {
        "FOO": "bar",
        "OATTY_API_TOKEN": "${env:OATTY_API_TOKEN}"
      },
      "cwd": "/path/optional",
      "disabled": false,
      "tags": ["code", "gh"]
    },
    "remote-example": {
      "baseUrl": "https://mcp.example.com",
      "headers": {
        "Authorization": "Bearer ${secret:EXAMPLE_TOKEN}"
      },
      "disabled": false
    }
  }
}
```

Notes:
- Stdio requires `command` (and optional `args`, `env`, `cwd`).
- HTTP/SSE requires `baseUrl` (and optional `headers`).
- `${env:NAME}` pulls from the environment; `${secret:NAME}` resolves via OS keychain service `oatty-mcp`.
- Server names must match `^[a-z0-9._-]+$`.
- Oatty expects token-based auth (static bearer tokens, API keys, etc.) via `headers` or `${secret:...}`. OAuth/PKCE flows are not yet integrated; use an `Authorization` header or OpenAPI import instead.

TUI Add Plugin view:
- The Oatty TUI provides an Add Plugin panel with a transport radio selector: `Transport: [✓] Local   [ ] Remote`.
- Selecting Local exposes `command` and `args`; Remote exposes `baseUrl`.
- Keyboard: when the radio is focused, use Left/Right to change and Space/Enter to toggle.

Programmatic APIs:
- `config::load_config()` reads, interpolates, and validates without requiring a Tokio runtime.
- `config::save_config(&cfg)` writes back using pretty JSON.

## Public API highlights

- `PluginEngine`
  - `start()`, `stop()` — manage runtime
  - `start_plugin(name)`, `stop_plugin(name)`, `restart_plugin(name)`
  - `get_plugin_detail(name) -> PluginDetail`
  - `list_plugins() -> Vec<PluginDetail>`
  - `get_plugin_status(name) -> PluginStatus`
- `client::McpClientManager`
  - `start_plugin(name)`, `stop_plugin(name)`, `restart_plugin(name)`
  - `get_client(name) -> Option<Arc<Mutex<McpClient>>>`
  - `get_plugin_health(name) -> Option<HealthStatus>`
- `provider::McpProvider`
  - `new(plugin, tool, engine)` -> `McpProvider`
  - `initialize()` -> fetch tool metadata and build contract
  - `fetch_values(arguments) -> Vec<Value>`
  - `get_contract() -> ProviderContract`

Re-exports from `rmcp` allow calling tools and working with content types.

## Usage examples

Create and start the engine:

```rust
use oatty_mcp::{config, PluginEngine};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = config::load_config()?; // synchronous file read/interpolation
    let engine = PluginEngine::new(cfg)?;
    engine.start().await?;

    // Start a specific plugin by name (from config mcpServers keys)
    // engine.start_plugin("server-name").await?;

    Ok(())
}
```

Call a tool through a provider:

```rust
use oatty_mcp::{plugin::PluginEngine, provider::{McpProvider, McpProviderOps}};
use serde_json::json;
use std::sync::Arc;

# async fn demo(engine: Arc<PluginEngine>) -> anyhow::Result<()> {
    // Ensure the plugin is running first (via engine/client manager)
    // engine.start_plugin("server-name").await?;

    let mut provider = McpProvider::new("server-name", "list-repos", Arc::clone(&engine))?;
    provider.initialize().await?; // fetch tool metadata, build contract

    let args = serde_json::Map::from_iter([
        ("owner".to_string(), json!("example"))
    ]);

    let items = provider.fetch_values(&args).await?;
    for item in items { println!("{}", item); }
#   Ok(())
# }
```

## Logging and auditing

- Logs are buffered per plugin using a ring buffer (`logging::LogRingBuffer`) and formatted with redaction (`logging::LogFormatter`).
- Audit events (start, stop, restart, tool invoke, health checks) are written to `~/.config/oatty/mcp-audit.jsonl` by default.
- Use `LogManager::export_logs(plugin, path)` to export with redaction, or `export_logs_with_redaction(..., false)` for raw logs.

## Health monitoring

- `HealthMonitor` tracks plugin health with periodic checks.
- Transports implement `health_check()`; `McpClient::health_check()` updates `HealthStatus` (latency, errors).

## HTTP/SSE transport

- `client/http.rs` builds reqwest clients with optional auth headers, resolves `ssePath`, and
  constructs `SseClientTransport` handles.
- OAuth-style tokens can be pulled from the OS keyring; Basic credentials support `${secret:}`
  interpolation before request execution.

## Environment

Helpful env for development/integration:
- `RUST_LOG=debug`
- `MCP_CONFIG_PATH=~/.config/oatty/mcp.json`

## Error handling

- Binaries and user flows use `anyhow::Result` in this crate’s constructors.
- Library errors modelled via `types::errors` (`McpError`, `PluginError`, `LogError`).

## Development

- Build: `cargo build --workspace`
- Tests: `cargo test --workspace`
- Lint: `cargo clippy --workspace -- -D warnings`
- Format: `cargo fmt --all`

Code style:
- Rust 2024, 4 spaces indent, width 100, consistent naming and error handling (`thiserror`).

## Security

- Never log secrets. Redaction utilities are applied in formatters.
- `${secret:NAME}` uses the OS keychain (`keyring-rs`) with the service name `oatty-mcp`.

## License

See the workspace license in the repository root.
