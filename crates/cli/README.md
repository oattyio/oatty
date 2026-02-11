Oatty CLI (Rust) — Binary Entry

Overview
- Entrypoint for the Rust-based Oatty CLI workspace.
- Builds a dynamic Clap command tree from the registry (derived from OpenAPI documents) and delegates:
  - No subcommand: launches the interactive TUI (terminal UI).
  - Group + command: executes the command via the API client.
  - Workflow subcommands (feature-gated): preview/run YAML/JSON workflows using the engine.

Key Features
- Dynamic CLI: Commands, positionals, and flags come from the schema-derived registry.
- Global flags:
  - `--json`: print raw JSON responses (when implemented for each command).
  - `--verbose`: more verbose logs (via `RUST_LOG`).
- TUI handoff: Running `oatty` with no subcommands opens the TUI (`oatty-tui`).
- Workflows: `oatty workflow ...` commands are always available and operate on local files or workflows bundled in catalogs.
- Import command: `oatty import <path|url>` auto-detects and imports either OpenAPI catalogs or workflow manifests.

Auth & Config
- Auth:
  - Configure authorization headers per catalog (TUI Library component or registry config).
- Base URL: derived from OpenAPI `servers` metadata in the registry.
- Headers: `Accept: application/json`, plus a sensible `User-Agent`.

Usage
- Launch TUI (no subcommand):
  - `cargo run -p oatty-cli` (or installed binary `oatty`)
- Execute a command directly:
  - `cargo run -p oatty-cli -- apps info <app>`
- Enable workflows:
  - `cargo run -p oatty-cli -- workflow preview --file workflows/create_app_and_db.yaml`
- Import a catalog from OpenAPI:
  - `cargo run -p oatty-cli -- import schemas/samples/render-public-api.json --kind catalog`
- Import a workflow manifest:
  - `cargo run -p oatty-cli -- import workflows/create_app_and_db.yaml --kind workflow`

Development
- Built from `oatty_registry::Registry::from_embedded_schema()`, which walks the OpenAPI-derived manifest and produces `CommandSpec` entries.
- CLI glue in `src/main.rs`:
  - Builds Clap from registry.
  - Routes to TUI when no subcommand.
  - Implements workflow subcommands under `workflow`.
  - Executes requests with `oatty_api::OattyClient`.

Troubleshooting
- “Unknown command …” — Verify the group/sub form (e.g., `apps info`, not `apps:info`).
- 401 Unauthorized — Configure the catalog's authorization headers.
- Network errors — Check connectivity, proxies, and TLS; `RUST_LOG=info` for more detail.
