Overview
- Lightweight workflow engine for orchestrating sequences of Heroku CLI commands.

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

CLI Integration
- When `FEATURE_WORKFLOWS=1`:
  - `heroku workflow list`
  - `heroku workflow preview --file <path> [--name <workflow>]`
  - `heroku workflow run --file <path> [--name <workflow>]`

Usage
```bash
FEATURE_WORKFLOWS=1 cargo run -p heroku-cli -- workflow preview --file workflows/example.yaml
```

Future Work
- Execution with progress reporting and per-task logs.
- Richer expressions and branching.
- Provider-backed value resolution at plan-time.

