# Architecture Overview

- Core Crates: `crates/cli` (binary entry, args dispatch, launches TUI), `crates/tui` (Ratatui UI, focus, autocomplete, tables, theme), `crates/registry` (loads command manifest), `crates/registry-gen` (schema → manifest generator + provider inference), `crates/engine` (workflow orchestration, templating, step I/O), `crates/api` (`reqwest` client, auth, retries), `crates/util` (logging, redaction, caching, JSON helpers).

- Command Spec & Manifest: Commands are identified by `group` + `name` (e.g., `apps info`). Fields are `positional_args` or `flags`. The manifest is generated at build-time by `crates/registry-gen` from the API schema and embeds per-field `provider` metadata directly in `CommandSpec`.

  - Provider shape (embedded):
    - `ValueProvider::Command { command_id: String, binds: Vec<Bind> }`
    - `Bind { provider_key: String, from: String }`
  - Example: `apps info <app>` → positional `app` carries `provider: Command { command_id: "apps:list", binds: [] }`
  - Example with bindings: `addons info <app> <addon>` → positional `addon` carries `provider: Command { command_id: "addons:list", binds: [{ provider_key: "app", from: "app" }] }`

- Provider Inference (registry-gen): Two-pass inference attaches providers conservatively.
  - Build `<group>:<name>` index, detect groups with `list`.
  - Positionals: walk `spec.path` and bind provider from the immediately preceding concrete segment (e.g., `/addons/{addon}/config` → group `addons` → `addons:list`).
  - Flags: map flag names to plural groups via a synonyms table + conservative pluralization; bind `<group>:list` when present.
  - High-reliability bindings:
    - Bind provider path placeholders from earlier consumer positionals (via name synonyms).
    - Bind required provider flags only when they are in a safe set (app/app_id, addon/addon_id, pipeline, team/team_name, space/space_id, region, stack) and can be sourced either from earlier positionals or from consumer required flags (same/synonym name).
    - If any required provider input cannot be satisfied, no provider is attached for that field.

- **Value Providers:** Pluggable sources for dynamic suggestions:
  - **core:** API-backed (apps, addons, permissions, users).
  - **workflow:** read prior step outputs (e.g., `workflow:from(task, jsonpath)`).
  - **plugins (MCP):** external providers (planned). Today the implementation ships with a registry-backed provider and TTL caching; MCP plugins are described in `plans/PLUGINS.md` and may arrive later. Providers declare inputs (e.g., `partial`, `argOrFlag`), outputs (`label`, `value`, `meta`), TTL, and auth needs. See `plans/VALUE_PROVIDERS.md`.

- Execution Flow: CLI/TUI loads manifest; suggestion building queries providers asynchronously with caching. Command execution uses `exec_remote` (util) with proper Range header handling and logs/pagination parsing. The workflow engine supports templating and multi-step runs.

- Value Providers at Runtime:
  - Registry-backed provider (TUI) reads `provider` metadata from the manifest, resolves bound inputs from the user’s current input (earlier positionals + provided flags), and fetches via the same HTTP helpers with a short TTL cache. When required bound inputs are missing, it returns no suggestions (UI remains predictable).
  - Engine provider fetch resolves provider paths with `build_path` and includes leftover bound inputs as query params for GET/DELETE; non-GET requests receive JSON bodies.

- **Workflow Engine:** Runs multi-step workflows, manages dependencies, passes step outputs into later steps/providers, and ensures deterministic, replayable runs. See `plans/WORKFLOWS.md`

- **TUI Layer:** Guided/Power modes, autocomplete surfaces provider results, focus management for forms/tables, theming from `plans/THEME.md`, accessibility + UX patterns from `plans/FOCUS_MANAGEMENT.md`, general guidelines from `plans/UX_GUIDELINES.md`, autocomplete from `plans/AUTOCOMPLETE.md` and workflow.
  - State ownership: top-level components (palette, builder, logs, help, table) keep their state on `app::App` for coordination; nested subcomponents (e.g., pagination inside the table) may keep private state and be composed by the parent. See AGENTS.md for the component cookbook.

## Focus Management

See plans/FOCUS_MANAGEMENT.md for details on the rat-focus model (flags, local focus rings, and traversal rules). It documents the root ring (palette/logs), builder rings, and the table ↔ pagination navigation flow (Grid ↔ First ↔ Prev ↔ Next ↔ Last buttons). 

- API & Security: `reqwest` + TLS; auth via `HEROKU_API_KEY`. Redaction patterns (`token`, `password`, `secret`, etc.) applied to logs. Provider results are cached with a TTL in the TUI.

- Example: `addons info <app> <addon>`
  - Provider: `addons:list` exists at `/apps/{app}/addons`.
  - Binding: `{ provider_key: "app", from: "app" }` attaches to the `addon` positional.
  - TUI resolves `app` from the user’s input, fetches app-scoped addon names, and suggests values for `addon`.
