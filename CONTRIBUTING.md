# Contributing

Thank you for your interest in contributing!

## Setup

1. Install Rust (stable) and components: `rustup component add clippy rustfmt`.
2. Build workspace: `cargo build --workspace`.
3. Run checks before opening a PR:
   - `cargo fmt --all --check`
   - `cargo clippy --workspace -- -D warnings`
   - `cargo test --workspace`

## Guidelines

- Keep features deterministic and testable.
- Prefer small, focused pull requests with tests.
- Redact tokens/secrets in logs and test snapshots.
- Keep docs/spec updates aligned with implementation changes (`specs/` + root docs).
