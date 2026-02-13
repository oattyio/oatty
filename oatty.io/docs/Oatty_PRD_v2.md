# Oatty Documentation & Adoption PRD

## Version 2.0 -- Strategic Edition

------------------------------------------------------------------------

# 1. Executive Summary

Oatty introduces a new operational model: a unified execution surface
for vendor APIs that combines schema‑derived commands, an interactive
TUI, filesystem workflows, and MCP extensibility.

This is not a documentation refresh.

This is a category education initiative designed to:

- Reduce time‑to‑first‑workflow
- Standardize operational patterns
- Decrease custom script creation
- Increase cross‑vendor consistency
- Make automation approachable without sacrificing control

Documentation is a primary product surface. Adoption depends on it.

------------------------------------------------------------------------

# 2. Product Context

Modern vendor tooling suffers from:

- Thin CLI wrappers over powerful APIs
- Fragmented MCP integrations
- Partial coverage of API surface area
- Brittle automation scripts

Oatty solves this by treating APIs as inputs, not boundaries.

OpenAPI → Registry → CLI/TUI → Workflows → MCP → Human Review →
Execution

Documentation must teach this mental model clearly and progressively.

------------------------------------------------------------------------

# 3. Problem Statement

Users struggle with:

1. Memorization-heavy CLIs
2. Inconsistent vendor tooling
3. Script-based automation that is unreviewable and unsafe
4. Limited discoverability of available API capabilities

The result: - Slower onboarding - Operational drift - Increased
cognitive load - Duplicated tooling effort

------------------------------------------------------------------------

# 4. Goals & Success Metrics

## Primary Goals

- Reduce time-to-first-success to under 10 minutes.
- Reduce multi-step manual CLI operations.
- Increase workflow creation within first session.
- Improve clarity of mental model.

## Definition of First Success

First Success =

User imports an OpenAPI catalog, Discovers a command via search,
Executes it, Creates or runs a simple workflow.

------------------------------------------------------------------------

# 5. Core UX Principles

## 1. Discoverability

All commands, workflows, and extensions are searchable and browsable.

## 2. Simplicity

Each screen performs one job: - Search - Inspect - Execute - Monitor

## 3. Speed

Keyboard-first, low-latency interaction. Fast startup. Reliable
execution.

## 4. Consistency

One command model drives: - CLI - TUI - Workflows - MCP plugins

------------------------------------------------------------------------

# 6. Personas

## Platform Engineer

Needs: - Cross-vendor orchestration - Repeatable workflows - Consistency

Success looks like: Creating and sharing workflows in under 60 minutes.

## DevOps Engineer

Needs: - Operational visibility - Fast execution - Reliable automation

Success looks like: Replacing a script with a validated workflow.

## Tooling Builder

Needs: - Extensibility - MCP integration - Structured execution model

Success looks like: Publishing a plugin that integrates seamlessly.

------------------------------------------------------------------------

# 7. Golden Path (Canonical Experience)

This section must exist in final docs.

## Step 1: Import an OpenAPI Spec

## Step 2: Search for a Command in TUI

## Step 3: Execute Command

## Step 4: Create a Multi-Step Workflow

## Step 5: Add a Provider for Autocomplete

## Step 6: Run Workflow with Human Confirmation

This flow becomes: - Demo script - Blog post - Website walkthrough -
Conference narrative

------------------------------------------------------------------------

# 8. Scope & Non-Goals

## In Scope

- Quick start guide
- Conceptual overview
- Workflow tutorials
- Provider examples
- MCP extension overview
- Troubleshooting section

## Not in Scope (Phase 1)

- Deep registry internals
- Advanced MCP protocol mechanics
- Low-level engine architecture
- Plugin authoring deep dive

------------------------------------------------------------------------

# 9. Risks & Mitigation

## Risk: Over-Ambition (NL/AI focus)

Mitigation: Position NL as an enhancement layer, not core functionality.

## Risk: Cognitive Overload

Mitigation: Progressive disclosure. Golden path orientation. Single-task
screens.

------------------------------------------------------------------------

# 10. Strategic Framing

This initiative is not about improving documentation.

It is about accelerating adoption of a new operational paradigm.

Documentation must: - Reduce friction - Clarify mental model - Encourage
safe experimentation - Build confidence quickly

------------------------------------------------------------------------

# 11. Long-Term Vision

Oatty becomes the standard execution surface for API operations.

Documentation evolves into: - A reference - A playbook - A learning
system

------------------------------------------------------------------------

End of PRD.
