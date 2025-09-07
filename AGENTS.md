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
See [ARCHITECTURE.md](ARCHITECTURE.md) for a full overview of crates, command/registry design, ValueProviders, execution flow, TUI UX, and security/caching.

**TUI Components**
- **Location:** `crates/tui/src/ui/components/` with one folder per feature (e.g., `palette/`, `table/`, `logs/`, `builder/`, `help/`, `pagination/`). Each submodule typically has `mod.rs`, a main component file (e.g., `palette.rs`), and an optional `state.rs` plus helpers.
- **Trait:** All renderable pieces implement `ui::components::component::Component`, which defines `init()`, `handle_events()`, `handle_key_events()`, `handle_mouse_events()`, `update()`, and `render(frame, area, app)`. Most components implement `handle_key_events` and `render` and mutate local state under `app.*` directly; cross-cutting actions go through `app.update(Msg)` and return `Vec<Effect>`.
- **State:** App-level owns state structs (e.g., `PaletteState`, `BuilderState`, `TableState`) in `app::App`. Components are thin, mostly stateless render/controllers that read/write `app.*`. Put UI-specific state in `state.rs` under each component folder.
  - State ownership guideline: Top-level components (palette, builder, logs, help, table) keep their `*State` on `app::App` so other parts of the UI can coordinate. Nested/leaf subcomponents (e.g., `PaginationComponent` inside the table) may encapsulate their own `*State` privately and be composed by the parent component.
- **Focus:** Use `rat_focus::FocusFlag` per focusable area (e.g., `builder.search_flag`, `table.grid_f`). Build focus rings via `FocusBuilder` to cycle focus on Tab/BackTab. Focus affects styling through theme helpers.
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
  - Handle focus-specific keys inside the component, using `FocusFlag`s on your state for routing (see `BuilderComponent::handle_*_keys`).
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
  - For overlays, clear the area with `widgets::Clear` and use a centered rect (`ui::utils::centered_rect`). See `HelpComponent`, `BuilderComponent`, and `TableComponent` for examples.
- **Pagination Pattern:**
  - If your view shows pageable API data, compose `PaginationComponent` like `TableComponent` does and expose focus items via `FocusFlag`s for Tab/BackTab cycling.

**Design Conventions**
- **Local-first updates:** UI interactions update `app.<feature>` state directly; reserve `Msg` + `Effect` for cross-feature actions (open/close modals, run, copy, pagination fetch).
- **Focus-normalization:** Provide a `normalize_focus()` on state to ensure a valid initial focus when made visible (see `BuilderState::normalize_focus`).
- **Performance:** Precompute expensive view models in state reducers (`apply_result_json` builds table rows/columns once), then keep `render()` side-effect free except drawing.
- **Security:** Redact sensitive values before display/copy (`heroku_util::redact_sensitive`). This is enforced in logs and detail views; reuse that pattern for new components.

**Testing Tips**
- **Unit tests:** Co-locate simple reducers/selectors under `#[cfg(test)]` in `state.rs` or the component module. Favor pure functions for parsing/formatting.
- **Manual checks:** Run `cargo run -p heroku-cli` and verify focus, key handling, and styling in a small terminal. Use `RUST_LOG=debug` and `DEBUG=1` to surface useful info.
- **CI hygiene:** `cargo fmt --all`, `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace` must be clean.
