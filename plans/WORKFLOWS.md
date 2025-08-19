# Heroku CLI — Workflow System

## Overview
Workflows allow chaining multiple API calls with:
- Named tasks.
- Variable substitution from previous results.
- Simple `if` conditionals.
- Schema-based command validation.

---

## Example (YAML)
```yaml
workflows:
  deploy_app:
    tasks:
      - name: create_app
        command: apps:create
        with:
          name: my-app
      - name: release
        command: releases:create
        with:
          app: ${{tasks.create_app.output.name}}
      - name: notify
        if: ${{tasks.release.output.status == "succeeded"}}
        command: notifications:send
        with:
          message: "App deployed successfully!"
```

---

## Execution Model
- Each task:
  - Runs a command.
  - Captures output as JSON.
  - Makes values available for substitution.
- Supports simple conditionals (`if`).

---

## TUI Integration
- **Unified UI**: workflows shown like commands.
- **Expanded steps view**:
  - Preview (dry-run).
  - Show data dependencies.
  - User may select/deselect tasks.
- **Run view**:
  - Shows live progress (✓/✗).
  - Captures logs per step.

---

## Benefits
- Declarative, repeatable automation.
- Schema ensures valid inputs/outputs.
- Natural fit for both new and expert users.
