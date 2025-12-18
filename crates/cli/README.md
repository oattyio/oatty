Oatty CLI (Rust) — Binary Entry

Overview
- Entrypoint for the Rust-based Oatty CLI workspace.
- Builds a dynamic Clap command tree from the registry (derived from the Hyper-Schema) and delegates:
  - No subcommand: launches the interactive TUI (terminal UI).
  - Group + command: executes the command via the API client.
  - Workflow subcommands (feature-gated): preview/run YAML/JSON workflows using the engine.

Key Features
- Dynamic CLI: Commands, positionals, and flags come from the schema-derived registry.
- Global flags:
  - `--json`: print raw JSON responses (when implemented for each command).
  - `--verbose`: more verbose logs (via `RUST_LOG`).
- TUI handoff: Running `oatty` with no subcommands opens the TUI (`oatty-tui`).
- Workflows (optional): `FEATURE_WORKFLOWS=1` enables `oatty workflow ...` commands.

Auth & Config
- Auth (handled by `oatty-api`):
  - `HEROKU_API_KEY` environment variable.
- Default base URL: `https://api.heroku.com`.
- Headers: Accept `application/vnd.heroku+json; version=3`, a sensible `User-Agent`.

Usage
- Launch TUI (no subcommand):
  - `cargo run -p oatty-cli` (or installed binary `oatty`)
- Execute a command directly:
  - `cargo run -p oatty-cli -- apps info <app>`
  - With auth: `HEROKU_API_KEY=... cargo run -p oatty-cli -- apps info <app>`
- Enable workflows:
  - `FEATURE_WORKFLOWS=1 cargo run -p oatty-cli -- workflow preview --file workflows/create_app_and_db.yaml`

Development
- Built from `oatty_registry::Registry::from_embedded_schema()`, which walks the Hyper-Schema and produces `CommandSpec` entries.
- CLI glue in `src/main.rs`:
  - Builds Clap from registry.
  - Routes to TUI when no subcommand.
  - Binds workflow subcommands behind `FEATURE_WORKFLOWS`.
  - Executes requests with `oatty_api::OattyClient`.

Troubleshooting
- “Unknown command …” — Verify the group/sub form (e.g., `apps info`, not `apps:info`).
- 401 Unauthorized — Set `HEROKU_API_KEY`.
- Network errors — Check connectivity, proxies, and TLS; `RUST_LOG=info` for more detail.

