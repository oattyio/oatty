## ValueProviders — Detailed Design Document

This document describes the **ValueProvider** system for workflows and command execution in the modernized Oatty CLI TUI. It consolidates earlier discussion threads into a refined technical spec that can be used as the basis for design, prototyping, and further refinement.

---

## 1. Purpose

ValueProviders enable **dynamic and context-aware completion of values** for command flags and positional arguments. They bridge between:

- **Schema-driven flags/args** (from the JSON Hyper-Schema).
- **Real-world dynamic data** (apps, addons, regions, pipelines, team members, etc.).
- **Workflow orchestration** (reusing outputs of previous steps as inputs).

This makes commands more powerful and user-friendly in both Guided and Power modes, while also enabling **automated workflows** to be parameterized with live system data.

Note on current implementation (embed + bindings):
- Provider metadata is embedded directly in the generated `CommandSpec` for each field as `ValueProvider::Command { command_id, binds }`.
- `binds: Vec<Bind>` specifies how provider inputs (e.g., path placeholders, required provider flags) are satisfied from consumer inputs already entered (earlier positionals, required flags with safe names).
- The TUI’s runtime `ValueProvider` trait receives an additional `inputs` map derived from the user’s current command input to resolve provider paths and query params before fetching suggestions.
- High-reliability strategy only: required provider flags are bound from a curated safe set (app/app_id, addon/addon_id, pipeline, team/team_name, space/space_id, region, stack), and only from consumer required flags or earlier positionals; otherwise the provider is omitted.

---

## 2. Design Principles

- **Declarative first**: Each command’s schema specifies which parameters can be powered by a provider.
- **Async-capable**: ValueProviders must support async I/O (API calls, plugin requests).
- **Cache-aware**: To avoid latency, results should be cached with TTLs.
- **Composable**: Providers may chain; one provider can depend on another (e.g., list apps → list addons for that app).
- **Safe**: Never suggest sensitive values (tokens, secrets).
- **Pluggable**: Plugins (via MCP) can contribute their own providers.

---

## 3. Provider Types

### 3.1 Static Providers
- Derived from the schema itself.
- Examples: enum lists (`region = ["us", "eu", "tokyo"]`).
- Always available synchronously.

### 3.2 Built-in Dynamic Providers
- Provided by the core CLI, backed by API calls.
- Examples:
  - `apps list` → returns app names.
  - `addons list` → returns addons for given app.
  - `pipelines list` → returns pipelines visible to user.
  - `teams list` → returns team slugs.

### 3.3 Workflow Providers
- Extract values from **previous workflow steps**.
- Example: Task A creates an app and returns `{ "name": "demo-app" }`.
- Task B uses `${{tasks.A.output.name}}` as a parameter.

### 3.4 Plugin Providers (MCP)
- Exposed by MCP plugins via `autocomplete(tool, field, partial)`.
- Example: A Postgres plugin offering `databases list` values for `--db`.
- Host integrates plugin-suggested values with schema-defined flags.

---

## 4. Provider Invocation Lifecycle

1. **User types a flag** (`--app`) or Guided UI highlights a field.
2. CLI checks schema for associated provider ID (e.g., `"provider": "apps list"`).
3. **Resolution order**:
   - Static values (enum/defaults).
   - Cached results for this provider.
   - Async provider invocation (API or MCP).
4. Results are scored and merged with **history values**.
5. Suggestions shown in:
   - Power mode autocomplete popup.
   - Guided mode dropdown/list.

---

## 5. Data Contract

### 5.1 Provider Metadata
```json
{
  "flag": "--app",
  "provider": "apps list",
  "required": true,
  "multiple": false
}
```

### 5.2 Provider Response
```json
[
  { "label": "demo-app", "id": "app-123", "meta": "owner: justin@example.com" },
  { "label": "billing-svc", "id": "app-456", "meta": "team: infra" }
]
```

### 5.3 CompletionItem (internal)
```rust
pub struct CompletionItem {
    pub display: String,        // Shown to user
    pub insert_text: String,    // Value inserted into command line
    pub kind: ItemKind,         // Value | History | Provider
    pub meta: Option<String>,   // Extra context (owner, team, region)
    pub score: i64,             // Ranking
}
```

---

## 6. Caching Strategy

- **Static**: Cached forever (schema enums).
- **Dynamic**: Cache with TTL (e.g., 30–120s).
- **Workflow outputs**: Valid for duration of workflow run only.
- **Plugin/MCP**: Allow plugin to specify TTL or fallback to default (e.g., 30s).
- Use **LRU cache** keyed by `(provider_id, args, partial)`.

---

## 7. Workflow Integration

- Each task output is stored in a structured map:
  ```json
  { "tasks": { "create_app": { "output": { "name": "demo-app", "id": "app-123" } } } }
  ```
- Later tasks can use templating:
  - `${{tasks.create_app.output.name}}`
- Host resolves templates by substituting provider outputs at runtime.
- In Guided mode, these show up as **pre-filled default values** with an indication of their source.

---

## 8. Error Handling

- Provider fails (network, plugin crash):
  - Show cached values if available.
  - Otherwise, fall back to manual entry with inline error (`⚠ unable to fetch apps, type manually`).
- Providers must always return a **bounded list**; large result sets should be truncated with “load more.”

---

## 9. UX Considerations

- **Power Mode**
  - Autocomplete popup shows provider-sourced values with context in dimmed text.
  - Async results appear incrementally (spinner until ready).
- **Guided Mode**
  - Fields with providers render as dropdowns.
  - Async fetch shows spinner; fallback to manual text input if fails.
- **Transparency**
  - Values from workflow outputs clearly labeled (e.g., “from task: create_app”).
  - Plugin-provided values badged `[PLG]`.

---

## 10. Security

- Providers must **filter sensitive fields**: API tokens, credentials, DB URLs.
- Cache is in-memory only (not persisted to disk).
- Plugins can suggest values but not inject ANSI or escape sequences.

---

## 11. Extensibility

- Add new providers without changing CLI core.
- Plugins can register providers by returning:
  ```json
  {
    "tool": "db-tools",
    "autocomplete": {
      "flag": "--db",
      "method": "listDatabases"
    }
  }
  ```
- CLI maps flag → MCP autocomplete call.

---

## 12. Next Steps for Refinement

- Define a **registry format** for mapping flags → provider IDs.
- Formalize the **template language** for workflow interpolation (`${{...}}`).
- Establish **error contracts** for providers (timeout, empty, unauthorized).
- Prototype Guided UI integration with async dropdown population.
- Build test harnesses for providers with simulated responses.

---

## 13. Open Questions

- Should providers support **filtering by partial input** (prefix search) to avoid fetching entire datasets?
- Do we need a **global registry** of available providers, or should they be discovered per-command from schema + plugins?
- How to handle **multi-value flags** (e.g., `--var KEY=VALUE`) in providers?
- Should **workflow outputs** be persisted for later sessions, or ephemeral only?

---
