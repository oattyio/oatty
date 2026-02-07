# MCP Workflow Authoring & Execution Design

## 1. Purpose

Define the MCP surface (resources, prompts, tools, and tasks) required for an LLM to author, validate,
save, and execute workflows on a user's behalf. This document maps directly to the workflow authoring
specification in `specs/WORKFLOWS.md` and the runtime engine in `crates/engine/src/workflow/`.

Goals:
- Provide a deterministic, auditable interface for workflow authoring and execution.
- Allow an LLM to safely discover, create, and manage workflow manifests.
- Support long-running execution with incremental status and output capture.
- Preserve user control by separating safe, standard, and destructive operations.

Non-goals:
- Define new workflow schema fields (see `specs/WORKFLOWS.md`).
- Replace existing CLI and TUI behavior.

---

## 2. Core Principles

1. **Schema-first authoring**: Every workflow manifest must validate against the schema defined in
   `crates/types/src/workflow.rs` and documented in `specs/WORKFLOWS.md`.
2. **Deterministic context**: Use MCP resources for schema, specs, and manifests to avoid
   tool-driven side effects when reading data.
3. **Explicit safety boundaries**: Separate read-only (safe), mutation (standard), and destructive
   operations.
4. **Long-lived orchestration**: Use MCP tasks to coordinate multi-step flows and stream progress.
5. **Runtime filesystem ownership**: Workflows are loaded and persisted from a runtime directory
   derived from registry config paths (not embedded in generated manifests at build time).

---

## 3. MCP Resources

Resources are read-only and cacheable. They should provide deterministic context for LLM reasoning
without side effects.

### 3.1 Workflow Specs and Schemas
- `workflow.spec`: `specs/WORKFLOWS.md`.
- `workflow.tui_spec`: `specs/WORKFLOW_TUI.md` (optional).
- `workflow.schema`: JSON schema generated from `crates/types/src/workflow.rs`.

### 3.2 Workflow Manifests
- `workflow.manifests`: list of workflow identifiers with metadata (title, description, file path, format, version hash).
- `workflow.manifest:{workflow_id}`: manifest YAML or JSON content from the runtime workflow directory.

### 3.3 Workflow Storage Location

Workflow files are loaded and persisted from a single runtime directory using the same
resolution rules as the registry:

1. If `REGISTRY_WORKFLOWS_PATH` is set, use that absolute/expanded path.
2. Otherwise, derive from `default_config_path()` in `crates/registry/src/config.rs`:
   `<dirname(default_config_path())>/workflows`.
3. Because `default_config_path()` already honors `REGISTRY_CONFIG_PATH`, changing that env var
   changes the default workflow directory as well.

This directory is the source of truth for:
- `workflow.list`
- `workflow.get`
- `workflow.save`
- `workflow.rename`
- `workflow.delete`

### 3.4 Command and Provider Catalogs
- `command.catalog`: list commands, schemas, argument names, and body schemas.
- `provider.catalog`: list providers, select fields, and required parameters.

---

## 4. MCP Prompts

Prompts standardize authoring behavior and ensure consistent schema compliance.

### 4.1 Authoring Prompts
- `workflow.author`
  - Inputs: user goal, target system, constraints.
  - Output: new workflow draft in YAML.

- `workflow.extend`
  - Inputs: existing manifest, desired changes.
  - Output: updated manifest with preserved ordering.

### 4.2 Validation and Repair Prompts
- `workflow.fix_validation`
  - Inputs: manifest plus validation errors.
  - Output: corrected manifest.

### 4.3 Execution Prompts
- `workflow.run_with_inputs`
  - Inputs: manifest, required inputs, validation rules.
  - Output: resolved inputs or clarifying questions.

---

## 5. MCP Tools

The tool surface is grouped by safety level. Tool definitions must include
`execution_type` and `http_method` metadata for the MCP server to route.

### 5.1 Safe Tools (Read-only)
- `workflow.list`
  - Returns: list of manifests with metadata.

- `workflow.get`
  - Parameters: `workflow_id`.
  - Returns:
    - `workflow_id`
    - `path`
    - `format` (`yaml` or `json`)
    - `content` (raw text for editing)
    - `parsed` (normalized JSON object)
    - `version` (content hash for optimistic concurrency checks).

- `workflow.validate`
  - Parameters: manifest content.
  - Returns: structured errors and warnings.

- `workflow.preview_inputs`
  - Parameters: manifest content.
  - Returns: required inputs, defaults, validation rules.

- `workflow.preview_rendered`
  - Parameters: manifest + candidate inputs.
  - Returns: rendered steps with resolved templates.

- `command.list`
  - Returns: command list with schemas and argument names.

- `provider.list`
  - Returns: provider list with select metadata and required parameters.

### 5.2 Standard Tools (Mutating but reversible)
- `workflow.save`
  - Parameters:
    - manifest content
    - optional overwrite flag
    - optional expected `version` (from `workflow.get`) for optimistic concurrency.
  - Behavior:
    - validates schema before write
    - writes atomically to runtime workflow directory
    - updates in-memory workflow registry
    - synchronizes synthetic workflow command availability in command discovery.
  - Returns: path, normalized workflow identifier, and new `version`.

- `workflow.rename`
  - Parameters: `workflow_id`, `new_id`.
  - Behavior: renames file and workflow identifier, then synchronizes registry and synthetic commands.
  - Returns: updated metadata and new `version`.

- `workflow.resolve_inputs`
  - Parameters: manifest + partial inputs.
  - Returns: defaults, resolved values, validation status, and `ready`.
  - `ready` is true only when required inputs are present and provider resolutions have no `prompt`/`error` outcomes.

- `workflow.run`
  - Parameters: `workflow_id` or manifest content, plus input values.
  - Returns: `run_id` and task handle metadata when client negotiates task mode.

- `workflow.step_plan`
  - Parameters: manifest + inputs.
  - Returns: ordered step plan with conditions and dependencies.

### 5.3 Destructive Tools
- `workflow.delete`
  - Parameters: `workflow_id`.
  - Behavior: removes filesystem entry and synchronizes registry/synthetic commands.
  - Returns: deletion confirmation.

- `workflow.cancel`
  - Parameters: `operation_id` (task operation identifier).
  - Returns: cancellation status.

- `workflow.purge_history`
  - Parameters: `workflow_id` or input keys.
  - Returns: summary of removed entries.

### 5.4 Catalog Import and Runtime Management Tools

These tools allow an LLM to add OpenAPI-backed catalogs to the runtime without manual file edits.
They apply the same runtime path rules used by registry/workflow config resolution and must never
write outside user-approved workspace/runtime paths.

#### 5.4.1 Safe Tools (Read-only)

- `catalog.validate_openapi`
  - Parameters:
    - `source` (local path or URL)
    - optional `source_type` (`path` | `url`, inferred when omitted)
  - Behavior:
    - Loads source content.
    - Parses OpenAPI document and validates minimum import requirements.
    - Does not mutate runtime state.
  - Returns:
    - `valid` (`true`/`false`)
    - `document_kind` (`openapi_2` | `openapi_3` | `unknown`)
    - `operation_count`
    - `warnings[]`
    - `violations[]` (section 10.2 shape)

- `catalog.preview_import`
  - Parameters:
    - `source`
    - optional `source_type`
    - `catalog_title`
    - optional `vendor`
    - optional `base_url`
    - optional `include_command_preview` (`true`/`false`, default `false`)
  - Behavior:
    - Performs full dry-run import planning without writing files.
    - Computes resulting catalog metadata and generated command summary.
  - Returns:
    - `catalog` (normalized title/vendor/base_url)
    - `projected_command_count`
    - `projected_group_prefixes[]`
    - `provider_contract_count`
    - `warnings[]`
    - optional `command_preview[]` (token-capped list when requested)

#### 5.4.2 Standard Tools (Mutating but reversible)

- `catalog.import_openapi`
  - Parameters:
    - `source`
    - optional `source_type`
    - `catalog_title`
    - optional `vendor`
    - optional `base_url`
    - optional `overwrite` (`false` default)
    - optional `enabled` (`true` default)
  - Behavior:
    - Validates OpenAPI source before any mutation.
    - Writes/updates runtime catalog config + generated manifest artifacts.
    - Reloads in-memory command registry and refreshes command search state.
    - Fails with `conflict` when target catalog exists and `overwrite != true`.
  - Returns:
    - `catalog_id`
    - `catalog_path`
    - `manifest_path`
    - `command_count`
    - `provider_contract_count`
    - `warnings[]`

- `catalog.set_enabled`
  - Parameters:
    - `catalog_id` or `catalog_title`
    - `enabled` (`true`/`false`)
  - Behavior:
    - Toggles runtime availability without deleting persisted catalog artifacts.
    - Refreshes in-memory command registry and command search state.
  - Returns:
    - `catalog_id`
    - `enabled`
    - `command_count_after_toggle`

#### 5.4.3 Destructive Tools

- `catalog.remove`
  - Parameters:
    - `catalog_id` or `catalog_title`
    - optional `remove_manifest` (`false` default)
  - Behavior:
    - Removes catalog entry from runtime config.
    - Optionally removes generated manifest artifacts when `remove_manifest=true`.
    - Refreshes in-memory command registry and command search state.
  - Returns:
    - `removed_catalog_id`
    - `manifest_removed` (`true`/`false`)
    - `remaining_catalog_count`

#### 5.4.4 Tooling Guidance for LLMs

- Prefer: `catalog.validate_openapi` -> `catalog.preview_import` -> `catalog.import_openapi`.
- Use local path sources by default; use URL only when explicitly requested.
- Do not call `catalog.remove` unless the user explicitly asks for deletion.
- Always surface `warnings[]` and `violations[]` back to the user during import flows.

---

## 6. MCP Tasks

Tasks are required for long-running or multi-step operations to avoid tool call timeouts.
They expose status events and intermediate results to the client.

Note: MCP tasks are currently marked experimental by the protocol, but are adopted here
because workflow execution UX requires progress streaming and resumable polling semantics.

### 6.1 `workflow.author_and_run`
1. Author draft from prompt.
2. Validate and repair until clean or user cancels.
3. Save manifest.
4. Resolve inputs, prompt user for missing values.
5. Execute workflow and stream updates.
6. Summarize results with outputs and errors.

### 6.2 `workflow.execute`
1. Start workflow run.
2. Stream step status updates and outputs.
3. Provide final summary and outputs.

### 6.3 `workflow.repair_and_rerun`
1. Validate failed manifest/run.
2. Apply repair prompt.
3. Save and re-run.

---

## 7. Sampling Strategy

Sampling is recommended for generating large structured blocks such as input maps
or step definitions. Each sampling pass should be followed by validation to
ensure schema compliance.

Recommended sampled blocks:
- Input definitions for new workflows.
- Step lists and output contracts.
- Provider argument maps.

---

## 8. Execution Flow Mapping

The proposed tools map directly to existing engine behavior:

- Schema validation: `crates/types/src/workflow.rs`
- Input resolution and templating: `crates/engine/src/resolve.rs`
- Execution pipeline: `crates/engine/src/workflow/`
- Persistence: runtime workflow directory via `default_workflows_path()` in `crates/registry/src/config.rs`
- Runtime loading: `crates/registry/src/workflows.rs`
- Build-time behavior: workflows are not bundled into `RegistryManifest` by `crates/registry-gen`.

---

## 9. Safety and Authorization

- Safe tools cannot mutate or execute.
- Standard tools allow writes to the runtime workflow directory and execution; require explicit
  model intent and user confirmation where possible.
- Destructive tools require explicit confirmation and should never be invoked
  automatically by prompts.

---

## 10. Error Handling Expectations

All workflow tools and tasks return structured machine-readable errors with
high-fidelity debugging fields.

### 10.1 Error Payload Shape

`ErrorData.data` must include:
- `error_code` (stable string identifier, example `WORKFLOW_VALIDATION_FAILED`)
- `category` (`validation` | `not_found` | `conflict` | `execution` | `internal`)
- `message` (unambiguous human-readable detail)
- `context` (structured identifiers: `workflow_id`, `run_id`, `step_id`, `path`, optional `line`, `column`)
- `retryable` (`true`/`false`)
- `suggested_action` (single explicit next action)
- `correlation_id` (for log correlation)

### 10.2 Validation Errors

Validation errors also include:
- `violations[]` where each item contains:
  - `path`
  - `rule`
  - `message`
  - optional `expected`
  - optional `actual`

### 10.3 Execution Errors

Execution errors also include:
- `step_id`
- `command`
- `response_payload` (when available and safe to expose).

### 10.4 Task Status

Long-running tasks must emit periodic status updates with:
- `run_id`
- current step identifier/name
- progress state (`pending` | `running` | `succeeded` | `failed` | `canceled`)
- latest summary message.

---

## 11. Open Questions

- Should workflow manifests include optional metadata headers (for example,
  `author`, `created_at`) or remain pure workflow YAML/JSON payloads?

---

## 12. References

- `specs/WORKFLOWS.md`
- `specs/WORKFLOW_TUI.md`
- `specs/VALUE_PROVIDERS.md`
- `crates/engine/src/workflow/`
- `crates/types/src/workflow.rs`

---

## 13. MCP Tasks Compatibility

Workflow execution tasks target the MCP Tasks utility as documented in:
- `https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks`

### 13.1 Capability Requirements

- Servers implementing long-running workflow execution must expose tasks capability.
- Clients invoking task-backed workflow execution must support task lifecycle APIs
  (`tasks/create`, `tasks/get`, and result retrieval flow).

### 13.2 Fallback Behavior

When task capability is unavailable, the server must fail fast for task-required operations
with a structured non-retryable error:
- `error_code`: `TASK_CAPABILITY_REQUIRED`
- `category`: `execution`
- `message`: explicit statement that the operation requires MCP tasks support
- `suggested_action`: connect with a tasks-capable MCP client.

Long-running workflow execution must not silently degrade to best-effort synchronous tool
execution, because this creates timeout-prone behavior and inconsistent UX.

---

## 14. Proposed Source Layout

Workflow MCP code should be organized under a single feature root to keep tool/task/resource/prompt
logic cohesive and avoid cross-module sprawl.

### 14.1 MCP Server Entry Points

- `crates/mcp/src/server/core.rs` — top-level routing and registration; delegates workflow operations to `server/workflow/`.
- `crates/mcp/src/server/http.rs` — HTTP transport host utilities (existing).
- `crates/mcp/src/server/schemas.rs` — non-workflow shared schema helpers (existing).
- `crates/mcp/src/server/workflow/mod.rs` — workflow feature root export and registration.

### 14.2 Workflow Feature Root

- `crates/mcp/src/server/workflow/mod.rs` — wiring for tools, tasks, resources, prompts.
- `crates/mcp/src/server/workflow/tools/` — MCP tool handlers.
- `crates/mcp/src/server/workflow/tasks/` — MCP task handlers and task lifecycle orchestration.
- `crates/mcp/src/server/workflow/resources/` — MCP resource handlers.
- `crates/mcp/src/server/workflow/prompts/` — MCP prompt handlers.
- `crates/mcp/src/server/workflow/services/` — shared operational services (storage adapter, sync, validation orchestration).
- `crates/mcp/src/server/workflow/types/` — request/response payload schemas and DTOs.
- `crates/mcp/src/server/workflow/errors/` — typed workflow errors + conversion to MCP `ErrorData`.

### 14.3 Tool Modules

- `crates/mcp/src/server/workflow/tools/manifest.rs` — `workflow.list`, `workflow.get`, `workflow.save`, `workflow.rename`, `workflow.delete`.
- `crates/mcp/src/server/workflow/tools/inputs.rs` — `workflow.preview_inputs`, `workflow.resolve_inputs`.
- `crates/mcp/src/server/workflow/tools/execution.rs` — `workflow.run`, `workflow.cancel`, `workflow.step_plan`, `workflow.preview_rendered`.
- `crates/mcp/src/server/workflow/tools/history.rs` — `workflow.purge_history` (and any future history reads).

### 14.4 Services

- `crates/mcp/src/server/workflow/services/storage.rs` — adapter over `oatty-registry` runtime workflow storage.
  - Must not duplicate canonical filesystem resolution rules implemented in `crates/registry/src/config.rs`
    and `crates/registry/src/workflows.rs`.
- `crates/mcp/src/server/workflow/services/sync.rs` — post-mutation synchronization:
  - in-memory workflow registry updates
  - synthetic workflow command availability updates.
- `crates/mcp/src/server/workflow/services/validation.rs` — workflow schema/business validation helpers reused by tools/tasks.

### 14.5 Resources and Prompts

- `crates/mcp/src/server/workflow/resources/workflow_spec.rs` — `specs/WORKFLOWS.md`.
- `crates/mcp/src/server/workflow/resources/workflow_schema.rs` — workflow schema resource.
- `crates/mcp/src/server/workflow/resources/workflow_manifest.rs` — list/get manifest resources.
- `crates/mcp/src/server/workflow/resources/provider_catalog.rs` — provider catalog resource.
- `crates/mcp/src/server/workflow/resources/command_catalog.rs` — command catalog resource.
- `crates/mcp/src/server/workflow/prompts/workflow_author.rs`
- `crates/mcp/src/server/workflow/prompts/workflow_extend.rs`
- `crates/mcp/src/server/workflow/prompts/workflow_fix_validation.rs`
- `crates/mcp/src/server/workflow/prompts/workflow_run_with_inputs.rs`

### 14.6 Tasks

- `crates/mcp/src/server/workflow/tasks/author_and_run.rs`
- `crates/mcp/src/server/workflow/tasks/execute.rs`
- `crates/mcp/src/server/workflow/tasks/repair_and_rerun.rs`
- `crates/mcp/src/server/workflow/tasks/state.rs` — internal task state tracking and progress snapshots.

### 14.7 Type and Error Boundaries

- `types/` is public API contract surface for MCP handlers (stable payload schema).
- `errors/` is the sole mapping boundary from internal errors to `ErrorData` shape defined in section 10.
- `services/` owns side effects and persistence integration; handlers should remain thin orchestration layers.

---

## 15. Phased Implementation Plan

### 15.1 Phase 1 (Core Author + Execute)

- Implement tools:
  - `workflow.list`
  - `workflow.get`
  - `workflow.validate`
  - `workflow.save`
  - `workflow.delete`
  - `workflow.run`
- Implement task-backed execution for long-running runs (`workflow.run` + task lifecycle support).
- Implement structured error payload shape from section 10.

### 15.2 Phase 2 (Workflow Editing and Planning)

- Implement tools:
  - `workflow.rename`
  - `workflow.step_plan`
  - `workflow.preview_inputs`
  - `workflow.preview_rendered`
  - `workflow.resolve_inputs`
- Harden optimistic concurrency/version checks for concurrent edits.

### 15.3 Phase 3 (Extended Context and Operations)

- Implement workflow resources and prompts under the workflow feature root.
- Implement history operations (`workflow.purge_history`, optional reads).
- Expand authoring/repair orchestrations (`workflow.author_and_run`, `workflow.repair_and_rerun`).

### 15.4 Phase 4 (Catalog Runtime Import)

- Implement tools:
  - `catalog.validate_openapi`
  - `catalog.preview_import`
  - `catalog.import_openapi`
  - `catalog.set_enabled`
  - `catalog.remove`
- Reuse existing registry/config loading and runtime reload paths; do not introduce build-time
  catalog embedding for user-imported OpenAPI definitions.

### 15.5 Future Enhancement (Deferred)

- Evaluate precomputing and persisting command search haystacks during OpenAPI/catalog import.
  - Goal: reduce query-time CPU by avoiding repeated haystack assembly and normalization.
  - Tradeoff: larger persisted manifest artifacts and tighter coupling between manifest format
    and search-tokenization/scoring rules.
  - Guardrail: keep runtime-dynamic catalog metadata (title/vendor/description edits) coherent,
    either by recomputing dynamic search fragments at runtime or invalidating persisted haystacks
    when catalog metadata changes.
