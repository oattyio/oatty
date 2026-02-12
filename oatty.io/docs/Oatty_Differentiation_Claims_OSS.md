# Oatty --- Evidence-Backed Differentiation (OSS Edition)

This document outlines concrete, defensible differentiation claims for
Oatty. Each claim includes:

-   The Claim
-   The Evidence (structural, not marketing)
-   Why It Matters to OSS Developers
-   Where Incumbents Fall Short
-   Honest Tradeoffs

No hype. No hand-waving.

------------------------------------------------------------------------

# 1. Explicit Dependency Enforcement (Not Implicit Guesswork)

## Claim

Oatty enforces explicit data dependencies between workflow steps before
execution.

## Evidence

-   Provider-backed inputs must declare `depends_on` mappings.
-   Runtime normalization blocks execution if dependency declarations
    are missing or mismatched.
-   Structured validation errors include actionable feedback and
    violation details.

## Why It Matters

Most bash scripts silently fail at runtime when an upstream value
changes shape. Oatty blocks invalid workflows before execution.

That prevents: - Hidden coupling - Accidental undefined values - Fragile
refactors

## Where Others Fall Short

Shell: implicit variable passing, no validation. GitHub Actions:
dependency order is implicit via step ordering. MCP-only: tool chaining
relies on LLM interpretation.

## Tradeoff

Requires slightly more upfront structure in workflow definitions.

------------------------------------------------------------------------

# 2. Deterministic Orchestration with Local Control

## Claim

Oatty provides structured orchestration (branching, polling, retries)
without requiring CI or cloud-native engines.

## Evidence

-   Declarative `repeat` with timeout and max attempts.
-   Conditional step execution via `if`.
-   Pause / resume control commands for in-flight runs.

## Why It Matters

You get deterministic orchestration locally without needing: - GitHub
Actions - Step Functions - Temporal - CI pipeline glue

This keeps automation: - Fast - Debuggable - Local-first

## Where Others Fall Short

Shell: manual loops and retry logic. CI: remote-only execution and
debugging friction. MCP-only: non-deterministic execution ordering.

## Tradeoff

No first-class step-level resume yet (planned).

------------------------------------------------------------------------

# 3. Multi-Vendor Surface Compression

## Claim

Oatty collapses multiple vendor CLIs into a single execution model.

## Evidence

-   Schema-driven command ingestion across APIs.
-   Unified workflow engine regardless of vendor.
-   Shared retry, validation, and error semantics.

## Why It Matters

Without Oatty, multi-vendor workflows require:

-   Multiple CLIs
-   Different command grammars
-   Different output formats
-   Different retry semantics

Oatty reduces this to one mental model.

## Where Others Fall Short

Vendor CLIs scale linearly with vendor count. CI workflows fragment
execution semantics. MCP tools lack deterministic orchestration.

## Tradeoff

Requires API schema ingestion and normalization upfront.

------------------------------------------------------------------------

# 4. Structured, Actionable Failure Surfaces

## Claim

Oatty surfaces categorized, structured workflow validation and runtime
errors.

## Evidence

-   Error objects include code, category, suggestion, correlation_id.
-   Validation errors block execution prior to runtime failure.

## Why It Matters

Debugging shell scripts often means reading ambiguous stderr output.
Oatty makes failures diagnosable and actionable.

## Where Others Fall Short

Shell: log parsing required. CI: remote log digging. MCP-only: tool
failure ambiguity.

## Tradeoff

Adds structured validation layer complexity.

------------------------------------------------------------------------

# 5. Context-Aware Input Resolution

## Claim

Oatty supports provider-backed input resolution tied to resolved
upstream state.

## Evidence

-   `workflow.resolve_inputs` readiness checks.
-   Provider bindings receive resolved input context.
-   Dependency alignment enforced via validation rules.

## Why It Matters

Autocomplete becomes deterministic once bound. No guessing. No fragile
runtime interpolation.

## Where Others Fall Short

Shell: manual interpolation. CI: no dynamic resolution. MCP-only:
LLM-dependent context inference.

## Tradeoff

Provider system requires maintenance and version alignment.

------------------------------------------------------------------------

# 6. Reduced Coordination Cost Across Tools

## Claim

Oatty reduces the number of mental models required to operate APIs.

## Evidence

-   Unified CLI/TUI/workflow model.
-   Shared execution semantics across vendors.
-   Single validation and retry abstraction.

## Why It Matters

Traditional stacks require juggling:

bash + curl + jq + CI YAML + vendor CLIs

Oatty collapses that surface area.

## Where Others Fall Short

Every additional vendor increases cognitive load. Tool fragmentation
compounds over time.

## Tradeoff

Requires commitment to Oatty's execution model.

------------------------------------------------------------------------

# Bottom Line

Oatty is not "another CLI."

It provides:

-   Explicit dependency enforcement
-   Deterministic orchestration
-   Multi-vendor surface compression
-   Structured error semantics
-   Context-aware resolution
-   Reduced coordination cost

For OSS developers, that translates to:

-   Less glue code
-   Fewer hidden traps
-   Faster iteration
-   Safer automation

That is the real differentiation.
