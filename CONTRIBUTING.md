# Contributing

Thank you for your interest in contributing!

## Setup

1. Install Rust (stable) and components: `rustup component add clippy rustfmt`.
2. Build workspace: `cd next-gen-cli && cargo build --workspace`.

## Guidelines

- Keep features deterministic and testable.
- Prefer small, focused pull requests with tests.
- Redact tokens/secrets in logs and test snapshots.
