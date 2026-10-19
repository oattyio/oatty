# Repository Guidelines

Keep changes focused, readable, and idiomatic. Preserve existing behavior and public APIs unless the task explicitly
requires a change. Prefer small functions with one responsibility, descriptive names, typed errors, and tests for
behavioral logic.

## Workspace Layout

- Rust workspace crates:
  - `crates/cli` (`oatty`) — binary entrypoint, CLI routing, and TUI startup.
  - `crates/tui` — Ratatui UI, focus/state management, and interactive views.
  - `crates/registry` — runtime catalog and workflow loading.
  - `crates/registry-gen` — OpenAPI-to-registry manifest generation.
  - `crates/engine` — workflow execution and value-provider logic.
  - `crates/api` — HTTP execution helpers.
  - `crates/mcp` — MCP server, plugin runtime, clients, and tool routing.
  - `crates/util` — shared redaction, schema, storage, and formatting utilities.
  - `crates/types` — shared domain, message, and effect types.
- Supporting areas: `schemas/` for OpenAPI samples, `specs/` for as-built specifications, `assets/` for media,
  `scripts/` for development/release utilities, and `oatty.io/` for the static documentation website.
- Runtime workflows are loaded from `~/.config/oatty/workflows` or `REGISTRY_WORKFLOWS_PATH`; there is no required root
  `workflows/` directory.
- Tests live inline under `#[cfg(test)]` or in `crates/<name>/tests/`.
- See `ARCHITECTURE.md` for the current system overview and crate relationships.

## Build, Test, and Development Commands

- Build like CI: `cargo build --workspace --locked --all-targets`.
- Fast compile check: `cargo check --workspace`.
- Launch the TUI: `cargo run -p oatty`.
- Run a CLI command: `cargo run -p oatty -- <group> <command> [flags]`.
- Test the workspace: `cargo test --workspace --locked`.
- Run one test: `cargo test --workspace --locked <test_name>`.
- Test one crate: `cargo test -p <package_name> --locked`.
- Lint like CI: `cargo clippy --workspace --all-targets -- -D warnings`.
- Check formatting: `cargo fmt --all -- --check`; apply formatting with `cargo fmt --all`.
- Build docs: `cargo doc --workspace --no-deps`.
- Build/check the website: `npm --prefix oatty.io run build` and `npm --prefix oatty.io run check:static`.
- Test the website: `npm --prefix oatty.io run test` (runs the static-site verifier tests and script tests).

Useful environment variables include `OATTY_LOG=error|warn|info|debug|trace`, `TUI_THEME`, `MCP_CONFIG_PATH`,
`REGISTRY_CONFIG_PATH`, `REGISTRY_CATALOGS_PATH`, and `REGISTRY_WORKFLOWS_PATH`.

## Code Search and Structural Matching

- Prefer `ast-grep` when a search depends on syntax: calls, method chains, implementations, control-flow shapes, or
  other structural patterns. For Rust, start with `ast-grep --lang rust -p '<pattern>'` and use metavariables such as
  `$EXPRESSION` and `$$$ARGUMENTS`.
- Use `rg` or `rg --files` for literal text, regular expressions, filenames, configuration, documentation, logs, and
  initial discovery when the desired syntax shape is unknown.
- Inspect matches before applying an `ast-grep` rewrite, scope transformations narrowly, and review the resulting diff.
  Use `ast-grep --help` for relational rules, constraints, and advanced rewrites.

## Task Planning and Sub-Agent Delegation

Use delegation to keep the primary assistant's context focused, parallelize independent investigation, and obtain
independent implementation, testing, or review perspectives. Delegation is proportional to the task: handle small,
tightly coupled changes directly, and use sub-agents when work can be separated by responsibility or file ownership.
Plans should identify useful delegation boundaries, dependencies, shared-file risks, and acceptance evidence without
requiring a fixed number of agents, fixed roles, or a prescribed sequence.

The orchestrator remains accountable for the complete result. It must:

- read the complete request, plan, and applicable repository instructions itself;
- inspect the working tree and preserve unrelated user changes;
- define scope, invariants, public contracts, completion criteria, and validation expectations before delegating;
- give each agent a bounded responsibility, explicit file access, and permission or prohibition to edit;
- avoid concurrent edits to the same file and sequence dependent work when agents share a worktree;
- independently inspect agent findings, edits, and the combined diff rather than accepting completion claims;
- resolve conflicts between implementation choices, test expectations, and review findings;
- perform or confirm final validation and report results, residual risks, and pre-existing failures.

Do not delegate interpretation of `AGENTS.md`, final scope decisions, destructive Git operations, or final acceptance.
Keep the task tree shallow and auditable; sub-agents should not create further agents unless the plan explicitly calls
for deeper delegation and the orchestrator retains clear ownership.

Every delegated task should provide the smallest context packet that allows independent work:

```text
Objective and completion criteria:
Allowed files and edit permissions:
Relevant reference files and repository instructions:
Behavioral invariants and public contracts:
Known baseline behavior or failures:
Required validation:
Explicit non-goals:
Expected handoff:
```

Keep assignments focused and avoid asking an agent to rediscover the whole architecture. Agents must read the relevant
files before editing, inspect affected call sites, preserve contracts, avoid unrelated cleanup, and never weaken a test
merely to make it pass. A handoff should list inspected or modified files, decisions, commands and results, findings,
and unresolved concerns. Review findings should be ordered by severity and include exact locations, triggering
conditions, and minimal remedies; avoid speculative style-only feedback.

Parallelize read-only investigation and edits to disjoint files. Re-read current contents before integrating advice or
patches because another agent may have changed the shared worktree. When delegation is unavailable or not worthwhile,
the primary assistant should still keep implementation, verification, and review as distinct responsibilities. If work
cannot proceed without guessing about scope, contracts, ownership, or overlapping changes, stop and report the exact
location, conflicting invariant, and smallest viable options.

## Rust Style and Documentation

- The workspace uses Rust 2024 with a minimum Rust version of 1.93.0. Use four-space indentation and the 140-column
  limit in `rustfmt.toml`.
- Follow Rust naming conventions. Prefer clear, descriptive names over new abbreviations, while retaining established
  domain and API names such as `MCP`, `TUI`, `Msg`, and `ctx` when consistency requires them.
- Group imports as standard library, external crates, then internal modules; keep `use` statements at module scope
  except where a narrower scope materially improves clarity.
- Use `anyhow` at application/orchestration boundaries and `thiserror` for reusable typed errors. Follow the surrounding
  crate's established public error contract.
- Add module documentation and document public APIs with their purpose, important arguments, return values, errors,
  invariants, and side effects. Include examples when they clarify non-obvious usage; do not pad trivial private helpers.
- Extract helpers when a function mixes responsibilities, has deep nesting, or grows beyond roughly 30–50 lines.
  Keep parameters focused and return types explicit.
- Comments should explain business rules or non-obvious decisions, not restate the code. Do not leave stray `dbg!` or
  diagnostic `println!` calls.

## TUI Architecture and Components

- Components live in `crates/tui/src/ui/components/`. Feature folders include `palette`, `browser`, `results`, `logs`,
  `library`, `plugins`, `workflows`, `mcp_server`, `theme_picker`, and `help`; reusable leaf views live in `common`.
- A feature usually contains `mod.rs`, `<name>_component.rs`, and optional `state.rs`, nested components, or helpers.
- Renderable controllers implement `ui::components::component::Component`. The current lifecycle surface includes
  `handle_message`, key/mouse handlers, `render`, `get_hint_spans`, `get_preferred_layout`, and route enter/exit hooks.
- `App` owns cross-feature state such as `PaletteState`, `BrowserState`, `ResultsTableState`, `LogsState`, `PluginsState`,
  and `WorkflowState`. Nested/leaf state may remain private to its parent component.
- Prefer direct `app.<feature>` mutation for local UI interactions. Use `App::update(&Msg)` for cross-feature transitions
  and return `Vec<Effect>` for side effects. `Msg` and `Effect` are defined in `oatty_types`; the runtime executes effects
  through `cmd::run_from_effects` and `cmd::run_cmds`.
- Top-level composition and modal routing live in `ui/main_component.rs`; terminal setup, event routing, rendering, and
  effect execution live in `ui/runtime.rs`.
- Build focus rings with `rat_focus::FocusFlag`, `FocusBuilder`, and `HasFocus`. Route keys only to the active/focused
  component and ensure newly visible views receive a valid focus target.
- Use `ui::theme::theme_helpers` and `Theme` role methods rather than hard-coded colors. Compose layouts with
  `ratatui::layout::Layout`; use `ui::utils::centered_rect` for centered overlays and character counts for cursor
  placement.
- Keep `render` limited to drawing, cursor placement, and render-cache bookkeeping. Precompute expensive view models in
  state reducers such as `ResultsTableState::apply_result_json`.
- Shared renderers receive explicit state references—for example, `ResultsTableView::render_results` operates on a
  `ResultsTableState` selected by its controller.

When adding a component, export it from `ui/components/mod.rs`, add its state to `App` only when other features must
coordinate with it, compose it in `ui/main_component.rs`, and wire event/effect handling through the existing runtime.
For overlays, reuse the existing modal routing and shared modal/view helpers.

## Security and Runtime Configuration

- Redact sensitive values before logging, displaying, persisting, or copying them. Reuse
  `oatty_util::redact_sensitive` and the existing secret-detection helpers.
- Use `${secret:NAME}` for keychain-backed values and `${env:NAME}` for environment-backed configuration. Secrets use
  the OS keychain through `keyring-rs` unless the configured backend explicitly selects environment variables.
- MCP configuration defaults to `~/.config/oatty/mcp.json` and supports stdio and Streamable HTTP transports.
- Network access uses `reqwest` with TLS. Never commit tokens, credentials, private endpoints, or raw sensitive logs.

## Testing and Change Validation

- Co-locate unit tests with pure reducers, parsers, formatters, and other business logic. Use integration tests for
  cross-crate behavior and `#[tokio::test]` for async paths.
- Keep tests deterministic. Add coverage for behavior changes and regression fixes.
- Manually exercise affected TUI focus, keyboard, mouse, layout, and styling behavior in a small terminal when UI code
  changes. Use `OATTY_LOG=debug` for diagnostics; normal TUI operation should not write uncontrolled output to the
  alternate screen.
- Before handing off code, run formatting, targeted tests, relevant crate/workspace tests, and Clippy in proportion to
  the change. Full CI validation is `cargo build --workspace --locked --all-targets`, `cargo test --workspace --locked`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and
  `cargo doc --workspace --no-deps`.

## Commits and Pull Requests

- Use Conventional Commit prefixes such as `feat:`, `fix:`, `refactor:`, `chore:`, and `docs:`.
- Keep diffs focused and avoid unrelated cleanup. Explain the rationale and any architectural decision; preserve
  compatibility unless a breaking change is explicit.
- Pull requests should include a clear summary, linked issues when applicable, exact validation commands, and terminal
  output or before/after screenshots for user-visible CLI/TUI changes.
