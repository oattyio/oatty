Heroku CLI (Rust) — Binary Entry

Overview
- Entrypoint for the Rust-based Heroku CLI workspace.
- Builds a dynamic Clap command tree from the registry (derived from the Hyper-Schema) and delegates:
  - No subcommand: launches the interactive TUI (terminal UI).
  - Group + command: executes the command via the API client (with optional dry-run).
  - Workflow subcommands (feature-gated): preview/run YAML/JSON workflows using the engine.

Key Features
- Dynamic CLI: Commands, positionals, and flags come from the schema-derived registry.
- Global flags:
  - `--json`: print raw JSON responses (when implemented for each command).
  - `--dry-run`: print a request plan instead of executing the HTTP call.
  - `--verbose`: more verbose logs (via `RUST_LOG`).
- TUI handoff: Running `heroku` with no subcommands opens the TUI (`heroku-tui`).
- Workflows (optional): `FEATURE_WORKFLOWS=1` enables `heroku workflow ...` commands.

Auth & Config
- Auth precedence (handled by `heroku-api`):
  - `HEROKU_API_KEY` environment variable.
  - `~/.netrc` token (basic parser).
- Default base URL: `https://api.heroku.com`.
- Headers: Accept `application/vnd.heroku+json; version=3`, a sensible `User-Agent`.

Usage
- Launch TUI (no subcommand):
  - `cargo run -p heroku-cli`
- Execute a command directly:
  - `cargo run -p heroku-cli -- apps info <app>`
  - Dry-run: `cargo run -p heroku-cli -- apps list --dry-run`
  - With auth: `HEROKU_API_KEY=... cargo run -p heroku-cli -- apps info <app>`
- Enable workflows:
  - `FEATURE_WORKFLOWS=1 cargo run -p heroku-cli -- workflow preview --file workflows/create_app_and_db.yaml`

Development
- Built from `heroku_registry::Registry::from_embedded_schema()`, which walks the Hyper-Schema and produces `CommandSpec` entries.
- CLI glue in `src/main.rs`:
  - Builds Clap from registry.
  - Routes to TUI when no subcommand.
  - Binds workflow subcommands behind `FEATURE_WORKFLOWS`.
  - Executes requests with `heroku_api::HerokuClient` or prints dry-run plans.

Troubleshooting
- “Unknown command …” — Verify the group/sub form (e.g., `apps info`, not `apps:info`).
- 401 Unauthorized — Set `HEROKU_API_KEY` or configure `~/.netrc`.
- Network errors — Check connectivity, proxies, and TLS; `RUST_LOG=info` for more detail.

