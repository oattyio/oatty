# Repository Guidelines

## Project Structure & Module Organization
- Workspace crates: `crates/cli` (binary), `crates/tui`, `crates/registry`, `crates/engine`, `crates/api`, `crates/util`.
- Supporting assets: `schemas/` (schema files), `workflows/` (sample workflow YAML/JSON), `plans/` (design notes).
- Tooling/config: `Cargo.toml`, `rustfmt.toml`, `.github/`, `.vscode/`.

Example:
```
crates/
  cli/src/main.rs      # entrypoint
  registry/            # schema → command registry
  tui/                 # Ratatui UI
```

## Build, Test, and Development Commands
- Build all: `cargo build --workspace`
- Run CLI: `cargo run -p heroku-cli -- <group> <command> [flags]`
  - TUI mode: `cargo run -p heroku-cli` (no args)
- Tests: `cargo test --workspace`
- Lint: `cargo clippy --workspace -- -D warnings`
- Format: `cargo fmt --all`

Helpful env vars: `RUST_LOG=debug`, `HEROKU_API_KEY=...`, `FEATURE_WORKFLOWS=1`, `DEBUG=1`.

## Coding Style & Naming Conventions
- Language: Rust 2018 edition; 4‑space indent, line width 100 (see `rustfmt.toml`).
- Use `cargo fmt` and fix all `clippy` warnings before pushing.
- Naming: modules/files `snake_case`, types/enums `PascalCase`, functions/vars `snake_case`, constants `SCREAMING_SNAKE_CASE`.
- Errors: prefer `anyhow::Result` in apps and `thiserror` in libraries.
- Crate names follow `heroku-*` (e.g., `heroku-api`, `heroku-cli`).

## Testing Guidelines
- Unit tests alongside code with `#[cfg(test)] mod tests { ... }`.
- Integration tests in `tests/` per crate (if needed).
- Async: use `#[tokio::test]` where applicable.
- Run `cargo test --workspace` locally and ensure deterministic output.

## Commit & Pull Request Guidelines
- Commits: use Conventional Commits (e.g., `feat:`, `fix:`, `refactor:`). Recent history uses `feat:`.
- PRs must include:
  - Clear summary, linked issues, and rationale.
  - Before/after screenshots or terminal output for TUI/CLI changes.
  - Validation steps: exact commands to build/run/test.
  - Checklist: `fmt` + `clippy` clean; no stray `dbg!`/`println!`.

## Security & Configuration Tips
- Never commit secrets; prefer `HEROKU_API_KEY` (over `~/.netrc`).
- Redaction utilities mask sensitive values in logs/dry‑runs; still avoid pasting tokens in PRs.
- Network calls use `reqwest` + TLS; set `RUST_LOG=info|debug` for diagnostics.
