# Workflows Authoring Specification

This document is the canonical source for writing workflow manifests that power the
Oatty CLI TUI. It captures every supported field in the YAML schema, explains how the engine
interprets those fields, and cross-links the companion UX specifications:

- `specs/WORKFLOW_TUI.md` — input collection, manual entry behaviours, run view UI
- `specs/WORKFLOW_RUN_EXECUTION_PLAN.md` — execution pipeline, pause/resume/cancel controls
- `specs/WORKFLOW_VALUE_PROVIDERS_UX.md` — provider discovery and picker ergonomics

The definitions below mirror the shared Rust model in `crates/types/src/workflow.rs`. When the
schema evolves, keep this file, the Rust structures, and the UI/engine specs in sync.

---

## 1. Manifest Anatomy

Workflows are authored as YAML (or JSON) files under `workflows/`. Order matters: inputs are
presented to the user in the order they are written, and steps execute sequentially.

```yaml
workflow: app_with_db                 # required identifier
title: "Create App with Postgres"     # optional friendly title
description: "Creates an app, provisions Postgres, seeds config."

inputs:
  app_name:
    description: "App name shown in dashboards and CLI"
    type: string
    validate:
      required: true
      pattern: "^[a-z](?:[a-z0-9-]{1,28}[a-z0-9])$"
  region:
    provider: regions list
    select:
      value_field: name
      display_field: name
    default:
      from: literal
      value: "us"

steps:
  - id: create_app
    run: apps create
    body:
      name: ${{ inputs.app_name }}
      region: ${{ inputs.region }}
  - id: add_pg
    run: apps addons:create
    with:
      app: ${{ inputs.app_name }}
    body:
      plan: ${{ inputs.addon_plan }}
```

---

## 2. Top-level Metadata

| Field       | Type     | Default | Notes                                                                                                       |
|-------------|----------|---------|-------------------------------------------------------------------------------------------------------------|
| `workflow`  | string   | —       | Required unique identifier (snake case recommended). Used for routing, telemetry, CLI lookups.             |
| `title`     | string   | `null`  | Optional human-friendly label. Shown in workflow pickers and run views when present.                       |
| `description` | string | `null`  | Optional long description rendered in selectors, detail panes, and run summaries.                          |
| `inputs`    | mapping  | `{}`    | Ordered map of input definitions. Keys become input identifiers referenced by steps and bindings.          |
| `steps`     | array    | `[]`    | Ordered list of execution steps. Must contain at least one entry.                                          |

All other behaviour is driven by nested structures documented below.

---

## 3. Inputs

Inputs collect values before execution. The schema preserves the authoring order using
`IndexMap`, so declare inputs in the sequence you want to present them. Input keys should be
lower snake case and match the identifiers you reference via `${{ inputs.<name> }}`.

### 3.1 Input Definition Fields

| Field            | Type                         | Default        | Purpose |
|------------------|------------------------------|----------------|---------|
| `description`    | string                       | `null`         | Copy shown in the input list and detail pane. |
| `name`           | string                       | `null`         | Optional human-friendly label rendered in collectors and detail panes. |
| `type`           | string                       | `null`         | Optional hint (`string`, `number`, `boolean`, `array<uuid>`, etc.) used by manual entry UX. |
| `provider`       | string \| object             | `null`         | Value provider identifier. May be a shorthand string (`apps list`) or a detailed object `{ id, field }`. |
| `select`         | object                       | `{}`           | Describes how to extract fields from provider records (see §3.2). |
| `mode`           | enum (`single` \| `multiple`)| `single`       | Selection mode for provider-backed inputs. |
| `cache_ttl_sec`  | integer                      | `null`         | Override cache time-to-live (seconds) for provider results. |
| `on_error`       | enum (`manual` \| `cached` \| `fail`) | `null` | Provider failure policy (prompt manual entry, surface cached results, or abort). |
| `default`        | object                       | `null`         | Default sourcing strategy (see §3.3). |
| `provider_args`  | map<string, argument value>  | `{}`           | Provider arguments keyed by name. Values can be literal templates or structured bindings (see §4). |
| `depends_on`     | map<string, argument value>  | `{}`           | Lightweight dependency bindings that auto-populate provider arguments and gate readiness (see §4.3). |
| `join`           | object                       | `null`         | Configuration for concatenating multi-select values (see §3.4). |
| `optional`       | bool                         | `false`        | When `true`, unresolved inputs do not block the Run button. |
| `validate`       | object                       | `null`         | Client-side validation rules (see §5). |
| `placeholder`    | string                       | `null`         | Placeholder text for manual entry and filter controls. |
| `enum` / `enumerated_values` | array<JSON>     | `[]`           | Author-supplied literal options used by manual entry UI and validation. |

Inputs without a `provider` rely on manual entry; the TUI surfaces controls derived from `type`,
`enum`, and `validate`.

### 3.2 Provider Selection Metadata

When `provider` is present, configure the `select` block to control how each result populates the
input:

| Field         | Type   | Default | Notes |
|---------------|--------|---------|-------|
| `value_field` | string | `null`  | Field to submit as the resolved value (stored in `inputs.<name>`). |
| `display_field` | string | `null` | Field rendered as the primary label in tables/lists. |
| `id_field`    | string | `null`  | Stable identifier for caching, analytics, and change detection. |

When `select` is omitted, the engine passes the full provider item to the UI. Use this for JSON
objects that must be inspected in the detail pane before committing a value.

### 3.3 Default Value Sources

The `default` block specifies an initial value. It contains `from` (required) and optional
`value` payload:

| `from` value        | Description                                                                                  |
|---------------------|----------------------------------------------------------------------------------------------|
| `history`           | Pull the most recent value entered for this input (per user).                                |
| `literal`           | Use `default.value` as-is (may include templates).                                           |
| `env`               | Resolve an environment variable (`value` must contain the name or template).                 |
| `workflow_output`   | Reference prior workflow run outputs via templating (for replay scenarios).                  |

Defaults are materialised as soon as a run state is created. Literal and workflow-output defaults
run through the normal interpolation pipeline so templates can reference previously seeded inputs
or step outputs. Environment defaults first consult the run context's environment map (populated by
callers) and then fall back to process variables. History defaults load from the per-user history
store: the engine fetches the most recent persisted value, validates it against the input schema,
and applies it when safe. Values that fail validation or hit the redaction heuristics are ignored
so the input remains pending. Successful workflow runs persist history defaults automatically for
future sessions.

Providing both `default` and `provider` enables an initial selection while still allowing edits.

### 3.4 Joining Multi-select Values

When `mode: multiple`, you may need to collapse selections into a single string argument. Set
`join` to control rendering:

| Field       | Type    | Description                                 |
|-------------|---------|---------------------------------------------|
| `separator` | string  | Characters inserted between values.         |
| `wrap_each` | string? | Optional wrapper (e.g., `"\""` for quotes). |

Joined values feed into templated arguments via `${{ inputs.<name> }}`.

---

## 4. Provider Arguments, Dependencies, and Bindings

### 4.1 Literal Arguments

Entries in `provider_args` or `depends_on` can be literal strings. Literals support full template
interpolation (see §7):

```yaml
provider_args:
  app: ${{ inputs.app_name }}
  region: ${{ env.REGION }}
```

### 4.2 Structured Bindings

To reference nested JSON from previously resolved data, use the object form:

```yaml
provider_args:
  app:
    from_step: create_app
    path: output.id
    required: true
    on_missing: prompt         # prompt | skip | fail
```

Binding fields:

| Field        | Description |
|--------------|-------------|
| `from_step`  | Step identifier to pull from (`steps.<id>.output`). |
| `from_input` | Input identifier to reuse (`inputs.<name>`). Mutually exclusive with `from_step`. |
| `path`       | Dot-delimited path relative to the chosen source. Omit to use the full value. |
| `required`   | Marks the binding as essential. Defaults to `false`. |
| `on_missing` | Overrides the missing-value policy. Defaults to `fail` when `required`, otherwise `prompt`. |

The engine produces outcomes that the TUI renders as automatic resolutions, prompts, or errors
depending on the policy.

### 4.3 `depends_on`

`depends_on` shares the same value shape as `provider_args` but communicates two additional pieces
of intent:

1. Inputs remain disabled (status **Waiting**) until dependencies resolve.
2. The resolved values automatically seed matching provider arguments without requiring duplicate
entries in `provider_args`.

Use `depends_on` for straightforward “copy another input” scenarios and reserve structured
`provider_args` for more complex binding logic.

---

## 5. Validation

Attach a `validate` block to enforce client-side rules before enabling the Run button:

| Field         | Type               | Default | Notes |
|---------------|--------------------|---------|-------|
| `required`    | bool               | `false` | When `false`, empty values are allowed unless `optional` is also `false`. |
| `enum`        | array<JSON>        | `[]`    | Allowed literal values. Works with manual entry and provider selections. |
| `pattern`     | string (regex)     | `null`  | Rust-style regular expression applied to string values. |
| `min_length`  | integer            | `null`  | Minimum string length. |
| `max_length`  | integer            | `null`  | Maximum string length. |

Validation messages surface inline in the input list and manual entry modal. For complex types,
validation runs against the rendered string representation (for example, after `join` transforms a
multi-select). The TUI spec (`WORKFLOW_TUI.md`) describes how errors are displayed.

---

## 6. Steps

Steps execute sequentially (honouring any `depends_on` order constraints). Each entry is a map:

| Field          | Type              | Default | Purpose |
|----------------|-------------------|---------|---------|
| `id`           | string            | —       | Required unique identifier used by bindings and telemetry. |
| `run`          | string            | —       | Command identifier (`group:command`) resolved via the CLI registry. |
| `description`  | string            | `null`  | Optional copy shown in the run timeline and detail panes. |
| `depends_on`   | array<string>     | `[]`    | Step identifiers that must finish before this step can start. |
| `when` / `if`  | string            | `null`  | Conditional expression that must evaluate truthy for the step to run. Accepts raw expressions (`inputs.flag`) or `${{ ... }}` wrappers. |
| `with`         | map<string, JSON> | `{}`    | Flag/argument map (values support templates). |
| `body`         | JSON value        | `null`  | Request body payload (fully templated). |
| `repeat`       | object            | `null`  | Polling configuration for loops (see §6.1). |
| `output_contract` | object         | `null`  | Declares exported fields for downstream bindings (see §6.2). |

The condition parser understands logical operators (`&&`, `||`), unary negation (`!`), equality and
inequality comparisons (including `null` checks), and truthiness evaluation for resolved values.
When `${{ ... }}` wrappers are used the engine strips them before evaluation, so both `when:
${{ inputs.flag }}` and `when: inputs.flag` behave equivalently.

### 6.1 Repeat Configuration

Use `repeat` to describe polling or retry loops:

| Field         | Type    | Default | Notes |
|---------------|---------|---------|-------|
| `until`       | string  | `null`  | Expression (using `${{ ... }}` syntax) evaluated after each attempt. |
| `every`       | string  | `null`  | Interval between attempts (human-friendly strings such as `10s`). |
| `timeout`     | string  | `null`  | Maximum wall-clock duration (e.g., `5m`). |
| `max_attempts`| integer | `null`  | Hard cap on attempt count. |

The execution plan (see `WORKFLOW_RUN_EXECUTION_PLAN.md`) covers how the runner evaluates these
fields and emits status updates.

### 6.2 Output Contracts

Output contracts document the shape of a step’s results and annotate fields with semantic tags to
guide auto-mapping:

```yaml
steps:
  - id: create_app
    run: apps create
    output_contract:
      fields:
        - name: id
          tags: [app_id]
          type: string
        - name: name
          tags: [app_name]
          description: "Display name shown in dashboards"
```

Each field entry supports:

| Field        | Description |
|--------------|-------------|
| `name`       | JSON field exported to bindings (`steps.<id>.output.<name>`). |
| `tags`       | Semantic labels (`app_id`, `addon_name`, etc.) that provider contracts consume. |
| `description`| Optional helper text for detail panes. |
| `type`       | Optional type hint (string, uuid, array<object>, etc.). |

---

## 7. Templates and Expressions

The engine resolves `${{ ... }}` expressions across literals, arguments, and step payloads using
`crates/engine/src/resolve.rs`. Supported contexts:

- `inputs.<name>` — previously resolved inputs (`serde_json::Value`)
- `steps.<id>.output.<path>` — JSON output from completed steps
- `env.<VAR_NAME>` — environment variables injected when the workflow runs

Expressions support:

- Equality checks (`${{ inputs.region == "us" }}`)
- Truthiness (`${{ steps.deploy.output.success }}`)
- Array membership (`${{ steps.promote.output.targets.includes(inputs.app) }}`)

Templates are resolved recursively through nested objects and arrays. When referencing entire JSON
values, prefer structured bindings (§4.2) so the engine can validate types and missing data.

---

## 8. Provider Error Handling and Caching

Providers may fail because APIs are unavailable or credentials expire. The following knobs control
behaviour:

- `cache_ttl_sec` adjusts how long results stay fresh. When omitted, the runtime default applies.
- `on_error: manual` opens the manual entry modal so the user can supply a value.
- `on_error: cached` surfaces the last cached provider response (if available).
- `on_error: fail` blocks the workflow and surfaces an error message.

The runtime also badges inputs whose dependencies (`depends_on`) are unresolved so users understand
why a provider is waiting.

---

## 9. Manual Entry Enhancements

Inputs without providers, or those whose providers fail and fall back to manual entry, use metadata
from the definition to render typed controls:

- `type` informs keyboard handling (`boolean`, `number`, `string`, `array`) and formatting.
- `enum` / `enumerated_values` produces radio/select inputs with optional filter support.
- `placeholder` seeds empty states.
- Validation (required, pattern, min/max length) displays inline errors before accepting values.

See `specs/WORKFLOW_TUI.md` for control layouts and focus behaviour.

---

## 10. Cross-Spec References

- **Collection experience**: `specs/WORKFLOW_TUI.md`
- **Collector manual entry component**: `crates/tui/src/ui/components/workflows/collector/manual_entry/`
- **Run view and execution controls**: `specs/WORKFLOW_RUN_EXECUTION_PLAN.md`
- **Provider registry contracts**: `specs/VALUE_PROVIDER_REGISTRY.md`
- **Runtime schema**: `crates/types/src/workflow.rs` (mirror changes here)

When adding new fields, update both the Rust types and this document, then cross-link any UI or
engine specs affected by the change.

---

## 11. Change Log

- **2025-10-17** — Expanded specification to cover workflow titles, provider bindings, validation,
  repeat loops, manual entry metadata, and cross-spec references. Document now mirrors the entire
  schema implemented in `crates/types/src/workflow.rs`.
