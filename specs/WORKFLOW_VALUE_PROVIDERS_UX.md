# Workflow ValueProviders UX — TUI Design

This document outlines how ValueProviders should be expressed in workflow YAML definitions **and** how they should surface in the TUI. The goal is to make workflows both declarative and highly usable.

---

## 1. Workflow YAML Extensions for ValueProviders

### Basic Provider Declaration
```yaml
inputs:
  app:
    provider:
      id: apps:list
      field: name   # targeted field in the JSON array
    description: "Select an app to operate on"
```

### Multiple Values Example
```yaml
inputs:
  target_apps:
    provider:
      id: apps:list
      field: name
    multiple: true
    description: "Choose one or more target apps"
```

### Workflow Output as Provider
```yaml
steps:
  - id: create_app
    run: apps:create
    with:
      name: demo-${{ uuid() }}

  - id: configure
    run: apps:config-vars:update
    with:
      app:
        provider:
          id: workflow
          from: tasks.create_app.output.name
```

---

## 2. TUI Interaction Model

### a) Input Prompt with ValueProvider
- When a workflow is executed in **Guided Mode**, each input powered by a provider renders as a **dropdown list** or **multi-select**.
- If provider fetch fails, the field falls back to **manual entry** with an inline warning.

### Annotated ASCII: Dropdown (Single-Select)
```
┌───────────────────────────────────────────────┐
│ Select an app to operate on                   │
│                                               │
│   → demo-app         (owner: justin@example)  │
│     billing-svc      (team: infra)            │
│     customer-api     (team: payments)         │
│                                               │
│   [↑/↓ to navigate, Enter to select]          │
└───────────────────────────────────────────────┘
```

### Annotated ASCII: Multi-Select
```
┌───────────────────────────────────────────────┐
│ Choose one or more target apps                │
│                                               │
│ [x] demo-app         (owner: justin@example)  │
│ [ ] billing-svc      (team: infra)            │
│ [ ] customer-api     (team: payments)         │
│                                               │
│   [Space to toggle, Enter to confirm]         │
└───────────────────────────────────────────────┘
```

### b) Workflow Execution View
Each step in the workflow is shown as a row in the **workflow table** with status icons.

```
┌───────────────────────────────────────────────┐
│ Workflow: provision_and_promote               │
├───────────────┬─────────────┬─────────────────┤
│ Step          │ Status      │ Details         │
├───────────────┼─────────────┼─────────────────┤
│ create_build  │ ● Running   │ app=staging-app │
│ promote       │ ○ Pending   │                 │
└───────────────┴─────────────┴─────────────────┘
```

Legend:
- `● Running`
- `✔ Succeeded`
- `✖ Failed`
- `○ Pending`

### c) Key/Value Viewer Integration
Step outputs can be inspected using the **key/value viewer**, with selection support for downstream steps.

```
┌───────────────────────────────────────────────┐
│ Output: create_app                            │
├───────────────┬───────────────────────────────┤
│ name          │ demo-1234                     │
│ id            │ app-987                       │
│ owner         │ justin@example.com            │
└───────────────┴───────────────────────────────┘
```

If a later input is bound to `${{tasks.create_app.output.name}}`, the TUI shows it as **pre-filled** with a badge:
```
App: demo-1234   [from task: create_app]
```

---

## 3. UX Notes

- **Async Providers**: while fetching, show spinner rows (`…loading`).
- **Provider metadata**: display inline (owner, team, region) dimmed.
- **Fallback**: manual text input always available.
- **Multi-value support**: toggle via `[ ]` checkboxes.
- **Transparency**: workflow-sourced values visibly badged.

---

## 4. Next Steps

- Implement provider YAML contract with `id` + `field`.
- Extend existing table & key/value viewers with select/toggle.
- Add Guided Mode renderer for dropdowns/multi-selects.
- Design error-handling views for provider failures.

