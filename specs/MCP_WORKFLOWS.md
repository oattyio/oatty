# MCP_WORKFLOWS.md

As-built specification for workflow resources, prompts, and tools exposed by the MCP server.

## Scope

Primary implementation files:
- `/Users/justinwilaby/Development/next-gen-cli/crates/mcp/src/server/core.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/mcp/src/server/workflow/resources.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/mcp/src/server/workflow/prompts.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/mcp/src/server/workflow/tools/*`
- `/Users/justinwilaby/Development/next-gen-cli/crates/mcp/src/server/workflow/services/*`

## Runtime storage model

Workflow manifests are filesystem-backed at runtime via workflow storage services.
Operations (`list/get/save/rename/delete/import/export`) read/write this runtime directory and then synchronize registry workflow state and synthetic workflow commands.

## Exposed workflow resources

Implemented resource URIs include:
- `oatty://workflow/spec`
- `oatty://workflow/tui-spec`
- `oatty://workflow/schema`
- `oatty://workflow/manifests`
- `oatty://workflow/provider-catalog`
- `oatty://workflow/command-catalog`
- `oatty://workflow/manifest/{workflow_id}`

Resource handlers return deterministic read-only payloads or structured MCP errors.

## Exposed workflow prompts

Implemented prompt names include:
- `workflow.author`
- `workflow.extend`
- `workflow.fix_validation`
- `workflow.run_with_inputs`

These are served as prompt templates and argument-validated in prompt handlers.

## Exposed workflow tools

Implemented workflow tool surface includes:
- `workflow.list`
- `workflow.get`
- `workflow.validate`
- `workflow.save`
- `workflow.export`
- `workflow.import`
- `workflow.rename`
- `workflow.delete`
- `workflow.preview_inputs`
- `workflow.resolve_inputs`
- `workflow.run`
- `workflow.step_plan`
- `workflow.preview_rendered`
- `workflow.cancel`
- `workflow.purge_history`
- `workflow.author_and_run`
- `workflow.repair_and_rerun`

## Input resolution and readiness semantics

- `workflow.resolve_inputs` applies defaults, evaluates provider bindings, validates values, and returns readiness metadata.
- Readiness includes required input completeness and provider outcome status.
- `workflow.run` rejects unresolved provider prompt/error situations with structured errors.

## Manifest validation semantics

- `workflow.validate` / `workflow.save` enforce a hard provider dependency rule from runtime normalization:
  - For provider-backed inputs, any upstream-referencing `provider_args.<arg>` must declare a matching `depends_on.<arg>`.
  - Upstream-referencing means:
    - binding form with `from_input` or `from_step`
    - literal template form with `${{ inputs.* }}` or `${{ steps.* }}`
- Missing or invalid `depends_on` mappings are returned as validation failures.

## Execution behavior

- `workflow.run` currently executes synchronously in tool implementation and returns run results/outputs.
- Response includes execution mode metadata and task-mode recommendation flags.
- Task-capability path is supported through MCP operation processor integration (`workflow.cancel` targets operation IDs).

## Error contract

Workflow tools emit structured MCP error data with:
- stable error codes
- actionable `next_step` guidance
- contextual details (workflow id, path, version, etc.)
- validation violations where applicable (including `violations[]` in schema/parse failures)

## Correctness notes

- This file is as-built. Keep it synchronized with tool registration/descriptions in `core.rs` and handlers under `server/workflow/tools`.
- Planned async/task execution changes must be recorded only after implementation changes land.


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/WORKFLOWS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/WORKFLOW_TUI.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/VALUE_PROVIDERS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/MCP_CATALOG_TOOLS.md`
