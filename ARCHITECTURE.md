# Architecture Overview

- **Core Crates:** `crates/cli` (binary entry, args dispatch, launches TUI), `crates/tui` (Ratatui UI, focus, autocomplete, tables, theme), `crates/registry` (loads schemas + value-provider registry), `crates/engine` (workflow orchestration, templating, step I/O), `crates/api` (`reqwest` client, auth, retries), `crates/util` (logging, redaction, caching, JSON helpers).

- **Command Spec & Registry:** Commands identified by `group` + `name` (e.g., `enterprise-accounts members:create`). Fields addressed as `positional:<name>` or `flag:--<name>`. A YAML/JSON ValueProvider Registry (`format: vp-registry@1`, `version: 1.1.0`) maps command fields to providers with per-field behavior (`partial_required`, `debounce_ms`, `incremental`, `max_items`, `cache_scope`).

- **Value Providers:** Pluggable sources for dynamic suggestions:
  - **core:** API-backed (apps, addons, permissions, users).
  - **workflow:** read prior step outputs (e.g., `workflow:from(task, jsonpath)`).
  - **plugins (MCP):** external providers. Providers declare inputs (e.g., `partial`, `argOrFlag`), outputs (`label`, `value`, `meta`), TTL, and auth needs.

- **Execution Flow:** CLI/TUI loads schemas + registry, resolves providers per field, fetches suggestions async with caching and debouncing, and inserts `output_value` (overrideable). Array flags accumulate values; positionals/flags can template args from other fields or workflow context.

- **Workflow Engine:** Runs multi-step workflows, manages dependencies, passes step outputs into later steps/providers, and ensures deterministic, replayable runs.

- **TUI Layer:** Guided/Power modes, autocomplete surfaces provider results, focus management for forms/tables, theming from `plans/THEME.md`, accessibility + UX patterns from `plans/FOCUS_MANAGEMENT.md` and `plans/UX_GUIDELINES.md`.

## Focus Management

See plans/FOCUS_MANAGEMENT.md for details on the rat-focus model (flags, local focus rings, and traversal rules). It documents the root ring (palette/logs), builder rings, and the table ↔ pagination navigation flow (Grid ↔ First ↔ Prev ↔ Next ↔ Last buttons). 

- **API & Security:** `reqwest` + TLS; auth via `HEROKU_API_KEY`. Redaction patterns (`token`, `password`, `secret`, etc.) applied to logs. Caching uses TTL; defaults configurable in the registry.

- **Example:** `enterprise-accounts members:create` maps `enterprise_account` → `enterprise:accounts`, `--user` → `accounts:lookup`, `--permissions` → `enterprise:permissions` with `enterprise_account` templated into the provider args.
