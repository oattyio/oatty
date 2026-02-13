# Oatty.io Documentation PRD

## Metadata
- Owner: Product + DX
- Status: Draft
- Last updated: 2026-02-11
- Scope: Documentation experience for `https://oatty.io`

## Problem Statement
Users can run Oatty once they discover the right commands, but learning and adoption are slowed by gaps in discoverability, guided onboarding, and cross-linking between concepts, guides, and reference material. We need a documentation experience that supports both first-time learning and fast day-2 lookup.

## Goals
1. Make first success fast with a clear Quick Start path.
2. Improve discoverability of advanced capabilities (workflows, value providers, MCP integrations).
3. Provide reliable reference material that matches shipping behavior.
4. Reduce support and trial-and-error by improving troubleshooting and examples.
5. Highlight Oatty's high-value NL-assisted, human-in-the-loop execution model for workflow and command operations.

## Strategic Value Proposition
Oatty enables a human-in-the-loop execution cycle where natural-language requests can drive workflow management, command discovery, and safe execution decisions through MCP tooling. Documentation should make this value explicit with practical, guarded execution patterns.

## Competitive Landscape (Adoption Context)
Primary alternatives users migrate from:
1. Vendor-specific CLIs + shell scripts.
2. Ad hoc API calls (curl/Postman collections) with local runbooks.
3. MCP-only integrations without a unified workflow-first operator surface.

Oatty documentation must show clear differentiation:
1. One command/workflow model across CLI, TUI, and MCP.
2. Human-in-the-loop execution with confirmation and recovery guidance.
3. Progressive onboarding from discovery to reusable workflow execution.

Adoption win/loss criteria:
1. Win: user reaches first workflow execution and adopts repeat usage.
2. Loss: user completes only one-off command usage and returns to scripts/tool sprawl.

## Positioning
1. Oatty is an execution plane for API operations, not only a command runner.
2. The docs should reinforce unification:
   - Schema-derived commands
   - Interactive TUI exploration
   - Deterministic workflows
   - MCP extensibility
3. Documentation is a product adoption surface, not a support-only artifact.

## Non-Goals
1. Building a full docs CMS in phase 1.
2. Publishing versioned docs in phase 1 (capture requirements now, defer implementation).
3. Rewriting all existing product copy before launch.

## Target Audiences
### Primary Personas (Core 3)
1. Platform Engineer: needs cross-vendor consistency, reusable workflows, and team-level standardization.
2. DevOps/SRE Engineer: needs operational visibility, fast/reliable execution, and safe automation replacement for scripts.
3. Tooling & Integration Builder: needs MCP extensibility, integration patterns, and a structured execution model.

### Secondary Stakeholder
1. Security/Compliance Reviewer: validates guardrails, secrets posture, and approval patterns; not the primary day-to-day docs consumer for phase 1.

### User Journey Stages (Not Separate Personas)
1. New user onboarding.
2. Activated user (first successful command/workflow).
3. Returning/advanced user (reference + optimization workflows).

## Persona Use Cases (Relatable Mapping)
This section defines high-impact “today vs with Oatty” mappings to anchor documentation in real workflows.

1. Platform Engineer
   - If you normally juggle inconsistent vendor CLIs, Oatty lets you use one consistent command/workflow surface.
   - If you normally coordinate multi-step operations manually, Oatty lets you encode and share deterministic workflows.
2. DevOps/SRE Engineer
   - If you normally maintain brittle runbooks/scripts, Oatty lets you replace them with validated workflow execution paths.
   - If you normally debug operational failures ad hoc, Oatty provides structured logs and repeatable execution context.
3. Tooling & Integration Builder
   - If you normally juggle separate scripts/tools for integrations, Oatty lets you manage MCP plugins and run operations from one interface.
   - If you normally translate natural-language intent into many manual CLI steps, Oatty lets you use MCP workflow management and command discovery/execution to keep a human-in-the-loop while reducing manual glue work.
4. Cross-cutting CI/CD scenario (primarily DevOps/SRE + Platform)
   - If you normally maintain brittle pipeline glue code, Oatty lets you reuse workflow definitions in local and CI environments.
   - If you normally pass secrets inconsistently, Oatty supports explicit secret patterns and backend-aware handling.

## Validation Plan (Personas and Use Cases)
1. Current persona mappings are initial assumptions based on product direction.
2. During phase 1, validate with:
   - User interviews
   - Docs telemetry (paths, completions, drop-offs)
   - Support/question trends
3. Update persona/use-case language at the end of the baseline capture window.

## UX Principles
1. Dual-path navigation:
   - Learn path for progressive onboarding (React Learn-like flow).
   - Reference path for direct lookup.
2. Progressive disclosure:
   - Start with simple end-to-end outcomes, then reveal advanced options.
3. Task-first guides:
   - Focus on “how to do X,” not only “what X is.”
4. Tight feedback loops:
   - Include expected outputs and validation checks for each guide.
5. Predictable wayfinding:
   - Persistent left nav + in-page TOC + related-links footer.

## Adoption Funnel (Docs-Aligned)
1. Curiosity: user lands on docs and understands Oatty's value quickly.
2. Activation: user executes a first command.
3. Value realization: user runs or creates a first workflow.
4. Commitment: user replaces an existing script/process with workflow-driven execution.
5. Advocacy: user shares workflow patterns or extends Oatty via MCP/plugin paths.

## Activation Definition (Docs)
First success within 10 minutes:
1. Import OpenAPI catalog/spec (or use an existing catalog).
2. Discover command via search.
3. Execute command successfully.
4. Run/create a simple workflow.

Documentation implications:
1. Quick Start must explicitly map to the four activation steps above.
2. Each step includes expected output and one common failure/fix.

## Information Architecture (v1)
1. Quick Start
2. Learn
   - Getting Oriented (TUI interaction model)
   - Core concepts
   - Workflows
   - Value providers
   - MCP model
3. Guides
   - Task-oriented walkthroughs
4. Reference
   - CLI/TUI commands and keybindings
   - Config and environment variables
   - Workflow schema and provider semantics
5. Troubleshooting
6. Recipes

## Core Requirements
### R1: Quick Start
1. Install + run first command.
2. Run TUI and execute a basic task.
3. Create/import a basic workflow and run it.
4. Show expected output for each step.

### R1.5: Foundational Orientation
1. Document focus and navigation model (`Tab`/`BackTab`, active pane, list focus).
2. Document keyboard and mouse interaction conventions.
3. Document where to find help and hint spans in the UI.
4. Document modal interaction conventions (`Esc`, confirm/cancel patterns).
5. Document how terminal size changes layout and interaction surfaces.
6. Provide a short troubleshooting map for interaction confusion (focus, selection, layout).

### R2: Feature Documentation Coverage
1. CLI command model and registry behavior.
2. TUI capabilities and navigation model.
3. Workflows: authoring, validation, execution lifecycle.
4. Value providers: usage, constraints, `depends_on` expectations.
5. MCP plugins and server/client integration.
6. Secrets strategy, including backend selection and CI patterns.

### R3: Learn Experience
1. Sequential modules with estimated completion times.
2. Clear “what you will build/learn” sections.
3. “Next step” and prerequisite linking across modules.

### R4: Reference Experience
1. Fast-scannable pages with consistent structure.
2. Canonical examples and schema snippets.
3. Environment variable reference page.

### R5: Troubleshooting
1. Top recurring errors mapped to root cause and fix.
2. “If you see X, do Y” patterns.
3. Debugging checklist for workflow/provider failures.

### R6: Quality + Maintenance
1. Every feature PR that changes behavior updates relevant docs.
2. Docs lint/check integrated into CI.
3. Defined owner/reviewer checklist.
4. Docs quality checks enforce presence and format of the "What you'll learn" summary card.

### R7: Persona-Driven Use Case Coverage
1. Documentation includes high-impact “If you do X today, Oatty enables Y” mappings for each primary persona.
2. Quick Start and Learn pages link to relevant persona use cases.
3. At least one task guide exists for each of the three primary personas in MVP scope.

### R8: NL + Human-in-the-Loop MCP Flows
1. Documentation explicitly covers how MCP workflow management + command discovery/execution support natural-language-driven operations with user confirmation points.
2. Provide at least one end-to-end guide showing:
   - Natural-language intent
   - Command/workflow resolution
   - Human review/approval checkpoints
   - Execution and verification
3. Clearly define guardrails for safe execution (confirmation, validation, and failure recovery paths).

### R9: Golden Path Consistency
1. Docs, demos, and onboarding must share one canonical path:
   - Import API/catalog
   - Discover command
   - Execute command
   - Compose workflow
   - Add provider
   - Run with confirmation
2. Quick Start and Learn modules should cross-link into this same path to avoid narrative drift.
3. Golden path recovery scenarios must be documented for:
   - Import failure
   - Empty/failed command discovery
   - Workflow execution failure

## Content Model
Each page should include:
1. Title + one-sentence intent.
2. "What you'll learn" summary card near the top.
3. Who this is for.
4. Prerequisites.
5. Steps/examples.
6. Expected result.
7. Common failure modes.
8. Related links.

## Phase 1 Scope Boundary
Timeline and staffing:
1. Target duration: 6-week execution window from kickoff.
2. Suggested staffing: 1 technical writer/content lead + 1 design/implementation owner + rotating SME reviewers.
3. Definition of done: phase-1 pages in scope are published and linked from docs navigation.

Explicit exclusions for phase 1:
1. Full versioned documentation system.
2. Deep plugin-authoring internals and low-level engine architecture.
3. Advanced MCP protocol internals.

Phase 2 triggers:
1. Quick Start completion and time-to-first-success metrics stabilize at target for two consecutive review cycles.
2. Phase-1 docs support load decreases for setup/workflow basics.
3. Decision log items marked blocking are resolved.

## Proposed MVP Deliverables (Phase 1)
1. `Quick Start` page.
2. `Learn` landing and first 4 modules:
   - Getting Oriented (focus, keyboard/mouse, hints/help, layout behavior)
   - Oatty mental model
   - Workflows basics
   - MCP basics
3. `Guides`:
   - Import/remove workflow
   - Provider-backed inputs with `depends_on`
   - Running in CI with secure secrets
   - Persona-based “common tasks to Oatty workflows” mapping page
   - NL-to-execution human-in-the-loop guide using MCP workflow management
4. `Reference`:
   - Env vars/config
   - Workflow schema essentials
   - TUI keybindings
5. `Troubleshooting` starter page.

## UX/Navigation Requirements (Site)
1. Left-side docs navigation with collapsible sections.
2. In-page heading TOC for long pages.
3. Prev/next navigation.
4. Search entry point in header (phase 1 may be basic client-side).
5. Mobile-responsive docs nav and content layout.

## Metrics & Instrumentation Plan
Outcome metrics (with initial targets):
1. Time-to-first-success: median under 10 minutes.
2. Time-to-first-workflow: target under 20 minutes for first-session users.
3. Quick Start completion rate: target 60%+ for docs entrants.
4. First-session workflow execution rate: target 50%+ for activated users.
5. Search-to-click success rate: target 70%+ on docs search interactions.

Baseline and ownership:
1. Baseline capture window: first two weeks after instrumentation launch.
2. Metric owner: Product + DX.
3. Review cadence: weekly during phase 1, monthly after stabilization.
4. Escalation threshold: two consecutive review periods below target on any activation metric.

Event taxonomy (minimum required):
1. `docs_page_view` (with page identifier and section).
2. `docs_summary_card_view` (page + card present).
3. `docs_quick_start_step_complete` (step identifier).
4. `docs_cta_click` (target route/link type).
5. `docs_search_query` and `docs_search_result_click`.
6. `docs_feedback_submitted` (optional phase 1 if available).

## Risks and Mitigations
1. Risk: docs drift from shipped behavior.
   - Mitigation: docs update checklist in PR template + ownership.
2. Risk: too much conceptual depth early.
   - Mitigation: strict progressive disclosure and concise modules.
3. Risk: weak discoverability of advanced features.
   - Mitigation: cross-links + recipes + surfaced “advanced next steps.”
4. Risk: over-abstract messaging that hides practical value.
   - Mitigation: lead every major section with concrete task outcomes and examples.

## Decision Log
1. Versioning model (`latest-only` vs release-based):
   - Decision needed by: end of week 2
   - Owner: Product + DX
   - Blocks: long-term nav architecture
   - Fallback: latest-only
2. Search implementation scope (phase 1 vs phase 2 full-text):
   - Decision needed by: end of week 1
   - Owner: Design/Engineering
   - Blocks: header/nav implementation details
   - Fallback: basic client-side section search
3. Reference generation strategy (generated vs hand-authored):
   - Decision needed by: end of week 3
   - Owner: Engineering docs owner
   - Blocks: reference page production velocity
   - Fallback: hand-authored phase-1 essentials
4. Interactive snippets in phase 1:
   - Decision needed by: end of week 4
   - Owner: Product + Design
   - Blocks: implementation complexity and QA
   - Fallback: static copy-paste examples with expected output

## Launch Alignment
1. Docs must be ready before public launch announcements.
2. Launch channels (site announcement, developer communities, social posts) should link to Quick Start and Golden Path guide.
3. Early-access onboarding should route users through Quick Start + one persona-specific guide.

## Acceptance Criteria
1. Quick Start enables a new user to complete one end-to-end flow without external help.
2. Foundational orientation docs enable users to reliably navigate TUI interactions without trial-and-error.
3. Workflow + provider docs explicitly cover dependency rules and common failures.
4. Secrets documentation includes local and CI-safe patterns.
5. Learn and Reference paths are both present and reachable in one click from docs landing.
6. Initial troubleshooting page resolves top known failure scenarios.
7. Persona use-case mappings help users identify at least one immediately relevant Oatty workflow per persona.
8. NL + MCP human-in-the-loop guide demonstrates explicit review checkpoints before execution.
9. Golden Path is represented consistently across Quick Start, Learn, and Guides.
10. Every docs page includes a top summary card describing what the user will learn.
11. Docs checks fail when a page is missing the required summary card or violates the 3-5 bullet guidance.
