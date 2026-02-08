# Development Guide

## Prerequisites

1. Rust stable toolchain (managed by `rustup`)
2. `clippy` and `rustfmt`

```bash
rustup component add clippy rustfmt
```

## Build and test

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all --check
```

## Running

```bash
# TUI mode
cargo run -p oatty

# CLI mode
cargo run -p oatty -- --help
```

## Runtime configuration

```bash
OATTY_LOG=debug
TUI_THEME=dracula
MCP_CONFIG_PATH=~/.config/oatty/mcp.json
REGISTRY_CONFIG_PATH=~/.config/oatty/registry.json
REGISTRY_CATALOGS_PATH=~/.config/oatty/catalogs
REGISTRY_WORKFLOWS_PATH=~/.config/oatty/workflows
RUST_BACKTRACE=1
```

## Catalog import workflow

If the registry is empty:

1. Open TUI (`cargo run -p oatty`)
2. Use Library import for an OpenAPI file or URL
3. Save catalog metadata + prefix

Artifacts are persisted under `~/.config/oatty` unless overridden by environment variables.

## Workflow development

- Runtime workflow manifests are read from `default_workflows_path()`.
- For source-controlled authoring, keep workflow files in-repo and use MCP workflow import/export tools to sync with runtime storage.

## Project layout

```text
crates/
  cli/          Binary entrypoint
  tui/          Terminal UI
  registry/     Runtime catalog/workflow loading
  registry-gen/ OpenAPI manifest generation
  engine/       Workflow execution
  api/          HTTP helpers
  mcp/          MCP server and plugin runtime
  util/         Shared utilities
  types/        Shared model types
schemas/        OpenAPI inputs and samples
specs/          As-built product specifications
```

## Troubleshooting

- **Empty command surface**: import at least one catalog in Library.
- **No workflow entries**: verify `REGISTRY_WORKFLOWS_PATH` and file extensions (`.yaml`, `.yml`, `.json`).
- **MCP issues**: validate `MCP_CONFIG_PATH` and inspect Logs view.
- **TUI rendering noise**: avoid writing to stdout/stderr from dependencies while alternate screen is active.

## References

- `README.md`
- `ARCHITECTURE.md`
- `CONTRIBUTING.md`
- `specs/`
