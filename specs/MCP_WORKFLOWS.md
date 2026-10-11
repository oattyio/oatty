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

Implemented workflow tool surface (9 tools):
- `workflow_get`
- `workflow_validate`
- `workflow_save`
- `workflow_export`
- `workflow_rename`
- `workflow_delete`
- `workflow_preview`
- `workflow_resolve_inputs`
- `workflow_run`

All workflow tools emit structured MCP errors with stable error codes, actionable `next_step` guidance, and contextual details. Workflow inventory is served via the `oatty://workflow/manifests` resource rather than a dedicated `workflow_list` tool.

### Administrative tools (`workflow_admin_tools` feature)

`workflow_purge_history` is gated behind the `workflow_admin_tools` server feature flag.
The feature is **disabled by default** so the tool does not consume MCP client context.
When the feature is enabled, `workflow_purge_history` is registered as a destructive tool
that requires `workflow_id` or `input_keys` as a filter.

### Migration table for clients using removed tools

| Removed tool | Replacement approach |
| --- | --- |
| `workflow_list` | Read `oatty://workflow/manifests` resource to list manifest identifiers. |
| `workflow_import` | Use `workflow_save` with `input_path` pointing to the source file. |
| `workflow_preview_inputs` | Use `workflow_preview` with `view: inputs`. |
| `workflow_step_plan` | Use `workflow_preview` with `view: plan`. |
| `workflow_preview_rendered` | Use `workflow_preview` with `view: rendered`. |
| `workflow_cancel` | Use the standard MCP `tasks/cancel` method with the operation identifier. |
| `workflow_author_and_run` | Compose `workflow_validate` → `workflow_save` → `workflow_resolve_inputs` → `workflow_run`. |
| `workflow_repair_and_rerun` | Repair the manifest through client-side logic, then use the validate/save/run sequence. |
| `workflow_purge_history` | Gated behind `workflow_admin_tools` feature (disabled by default); when enabled, call `workflow_purge_history` directly. |

## Command discovery metadata for authoring

- Workflow authoring flows rely on MCP command discovery via `search_commands`.
- Recommended authoring sequence is:
  1. `search_commands` for discovery.
  2. `get_command` for exact single-command argument/flag/schema inspection.
  3. Workflow drafting/validation once command IDs and schemas are confirmed.
- `search_commands` supports `include_inputs` for metadata enrichment:
  - `required_only`: includes required input metadata plus compact `output_fields`.
  - `full`: includes positional/flag metadata, `output_schema`, and compact `output_fields`.
- `output_fields`/`output_schema` are intended to help map upstream step outputs into downstream provider bindings and step inputs.

### Typed `run_*` inputs for command execution

- MCP `run_safe_command`, `run_command`, and `run_destructive_command` accept `named_flags` values as typed JSON:
  - scalar values (string/number/boolean)
  - arrays
  - objects
- This enables direct command execution for APIs that require structured payload fields.
- Example:

```json
{
  "canonical_id": "vercel projects:env:create",
  "positional_args": ["my-project"],
  "named_flags": [
    ["upsert", "true"],
    ["key", "DATABASE_URL"],
    ["value", "postgres://..."],
    ["type", "encrypted"],
    ["target", ["production", "preview", "development"]]
  ]
}
```

## Workflow authoring policy (LLM-facing)

- Provider-first for enumerable/list-selection fields:
  - Use provider-backed inputs when values are discoverable and bounded (for example `owner_id`, `project_id`, `service_id`, `domain`, `env_group`).
- Hybrid/manual policy for transformation-heavy fields:
  - Keep manual inputs where user intent and cross-system mapping are required (for example build/runtime/service detail transformations).
- Preflight requirement before full manifest drafting:
  - Confirm required catalogs exist and are enabled.
  - Confirm required HTTP commands are discoverable.
  - If either check fails, run OpenAPI validation/import flow before continuing workflow authoring.

## Input resolution and readiness semantics

- `workflow_resolve_inputs` applies defaults, evaluates provider bindings, validates values, and returns readiness metadata.
- Readiness includes required input completeness and provider outcome status.
- `workflow_run` rejects unresolved provider prompt/error situations with structured errors.

## Manifest validation semantics

- `workflow_validate` accepts exactly one source: `workflow_id`, `manifest_content`, or `input_path`.
- `workflow_validate` / `workflow_save` enforce a hard provider dependency rule from runtime normalization:
  - For provider-backed inputs, any upstream-referencing `provider_args.<arg>` must declare a matching `depends_on.<arg>`.
  - Upstream-referencing means:
    - binding form with `from_input` or `from_step`
    - literal template form with `${{ inputs.* }}` or `${{ steps.* }}`
- Missing or invalid `depends_on` mappings are returned as validation failures.
- Validation preflight also checks `requires.catalogs[]` against loaded registry catalogs.
  - Missing catalog requirements are emitted as structured violations at `$.requires.catalogs[index]`.
  - Violations include actionable install guidance and preserve requirement metadata (`vendor`, `title`, `source`, `source_type`).
- `workflow_validate` honors an explicit `format` parameter for all sources including `input_path`, falling back to path-derived inference when omitted. This matches `workflow_save` behavior.

## Execution behavior

- `workflow_run` currently executes synchronously in tool implementation. Step results and aggregated outputs are opt-in.
- Responses include the run identifier, workflow identifier, status, and supplied/resolved inputs; result and output payloads remain opt-in.
- Task cancellation uses the standard MCP task protocol (`tasks/cancel`), not a dedicated workflow tool.
- `workflow_run` accepts `format` as an explicit override for inline manifests, consistent with `workflow_validate`.

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
