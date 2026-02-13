# Oatty Landing Page Marketing Strategy – v3

**Broad Appeal + Conceptual Positioning with Migration as One Example**  
*Revised February 2026 – Justin Wilaby (@JustinWilaby)*

## Core Positioning Statement (Hero / Tagline Level)

**Operate APIs from one consistent surface — with natural language assistance and built-in safety checks.**

Oatty combines schema-imported commands, interactive TUI discovery, reusable workflows, and MCP extensibility into a
single tool.

Use natural language to describe your goal → Oatty helps discover commands and suggest workflows → you review previews,
validate steps, and confirm before anything runs.

TUI for guided daily work, CLI for scripts and CI, MCP for AI tooling integration.

## Why This Matters (The Broader Problem We Solve)

Modern API operations are fragmented:

- Vendor CLIs vary in syntax, auth models, and output formats
- Many APIs require manual scripts or partial wrappers
- Multi-vendor tasks demand custom integration code
- Automation often breaks when APIs change
- AI assistants can struggle with large command sets and safe execution

Oatty helps by providing:

- Consistent commands from your imported OpenAPI schemas
- Fuzzy search to locate operations quickly
- Natural language assistance to suggest multi-step workflows
- Previews, validation, and user confirmation before execution
- The same catalog and workflows usable in TUI, CLI, and MCP contexts

Result: more structured, less frustrating API work — especially across multiple services.

## Key Conceptual Language – Internal Reference Only

(Use sparingly; prefer concrete descriptions)

- Natural language assistance for workflow suggestions
- Schema-based command discovery
- Human-in-the-loop execution with previews and confirmation
- Unified experience across TUI, CLI, and MCP
- Reusable, auditable workflows
- Safer multi-vendor operations

Avoid: agentic, zero-glue, instantly, 100% coverage, fully automatic execution, etc.

## Hero Section Recommendation

**Headline**  
Natural language assistance for multi-vendor API operations — with safety built in

**Subheadline**  
Import schemas from the services you use.  
Describe your goal in plain English.  
Oatty suggests commands and workflows.  
You review previews, validate, and confirm before running.

**Visual / Demo**  
Looping video or animated sequence showing:

1. NL prompt entered
2. Schemas referenced / commands discovered
3. Workflow suggestion appears (YAML preview)
4. Validation checks + confirmation step
5. Execution logs (include a brief failure → retry / correction moment for realism)

**Primary CTA**  
Watch the Demo  
**Secondary CTA**  
Start with Quick Start

## Problem → Solution Framing

**Problem Cards**

- Inconsistent syntax and conventions across vendor CLIs
- Need for manual scripts when coverage is incomplete
- Complex multi-vendor tasks requiring custom integration code
- Brittle automation that breaks on API changes
- Limited safe controls when using AI to run operations

**Solution Card**

Oatty provides:  
✓ Commands derived from your imported schemas  
✓ Search to find operations quickly  
✓ Natural language assistance to suggest workflows  
✓ Previews, validation, and user confirmation before run  
✓ Consistent experience in interactive TUI, CLI scripts, and MCP tooling

## How It Works (Short Trust-Building Section – 3 Steps)

Add this after Hero or before Features:

1. **Import schemas**  
   Add OpenAPI documents from the APIs you work with — Oatty builds a unified command catalog.

2. **Describe your goal**  
   Type what you want to accomplish (e.g. "move my app and database from one platform to another"). Oatty searches the
   catalog and suggests relevant commands or workflows.

3. **Review and run safely**  
   See a clear preview of the suggested steps. Validation checks highlight potential issues. Confirm before execution.
   Monitor logs in real time and adjust as needed.

## Featured Use-Case Examples (Broad Appeal)

**Section Title:** See It in Action

1. **Orchestrate tasks across services**  
   "Set up staging environments across multiple providers" → suggested steps, preview, confirmation, execute.

2. **Platform migrations** (concrete example)  
   "Move my Postgres database and web app from Vercel to Render" → imports schemas → discovers export/import/deploy
   commands → suggests workflow → you review steps → confirm and run.

3. **Bulk updates & rotations**  
   "Update connection strings and rotate keys across several services"

4. **Reporting & compliance flows**  
   "Pull logs from multiple APIs and generate a summary"

5. **AI-assisted operations**  
   Expose your catalog to Claude/Gemini/etc. via MCP — with the same preview and confirmation controls.

**Visual treatment:**  
Hero card for the migration example (GIF/screenshots showing preview + confirmation + logs). Smaller cards for others.

## Features Section Order

1. **Natural Language Assistance + Safe Execution** (lead full-width card)
    - Screenshot: prompt → suggestion → preview/validation → confirmation
    - Emphasize: describe goal → get suggestions → you control execution

2. Interactive TUI with fuzzy search
3. Schema-based command catalog
4. Reusable workflows
5. Value providers for smart inputs
6. MCP server (expose to AI tools)
7. MCP client (integrate other tools)
8. Execution logs & history

## Installation / Quick Start Teaser

**Try it yourself block** (realistic/current syntax):
