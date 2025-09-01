# Repository Guidelines

## Project Structure & Module Organization
- Workspace crates: `crates/cli` (binary), `crates/tui`, `crates/registry`, `crates/engine`, `crates/api`, `crates/util`.
- Supporting assets: `schemas/` (schemas), `workflows/` (sample workflow YAML/JSON), `plans/` (design notes).
- Example layout:
  - `crates/cli/src/main.rs` — CLI entrypoint
  - `crates/tui/` — Ratatui UI
  - `crates/registry/` — schema → command registry
  - Tests live inline (`#[cfg(test)]`) or under each crate’s `tests/`.

## Build, Test, and Development Commands
- Build all: `cargo build --workspace` — compiles every crate.
- Run CLI: `cargo run -p heroku-cli -- <group> <command> [flags]`.
- TUI mode: `cargo run -p heroku-cli` — launches Ratatui UI.
- Tests: `cargo test --workspace` — run unit/integration tests.
- Lint: `cargo clippy --workspace -- -D warnings` — fail on warnings.
- Format: `cargo fmt --all` — apply repo `rustfmt` settings.
- Helpful env: `RUST_LOG=debug`, `HEROKU_API_KEY=…`, `FEATURE_WORKFLOWS=1`, `DEBUG=1`.

## Coding Style & Naming Conventions
- Edition: Rust 2024; indent 4 spaces; max width 100 (see `rustfmt.toml`).
- Naming: modules/files `snake_case`; types/enums `PascalCase`; functions/vars `snake_case`, do not abbreviate; constants `SCREAMING_SNAKE_CASE`.
- Errors: apps use `anyhow::Result`; libraries prefer `thiserror`.
- Keep diffs minimal; run `cargo fmt` and fix all `clippy` issues before pushing.

## Testing Guidelines
- Unit tests inline with code: `#[cfg(test)] mod tests { … }`.
- Integration tests under `crates/<name>/tests/` when needed.
- Async tests: `#[tokio::test]` where appropriate.
- Ensure deterministic output; run `cargo test --workspace` locally.

## Commit & Pull Request Guidelines
- Commits: Conventional Commits (e.g., `feat:`, `fix:`, `refactor:`). Match recent `feat:` usage.
- PRs include: clear summary, linked issues, rationale; before/after screenshots or terminal output for TUI/CLI; validation steps (exact build/run/test commands).
- Checklist: `cargo fmt` + `clippy` clean; no stray `dbg!`/`println!`.

## Security & Configuration Tips
- Never commit secrets; prefer `HEROKU_API_KEY` to `~/.netrc`.
- Redaction utilities mask sensitive values in logs; still avoid pasting tokens.
- Network via `reqwest` + TLS; set `RUST_LOG=info|debug` for diagnostics.

## Architecture Overview
See `ARCHITECTURE.md` for a full overview of crates, command/registry design, ValueProviders, execution flow, TUI UX, and security/caching.
