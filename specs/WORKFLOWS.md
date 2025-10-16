# WORKFLOWS — Comprehensive Specification

This document defines reusable workflow patterns for the Heroku CLI TUI, based on the available commands in the current `manifest.json` and enhanced with **ValueProviders** for dynamic, schema‑aware parameter. see `VALUE_PROVIDERS.md`

---

## 1. Principles

* **Declarative first**: Workflows are YAML files that declare steps in order.
* **ValueProviders**: Inputs can be sourced from:

  * **Static enums** (from schema)
  * **Dynamic built‑ins** (`apps list`, `addons list`, `pipelines list`, `teams list`, etc.)
  * **Workflow outputs** (`${{steps.<id>.output.<field>}}`)
  * **Plugin providers** (via MCP)
* **Resilient**: Providers support caching, async refresh, and fallbacks.
* **Composable**: Outputs from one step can feed into later steps.
* **User-friendly**: The TUI surfaces provider‑backed inputs with searchable dropdowns and details.

---

## 2. Other candidate workflows ideas:

* **Dyno/process management**: scale up/down, restart, resize
* **Releases**: create new release, rollback, set description, list previous releases
* **Review Apps pipeline config**: enable auto‑create/destroy, manage settings
* **Database lifecycle**: backups, restores, followers, maintenance
* **Classic log drains**: create/delete/tail beyond `telemetry-drains`

---

## 3. TUI Integration with ValueProviders

Key points:

* Provider‑backed inputs declare `provider` and `select` fields for dynamic values.
* Guided mode shows provider results in **Table selectors** with optional detail panes.
* Power mode uses providers for **autocomplete suggestions**.
* Fallbacks and cache status are visible inline (with icons/labels).

---

## 2.x Dependent Providers & Auto-Mapping from Previous Outputs

### Goal

Allow a provider’s arguments to be **derived automatically** from values produced by prior inputs or steps, avoiding a JSON field selection step whenever we can.

### Authoring (YAML)

```yaml
inputs:
  app:
    provider: apps list
    select: { value_field: name, display_field: name, id_field: id }

  addon:
    provider: addons list
    provider_args:
      app:
        from_step: create_app           # or `from_input: app`
        path: output.name               # JSON path relative to the step/input
        required: true
        on_missing: prompt              # prompt | fail | skip
    select: { value_field: name, display_field: name, id_field: id }
```

**Shorthand (templating):**

```yaml
provider_args: { app: ${{ steps.create_app.output.name }} }
```

### Auto-Mapping Algorithm (Runtime)

1. **Explicit mapping wins**: If `provider_args.*` references a path, resolve it.
2. **Heuristic mapping** (when explicit not provided):

   * Inspect recent **inputs** and **step outputs** for likely candidates.
   * Rank by:

     1. name match to arg (`app`, `pipeline`, `addon`, etc.)
     2. schema type tags (e.g., `app_id`, `app_name`)
     3. common aliases (`name`, `id`, `slug`)
   * If a single unambiguous candidate remains → auto-select and badge `auto→ steps.create_app.output.name`.
3. **Ambiguity**: When 2+ candidates remain, toggle the inline **Field Picker pane** (below) to finish mapping.
4. **Persistence**: Remember the user’s choice for the rest of the run; optionally persist per-workflow.

### Field Picker UI (only when needed)

```
┌ Choose field for provider arg: app (addons list) ──────────────────────┐
│ Auto candidates:                                                       │
│  1) steps.create_app.output.name        → "billing-svc"                │
│  2) steps.create_app.output.id          → "app-456"                    │
│  3) inputs.app                           → "billing-svc"               │
│                                                                        │
│ Browse any JSON:                                                       │
│  > steps.<…>  (←/→ expand & collapse • ↑/↓ move • type to filter)      │
└────────────────────────────────────────────────────────────────────────┘
```

**Keybindings:** `f` toggle picker pane • type or `/` to filter • `↑/↓` move • `←/→` expand/collapse • `Enter` select • `Esc` return to candidates.

The picker renders inline inside the Guided Input Collector so users can review argument contracts, cache badges, and schema tags while browsing context data. When the picker is active, typed characters update the filter instead of moving the selection.

### Badging & Inspectability

* Inline arg preview shows: `app = auto→ steps.create_app.output.name`.
* Press `i` to open a small popover with the exact template path and current value.

### Errors

* If the resolved path is missing:

  * `on_missing: prompt` → open the Field Picker pane for manual selection.
  * `on_missing: skip` → disable this provider field and allow manual entry.
  * `on_missing: fail` → block run and show actionable error.

### Contracts

* Providers should declare **arg contracts** (e.g., `app` accepts `name|id`).
* Steps can declare **output contracts** to help heuristics:

```yaml
steps:
  - id: create_app
    run: apps create
    output_contract:
      fields:
        - name: name
          tags: [app_name]
        - name: id
          tags: [app_id]
```

### Minimal JSON-Path

* Support a constrained path syntax: `inputs.X`, `steps.Y.output.Z`, with dot access and array indices.
* No arbitrary expressions—keep resolvers predictable and fast.

---

# 5. Dependent Providers in Workflows

Some providers require arguments derived from earlier inputs or step outputs. Prefer **auto-mapping** to avoid forcing the user into a JSON field picker.

## 5.1 Authoring Syntax

```yaml
inputs:
  app:
    provider: apps list
    select: { value_field: name, display_field: name, id_field: id }

  addon:
    provider: addons list
    provider_args:
      app:
        from_step: create_app           # or from_input: app
        path: output.name               # minimal JSON path
        required: true
        on_missing: prompt              # prompt | fail | skip
    select: { value_field: name, display_field: name, id_field: id }
```

**Shorthand:**

```yaml
provider_args: { app: ${{ steps.create_app.output.name }} }
```

## 5.2 Output Contracts for Better Auto-Mapping

```yaml
steps:
  - id: create_app
    run: apps create
    output_contract:
      fields:
        - name: name
          tags: [app_name]
        - name: id
          tags: [app_id]
```

These tags guide heuristics when a provider arg could be satisfied by multiple fields.

`SchemaProperty` entries now mirror these contracts. During registry generation the resolver keeps
track of JSON type, required keys, array item shape, enumerated literals, any `format` hint, and
the tags surfaced through the workflow contracts. The Field Picker and auto-mapper consume this
metadata to:

* Prefer fields whose tags intersect with provider requirements (`app_id`, `pipeline_slug`, etc.).
* Render nested objects and arrays with accurate type badges (for example, `array<uuid>`).
* Show enum literals directly in the picker when manual disambiguation is needed.
* Flag missing-but-required keys before prompting the user.

## 5.3 Runtime Resolution Order

1. Use explicit templated path if present.
2. If missing, try heuristic auto-mapping using names/aliases and `output_contract` tags.
3. If ambiguous, open a **Field Picker** in the TUI; remember the choice for the session.

## 5.4 Error Policy

* `on_missing: prompt` → surface the Field Picker pane.
* `on_missing: skip` → allow manual entry.
* `on_missing: fail` → stop with actionable error.

---

# 6. Provider Registry & Arg Contracts

To make dependent providers reliable, each provider can declare the shape of its output and the fields that are valid for downstream consumption.

## 6.1 Provider Metadata Schema Extension

```yaml
providers:
  apps list:
    returns:
      fields:
        - name: id
          type: string
          tags: [app_id]
        - name: name
          type: string
          tags: [app_name, display]
        - name: owner
          type: string
  addons list:
    requires:
      - app   # must resolve to app_id or app_name
    returns:
      fields:
        - name: id
          type: string
          tags: [addon_id]
        - name: name
          type: string
          tags: [addon_name, display]
```

### Notes

* `requires` lists which arguments the provider needs and acceptable tags.
* `returns.fields` defines the shape of each item for `select.value_field`, `display_field`, and tagging.
* **Auto-mapping** uses these tags to match a provider’s required `app` parameter with a previous step’s output tagged as `app_id` or `app_name`.

## 6.2 Workflow Example with Registry-aware Provider

```yaml
inputs:
  app:
    provider: apps list
    select:
      value_field: name
      display_field: name
      id_field: id

  addon:
    provider: addons list
    provider_args:
      app: auto
    select:
      value_field: name
      display_field: name
      id_field: id
```

Here `app: auto` tells the engine to pick the most appropriate previous output tagged as `app_id` or `app_name`. If multiple matches exist, the TUI launches the Field Picker.

---

# 7. Widget Behaviors & Keybindings

(unchanged from previous section)

---

# 6. Provider Argument Contracts Registry (Robust Architecture)

A small, declarative registry lets providers advertise which arguments they accept and in what shapes (name vs id, etc.). The resolver uses this to validate, coerce, and auto-map fields from prior inputs/outputs—reducing ambiguity and picker pop-ups.

## 6.1 Registry Format (host-level)

```yaml
provider_arg_contracts:
  addons list:
    args:
      app:
        accepts: [app_name, app_id]
        prefer: app_id                 # used when both are present
        coerce:
          app_name: { type: string }
          app_id:   { type: string, pattern: "^app-[0-9a-z]+$" }
  pipelines list:
    args: {}
  pipeline-promotions create:
    args:
      pipeline: { accepts: [pipeline_id, pipeline_name], prefer: pipeline_id }
      source.app: { accepts: [app_id, app_name], prefer: app_id }
      targets[].app: { accepts: [app_id, app_name], prefer: app_id }
  telemetry-drains create:
    args:
      owner.space.name: { accepts: [space_name] }
  apps builds:create:
    args:
      app: { accepts: [app_id, app_name], prefer: app_id }
```

> **Paths** use dot notation; arrays indicated by `[]`. Contracts can be nested to match request bodies.

## 6.2 Step Output Contracts (producer side)

```yaml
steps:
  - id: create_app
    run: apps create
    output_contract:
      fields:
        - name: name
          tags: [app_name]
        - name: id
          tags: [app_id]
```

## 6.3 Resolution Algorithm (revisited)

1. **Explicit path** in YAML → resolve and validate against `accepts`.
2. **Heuristic auto-map** using:

   * exact tag match (e.g., need `app_id` → pick output tagged `app_id`).
   * name/alias similarity (`app`, `pipeline`, `addon`).
   * `prefer` hint when multiple `accepts` match.
3. **Coercion** if permitted (e.g., strip/format prefixes, convert numeric → string).
4. **Ambiguity** → open Field Picker, seeded with candidates that satisfy the contract.
5. **Validation** before run: if unresolved required args remain, block with actionable error.

## 6.4 Examples

### A) `addons list` needs `app`

```yaml
inputs:
  app:
    provider: apps list
    select: { value_field: name, display_field: name, id_field: id }

  addon:
    provider: addons list
    provider_args:
      app: ${{ steps.create_app.output.id }}   # satisfies app_id
    select: { value_field: name, display_field: name, id_field: id }
```

### B) `pipeline-promotions create` (nested args)

```yaml
inputs:
  pipeline:
    provider: pipelines list
    select: { value_field: id, display_field: name }
  from_app:
    provider: apps list
    select: { value_field: id, display_field: name }
  to_apps:
    provider: apps list
    select: { value_field: id, display_field: name }
    mode: multiple

steps:
  - id: promote
    run: pipeline-promotions create
    body:
      pipeline:
        id: ${{ inputs.pipeline }}
      source:
        app:
          id: ${{ inputs.from_app }}
      targets: "${{ inputs.to_apps.map(id => ({ app: { id } })) }}"
```

### C) `apps builds:create` preferring id but accepting name

```yaml
inputs:
  app:
    provider: apps list
    select: { value_field: id, display_field: name, id_field: id }
  tar_url:
    type: string

steps:
  - id: build
    run: apps builds:create
    with:
      app: ${{ inputs.app }}           # app_id chosen per contract
    body:
      source_blob:
        url: ${{ inputs.tar_url }}
```

## 6.5 Validation & Errors

* If a provided value doesn’t match any `accepts`, show: `app expected one of [app_id, app_name]; got object`.
* For nested paths, error paths use JSON pointer-style: `/targets/0/app/id`.
* Contracts can mark an arg `required: true`; missing required → block before execution.

## 6.6 Extensibility & Versioning

* Contracts versioned with `schema_version` (e.g., `1`).
* Plugins (MCP) can **extend** contracts by contributing fragments under their provider IDs.
* Host merges fragments by path (deep merge) with precedence: **workflow override > plugin > core**.

## 6.7 Performance

* Cache contract lookups by provider id.
* Pre-compile validators (regexes, required sets) on load.

## 6.8 Docs Generation

* The registry can auto-generate a **Provider Reference** section listing each provider, its arguments, accepted shapes, and examples.
