# Repository Guidelines
- Always organize code by breaking it down into focused responsibilities, cleaning up code for readability. apply idiomatic rust, expand abbreviations, add comprehensive doc blocks and breaking out long functions into shorter ones as needed

## Project Structure & Module Organization
- Workspace crates: `crates/cli` (binary), `crates/tui`, `crates/registry`, `crates/engine`, `crates/api`, `crates/util`, `crates/mcp` (MCP plugin infrastructure).
- Supporting assets: `schemas/` (schemas), `workflows/` (sample workflow YAML/JSON), `plans/` (design notes).
- Example layout:
  - `crates/cli/src/main.rs` — CLI entrypoint
  - `crates/tui/` — Ratatui UI
  - `crates/registry/` — schema → command registry
  - `crates/mcp/` — MCP plugin engine, client management, logging
  - Tests live inline (`#[cfg(test)]`) or under each crate's `tests/`.

## Build, Test, and Development Commands
- Build all: `cargo build --workspace` — compiles every crate.
- Run CLI: `cargo run -p oatty-cli -- <group> <command> [flags]`.
- TUI mode: `cargo run -p oatty-cli` — launches Ratatui UI, or run installed binary `oatty`.
- Tests: `cargo test --workspace` — run unit/integration tests.
- Single test: `cargo test --workspace <test_name>` — run specific test function.
- Single crate tests: `cargo test -p <crate_name>` — run tests for specific crate.
- Lint: `cargo clippy --workspace -- -D warnings` — fail on warnings.
- Format: `cargo fmt --all` — apply repo `rustfmt` settings.
- Helpful env: `OATTY_LOG=debug` (stderr logs are silenced during TUI), `OATTY_API_TOKEN=…`, `MCP_CONFIG_PATH=~/.config/oatty/mcp.json`.

## Coding Style & Naming Conventions
- Edition: Rust 2024; indent 4 spaces; max width 140 (see `rustfmt.toml`).
- Naming: modules/files `snake_case`; types/enums `PascalCase`; functions/vars `snake_case`, do not abbreviate; constants `SCREAMING_SNAKE_CASE`.
- Errors: apps use `anyhow::Result`; libraries prefer `thiserror`.
- Imports: Group std imports first, then external crates, then internal modules. Use `use` statements at file top.
- Keep diffs minimal; run `cargo fmt` and fix all `clippy` issues before pushing.

## Testing Guidelines
- Unit tests inline with code: `#[cfg(test)] mod tests { … }`.
- Integration tests under `crates/<name>/tests/` when needed.
- Async tests: `#[tokio::test]` where appropriate.
- Single test: `cargo test --workspace <test_name>` — run specific test function.
- Single crate tests: `cargo test -p <crate_name>` — run tests for specific crate.
- Ensure deterministic output; run `cargo test --workspace` locally.

## Commit & Pull Request Guidelines
- Commits: Conventional Commits (e.g., `feat:`, `fix:`, `refactor:`). Match recent `feat:` usage.
- PRs include: clear summary, linked issues, rationale; before/after screenshots or terminal output for TUI/CLI; validation steps (exact build/run/test commands).
- Checklist: `cargo fmt` + `clippy` clean; no stray `dbg!`/`println!`.

## Security & Configuration Tips
- Never commit secrets; prefer `OATTY_API_TOKEN` via environment for authentication.
- Redaction utilities mask sensitive values in logs; still avoid pasting tokens.
- Network via `reqwest` + TLS; set `OATTY_LOG=error|warn|info|debug|trace` for diagnostics (stderr logs are silenced during TUI).
- MCP plugins: Use `${secret:NAME}` interpolation for sensitive values; secrets stored in OS keychain via `keyring-rs`.
- MCP config: Located at `~/.config/oatty/mcp.json`; supports stdio and HTTP/SSE transports.

## Architecture Overview
See [ARCHITECTURE.md](ARCHITECTURE.md) for a full overview of crates, command/registry design, ValueProviders, execution flow, TUI UX, and security/caching.

## TUI Components
- **Location:** `crates/tui/src/ui/components/` with one folder per feature (e.g., `palette/`, `table/`, `logs/`, `browser/`, `help/`, `pagination/`). Each submodule typically has `mod.rs`, a main component file (e.g., `palette.rs`), and an optional `state.rs` plus helpers.
- **Trait:** All renderable pieces implement `ui::components::component::Component`, which defines `init()`, `handle_events()`, `handle_key_events()`, `handle_mouse_events()`, `update()`, and `render(frame, area, app)`. Most components implement `handle_key_events` and `render` and mutate local state under `app.*` directly; cross-cutting actions go through `app.update(Msg)` and return `Vec<Effect>`.
- **State:** App-level owns state structs (e.g., `PaletteState`, `BrowserState`, `TableState`) in `app::App`. Components are thin, mostly stateless render/controllers that read/write `app.*`. Put UI-specific state in `state.rs` under each component folder.
  - State ownership guideline: Top-level components (palette, browser, logs, help, table) keep their `*State` on `app::App` so other parts of the UI can coordinate. Nested/leaf subcomponents (e.g., `PaginationComponent` inside the table) may encapsulate their own `*State` privately and be composed by the parent component.
- **Shared views:** Complex renderers that need their own widget state but no business data live in `ui/components/common/`. Controllers (the structs implementing `Component`) pass explicit state references into these helpers—e.g., `ResultsTableView::render_results(frame, area, &app.table, focused, theme)`. This keeps rendering reusable across multiple instances of the same primary UI while the controller decides which slice of `App` state to expose.
- **Focus:** Use `rat_focus::FocusFlag` per focusable area (e.g., `browser.search_flag`, `table.grid_f`). Build focus rings via `FocusBuilder` to cycle focus on Tab/BackTab. Focus affects styling through theme helpers.
- **Theme:** Use `ui::theme::helpers` (`th::block`, `panel_style`, `selection_style`) and `Theme::border_style(focused)` instead of raw styles to keep a consistent look.
- **Integration:** Components are constructed in `crates/tui/src/lib.rs` and rendered from `ui/main.rs`. Key input routing also lives in `crates/tui/src/lib.rs` (`handle_key`) and delegates to the focused/visible component.

**Create A New Component**
- **Scaffold:**
  - Add `crates/tui/src/ui/components/<name>/` with `mod.rs`, `<name>.rs`, and optional `state.rs`/helpers.
  - Export from `crates/tui/src/ui/components/mod.rs` and re-export types as needed (`pub use <name>::NameComponent;`).
- **State:**
  - Define a `NameState` in `state.rs` for UI data and focus flags.
  - Derive/implement `Default` and add convenience selectors/reducers (getters and `reduce_*`/`apply_*` methods) to keep logic testable.
- **Component:**
  - Implement `Component` for `NameComponent` in `<name>.rs`.
  - Prefer local UI mutations on `app.<feature>`; for global actions use `effects.extend(app.update(Msg::...))` and return the `Vec<Effect>`.
  - Handle focus-specific keys inside the component, using `FocusFlag`s on your state for routing (see `BrowserComponent::handle_*_keys`).
- **Rendering:**
  - Lay out the area with `ratatui::layout::Layout`. Wrap sections in `th::block(theme, title, focused)` where appropriate.
  - Read styles from `app.ctx.theme`; avoid hard-coded colors.
  - Manage the cursor with `frame.set_cursor_position((x, y))` using character counts, not bytes (see `PaletteComponent::position_cursor`).
- **Wire-up:**
  - Instantiate your component in `crates/tui/src/lib.rs` alongside others.
  - Render it from `ui/main.rs` in the right layout slot.
  - Route keys in `handle_key` (in `crates/tui/src/lib.rs`) by delegating to `component.handle_key_events(app, key)` when your component is visible/focused.
- **Effects & Commands:**
  - Components return `Vec<app::Effect>`. The runtime maps these via `cmd::from_effects` and `cmd::run_cmds`.
  - Use `app.update(Msg)` for state transitions that may produce effects; only perform side-effects via returned `Effect`s.
- **Modal Pattern:**
  - For overlays, clear the area with `widgets::Clear` and use a centered rect (`ui::utils::centered_rect`). See `HelpComponent`, `BrowserComponent`, and `TableComponent` for examples.
- **Pagination Pattern:**
  - If your view shows pageable API data, compose `PaginationComponent` like `TableComponent` does and expose focus items via `FocusFlag`s for Tab/BackTab cycling.

## Design Conventions
- **Local-first updates:** UI interactions update `app.<feature>` state directly; reserve `Msg` + `Effect` for cross-feature actions (open/close modals, run, copy, pagination fetch).
- **Focus-normalization:** Provide a `normalize_focus()` on state to ensure a valid initial focus when made visible (see `BrowserState::normalize_focus`).
- **Performance:** Precompute expensive view models in state reducers (`apply_result_json` builds table rows/columns once), then keep `render()` side-effect free except drawing.
- **Security:** Redact sensitive values before display/copy (`oatty_util::redact_sensitive`). This is enforced in logs and detail views; reuse that pattern for new components.

## Testing Tips
- **Unit tests:** Co-locate simple reducers/selectors under `#[cfg(test)]` in `state.rs` or the component module. Favor pure functions for parsing/formatting.
- **Manual checks:** Run `cargo run -p oatty-cli` (or `oatty` if installed) and verify focus, key handling, and styling in a small terminal. Use `OATTY_LOG=debug` to surface useful info.
- **CI hygiene:** `cargo fmt --all`, `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace` must be clean.
- **Single test execution:** Use `cargo test --workspace <test_name>` to run specific tests during development.

## General Use Instructions for AI Assistants

When working on this codebase, follow these guidelines to ensure consistent, high-quality code:

### Code Organization & Refactoring
- **Break down long functions**: Functions over 50 lines should be decomposed into smaller, focused helper functions
- **Expand abbreviations**: Use full descriptive names instead of abbreviations (e.g., `ctx` → `context`, `msg` → `message`, `out` → `outcome`)
- **Add comprehensive documentation**: Every public function, struct, and enum should have detailed doc blocks explaining purpose, arguments, return values, and examples
- **Apply idiomatic Rust**: Follow Rust best practices, use proper error handling, and leverage Rust's type system effectively
- **Single responsibility principle**: Each function should have one clear purpose and responsibility

### Naming Conventions
- **Variables**: Use descriptive names that explain intent (e.g., `execution_outcome` instead of `out`, `selected_command_spec` instead of `spec`)
- **Functions**: Use verb-noun patterns for clarity (e.g., `handle_execution_completion`, `process_general_execution_result`)
- **Constants**: Use `SCREAMING_SNAKE_CASE` with descriptive names (e.g., `MAX_LOG_ENTRIES`)
- **Types**: Use `PascalCase` with clear, descriptive names
- **Modules**: Use `snake_case` with descriptive names

### Documentation Standards
- **Module-level docs**: Start each module with a comprehensive doc block explaining its purpose and responsibilities
- **Function docs**: Include purpose, arguments, return values, examples, and any side effects
- **Struct/Enum docs**: Explain the purpose, usage patterns, and relationships to other types
- **Inline comments**: Add comments for complex logic, business rules, and non-obvious decisions
- **Examples**: Provide usage examples in doc blocks where helpful

### Function Extraction Guidelines
- **Extract when**: Functions exceed 30-50 lines, have multiple responsibilities, or contain complex nested logic
- **Naming**: Use descriptive names that clearly indicate the function's purpose
- **Parameters**: Keep parameter lists focused and use descriptive names
- **Return values**: Make return types clear and well-documented
- **Error handling**: Use proper Rust error handling patterns (`Result<T, E>`, `Option<T>`)

### Code Quality Checklist
Before submitting changes, ensure:
- [ ] All functions have comprehensive documentation
- [ ] Abbreviations have been expanded to full descriptive names
- [ ] Long functions have been broken down into smaller, focused functions
- [ ] Variable names are descriptive and self-documenting
- [ ] Code follows idiomatic Rust patterns
- [ ] All linting errors have been resolved (`cargo clippy --workspace -- -D warnings`)
- [ ] Code is properly formatted (`cargo fmt --all`)
- [ ] All tests pass (`cargo test --workspace`)

### Refactoring Process
1. **Read and understand** the existing code structure and purpose
2. **Identify** areas for improvement (long functions, abbreviations, missing docs)
3. **Plan** the refactoring approach (what to extract, how to name things)
4. **Implement** changes incrementally, testing after each major change
5. **Verify** that functionality remains intact and code quality improves
6. **Document** any architectural decisions or patterns established

### Common Patterns to Apply
- **State management**: Use clear state structs with descriptive field names
- **Event handling**: Break down complex event handlers into focused methods
- **Rendering**: Separate rendering logic into focused, reusable functions
- **Error handling**: Use descriptive error types and proper error propagation
- **Resource management**: Use appropriate Rust patterns for cleanup and resource management

### Testing Considerations
- **Unit tests**: Add tests for extracted helper functions when they contain business logic
- **Integration tests**: Ensure refactored code still works correctly in the broader system
- **Manual testing**: Test UI changes manually to ensure behavior is preserved
- **Performance**: Verify that refactoring doesn't introduce performance regressions
- **Single test execution**: Use `cargo test --workspace <test_name>` to run specific tests during development

### Communication
- **Explain changes**: Document why changes were made and what benefits they provide
- **Preserve functionality**: Ensure that refactoring doesn't change external behavior
- **Maintain compatibility**: Keep public APIs stable unless explicitly changing them
- **Follow conventions**: Adhere to existing code patterns and architectural decisions
