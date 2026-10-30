![Oatty Logo](assets/branding/oatty-lockup-github-v6.1.svg)

[![CI](https://github.com/oattyio/oatty/actions/workflows/ci.yml/badge.svg)](https://github.com/oattyio/oatty/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/rust-1.93%2B-orange)
![License](https://img.shields.io/badge/license-MIT-blue)
![Status](https://img.shields.io/badge/status-experimental-yellow)

# Oatty

Give a connected agent access to APIs through Oatty's MCP tools and OpenAPI catalogs. Search, run, and inspect the same commands in the terminal interface.

Full documentation: https://oatty.io

- Quick Start: https://oatty.io/docs/quick-start
- Connect an agent to an API: https://oatty.io/docs/api-access
- Create an OpenAPI schema from documentation: https://oatty.io/docs/agent-authored-schema
- Correct a generated command: https://oatty.io/docs/repair-command
- Learn: https://oatty.io/docs/learn/getting-oriented
- Guides: https://oatty.io/docs/guides/sentry-bootstrap

![Oatty TUI (Dracula high contrast)](assets/readme/oatty-tui-dracula-hc.webp)

## What Oatty Does

- Imports OpenAPI schemas and turns them into runnable commands.
- Provides a keyboard-first TUI for discovery, inspection, and execution.
- Exposes commands and workflows through MCP for connected agents.
- Support for thousands of commands without context window saturation.
- Supports reusable, reviewable workflows for cross-system operations.
- Multiple themes and colors in a beautiful TUI.

## Install

### npm (recommended)

```bash
npm i -g oatty
oatty --help
```

### from source

```bash
cargo build --workspace
cargo run -p oatty -- --help
```

### Native updates

`oatty update` keeps the complete downloaded release archive, including dependency notices and required source. Each download is saved separately under:

- Linux: `$XDG_DATA_HOME/oatty/releases`, or `~/.local/share/oatty/releases`.
- macOS: `~/Library/Application Support/oatty/releases`.
- Windows: `%LOCALAPPDATA%\oatty\releases`.

The saved path is printed after installation unless `--quiet` is set, and included in an error if installation fails after saving. If Oatty cannot save the archive, it leaves the installed binary unchanged. Archives accumulate until you delete them manually. `--check`, an already current version, and npm-managed installs do not save archives; update npm installs with `npm update -g oatty`.

## 60-Second Quick Start

1. Launch TUI:

```bash
oatty
```

2. Import an OpenAPI catalog in Library (local file or URL).
3. Search commands in Run Command / Find.
4. Execute commands manually or with Workflows.
5. Use Workflows view to review and run multi-step operations.

For full walkthroughs: https://oatty.io/docs/quick-start

## CLI Basics

```bash
# Run TUI
oatty

# Run a command
oatty <group> <command> [flags]

# Import a catalog
oatty import <path-or-url>

# Workflow commands
oatty workflow list
oatty workflow preview --file workflows/example.yaml
```

## MCP and Agent Usage

Run Oatty as an MCP server so connected agents can discover and run catalog commands.

- MCP HTTP Server docs: https://oatty.io/docs/learn/mcp-http-server
- API access guide: https://oatty.io/docs/api-access
- Safety model: https://oatty.io/docs/learn/how-oatty-executes-safely

## Development

```bash
cargo fmt --all
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

Architecture overview: [ARCHITECTURE.md](ARCHITECTURE.md)
Contributing guide: [CONTRIBUTING.md](CONTRIBUTING.md)
Release process: [RELEASE.md](RELEASE.md)

## License

Oatty-owned code is licensed under the [MIT License](LICENSE). Third-party dependencies and samples retain their own terms. Release archives and the npm package include dependency notices and required source; see [sample attribution](schemas/README.md) for source-only fixtures.
