# Quick Start

Get Oatty running locally in a few minutes.

## Prerequisites

- Rust stable (`rustup`)
- A terminal that supports TUI rendering

## Build

```bash
cargo build --workspace
```

## Run

```bash
# TUI mode
cargo run -p oatty
```

```bash
# CLI mode
cargo run -p oatty -- --help
```

## First-time catalog import (required for command surface)

If no registry catalogs are configured yet:

1. Open the TUI: `cargo run -p oatty`
2. Open Library and import an OpenAPI document (file or URL)
3. Choose a command prefix and save

This writes:

- registry config: `~/.config/oatty/registry.json`
- catalog manifests: `~/.config/oatty/catalogs/`

## Workflows

Workflow manifests are runtime filesystem assets (not build-time bundled).

- default directory: `~/.config/oatty/workflows`
- override with: `REGISTRY_WORKFLOWS_PATH`

Common commands:

```bash
cargo run -p oatty -- workflow list
cargo run -p oatty -- workflow preview --file workflows/create_app_and_db.yaml
```

## Useful environment variables

```bash
OATTY_LOG=debug
TUI_THEME=dracula
MCP_CONFIG_PATH=~/.config/oatty/mcp.json
REGISTRY_CONFIG_PATH=~/.config/oatty/registry.json
REGISTRY_CATALOGS_PATH=~/.config/oatty/catalogs
REGISTRY_WORKFLOWS_PATH=~/.config/oatty/workflows
RUST_BACKTRACE=1
```

## Validate your setup

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all --check
```
