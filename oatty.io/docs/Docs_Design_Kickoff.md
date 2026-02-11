# Oatty Docs Design Kickoff

## Purpose
This artifact defines the initial design plan for `oatty.io` documentation as an adoption surface, with learning and discovery as primary outcomes.

## Product Scope To Prioritize
### TUI Primary Features
1. Library
2. Run Command
3. Search Command
4. Workflows
5. Plugins
6. MCP Server

### CLI Primary Features
1. Run command from catalog
2. Built-in commands

## Core Doc Strategy
1. Treat docs as product onboarding, not reference-only content.
2. Optimize for first successful action quickly, then progressive depth.
3. Preserve a dual path:
   - Learn path (guided progression).
   - Reference path (fast lookup).

## MVP Information Architecture
1. Quick Start
2. Learn
   - Getting Oriented (TUI interaction model)
   - Search and Run Commands
   - Library and Catalogs
   - Workflows
   - Plugins and MCP Server
3. Guides
   - Task-driven, persona-aligned workflows
4. Reference
   - CLI commands, TUI interactions, config/env vars
5. Troubleshooting
6. Recipes

## Navigation Model (v1)
1. Left persistent docs nav (desktop), drawer nav (mobile).
2. Right-side pinned "On this page" navigation (desktop) linking to in-page sections.
3. Mobile fallback for "On this page" as a collapsible section near the top of content.
4. Prev/Next links at bottom for Learn and Guide pages.
5. “Related Pages” block on every page.

## Page Inventory (Phase 1)
1. `quick-start.md`
2. `learn/getting-oriented.md`
3. `learn/search-and-run-commands.md`
4. `learn/library-and-catalogs.md`
5. `learn/workflows-basics.md`
6. `learn/plugins-and-mcp-server.md`
7. `guides/import-catalog.md`
8. `guides/run-first-workflow.md`
9. `guides/provider-backed-inputs.md`
10. `guides/nl-human-in-the-loop-execution.md`
11. `reference/cli-commands.md`
12. `reference/tui-interactions.md`
13. `reference/config-and-env.md`
14. `troubleshooting/common-errors.md`
15. `recipes/automation-patterns.md`

## Design Constraints
1. Keep docs readable on 13" laptop and mobile.
2. Ensure code blocks support copy affordance and line wrapping/scrolling.
3. Keep examples concise with expected output for every critical step.
4. Avoid deep nesting in nav for v1.
5. Each page includes a summary card near the top describing what the user will learn.
6. Accessibility baseline: WCAG 2.1 AA intent for docs UI.
7. Keyboard accessibility required for all docs navigation controls and in-page TOC interactions.
8. Verify heading structure and landmarks for screen-reader navigation.

## Global Page Pattern
1. Place a "What you'll learn" summary card directly below the page title/introduction.
2. Keep the card concise:
   - 3-5 bullets
   - Outcome-oriented language
   - Linked terms where useful
3. Apply this pattern to Quick Start, Learn, Guides, and Reference pages.
4. Include a short "If this fails" block for high-risk tasks with direct recovery links.

## "What You'll Learn" Card Component Spec
1. Required fields:
   - Title: `What you'll learn`
   - Bullet list: 3-5 items
2. Optional fields:
   - Estimated time (for Learn/Guides)
   - Prerequisite tag (when needed)
3. Content rules:
   - Use action/outcome phrasing (e.g., "After this page, you can...")
   - Keep each bullet to one concise sentence
4. Visual rules:
   - Render as a distinct card/panel
   - Keep placement consistent across all docs types

## Content Templates
### Quick Start Template
1. Outcome
2. Prerequisites
3. Steps
4. Expected result
5. What to do next

### Learn Module Template
1. What you will learn
2. Why it matters
3. Concepts
4. Guided steps
5. Check your understanding
6. Next module

### Guide Template
1. Use case
2. Inputs/requirements
3. Procedure
4. Validation
5. Failure modes and fixes

### Reference Template
1. Definition/scope
2. Syntax/options
3. Examples
4. Notes/constraints

## Discovery and Adoption Hooks
1. Persona-based “If you do X today, Oatty enables Y” callouts in Learn and Guides.
2. Golden Path reminders across Quick Start and Learn:
   - Import API/catalog
   - Discover command
   - Execute command
   - Compose workflow
   - Add provider
   - Run with confirmation
3. Clear surfacing of help affordances:
   - Hint spans
   - Keyboard focus model
   - Mouse interaction expectations
   - Layout behavior under terminal width changes

## Instrumentation Requirements (Phase 1)
1. Define and implement event taxonomy aligned to PRD:
   - `docs_page_view`
   - `docs_summary_card_view`
   - `docs_quick_start_step_complete`
   - `docs_cta_click`
   - `docs_search_query`
   - `docs_search_result_click`
2. Track Quick Start completion using explicit step completion markers.
3. Track clicks from Learn pages to product actions (command/workflow docs).
4. Track navigation path from landing to first workflow guide.

Completion definition (phase 1):
1. Quick Start completion = telemetry automatically records all required step-complete events for Quick Start.
2. Learn module completion = telemetry records module-end view plus next-step CTA view/click.

## Docs Quality Gates
1. Add a docs lint/check rule that fails when pages are missing the "What you'll learn" card.
2. Add a style check for 3-5 bullets in the card.
3. Include card presence in docs PR review checklist.

## Scannability Guardrails
1. Maximum introduction length before the card: 3 short paragraphs.
2. Card appears above the first major section header.
3. Prefer bullet phrasing that starts with an action verb or "After this page, you can...".

## Ready-to-Start Checklist
1. Confirm final docs URL structure.
2. Confirm left-nav section labels.
3. Confirm search strategy for phase 1.
4. Assign owner per phase-1 page.
5. Start with Quick Start + first two Learn modules.

## Content Ownership (RACI)
1. Content authoring:
   - Responsible: Technical writer / docs owner
   - Accountable: Product + DX lead
2. Technical accuracy review:
   - Responsible: Feature SME (engineering)
   - Accountable: Engineering lead for affected surface
3. UX/copy quality review:
   - Responsible: Design/content reviewer
   - Accountable: Product design lead
4. Publish approval:
   - Responsible: Docs owner
   - Accountable: Product + DX lead
