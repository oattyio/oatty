# Workflows Authoring (As-Built)

## Scope
This document describes the implemented workflow manifest model used by registry, engine, TUI, and MCP tooling.

## Canonical Schema Source
- `crates/types/src/workflow.rs`

The schema is serialized/deserialized from YAML or JSON.

## Top-Level Fields
- `workflow` (required identifier)
- `title` (optional)
- `description` (optional)
- `inputs` (`IndexMap<String, WorkflowInputDefinition>`; order-preserving)
- `steps` (`Vec<WorkflowStepDefinition>`; required non-empty)

## Input Definition (Implemented)
Supported fields include:
- metadata: `description`, `name`, `type`, `placeholder`
- provider config: `provider`, `select`, `provider_args`, `depends_on`
- selection behavior: `mode`, `join`
- defaults: `default` (`history`, `literal`, `env`, `workflow_output`)
- validation: `validate` (`required`, enum, regex, length)
- behavior: `optional`, `cache_ttl_sec`, `on_error`, `enum`

## Step Definition (Implemented)
Each step supports:
- `id`, `run`
- `description`
- `depends_on`
- `if` / `when`
- `with` map
- `body`
- `repeat` (`until`, `every`, `timeout`, `max_attempts`)
- `output_contract`

## Runtime Loading
- Workflows are loaded from filesystem at runtime.
- Registry loader reads recursive `yaml`/`yml`/`json` files from the runtime workflows directory.
- Runtime workflows are normalized and validated into `RuntimeWorkflow` before execution.

## Provider Dependency Validation (Hard Rule)
- For provider-backed inputs (`provider` set), any `provider_args.<arg>` value that references upstream workflow context must have a matching `depends_on.<arg>` binding.
- Upstream references include:
  - structured bindings with `from_input` or `from_step`
  - literal templates containing `${{ inputs.* }}` or `${{ steps.* }}`
- Missing/mismatched `depends_on` for those arguments is a validation error during runtime normalization.
- This rule prevents provider execution without explicitly declared upstream dependencies, reducing collector-time confusion.

## Execution Notes
- Workflow execution is driven by engine workflow runtime/runner modules.
- Input defaults are applied prior to execution.
- Step ordering is dependency-aware.
- Conditions and interpolation are evaluated against run context.

## Source Alignment
- `crates/types/src/workflow.rs`
- `crates/registry/src/workflows.rs`
- `crates/engine/src/workflow/document.rs`
- `crates/engine/src/workflow/runtime.rs`
- `crates/engine/src/workflow/runner.rs`


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/WORKFLOW_TUI.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/MCP_WORKFLOWS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/VALUE_PROVIDERS.md`
