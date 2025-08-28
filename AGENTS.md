# Repository Guidelines

## Project Structure & Module Organization
- Workspace crates: `crates/cli` (binary), `crates/tui`, `crates/registry`, `crates/engine`, `crates/api`, `crates/util`.
- Supporting assets: `schemas/` (schemas), `workflows/` (sample workflow YAML/JSON), `plans/` (design notes).
- Tooling: `Cargo.toml`, `rustfmt.toml`, `.github/`, `.vscode/`.

Example:
```
crates/
  cli/src/main.rs      # entrypoint
  registry/            # schema → command registry
  tui/                 # Ratatui UI
```

## Build, Test, and Development Commands
- Build all: `cargo build --workspace` — compiles every crate.
- Run CLI: `cargo run -p heroku-cli -- <group> <command> [flags]`.
- TUI mode: `cargo run -p heroku-cli` (no args) — launches Ratatui UI.
- Tests: `cargo test --workspace` — run unit/integration tests.
- Lint: `cargo clippy --workspace -- -D warnings` — fail on warnings.
- Format: `cargo fmt --all` — apply `rustfmt` settings.
- Helpful env: `RUST_LOG=debug`, `HEROKU_API_KEY=…`, `FEATURE_WORKFLOWS=1`, `DEBUG=1`.

## Coding Style & Naming Conventions
- Rust 2024; 4‑space indent; max line width 100 (`rustfmt.toml`).
- Naming: modules/files `snake_case`; types/enums `PascalCase`; functions/vars `snake_case`; constants `SCREAMING_SNAKE_CASE`.
- Errors: apps use `anyhow::Result`; libraries prefer `thiserror`.
- Keep changes minimal; run `cargo fmt` and fix all `clippy` issues before pushing.

## Testing Guidelines
- Unit tests inline with code: `#[cfg(test)] mod tests { … }`.
- Integration tests in `tests/` per crate when needed.
- Async: use `#[tokio::test]` where appropriate.
- Ensure deterministic output; run `cargo test --workspace` locally.

## Commit & Pull Request Guidelines
- Commits: Conventional Commits (e.g., `feat:`, `fix:`, `refactor:`). Follow recent `feat:` usage.
- PRs must include: clear summary, linked issues, rationale; before/after screenshots or terminal output for TUI/CLI; validation steps (exact build/run/test commands).
- Checklist: `cargo fmt` + `clippy` clean; no stray `dbg!`/`println!`.

## Security & Configuration Tips
- Never commit secrets; prefer `HEROKU_API_KEY` over `~/.netrc`.
- Redaction utilities mask sensitive values in logs/dry‑runs; still avoid pasting tokens.
- Network uses `reqwest` + TLS; set `RUST_LOG=info|debug` for diagnostics.
