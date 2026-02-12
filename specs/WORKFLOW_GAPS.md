# WORKFLOW_GAPS.md

As-built gap tracker for workflow capabilities that impact reliability, recovery, and competitive parity.

## Priority ordering

- `P0`: Resume and partial recovery.
- `P1`: Elevate `output_contract` to a first-class runtime and UX surface.
- `P2`: Reduce YAML cognitive load with guided authoring.

## P0: Resume and partial recovery

### Current state

- No step-level re-run.
- No checkpointing semantics exposed in spec.
- No partial execution plan rehydration documentation.

### Desired state

- Formal checkpoint graph.
- Durable step result persistence.
- Resume from step `N` (policy-driven: failed-only or failed+downstream).
- Execution plan diff visualization before resume.

### Acceptance criteria

- Workflow run artifacts persist enough state to reconstruct resume options.
- User can request partial resume from CLI, TUI, and MCP using explicit contracts.
- Resume operation validates dependency integrity before execution starts.
- System renders an execution plan diff (original vs. resumed path) before confirmation.
- Spec explicitly defines non-resumable cases and failure behavior.

### Competitive context

If workflows become longer and more critical, this is a direct competitive gap.

- AWS Step Functions has this.
- Temporal has this.
- GitHub Actions has partial support.

This is currently the largest structural weakness.

## P1: Output contract is underused

### Current state

`output_contract` is schema metadata primarily used by MCP tooling for LLM-guided workflow authoring. It is under-leveraged and mostly serves implementation support.

### Desired state

`output_contract` becomes:

- Downstream validation surface.
- Type-safety narrative across authoring and execution.
- Safe mapping layer for step outputs.
- UI visualization asset.

### Acceptance criteria

- Output contract is exposed and consumable in authoring flows for step chaining.
- Runtime validates downstream mappings against declared output contracts.
- Authoring UX surfaces contract fields and mapping confidence.
- Validation errors reference contract fields with actionable fixes.

If output contracts become first-class, Oatty's competitive delta increases materially.

## P2: YAML cognitive load

### Current state

Without a strong visual editor or guided authoring path, workflow YAML:

- Beats shell scripting for structure and repeatability.
- Does not beat mental simplicity for most operators.

Current MCP workflow authoring surface is documented in `/Users/justinwilaby/Development/next-gen-cli/specs/MCP_WORKFLOWS.md`.

### Desired state

- Guided, interactive workflow authoring with deterministic guardrails.
- Clear iteration loop: compose -> validate -> preview -> run/recover.
- YAML remains an artifact, not the only authoring interface.

### Acceptance criteria

- New users can author and validate a multi-step workflow through guided flows without manual YAML edits.
- Guided flow surfaces dependencies, providers, and runtime validation before execution.
- Users can safely adjust mappings and immediately preview resulting execution changes.

If authoring remains manual YAML-first, Oatty stays competitive but is less likely to be dominant.

Guiding question: Can a user orchestrate steps safely, interactively, and deterministically without losing control?

## Competitive messaging pivot

Position Oatty around:

- Explicit dependency declaration.
- Provider-aware deterministic orchestration.
- Runtime validation.
- Human guardrails at execution time.

This should be the primary differentiation narrative in workflow authoring and execution docs.
