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
    provider: apps:list                # built-in dynamic provider id
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
    provider: addons:list
    provider_args:
      app: ${ { inputs.app } }         # provider params can reference other inputs
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
    run: apps:config-vars:update
    with:
      app: ${ { inputs.app } }
    body:
      DB_ADDON: ${ { inputs.addon } }
```

### 1.3 Advanced: multiple selection & join

```yaml
inputs:
  collaborators:
    description: "Grant access to users"
    provider: users:list
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
    provider: apps:list
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
    provider: apps:list
    default:
      from: workflow_output
      value: ${ { tasks.create_app.output.name } }
```

### 1.6 Declarative validation

```yaml
inputs:
  region:
    provider: regions:enum
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
    provider: apps:list
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
┌─ Run: provision_and_promote ───────────────────────────────────────────┐
│ Required Inputs                                                        │
│                                                                        │
│ ▸ pipeline  [provider: pipelines:list]                                 │
│   ┌ Apps/Pipelines (loading… ⠋) ────────────────────────────────────┐  │
│   │ name                id             owner                        │  │
│   │ … (cached/async)                                                │  │
│   └─────────────────────────────────────────────────────────────────┘  │
│                                                                        │
│ ▸ staging_app  [provider: apps:list]                                   │
│   ┌ Apps (loaded 24s ago) ──────────────────────────────────────────┐  │
│   │ ▸ billing-svc        app-456       team: infra                  │  │
│   │   demo-app           app-123       owner: justin@example.com    │  │
│   └─────────────────────────────────────────────────────────────────┘  │
│                                                                        │
│ ▸ prod_app  [provider: apps:list]                                      │
│   (select from list or type to override)                               │
└────────────────────────────────────────────────────────────────────────┘
  ↑↓ navigate  •  Enter choose  •  / filter  •  r refresh  •  Tab next  •  F2 fallback to manual
```

### 2.3 Table Selector with Detail Pane (Single-select)

```
┌ Apps — Select one (apps:list) ────────────────────────────────────────┐
│ Filter: [bill]   Status: loaded (30s TTL)                             │
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
  ↑↓ move  •  Space select  •  Enter confirm  •  / search  •  s sort  •  r refresh

**Schema-aware badges**

- Display the JSON type using the enriched `SchemaProperty` metadata (`object`, `array<uuid>`,
  `enum`, etc.).
- Surface tags (for example, `app_id`, `pipeline_slug`) next to each candidate to clarify why it
  matches a provider requirement.
- When `enum_values` exists, render the literal set in the detail pane to aid manual overrides.
- Respect `required` keys by flagging missing fields before confirm.
```

### 2.4 Table Selector (Multi-select) with Chip Summary

```
┌ Users — Select multiple (users:list) ─────────────────────────────────┐
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

### 2.5 Provider Error + Fallback

```
┌ Apps (apps:list) ──────────────────────────────────────────────────────┐
│ ⚠ Unable to fetch apps (timeout).                                      │
│                                                                        |   
│ You can: [R]etry  •  [F2] Enter manually  •  [C]ached (12s old)        │
└────────────────────────────────────────────────────────────────────────┘
```

### 2.6 Chained Providers (Add-ons depend on App)

```
┌ Inputs ────────────────────────────────────────────────────────────────┐
│ app         [apps:list]    → selected: billing-svc                     │
│ addon       [addons:list]  (args: app=billing-svc)                     │
└────────────────────────────────────────────────────────────────────────┘
```

### 2.7.1 Run View (Steps, Logs, Outputs)
* **Sticky headers**: the Steps, Logs, and Outputs section headers and column headers remain fixed while the panel content scrolls.
* **Independent pagination**: each panel has its own paging controls and indicators.
```
┌─ Workflow: provision_and_promote ───────────────────────────────────────────────────────────────┐
│ Status: running • Started: 10:12:03 • Elapsed: 00:01:42 • [F1] Help • [q] Quit                  │
├──────────────────────────── Steps (Page 2/5) ───────────────────────────┬─ Logs (Page 1/8) ─────┤
│ Step                 │ Status     │ Details                            │ [↑/↓] scroll  [PgUp]   │
│──────────────────────┼────────────┼────────────────────────────────────│ [PgDn] page  [g/G]     │
│ ● upload_source      │ ok 0.8s    │ bytes=14.2MB, sha=…                │ top/bottom   [L] last  │
│ ● start_build        │ ok 0.5s    │ app=staging-app                    │ page         [r] foll  │
│ ● poll_build (3/?)   │ running    │ status=pending                     │ ow tail      [f] filt  │
│ ○ create_release     │ pending    │                                    │ er            [c] clr  │
│ ○ promote            │ pending    │                                    │                  log   │
│ ○ verify_release     │ pending    │                                    │ s                      │
│ ○ cleanup            │ pending    │                                    │                        │
│ … (11 items hidden)  │            │                                    │                        │
│────────────────────────────────────────────────────────────────────────│────────────────────────│
│ [First] [Prev]  Page 2/5  [Next] [Last]   •   [/] filter steps   •   [s] sort                   │
├──────────────────────────── Outputs (Page 1/3) ─────────────────────────────────────────────────┤
│ Key                  │ Value                                                                    │
│──────────────────────┼──────────────────────────────────────────────────────────────────────────│
│ start_build.id       │ bld-9876                                                                 │
│ start_build.slug     │ slug-3333                                                                │
│ source_blob.get_url  │ https://sources.heroku.com/...                                           │
│ … (more)                                                                                        │
│─────────────────────────────────────────────────────────────────────────────────────────────────│
│ [First] [Prev]  Page 1/3  [Next] [Last]   •   [y] copy value   •   [Enter] expand details       │
└─────────────────────────────────────────────────────────────────────────────────────────────────┘
```
### 2.7.2 Run View — Wide Mode (side panel paging + sticky column headers)
```
┌─ Workflow: build_from_tarball ──────────────────────────────────────────────────────────────────┐
│ Status: running • Build: bld-9876 • Elapsed: 00:00:57 • [t] Toggle layout • [q] Quit            │
├──────── Steps (Pg 1/4) ────────┬──────────────────────────────── Logs (Pg 3/12) ────────────────┤
│ Step            │ Status │Dur │ [↑/↓] move  [Space] details  │ [PgUp/PgDn] page  [End] tail     │
│─────────────────┼────────┼────┤──────────────────────────────┼──────────────────────────────────│
│ ● create_sources│ ok     │1.2s│ [f] filter                   │ [poll_build] status=pending …    │
│ ● upload        │ ok     │0.8s│                              │ [poll_build] status=pending …    │
│ ● start_build   │ ok     │0.5s│                              │ [poll_build] status=running …    │
│ ● poll_build    │ run    │12s │                              │ [build] slug=slug-3333           │
│ ○ finalize      │ pend   │    │                              │ …                                │
│ …               │        │    │                              │                                  │
│──────────────────────────────────────────────────────────────┼──────────────────────────────────│
│ [First] [Prev] Page 1/4 [Next] [Last]  • [s] sort by Status  │ [First] [Prev] Page 3/12 [Next]  │
│                                                         [Last]                                  │
├────────────────────────── Outputs (Pg 1/2, sticky head) ────────────────────────────────────────┤
│ Key                │ Value                                                                      │
│────────────────────┼────────────────────────────────────────────────────────────────────────────│
│ start_build.id     │ bld-9876                                                                   │
│ start_build.slug   │ slug-3333                                                                  │
│ source_blob.get_url│ https://sources.heroku.com/...                                             │
│ …                                                                                            ▼  │
│─────────────────────────────────────────────────────────────────────────────────────────────────│
│ [First] [Prev] Page 1/2 [Next] [Last]  • [y] copy  • [Enter] expand  • [/] filter outputs       │
└─────────────────────────────────────────────────────────────────────────────────────────────────┘
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

* `provider_args` can reference earlier inputs or task outputs using the same template language used in steps.

---

## 5) Authoring Patterns & Examples

### 5.1 Minimal single-select input

```yaml
inputs:
  app:
    provider: apps:list
    select: { value_field: name, display_field: name, id_field: id }
```

### 5.2 Multi-select with join

```yaml
inputs:
  reviewers:
    provider: users:list
    select: { value_field: email, display_field: email, id_field: id }
    mode: multiple
    join: { separator: "," }
```

### 5.3 Chained inputs

```yaml
inputs:
  app:
    provider: apps:list
    select: { value_field: name, display_field: name, id_field: id }
  addon:
    provider: addons:list
    provider_args: { app: ${ { inputs.app } } }
    select: { value_field: name, display_field: name, id_field: id }
```

### 5.4 Provider with defaults & fallback

```yaml
inputs:
  pipeline:
    provider: pipelines:list
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
