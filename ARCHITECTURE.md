# Architecture Overview

- **Core Crates:** `crates/cli` (binary entry, args dispatch, launches TUI), `crates/tui` (Ratatui UI, focus, autocomplete, tables, theme), `crates/registry` (loads schemas + value-provider registry), `crates/engine` (workflow orchestration, templating, step I/O), `crates/api` (`reqwest` client, auth, retries), `crates/util` (logging, redaction, caching, JSON helpers).

- **Command Spec & Registry:** Commands identified by `group` + `name` (e.g., `enterprise-accounts members:create`). Fields addressed as `positional:<name>` or `flag:--<name>`. A YAML/JSON ValueProvider Registry (`format: vp-registry@1`, `version: 1.1.0`) maps command fields to providers with per-field behavior (`partial_required`, `debounce_ms`, `incremental`, `max_items`, `cache_scope`). 

- **Value Providers:** Pluggable sources for dynamic suggestions:
  - **core:** API-backed (apps, addons, permissions, users).
  - **workflow:** read prior step outputs (e.g., `workflow:from(task, jsonpath)`).
  - **plugins (MCP):** external providers (planned). Today the implementation ships with a registry-backed provider and TTL caching; MCP plugins are described in `plans/PLUGINS.md` and may arrive later. Providers declare inputs (e.g., `partial`, `argOrFlag`), outputs (`label`, `value`, `meta`), TTL, and auth needs. See `plans/VALUE_PROVIDERS.md`.

- **Execution Flow:** CLI/TUI loads schemas + registry, resolves providers per field, and fetches suggestions via background tasks with TTL caching (refreshed on UI ticks). There is no explicit keystroke-level debounce at present. Inserted suggestion text uses provider `output_value` semantics where applicable. Today command execution supports a single value per flag; array-style accumulation is a future enhancement. Positionals/flags templating from workflow context is supported in the workflow engine and will be surfaced in the TUI incrementally.

- **Workflow Engine:** Runs multi-step workflows, manages dependencies, passes step outputs into later steps/providers, and ensures deterministic, replayable runs. See `plans/WORKFLOWS.md`

- **TUI Layer:** Guided/Power modes, autocomplete surfaces provider results, focus management for forms/tables, theming from `plans/THEME.md`, accessibility + UX patterns from `plans/FOCUS_MANAGEMENT.md`, general guidelines from `plans/UX_GUIDELINES.md`, autocomplete from `plans/AUTOCOMPLETE.md` and workflow.
  - State ownership: top-level components (palette, builder, logs, help, table) keep their state on `app::App` for coordination; nested subcomponents (e.g., pagination inside the table) may keep private state and be composed by the parent. See AGENTS.md for the component cookbook.

## Focus Management

See plans/FOCUS_MANAGEMENT.md for details on the rat-focus model (flags, local focus rings, and traversal rules). It documents the root ring (palette/logs), builder rings, and the table ↔ pagination navigation flow (Grid ↔ First ↔ Prev ↔ Next ↔ Last buttons). 

- **API & Security:** `reqwest` + TLS; auth via `HEROKU_API_KEY`. Redaction patterns (`token`, `password`, `secret`, etc.) applied to logs. Caching uses TTL; defaults configurable in the registry.

- **Example:** `enterprise-accounts members:create` maps `enterprise_account` → `enterprise:accounts`, `--user` → `accounts:lookup`, `--permissions` → `enterprise:permissions` with `enterprise_account` templated into the provider args.
