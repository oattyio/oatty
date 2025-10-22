# Workflow ValueProviders & TUI UX Spec (Heroku CLI TUI)

**Purpose**
Define how ValueProviders are declared and consumed in `WORKFLOWS.yaml`, and how the TUI should present provider-backed inputs using the existing Table and Key/Value viewer widgets (extended for selection).

---

## 1) YAML Authoring Conventions for ValueProviders

### 1.1 Input-level provider declaration

```yaml
inputs:
  app:
    description: "Target application"
    provider: apps list                # built-in dynamic provider id
    select:
      value_field: name                # value inserted into flags/args
      display_field: name              # primary display column in UI
      id_field: id                     # optional stable id for caching/telemetry
    mode: single                       # single | multiple
    cache_ttl_sec: 60                  # override default TTL
    on_error: manual                   # manual | fail | cached
    default:
      from: history                    # history | literal | env | workflow_output
      value: ""                        # optional (for literal/env)

  addon:
    description: "Choose an add-on for the selected app"
    provider: addons list
    provider_args:
      app: ${{ inputs.app }}           # provider params can reference other inputs
    select:
      value_field: name
      display_field: name
      id_field: id
    mode: single
```

> **Why `select`?** Providers often return rich objects (e.g., `{ id, name, meta }`). `select.value_field` specifies which field from each provider item is inserted as the actual argument value. `display_field` renders in the TUI list, and `id_field` stabilizes selection & caching.

### 1.2 Step-level usage

```yaml
steps:
  - id: set_config
    run: apps config-vars:update
    with:
      app: ${{ inputs.app }}
    body:
      DB_ADDON: ${{ inputs.addon }}
```

### 1.3 Advanced: multiple selection & join

```yaml
inputs:
  collaborators:
    description: "Grant access to users"
    provider: users list
    select:
      value_field: email
      display_field: email
      id_field: id
    mode: multiple
    join:
      separator: ","      # produces "a@x.com,b@y.com"
      wrap_each: ""        # e.g., '"' to wrap values with quotes
```

### 1.4 Filtering & sorting at source

```yaml
inputs:
  prod_app:
    provider: apps list
    provider_args:
      tag: production
      owner: me
    select:
      value_field: name
      display_field: name
      id_field: id
```

### 1.5 Defaults pulled from workflow outputs

```yaml
inputs:
  staged_app:
    provider: apps list
    default:
      from: workflow_output
      value: ${{ steps.create_app.output.name }}
```

### 1.6 Declarative validation

```yaml
inputs:
  region:
    provider: regions enum
    select:
      value_field: slug
      display_field: name
    validate:
      required: true
      enum: [us, eu, tokyo]
```

### 1.7 Fallbacks & UX hints

```yaml
inputs:
  app:
    provider: apps list
    select: { value_field: name, display_field: name, id_field: id }
    placeholder: "Start typing to search apps…"  # shown in TUI when empty
    on_error: manual
```

---

## 2) TUI UX — Annotated ASCII Layouts

> The TUI reuses the existing **Table** and **Key/Value Viewer** with small extensions: selection, search, and an optional detail pane. Below are the canonical screens.

### 2.1 Workflow Picker

```
┌─ Workflows ──────────────────────────────────────────────────────────┐
│ Search: [provi]                                                      │
├──────────────────────┬───────────────────────────────────────────────┤
│ provision_and_promote│ Create build, then promote across pipeline    │
│ app_with_db          │ Create app, set config, attach Postgres       │
│ cache_bust           │ Clear build cache, audit buildpacks           │
└──────────────────────┴───────────────────────────────────────────────┘
  ↑↓ select  •  / search  •  Enter run  •  Esc back
```

### 2.2 Input Collection View (Provider-backed fields)

```
┌─ Run: provision_and_promote ──────────────────────────────────────────────────────────────────────────────────┐
│ Steps & Inputs (left)                                                    │ Workflow Details (right)           │
│                                                                          │                                    │
│ ▸ pipeline        ✓ Looks good!                              [required]  │ Ready?:           ⚠ Waiting on app │
│ staging_app       ⚠ No value                                 [required]  │ Resolved steps:   1 / 3            │
│ prod_app          ⚠ No value                                             │ Next action:      staging_app      │
│                                                                          │ Selected values:                   │
│                                                                          │   • pipeline      → demo pipeline  │
│                                                                          │   • staging_app   → — pending —    │
│                                                                          │ Auto-reset note:  downstream steps │
│                                                                          │                   reset when a     │
│                                                                          │                   prior step edits.│
│                                                                          │                                    │
│                                                                          │ Errors & notes:   none             │
│                                                                          │                                    │
└───────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
Buttons: [Cancel]   [Run ✓]
  Esc cancel  •  ↑↓ navigate  •  Enter choose  •  / filter  •  r refresh  •  Tab focus  •  F2 fallback to manual  •  Enter run
```

- **Left column** is a compact step list rendered like the runtime List view in
  `workflows/input.rs` (status prefix, provider label, required badge, highlight marker) without
  inlining provider result tables, and each step stays on a single line for quick scanning.
- **Right column** summarizes workflow readiness: completion state, next unresolved step,
  currently selected values (show `— pending —` for unresolved items), cache freshness, and
  any validation errors.
- Unless step values are mutually exclusive of on-another e.g., no chained providers, when a previously
  resolved step changes, every chained step reverts to its default value (or unresolved if no default).
  The detail pane immediately reflects the reset so users see which values need attention before execution.
  The detail pane should show a note about the auto-reset behavior if applicable.
- The detail pane updates after every interaction (selection change, manual value entry,
  refresh, fallback), so the aggregate view always mirrors the live state.
- Manual entry is triggered by pressing `F2` when a provider is present or by default when pressing
  `Enter` when the provider is not present.
- Defaults declared in the workflow manifest populate the run context immediately. Literal and
  workflow-output defaults are interpolated before rendering, and environment defaults prefer the
  run context's environment map before falling back to OS variables. History defaults pull from the
  per-user history store (`~/.config/heroku/history.json` by default); successful runs persist
  values automatically, and the UI logs a friendly message whenever a stored value is skipped
  because it failed validation or matched redaction heuristics.
- The footer hosts **Cancel** (secondary) and **Run** (primary) buttons. Run stays disabled until every
  required input resolves; both buttons participate in the focus ring (Tab/Shift+Tab and Left/Right),
  accept `Enter`/`Space`, and handle mouse clicks. Cancel returns to the workflow list. Run currently
  calls a stubbed execution hook so backend wiring can land later without changing the UI contract.

### 2.3 Manual Entry View
Manual entry now adapts to the declared input type instead of always presenting a raw
string field. The modal keeps the compact centered layout but swaps the value area
based on the workflow schema:

```
┌─ Manual entry: pipeline ───────────────────────────────────────────────┐
│ Enter a value                                                          │
│  Value: my-pipeline                                                    │
└────────────────────────────────────────────────────────────────────────┘
 Esc cancel  •  Enter confirm

┌─ Manual entry: leader_instance_count ──────────────────────────────────┐
│ Enter an integer                                                       │
│  Value: 2                                                              │
└────────────────────────────────────────────────────────────────────────┘
 Esc cancel  •  Enter confirm

┌─ Manual entry: configure_follower ─────────────────────────────────────┐
│ Select true or false                                                   │
│ [True ✓]   [False]                                                     │
└────────────────────────────────────────────────────────────────────────┘
 Esc cancel  •  Space toggle  •  Enter confirm

┌─ Manual entry: follower_operation ─────────────────────────────────────┐
│ Choose from the available options                                      │
│ ┌────────────────────────────────────────────────────────────────────┐ │
│ │ create                                                             │ │
│ │ update ✓                                                           │ │
│ └────────────────────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────┘
 Esc cancel  •  ↑↓ move  •  Space confirm  •  Enter confirm
```

- **Text / number / integer** inputs reuse the shared `TextInputState` editing
  semantics (UTF-8 cursor movement, backspace) and run the existing validation
  pipeline (`pattern`, length limits, allowed values) before submission. Integer and
  number variants reject invalid characters and require a parsable value.
- **Boolean** inputs render two themed buttons. `Left/Right`, `0/1`, `T/F`, or `Space`
  flip the selection; `Enter` accepts. The footer hints update accordingly.
- **Enum** inputs render a scrollable list backed by the selector table styling. Up/Down,
  Home/End, mouse clicks, or `Space`/`Enter` confirm the highlighted option.
- Validation failures surface inline error text beneath the control without closing the
  modal. Successful confirmation stores the typed JSON value (string, number, boolean,
  or the literal enum value) in the workflow run state.
- Manual entry share the same focusing and hint helpers as other modals, so `Esc`
  always cancels and returns control to the collector without mutating the previous
  value.

### 2.4 Table Selector with Detail Pane (Single-select)

This view appears when the user highlights a step in the Input Collection list and presses `Enter`.
The top table lets them choose a row (provider item), which simultaneously populates the key/value
detail pane below. From that detail pane the user confirms the exact value fed into the workflow
step. Rows, headers, and selection states are powered by the reusable table component in
`crates/tui/src/ui/components/table`.

Controls:
- `Space`/`Enter` in the table selects a row and enables the `Apply` button.
- `Apply` (or pressing `Enter`) forwards the chosen value to the workflow step and closes the
  modal.
- `Cancel` (or `Esc`) closes the modal without changing the pending step.

```
┌ Apps — Select one (apps list) ────────────────────────────────────────┐
│ Filter: bill                                 Status: loaded           │
├──────────────┬───────────┬────────────────────────────────────────────┤
│ name         │ id        │ meta                                       │
├──────────────┼───────────┼────────────────────────────────────────────┤
│ ▸ billing-svc│ app-456   │ team: infra                                │
│   demo-app   │ app-123   │ owner: justin@example.com                  │
└──────────────┴───────────┴────────────────────────────────────────────┘
┌ Details (Key/Value) ───────────────────────────────────────────────────┐
│ name         : billing-svc                                             │
│ id           : app-456                                                 │
│ owner        : infra                                                   │
│ created_at   : 2025-03-08                                              │
└────────────────────────────────────────────────────────────────────────┘
Buttons: [Cancel]   [Apply ✓]
  Esc cancel  •  ↑↓ move  •  Space select  •  Enter confirm  •  r refresh

**Schema-aware badges**

- Display the JSON type using the enriched `SchemaProperty` metadata (`object`, `array<uuid>`,
  `enum`, etc.).
- Surface tags (for example, `app_id`, `pipeline_slug`) next to each candidate to clarify why it
  matches a provider requirement.
- When `enum_values` exists, render the literal set in the detail pane to aid manual overrides.
- Respect `required` keys by flagging missing fields before confirm.
```

### 2.5 Table Selector (Multi-select) with Chip Summary

```
┌ Users — Select multiple (users list) ─────────────────────────────────┐
│ Selected: [alice@x.com] [bob@y.com]                                   │
│ Filter: []                                                            │
├──────────────┬──────────┬──────────────┬──────────────────────────────┤
│ email        │ id       │ team         │ meta                         │
├──────────────┼──────────┼──────────────┼──────────────────────────────┤
│ ☑ alice@x.com│ u-101    │ platform     │ admin                        │
│ ☐ bob@y.com  │ u-102    │ product      │                              │
│ ☐ carol@z.com│ u-103    │ infra        │                              │
└──────────────┴──────────┴──────────────┴──────────────────────────────┘
  Space toggle  •  a select all (filtered)  •  x clear  •  Enter confirm
```

### 2.6 Provider Error + Fallback

```
┌ Apps (apps list) ──────────────────────────────────────────────────────┐
│ ⚠ Unable to fetch apps (timeout).                                      │
│                                                                        |
│ You can: [R]etry  •  [F2] Enter manually  •  [C]ached (12s old)        │
└────────────────────────────────────────────────────────────────────────┘
```

### 2.7 Chained Providers (Add-ons depend on App)

```
┌ Inputs ────────────────────────────────────────────────────────────────┐
│ app         [apps list]    → selected: billing-svc                     │
│ addon       [addons list]  (args: app=billing-svc)                     │
└────────────────────────────────────────────────────────────────────────┘
```

### 2.8.1 Run View (Steps, Outputs, Global Logs)
* **Sticky headers**: the Steps and Outputs section headers and column headers remain fixed while the panel content scrolls.
* **Global logs forwarding**: execution logs stream into the shared logs view (`logs::LogsComponent`). The run view surfaces a status hint (`[L] View logs`) instead of embedding a dedicated logs panel.
* **Inline status controls**: the footer presents `[Cancel]` and `[Pause]/[Continue]` buttons. Pause toggles to Continue when the run is paused; Cancel becomes disabled once the run is terminal or a cancellation request is pending.
* **Status messaging**: the header/footer append contextual messages (for example `aborting...` or `paused after current step`) emitted by the workflow runner.
```
┌─ Workflow: provision_and_promote ───────────────────────────────────────────────────────────────┐
│ running • Elapsed: 00:01:42 • Logs forwarded ([L] View) • awaiting build confirmation           │
│                                                                                                 │
├──────────────────────────── Steps ──────────────────────────────────────────────────────────────┤
│ Step                 │ Status           │ Details                                               │
│──────────────────────┼────────────┼─────────────────────────────────────────────────────────────│
│ ● upload_source      │ succeeded (0.8s) │ bytes=14.2MB, sha=…                                   │
│ ● start_build        │ succeeded (0.5s) │ app=staging-app                                       │
│ ● poll_build (3/?)   │ running          │ status=pending                                        │
│ ○ create_release     │ pending    │                                                             │
│ ○ promote            │ pending    │                                                             │
│ ○ verify_release     │ pending    │                                                             │
│ ○ cleanup            │ pending    │                                                             │
│─────────────────────────────────────────────────────────────────────────────────────────────────│
│                                                                                                 │
├──────────────────────────── Outputs ────────────────────────────────────────────────────────────┤
│ Key                  │ Value                                                                    │
│──────────────────────┼──────────────────────────────────────────────────────────────────────────│
│ start_build.id       │ bld-9876                                                                 │
│ start_build.slug     │ slug-3333                                                                │
│ source_blob.get_url  │ https://sources.heroku.com/...                                           │
│─────────────────────────────────────────────────────────────────────────────────────────────────│
│ [Cancel]   [Pause]   Status: running • awaiting build confirmation                              │
└─────────────────────────────────────────────────────────────────────────────────────────────────┘
```
### 2.8.2 Run View — Wide Mode (side panel paging + sticky column headers)
```
┌─ Workflow: build_from_tarball ───────────────────────────────────────────────────────────────────┐
│ Status: running • Build: bld-9876 • Elapsed: 00:00:57 • Logs forwarded ([L] View) • [t] Toggle   │
│ layout • [q] Quit                                                                                │
├──────── Steps ────────────────┬───────────────────────────── Outputs ────────────────────────────┤
│ Step            │ Status │Dur │ [↑/↓] move  [Space] details  │ Key                  │ Value      │
│─────────────────┼────────┼────┤──────────────────────────────┼──────────────────────┼────────────│
│ ● create_sources│ ok     │1.2s│ [/] filter                   │ start_build.id       │ bld-9876   │
│ ● upload        │ ok     │0.8s│                              │ start_build.slug     │ slug-3333  │
│ ● start_build   │ ok     │0.5s│                              │ source_blob.get_url  │ https://…  │
│ ● poll_build    │ run    │12s │                              │ …                    │            │
│ ○ finalize      │ pend   │    │                              │                      │            │
│ …               │        │    │                              │                      │            │
│──────────────────────────────────────────────────────────────┼──────────────────────┼────────────│
│ [s] sort by Status                                           │ [y] copy • [Enter] expand         │
│                                                              │ [/] filter outputs                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 3) Widget Behaviors & Keybindings

* **Navigation**: `↑↓` move, `PgUp/PgDn` page, `Home/End` boundary.
* **Search/Filter**: `/` opens incremental filter; type to narrow rows (prefix match on `display_field` + fuzzy on others).
* **Sort**: `s` cycles sort by `display_field`, then `id_field` (stable).
* **Refresh**: `r` re-invokes provider; respects `cache_ttl_sec` but allows manual override.
* **Selection**: `Space` toggles; `Enter` confirms.
* **Fallback**: `F2` manual text entry; validates against schema if `validate.enum` present.
* **Load More**: `L` requests next page if provider supports pagination.

---

## 4) Provider Contracts (Runtime)

### 4.1 Normalized result shape

Providers return arrays of uniform objects. The host maps fields according to `select`:

```json
[
  { "name": "billing-svc", "id": "app-456", "meta": "team: infra" }
]
```

* `value = item[select.value_field]`
* `display = item[select.display_field]`
* `stable_id = item[select.id_field]` (optional but recommended)

### 4.2 Caching

* Default TTL (e.g., 30s) if `cache_ttl_sec` not specified.
* Cache key: `(provider_id, provider_args, partial_filter)`.
* Workflow-run-scoped cache for outputs.

### 4.3 Error handling

* If provider fails: follow `on_error` policy — `manual`, `fail`, or `cached`.
* If `cached` and cache exists: present age badge (e.g., "loaded 24s ago").

### 4.4 Chaining

* `provider_args` can reference earlier inputs or step outputs using the same template language used in steps.
* `depends_on` offers shorthand bindings for provider arguments that mirror `provider_args`; values are merged at runtime so selectors automatically reuse previously chosen inputs.

---

## 5) Authoring Patterns & Examples

### 5.1 Minimal single-select input

```yaml
inputs:
  app:
    provider: apps list
    select: { value_field: name, display_field: name, id_field: id }
```

### 5.2 Multi-select with join

```yaml
inputs:
  reviewers:
    provider: users list
    select: { value_field: email, display_field: email, id_field: id }
    mode: multiple
    join: { separator: "," }
```

### 5.3 Chained inputs

```yaml
inputs:
  app:
    provider: apps list
    select: { value_field: name, display_field: name, id_field: id }
  addon:
    provider: addons list
    provider_args: { app: ${{ inputs.app }} }
    select: { value_field: name, display_field: name, id_field: id }
```

### 5.4 Provider with defaults & fallback

```yaml
inputs:
  pipeline:
    provider: pipelines list
    select: { value_field: name, display_field: name, id_field: id }
    default: { from: history }
    cache_ttl_sec: 45
    on_error: cached
```

---

## 6) Extending Existing Widgets

* **Table**: add selection state (☑/☐), sticky header, column resize, and a right-side detail pane toggle (`d`).
* **Key/Value Viewer**: add selectable rows; `Space` toggles active item; `Enter` confirms; supports copy-to-clipboard for value.
* **Status line**: shows provider id, cache age, and pagination state.
* **Progressive Disclosure** :For first-time users, the Workflow Picker + Input Collection views may feel dense. Consider collapsing advanced provider details behind an “info” key (e.g., i for inline docs, which is already specified for popovers).
* **Long Scroll Runs**: Complex workflows (like pipeline_bootstrap) could produce 10+ inputs and steps. Pagination and sticky headers in the Run View will be crucial for usability.
* **Error Recovery**: The fallback UX (manual entry, cached, retry) is sound, but the visual hierarchy of these actions should emphasize the recommended next step (perhaps color-coded: [R] green, [F2] gray, [C] dimmed).
---

## 7) Accessibility & Internationalization

* All actions must be keyboard-first.
* Announce async state changes (loading, error, loaded) via ARIA-like cues (for terminal, use concise status lines).
* Support wide glyphs and right-to-left text in table cells.

---

## 8) Telemetry (Optional)

* Record provider latency, cache hit ratio, and fallback rate (no PII; use `id_field` only).
* Emit anonymized counts of `manual vs provider` selections for UX tuning.

---

## 9) Open Questions

1. Should `select.display_field` support templating (e.g., "\${name} — \${meta}")?
2. Provider pagination contract: standardize on `{ items, next_cursor }`?
3. Global registry vs. per-command declaration for providers?
4. Should we allow per-input min-width / column list overrides for Table widget?
