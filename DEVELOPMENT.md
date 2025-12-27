# Development Setup Guide

This guide will help you set up your local development environment for the Oatty CLI (Rust).

## Prerequisites

### Required Software

1. **Rust Toolchain**

   - Install via [rustup](https://rustup.rs/)
   - This project requires Rust nightly (specified in `rust-toolchain.toml`)
   - The toolchain will be automatically selected when you work in this directory

2. **Build Dependencies**

   - macOS: `xcode-select --install`
   - Linux: `build-essential`, `pkg-config`, `libssl-dev`

3. **VS Code / Cursor**
   - Install [VS Code](https://code.visualstudio.com/) or [Cursor](https://cursor.sh/)
   - Recommended extensions are listed in `.vscode/extensions.json`

### Optional Tools

- **LLDB** - For debugging (comes with Xcode Command Line Tools on macOS)
- **cargo-watch** - For auto-recompiling on file changes: `cargo install cargo-watch`
- **cargo-edit** - For managing dependencies: `cargo install cargo-edit`
- **cargo-nextest** - Faster test runner: `cargo install cargo-nextest`

## Initial Setup

### 1. Clone and Navigate

```bash
cd /Users/jwilaby/Documents/dev/next-gen-cli
```

### 2. Verify Rust Installation

```bash
rustup show  # Should display nightly toolchain
rustc --version
cargo --version
```

### 3. Build the Project

```bash
# Build all workspace crates
cargo build --workspace

# Or build just the CLI
cargo build -p oatty-cli
```

This will:

- Download and compile all dependencies
- Generate the command manifest from schemas
- Produce debug binaries in `target/debug/`

### 4. Run Tests

```bash
# Run all tests
cargo test --workspace

# Run tests for a specific package
cargo test -p oatty-cli
```

### 5. Set Up Environment Variables

Create a `.env` file in the project root (or set in your shell profile):

```bash
# Required for API access
export HEROKU_API_KEY="your-heroku-api-key"

# Optional: Set log level (error|warn|info|debug|trace)
export OATTY_LOG="debug"

# Optional: Choose TUI theme (dracula|dracula_hc|nord|nord_hc)
export TUI_THEME="dracula"

# Optional: Enable debug mode
export DEBUG="1"

# Optional: MCP config path
export MCP_CONFIG_PATH="$HOME/.config/heroku/mcp.json"
```

## Development Workflow

### Running the Application

#### TUI Mode (Interactive)

```bash
# Launch the TUI
cargo run -p heroku-cli

# With debug logging
OATTY_LOG=debug cargo run -p oatty-cli

# With a specific theme
TUI_THEME=nord cargo run -p oatty-cli
```

#### CLI Mode (Non-Interactive)

```bash
# List apps
cargo run -p oatty-cli -- apps list

# Get app info
cargo run -p oatty-cli -- apps info my-app

# Create an app
cargo run -p oatty-cli -- apps create --name demo
```

### Development Commands

#### Format Code

```bash
# Format all code
cargo fmt --all

# Check formatting without making changes
cargo fmt --all --check
```

#### Lint Code

```bash
# Run clippy with warnings as errors
cargo clippy --workspace -- -D warnings

# Fix auto-fixable issues
cargo clippy --workspace --fix
```

#### Check Compilation

```bash
# Fast compilation check (no code generation)
cargo check --workspace
```

#### Watch Mode

```bash
# Auto-rebuild on file changes
cargo watch -x "build --workspace"

# Auto-run tests on file changes
cargo watch -x "test --workspace"
```

### Debugging

#### VS Code / Cursor Debug Configurations

The `.vscode/launch.json` file includes several pre-configured debug targets:

1. **Debug CLI - TUI Mode** - Launch the TUI with debugging
2. **Debug CLI - Apps List** - Debug the apps list command
3. **Debug CLI - Apps Info** - Debug apps info (prompts for app name)
4. **Debug CLI - Custom Command** - Debug with custom arguments
5. **Debug Registry Generator** - Debug the manifest generator
6. **Debug Unit Test** - Debug a specific test
7. **Debug All Tests** - Debug all tests in the workspace

To debug:

1. Set breakpoints by clicking in the gutter next to line numbers
2. Press `F5` or go to Run → Start Debugging
3. Select a debug configuration from the dropdown

#### Command-Line Debugging

```bash
# Run with backtrace
RUST_BACKTRACE=1 cargo run -p oatty-cli

# Run with full backtrace
RUST_BACKTRACE=full cargo run -p oatty-cli

# Use LLDB directly
rust-lldb target/debug/oatty
```

### VS Code Tasks

Press `Cmd+Shift+P` (macOS) or `Ctrl+Shift+P` (Linux/Windows) and type "Tasks: Run Task" to access:

- **cargo: Build Workspace** - Build all crates
- **cargo: Build CLI (Release)** - Build optimized release binary
- **cargo: Run TUI** - Launch the TUI
- **cargo: Run CLI - Apps List** - Run apps list command
- **cargo: Test Workspace** - Run all tests
- **cargo: Test Package** - Run tests for specific package
- **cargo: Clippy (All)** - Lint all code
- **cargo: Format** - Format all code
- **cargo: Format Check** - Check formatting
- **cargo: Clean** - Remove build artifacts
- **registry-gen: Generate Manifest** - Generate command manifest
- **Pre-commit Check** - Run format, clippy, and tests in sequence

### Registry Generator

The registry generator creates the command manifest from the Oatty schema:

```bash
# Generate JSON manifest (for inspection)
cargo run -p oatty-registry-gen -- --json \
    schemas/heroku-schema.enhanced.json \
    target/manifest.json

# Generate Postcard manifest (for production)
cargo run -p oatty-registry-gen -- \
    schemas/heroku-schema.enhanced.json \
    target/manifest.bin
```

The build scripts automatically generate the manifest when building the CLI.

## Project Structure

```
next-gen-cli/
├── crates/
│   ├── cli/        # Main CLI binary entry point
│   ├── tui/        # Terminal UI (Ratatui)
│   ├── registry/   # Command registry and schema
│   ├── registry-gen/ # Manifest generator
│   ├── engine/     # Workflow execution engine
│   ├── api/        # Oatty API client (targets Oatty endpoints)
│   ├── mcp/        # MCP plugin infrastructure
│   ├── util/       # Shared utilities
│   └── types/      # Shared type definitions
├── schemas/        # JSON Hyper-Schema definitions
├── workflows/      # Sample workflow YAML files
├── specs/          # Design documentation
└── .vscode/        # VS Code/Cursor configurations
```

## Testing Strategy

### Unit Tests

Located inline with code in `#[cfg(test)]` modules:

```bash
# Run all unit tests
cargo test --workspace --lib

# Run tests for specific crate
cargo test -p oatty-registry --lib
```

### Integration Tests

Located in `crates/*/tests/` directories:

```bash
# Run all integration tests
cargo test --workspace --test '*'

# Run specific integration test
cargo test -p oatty-registry-gen --test schema_tests
```

### Test-Driven Development

```bash
# Watch mode for TDD
cargo watch -x "test --workspace"

# Run tests with output
cargo test --workspace -- --nocapture

# Run specific test by name
cargo test test_name -- --exact
```

## Code Quality

### Pre-Commit Checklist

Before committing, ensure:

1. ✅ Code is formatted: `cargo fmt --all`
2. ✅ No clippy warnings: `cargo clippy --workspace -- -D warnings`
3. ✅ All tests pass: `cargo test --workspace`
4. ✅ No `dbg!()` or unnecessary `println!()` statements

Or run the automated task:

```bash
# In VS Code: Cmd+Shift+P → Tasks: Run Task → Pre-commit Check
```

### Code Style Guidelines

- **Indentation**: 4 spaces (configured in `.editorconfig` and `rustfmt.toml`)
- **Line Length**: 100 characters max
- **Naming**:
  - `snake_case` for functions, variables, modules
  - `PascalCase` for types, enums, traits
  - `SCREAMING_SNAKE_CASE` for constants
- **Documentation**: Add doc comments (`///`) for public APIs
- **Error Handling**: Use `anyhow::Result` in binaries, `thiserror` in libraries

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add support for team management commands
fix: resolve panic when schema is missing
refactor: extract command building logic to separate module
docs: update README with TUI controls
test: add integration tests for registry generation
```

## Troubleshooting

### Build Errors

**Error**: `error: could not find Cargo.toml`

- **Solution**: Ensure you're in the project root directory

**Error**: `error: toolchain 'nightly-...' is not installed`

- **Solution**: Run `rustup toolchain install nightly`

**Error**: Linker errors on Linux

- **Solution**: Install build dependencies: `sudo apt-get install build-essential pkg-config libssl-dev`

### Runtime Errors

**Error**: `Oatty API authentication failed`

- **Solution**: Set `HEROKU_API_KEY` environment variable with a valid API key

**Error**: TUI not rendering correctly

- **Solution**: Ensure your terminal supports 256 colors and UTF-8

**Error**: Keychain access prompts (macOS)

- **Solution**: See "Code Signing" section in main README

### Performance Issues

**Slow compilation**:

- Use `cargo check` instead of `cargo build` for faster feedback
- Consider using `sccache` or `mold` linker
- Use `cargo build --timings` to identify slow dependencies

**Large binary size**:

- The debug build is unoptimized and large
- Use `cargo build --release` for optimized, smaller binaries

## Advanced Topics

### Code Signing (macOS Only)

For development without repeated Keychain prompts:

```bash
# Create self-signed certificate
KEYCHAIN_PASSWORD='your-login-password' \
    scripts/macos/create-dev-cert.sh "next-gen-cli-dev (LOCAL)"

# Build and sign
cargo build -p oatty-cli
NEXTGEN_CODESIGN_ID="next-gen-cli-dev (LOCAL)" \
    NEXTGEN_CODESIGN_BIN=target/debug/oatty \
    scripts/macos/sign.sh
```

### Custom Schemas

To test with a custom schema:

1. Place your schema in `schemas/`
2. Update `crates/registry/build.rs` to reference it
3. Rebuild: `cargo clean && cargo build -p heroku-cli`

### MCP Plugin Development

MCP plugins extend the CLI with custom value providers:

1. Create plugin configuration in `~/.config/heroku/mcp.json`
2. Enable debug logging: `OATTY_LOG=debug`
3. Test plugin: Use TUI to trigger value provider

See `specs/PLUGINS.md` for details.

## Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [Ratatui Documentation](https://ratatui.rs/)
- [Clap Documentation](https://docs.rs/clap/)
- [Project Architecture](./ARCHITECTURE.md)
- [Contributing Guidelines](./CONTRIBUTING.md)

## Getting Help

- Check existing documentation in `specs/` and `README.md`
- Review `ARCHITECTURE.md` for system design
- Run with `OATTY_LOG=debug` for detailed logging
- Use `cargo doc --open` to browse generated documentation

## Next Steps

After setup:

1. 📖 Read `ARCHITECTURE.md` to understand the system design
2. 🎯 Review `specs/` documentation for specific features
3. 🚀 Pick a task from the issue tracker or roadmap
4. 💻 Start coding with the debug configurations!

Happy coding! 🦀
