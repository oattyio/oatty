heroku-engine — Workflows (Preview / Dry-run)

Overview
- Lightweight workflow engine for orchestrating sequences of Heroku CLI commands.
- Focused on deterministic “dry-run” planning; execution is intentionally minimal (CLI handles live execution).

Data Model
- `WorkflowFile { workflows: HashMap<String, Workflow> }`
- `Workflow { tasks: Vec<Task> }`
- `Task { name: String, command: String, with: serde_json::Value, if: Option<String> }`
- `ContextState { env: HashMap<String, String>, tasks: HashMap<String, TaskResult> }`
- `TaskResult { status: String, output: Value, logs: Vec<String> }`

Capabilities
- Load workflows from YAML/JSON files (`load_workflow_from_file`).
- Interpolate values: `${{ env.VAR }}`, `${{ tasks.<name>.output.<path> }}` (recursive in strings/arrays/objects).
- Conditions: simple equality in `if` expressions (e.g., `a == "b"`).
- Dry-run planner: `dry_run_plan(workflow, registry)`
  - Validates commands against the registry.
  - Resolves path placeholders from positional names; builds JSON body from non-positional fields.
  - Emits a plan per task: `{method, url, headers, body}`; marks tasks as skipped when condition is false.
  - No network calls; deterministic and side-effect free.

CLI Integration
- When `FEATURE_WORKFLOWS=1`:
  - `heroku workflow list`
  - `heroku workflow preview --file <path> [--name <workflow>]`
  - `heroku workflow run --file <path> [--name <workflow>] [--dry-run]`
  - For `--dry-run`, prints the JSON plan to stdout.

Usage
```bash
FEATURE_WORKFLOWS=1 cargo run -p heroku-cli -- workflow preview --file workflows/example.yaml
```

Future Work
- Execution with progress reporting and per-task logs.
- Richer expressions and branching.
- Provider-backed value resolution at plan-time.

