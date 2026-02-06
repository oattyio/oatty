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
5. **Clear ownership**: All files are written under `workflows/` and validated before execution.

---

## 3. MCP Resources

Resources are read-only and cacheable. They should provide deterministic context for LLM reasoning
without side effects.

### 3.1 Workflow Specs and Schemas
- `workflow.spec`: `specs/WORKFLOWS.md`.
- `workflow.tui_spec`: `specs/WORKFLOW_TUI.md` (optional).
- `workflow.schema`: JSON schema generated from `crates/types/src/workflow.rs`.

### 3.2 Workflow Manifests
- `workflow.manifests`: list of workflow identifiers with metadata (title, description, file path).
- `workflow.manifest:{workflow_id}`: manifest YAML or JSON content.

### 3.3 Command and Provider Catalogs
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
  - Returns: manifest content.

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
  - Parameters: manifest content, optional overwrite flag.
  - Returns: path and normalized workflow identifier.

- `workflow.rename`
  - Parameters: `workflow_id`, `new_id`.
  - Returns: updated metadata.

- `workflow.resolve_inputs`
  - Parameters: manifest + partial inputs.
  - Returns: defaults, resolved values, and validation status.

- `workflow.run`
  - Parameters: `workflow_id` or manifest content, plus input values.
  - Returns: `run_id`.

- `workflow.step_plan`
  - Parameters: manifest + inputs.
  - Returns: ordered step plan with conditions and dependencies.

### 5.3 Destructive Tools
- `workflow.delete`
  - Parameters: `workflow_id`.
  - Returns: deletion confirmation.

- `workflow.cancel`
  - Parameters: `run_id`.
  - Returns: cancellation status.

- `workflow.purge_history`
  - Parameters: `workflow_id` or input keys.
  - Returns: summary of removed entries.

---

## 6. MCP Tasks

Tasks should be used for long-running or multi-step operations. They expose
status events and intermediate results to the client.

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
- Persistence: `workflows/` directory via `crates/registry/src/config.rs`

---

## 9. Safety and Authorization

- Safe tools cannot mutate or execute.
- Standard tools allow writes to `workflows/` and execution; require explicit
  model intent and user confirmation where possible.
- Destructive tools require explicit confirmation and should never be invoked
  automatically by prompts.

---

## 10. Error Handling Expectations

- Validation errors are returned in a structured, machine-readable format.
- Execution errors include step identifier, command invoked, and any response
  payload.
- Long-running tasks must provide periodic status updates.

---

## 11. Open Questions

- What is the preferred schema validation format returned to MCP clients?
- Should workflow manifests be stored with explicit metadata headers (author,
  created_at) or remain pure YAML?
- Should input history be exposed to LLMs by default?

---

## 12. References

- `specs/WORKFLOWS.md`
- `specs/WORKFLOW_TUI.md`
- `specs/VALUE_PROVIDERS.md`
- `crates/engine/src/workflow/`
- `crates/types/src/workflow.rs`

---

## 13. Proposed Source Layout

This section proposes new MCP server modules for workflow-specific resources, prompts, tools, and tasks.
It extends the current server layout (`crates/mcp/src/server/core.rs`, `http.rs`, `schemas.rs`).

### 13.1 MCP Server Entry Points

- `crates/mcp/src/server/core.rs` — routing, guards, and execution dispatch.
- `crates/mcp/src/server/http.rs` — HTTP execution helpers (existing).
- `crates/mcp/src/server/schemas.rs` — tool/resource schemas (existing).
- `crates/mcp/src/server/workflow.rs` — workflow tool handlers and registration (new).
- `crates/mcp/src/server/resources.rs` — resource handlers (new).
- `crates/mcp/src/server/prompts.rs` — prompt templates (new).
- `crates/mcp/src/server/tasks.rs` — task orchestration entry points (new).

### 13.2 Workflow Tool Modules

- `crates/mcp/src/server/workflow/mod.rs` — exports and registration.
- `crates/mcp/src/server/workflow/manifest.rs` — read/write/validate workflow manifests.
- `crates/mcp/src/server/workflow/inputs.rs` — defaults, input validation, preview inputs.
- `crates/mcp/src/server/workflow/execution.rs` — run, cancel, step plans, status streaming.
- `crates/mcp/src/server/workflow/history.rs` — input history operations (purge, read).

### 13.3 Resources

- `crates/mcp/src/server/resources/mod.rs` — exports.
- `crates/mcp/src/server/resources/workflow_spec.rs` — `specs/WORKFLOWS.md`.
- `crates/mcp/src/server/resources/workflow_schema.rs` — JSON schema from `crates/types/src/workflow.rs`.
- `crates/mcp/src/server/resources/workflow_manifest.rs` — list/get manifest resources.
- `crates/mcp/src/server/resources/provider_catalog.rs` — providers.
- `crates/mcp/src/server/resources/command_catalog.rs` — commands.

### 13.4 Prompts

- `crates/mcp/src/server/prompts/mod.rs`
- `crates/mcp/src/server/prompts/workflow_author.rs`
- `crates/mcp/src/server/prompts/workflow_extend.rs`
- `crates/mcp/src/server/prompts/workflow_fix_validation.rs`
- `crates/mcp/src/server/prompts/workflow_run_with_inputs.rs`

### 13.5 Tasks

- `crates/mcp/src/server/tasks/mod.rs`
- `crates/mcp/src/server/tasks/workflow_author_and_run.rs`
- `crates/mcp/src/server/tasks/workflow_execute.rs`
- `crates/mcp/src/server/tasks/workflow_repair_and_rerun.rs`

### 13.6 Shared Types and Utilities

- `crates/mcp/src/server/types.rs` — shared request/response structs and error types.
- `crates/mcp/src/server/validation.rs` — validation helpers reused across tools and tasks.
