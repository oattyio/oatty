# WORKFLOWS — Comprehensive Specification

This document defines reusable workflow patterns for the Heroku CLI TUI, based on the available commands in the current `manifest.json` and enhanced with **ValueProviders** for dynamic, schema‑aware parameter collection.citeturn1file028†source

---

## 1. Principles

* **Declarative first**: Workflows are YAML files that declare tasks in order.
* **ValueProviders**: Inputs can be sourced from:
  * **Static enums** (from schema)
  * **Dynamic built‑ins** (`apps:list`, `addons:list`, `pipelines:list`, `teams:list`, etc.)
  * **Workflow outputs** (`${{tasks.<id>.output.<field>}}`)
  * **Plugin providers** (via MCP)
* **Resilient**: Providers support caching, async refresh, and fallbacks.
* **Composable**: Outputs from one task can feed into later tasks.
* **User-friendly**: The TUI surfaces provider‑backed inputs with searchable dropdowns and details.

---

## 2. Workflow Examples

### 2.1 Create App with Postgres

```yaml
workflow: app_with_db
inputs:
  app_name:
    description: "Name for new app"
    type: string
  region:
    provider: regions list
    select: { value_field: name, display_field: name }
    default: { value: "us" }
  addon_plan:
    description: "Database plan"
    provider: addons services:list
    select: { value_field: name, display_field: name }
steps:
  - id: create_app
    run: apps create
    body:
      name: ${{ inputs.app_name }}
      region: ${{ inputs.region }}

  - id: add_pg
    run: apps addons:create
    with: { app: ${{ inputs.app_name }} }
    body:
      plan: ${{ inputs.addon_plan }}
```

---

### 2.2 Safe Build & Deploy from Tarball

```yaml
workflow: build_from_tarball
inputs:
  app:
    provider: apps list
    select: { value_field: name, display_field: name, id_field: id }
  tar_path:
    description: "Path to tarball"
    type: file
steps:
  - id: create_sources
    run: sources create

  - id: upload
    run: shell curl_put
    with:
      url: ${{ steps.create_sources.output.source_blob.put_url }}
      file: ${{ inputs.tar_path }}

  - id: build
    run: apps builds:create
    with:
      app: ${{ inputs.app }}
    body:
      source_blob:
        url: ${{ steps.create_sources.output.source_blob.get_url }}

  - id: wait_for_build
    run: apps builds:info
    with:
      app: ${{ inputs.app }}
      build: ${{ steps.build.output.id }}
    repeat:
      until: ${{ ["succeeded","failed"].includes(step.output.status) }}
      every: 10s
```

---

### 2.3 Pipeline Bootstrap

```yaml
workflow: pipeline_bootstrap
inputs:
  pipeline_name:
    description: "Name for pipeline"
    type: string
  team:
    provider: teams list
    select: { value_field: id, display_field: name }
  dev_app:
    provider: apps list
    select: { value_field: name, display_field: name }
  staging_app:
    provider: apps list
    select: { value_field: name, display_field: name }
  prod_app:
    provider: apps list
    select: { value_field: name, display_field: name }
steps:
  - id: create_pipeline
    run: pipelines create
    body:
      name: ${{ inputs.pipeline_name }}
      owner: { id: ${{ inputs.team }} }

  - id: couple_dev
    run: pipeline-couplings create
    body:
      app: ${{ inputs.dev_app }}
      pipeline: ${{ steps.create_pipeline.output.id }}
      stage: development

  - id: couple_staging
    run: pipeline-couplings create
    body:
      app: ${{ inputs.staging_app }}
      pipeline: ${{ steps.create_pipeline.output.id }}
      stage: staging

  - id: couple_prod
    run: pipeline-couplings create
    body:
      app: ${{ inputs.prod_app }}
      pipeline: ${{ steps.create_pipeline.output.id }}
      stage: production
```

---

### 2.4 Collaborator Lifecycle

```yaml
workflow: collaborator_lifecycle
inputs:
  app:
    provider: apps list
    select: { value_field: name, display_field: name }
  user:
    description: "Email or user ID"
    type: string
  permissions:
    description: "Permissions for collaborator"
    type: array
    enum: [view, deploy, operate, manage]
steps:
  - id: add_collab
    run: teams apps:collaborators:create
    with: { app: ${{ inputs.app }} }
    body:
      user: ${{ inputs.user }}
      permissions: ${{ inputs.permissions }}
      silent: false

  - id: confirm
    run: teams apps:collaborators:info
    with:
      app: ${{ inputs.app }}
      collaborator: ${{ inputs.user }}
```

---

### 2.5 Telemetry Drain Setup for a Space

```yaml
workflow: space_with_otel
inputs:
  team:
    provider: teams list
    select: { value_field: id, display_field: name }
  space_name:
    description: "Name of new space"
    type: string
  region:
    provider: regions list
    select: { value_field: name, display_field: name }
  otlp_endpoint:
    description: "OpenTelemetry collector endpoint URL"
    type: string
steps:
  - id: create_space
    run: spaces create
    body:
      team: ${{ inputs.team }}
      name: ${{ inputs.space_name }}
      region: ${{ inputs.region }}

  - id: add_drain
    run: telemetry-drains create
    body:
      owner: { space: { name: ${{ inputs.space_name }} } }
      signals: ["traces", "metrics", "logs"]
      exporter:
        type: otlp_http
        endpoint: ${{ inputs.otlp_endpoint }}
```

---

## 3. Open Gaps (not in manifest.json)

Based on the earlier workflow ideation, these common needs are **not covered by current commands**:

* **Dyno/process management**: scale up/down, restart, resize
* **Releases**: create new release, rollback, set description, list previous releases
* **Review Apps pipeline config**: enable auto‑create/destroy, manage settings
* **Database lifecycle**: backups, restores, followers, maintenance
* **Classic log drains**: create/delete/tail beyond `telemetry-drains`

These would be candidates for adding new schema entries or plugin‑provided ValueProviders.

---

## 4. TUI Integration with ValueProviders

See [Workflow ValueProviders & TUI UX Spec (Heroku CLI TUI)](./Workflow%20ValueProviders%20%26%20TUI%20UX%20Spec%20%28heroku%20Cli%20Tui%29) for full widget examples.

Key points:

* Provider‑backed inputs declare `provider` and `select` fields for dynamic values.
* Guided mode shows provider results in **Table selectors** with optional detail panes.
* Power mode uses providers for **autocomplete suggestions**.
* Fallbacks and cache status are visible inline (with icons/labels).

---

✅ This rewrite gives you a **complete, provider‑aware `WORKFLOWS.md`** and ties it directly to the manifest commands. Next, we could extend this by **mapping every `manifest.json` command to at least one reference workflow** so authors have a full cookbook of ready‑to‑use recipes. Would you like me to generate that full mapping?
