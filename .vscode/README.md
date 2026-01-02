# VS Code / Cursor Configuration

This directory contains workspace-specific configurations for VS Code and Cursor.

## Files

- **`launch.json`** - Debug configurations for running and debugging the CLI, TUI, tests, and registry generator
- **`tasks.json`** - Build, test, lint, and format tasks accessible via Cmd+Shift+P → "Tasks: Run Task"
- **`settings.json`** - Rust-analyzer settings, editor preferences, and workspace configuration
- **`extensions.json`** - Recommended extensions for Rust development

## Quick Start

### Install Recommended Extensions

1. Open the Command Palette: `Cmd+Shift+P` (macOS) or `Ctrl+Shift+P` (Linux/Windows)
2. Type "Extensions: Show Recommended Extensions"
3. Click "Install All" for Workspace Recommendations

Essential extensions:

- **rust-analyzer** - Rust language support with IntelliSense
- **CodeLLDB** - Native debugger for Rust
- **Even Better TOML** - TOML syntax highlighting and validation

### Debugging

Press `F5` to start debugging with the default configuration, or:

1. Click the Debug icon in the Activity Bar (left sidebar)
2. Select a debug configuration from the dropdown
3. Press the green play button or `F5`

Available configurations:

- **Debug CLI - TUI Mode** - Launch the interactive TUI
- **Debug CLI - Apps List** - Test the apps list command
- **Debug CLI - Custom Command** - Debug with any command arguments
- **Debug Registry Generator** - Debug manifest generation
- **Debug Unit Test** - Debug a specific test

### Running Tasks

Access tasks via:

- Command Palette: `Cmd+Shift+P` → "Tasks: Run Task"
- Menu: Terminal → Run Task
- Keyboard: `Cmd+Shift+B` for default build task

Common tasks:

- **cargo: Build Workspace** - Build all crates (default build task, `Cmd+Shift+B`)
- **cargo: Run TUI** - Launch the TUI
- **cargo: Test Workspace** - Run all tests (default test task)
- **cargo: Clippy (All)** - Lint all code
- **cargo: Format** - Format all code
- **Pre-commit Check** - Run format, clippy, and tests in sequence

### Code Actions

Rust-analyzer provides many code actions accessible via:

- Lightbulb icon when hovering over code
- `Cmd+.` (macOS) or `Ctrl+.` (Linux/Windows)

Examples:

- Add missing imports
- Generate implementations
- Extract to function
- Add documentation
- Fill in match arms

### Settings

The workspace settings configure:

- **Rust-analyzer** with clippy on save
- **Editor** formatting on save, 100-char ruler
- **File associations** for Rust, TOML, YAML, JSON
- **Search excludes** to ignore `target/` directory
- **Terminal environment** with `RUST_BACKTRACE=1`

To override settings locally without committing, create:

- `.vscode/settings.local.json` (ignored by git)

## Keyboard Shortcuts

### Debugging

- `F5` - Start debugging
- `Shift+F5` - Stop debugging
- `Cmd+Shift+F5` - Restart debugging
- `F9` - Toggle breakpoint
- `F10` - Step over
- `F11` - Step into
- `Shift+F11` - Step out

### Building & Testing

- `Cmd+Shift+B` - Run build task
- `Cmd+Shift+T` - Run test task (if configured)

### Navigation

- `Cmd+P` - Quick open file
- `Cmd+Shift+O` - Go to symbol in file
- `Cmd+T` - Go to symbol in workspace
- `F12` - Go to definition
- `Cmd+F12` - Go to implementation
- `Shift+F12` - Find all references

### Code Actions

- `Cmd+.` - Show code actions
- `F2` - Rename symbol
- `Cmd+Shift+R` - Refactor

### Terminal

- `` Ctrl+` `` - Toggle terminal
- `Cmd+Shift+C` - Open new external terminal

## Customization

### Personal Settings

To customize settings without modifying the committed configuration:

1. Create `.vscode/settings.local.json`
2. Add your personal overrides:
   ```json
   {
     "editor.fontSize": 14,
     "rust-analyzer.cargo.features": ["my-feature"]
   }
   ```

This file is ignored by git.

### Additional Debug Configurations

To add custom debug configurations:

1. Edit `.vscode/launch.json`
2. Add a new configuration object
3. Commit if useful for the team, or add to `settings.local.json` for personal use

### Custom Tasks

To add project-specific tasks:

1. Edit `.vscode/tasks.json`
2. Add a new task object
3. Reference it in debug configurations or run manually

## Troubleshooting

### Rust-analyzer Issues

**Problem**: Rust-analyzer shows errors but code compiles

- **Solution**: Restart the language server via Command Palette → "Rust Analyzer: Restart Server"

**Problem**: Slow IntelliSense or high CPU usage

- **Solution**: Check `rust-analyzer.check.overrideCommand` in settings, or disable some features

**Problem**: Macros not expanding

- **Solution**: Ensure `rust-analyzer.procMacro.enable` is `true` in settings

### Debugger Issues

**Problem**: Debugger won't attach (macOS)

- **Solution**: Sign the binary (see main README under "Code Signing")
- Or temporarily add `com.apple.security.get-task-allow` to entitlements

**Problem**: Breakpoints not hitting

- **Solution**: Ensure you're building in debug mode (not release)
- Check that the source file matches the binary

**Problem**: "No debugger available" error

- **Solution**: Install CodeLLDB extension from the recommendations

### General Issues

**Problem**: Tasks not appearing

- **Solution**: Close and reopen the workspace, or reload window

**Problem**: Extensions not activating

- **Solution**: Ensure you opened the folder, not individual files
- Check Output panel for extension errors

## Security

### Storing API Tokens Securely

See **[SECURE_TOKENS.md](SECURE_TOKENS.md)** for comprehensive guide on securely storing bearer tokens (like `OATTY_API_TOKEN`) in VS Code.

**Quick Options:**
- ✅ `.env` file (gitignored, medium security)
- ✅ OS Keychain (encrypted, high security) - **Recommended**
- ✅ Shell profile (medium security)
- ❌ VS Code settings (not recommended)

## Resources

- [VS Code Rust Documentation](https://code.visualstudio.com/docs/languages/rust)
- [Rust-analyzer Manual](https://rust-analyzer.github.io/manual.html)
- [CodeLLDB Documentation](https://github.com/vadimcn/vscode-lldb/blob/master/MANUAL.md)
- [VS Code Debugging Guide](https://code.visualstudio.com/docs/editor/debugging)
- [VS Code Tasks Guide](https://code.visualstudio.com/docs/editor/tasks)

## Contributing

When adding new configurations:

- Document them in this README
- Ensure they work on macOS and Linux
- Use workspace variables like `${workspaceFolder}` instead of absolute paths
- Test configurations before committing
