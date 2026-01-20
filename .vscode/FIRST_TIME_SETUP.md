# First-Time Setup Checklist

Complete this checklist to get your development environment ready.

## ☐ Prerequisites

- [ ] **Rust installed** - Run `rustc --version` to verify

  - If not: Install from [rustup.rs](https://rustup.rs/)
  - Run: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

- [ ] **Rust toolchain** - Run `rustup show` to verify

  - The project will auto-select the toolchain via `rust-toolchain.toml`

- [ ] **VS Code or Cursor installed**
  - Download VS Code: https://code.visualstudio.com/
  - Or Cursor: https://cursor.sh/

## ☐ Install Extensions

### Essential (Required)

- [ ] **rust-analyzer** - Rust language support

  - ID: `rust-lang.rust-analyzer`
  - Provides IntelliSense, go-to-definition, etc.

- [ ] **CodeLLDB** - Native debugger
  - ID: `vadimcn.vscode-lldb`
  - Required for debugging Rust code

### Recommended

- [ ] **Even Better TOML** - TOML support

  - ID: `tamasfe.even-better-toml`

- [ ] **YAML** - YAML validation

  - ID: `redhat.vscode-yaml`

- [ ] **Markdown All in One** - Markdown tools

  - ID: `yzhang.markdown-all-in-one`

- [ ] **GitLens** - Enhanced Git integration

  - ID: `eamodio.gitlens`

- [ ] **Error Lens** - Inline error display
  - ID: `usernamehw.errorlens`

**Quick Install:**

1. Open Command Palette: `Cmd+Shift+P` (or `Ctrl+Shift+P`)
2. Type: "Extensions: Show Recommended Extensions"
3. Click "Install All" for Workspace Recommendations

## ☐ Environment Setup

- [ ] **Create .env file**

  ```bash
  cp .env.example .env
  ```

- [ ] **Add Oatty API Key** (choose one method)

  **Option A: Quick (.env file)**
  1. Get your API token from your provider's dashboard.
  2. Edit `.env` and set:
     ```bash
     OATTY_API_TOKEN=your-actual-api-key-here
     ```

  **Option B: Secure (OS Keychain)** ⭐
  1. Run: `./scripts/set-api-key.sh`
  2. Add to `~/.zshrc`:
     ```bash
     export OATTY_API_TOKEN=$(security find-generic-password -s "oatty-cli-api-token" -w 2>/dev/null)
     ```
  3. Reload: `source ~/.zshrc`

  See `.vscode/SECURE_TOKENS.md` for details

- [ ] **Configure log level (optional)**

  ```bash
  # In .env
  OATTY_LOG=debug  # Options: error|warn|info|debug|trace
  ```

- [ ] **Choose theme (optional)**
  ```bash
  # In .env
  TUI_THEME=dracula  # Options: dracula|dracula_hc|nord|nord_hc
  ```

## ☐ Initial Build

- [ ] **Build the workspace**

  ```bash
  cargo build --workspace
  ```

  - This will take a few minutes on first run
  - Downloads and compiles all dependencies
  - Generates the command manifest

- [ ] **Verify build succeeded**
  - Look for: "Finished dev [unoptimized + debuginfo]"
  - Binary location: `target/debug/oatty-cli`

## ☐ Run Tests

- [ ] **Run all tests**
  ```bash
  cargo test --workspace
  ```
  - Verifies everything is working correctly
  - Should show all tests passing

## ☐ Try It Out

- [ ] **Launch the TUI**

  ```bash
  cargo run -p oatty-cli
  ```

  - Should show the interactive terminal UI
  - Press `Ctrl+C` to exit

- [ ] **Run a CLI command**
  ```bash
  cargo run -p oatty-cli -- apps list
  ```
  - Should list your Oatty apps
  - Verifies API authentication works

## ☐ VS Code / Cursor Setup

- [ ] **Reload window**

  - Command Palette → "Developer: Reload Window"
  - Ensures all extensions are active

- [ ] **Verify rust-analyzer is working**

  - Open `crates/cli/src/main.rs`
  - Hover over a symbol - should show documentation
  - Look for green "rust-analyzer" in status bar

- [ ] **Test debugging**

  1. Open `crates/cli/src/main.rs`
  2. Set a breakpoint (click in gutter)
  3. Press `F5`
  4. Select "Debug CLI - TUI Mode"
  5. Verify debugger starts and hits breakpoint

- [ ] **Test tasks**

  1. Press `Cmd+Shift+P` (or `Ctrl+Shift+P`)
  2. Type "Tasks: Run Task"
  3. Select "cargo: Test Workspace"
  4. Verify tests run in terminal

- [ ] **Test build shortcut**
  - Press `Cmd+Shift+B` (or `Ctrl+Shift+B`)
  - Should run default build task

## ☐ Code Quality Tools

- [ ] **Format code**

  ```bash
  cargo fmt --all
  ```

  - Formats all Rust code
  - Also runs automatically on save in VS Code

- [ ] **Run clippy**

  ```bash
  cargo clippy --workspace -- -D warnings
  ```

  - Rust linter for common mistakes
  - Should show no warnings

- [ ] **Run all checks**
  ```bash
  make pre-commit
  ```
  - Runs format, clippy, and tests
  - Use before committing code

## ☐ Optional Tools

- [ ] **Install cargo-watch** (auto-rebuild on changes)

  ```bash
  cargo install cargo-watch
  ```

- [ ] **Install cargo-edit** (manage dependencies)

  ```bash
  cargo install cargo-edit
  ```

- [ ] **Install cargo-nextest** (faster test runner)

  ```bash
  cargo install cargo-nextest
  ```

- [ ] **Or install all at once**
  ```bash
  make install-tools
  ```

## ☐ macOS Code Signing (Optional)

Only needed if you're getting repeated Keychain prompts:

- [ ] **Create development certificate**

  ```bash
  KEYCHAIN_PASSWORD='your-login-password' \
    scripts/macos/create-dev-cert.sh "next-gen-cli-dev (LOCAL)"
  ```

- [ ] **Sign the binary**
  ```bash
  cargo build -p oatty-cli
  NEXTGEN_CODESIGN_ID="next-gen-cli-dev (LOCAL)" \
    NEXTGEN_CODESIGN_BIN=target/debug/oatty-cli \
    scripts/macos/sign.sh
  ```

See main README for details.

## ☐ Familiarization

- [ ] **Read QUICKSTART.md**

  - 5-minute overview of commands and usage

- [ ] **Skim DEVELOPMENT.md**

  - Comprehensive development guide
  - Bookmark for later reference

- [ ] **Review ARCHITECTURE.md**

  - Understand system design
  - Learn how components interact

- [ ] **Explore project structure**
  ```
  crates/
    cli/        # Main binary entry point
    tui/        # Terminal UI
    registry/   # Command registry
    engine/     # Workflow execution
    api/        # Oatty API client
    mcp/        # Plugin system
    util/       # Shared utilities
  ```

## ☐ Configure Git (If Contributing)

- [ ] **Set up Git hooks** (optional)

  ```bash
  # Create pre-commit hook
  cat > .git/hooks/pre-commit << 'EOF'
  #!/bin/bash
  set -e
  echo "Running pre-commit checks..."
  make pre-commit
  EOF
  chmod +x .git/hooks/pre-commit
  ```

- [ ] **Read CONTRIBUTING.md**
  - Learn commit message format
  - Understand PR process
  - Follow code style guidelines

## ✅ Setup Complete!

You should now have:

- ✅ Rust and tools installed
- ✅ Extensions installed and working
- ✅ Project built and tests passing
- ✅ Environment configured
- ✅ Debugging working
- ✅ Code quality tools ready

## Next Steps

1. **Start coding**: Pick a task or feature to work on
2. **Use the debugger**: Set breakpoints and explore the code
3. **Read the docs**: Dive deeper into DEVELOPMENT.md
4. **Ask questions**: Check existing docs or reach out to the team

## Quick Reference

### Common Commands

```bash
# Build
make build

# Test
make test

# Run TUI
make run-tui

# Run CLI
make run-cli ARGS="apps list"

# Format + Lint + Test
make pre-commit

# Show all commands
make help
```

### VS Code Shortcuts

- `F5` - Start debugging
- `Cmd+Shift+B` - Build
- `Cmd+Shift+P` - Command Palette
- `Cmd+P` - Quick open file
- `F12` - Go to definition

### Debugging

- Click gutter to set breakpoints
- `F5` to start debugging
- `F10` step over, `F11` step into
- Hover over variables to inspect

## Troubleshooting

### Build fails

→ Run `cargo clean && cargo build --workspace`

### rust-analyzer not working

→ Command Palette → "Rust Analyzer: Restart Server"

### Debugger won't attach

→ See "macOS Code Signing" section above

### API authentication fails

→ Check `OATTY_API_TOKEN` in `.env`

### Tests fail

→ Check that you're using the repo toolchain: `rustup show`

---

**Welcome to the project!** 🎉

If you've completed this checklist, you're ready to start developing. Happy coding! 🦀
