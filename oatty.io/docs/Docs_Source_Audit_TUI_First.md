# Docs Source Audit (TUI-First)

This artifact captures source-code-grounded documentation notes to keep docs aligned with shipped behavior.

## Authoring principle

- TUI is the primary user experience and should be documented first.
- CLI documentation should be presented as a fallback and automation surface.
- Before drafting any docs page, perform a source audit for that page's feature surface.

## Source-of-truth references

- TUI routes and main interaction orchestration:
  - `crates/tui/src/ui/main_component.rs`
  - `crates/tui/src/ui/components/nav_bar/state.rs`
- TUI feature modules:
  - `crates/tui/src/ui/components/palette/palette_component.rs`
  - `crates/tui/src/ui/components/browser/browser_component.rs`
  - `crates/tui/src/ui/components/library/library_component.rs`
  - `crates/tui/src/ui/components/workflows/workflows_component.rs`
  - `crates/tui/src/ui/components/workflows/input/input_component.rs`
  - `crates/tui/src/ui/components/workflows/run/run_component.rs`
  - `crates/tui/src/ui/components/plugins/plugin_editor/plugin_editor_component.rs`
  - `crates/tui/src/ui/components/mcp_server/mcp_server_component.rs`
  - `crates/tui/src/ui/components/logs/logs_component.rs`
- CLI fallback behaviors:
  - `crates/cli/src/main.rs`

## Foundational features (document early)

1. Primary navigation and focus model
   - Main views: Library, Command Runner, Find, Workflows, MCP Plugins, MCP HTTP Server.
   - Global focus movement uses `Tab` / `BackTab`.
   - Global affordances include logs toggle and theme picker (`Ctrl+L`, `Ctrl+T`).
2. Command execution flow
   - Command runner (`Palette`) with completions, accept/execute path, and help affordances (`F1`).
   - Command browser (`Find`) for searchable command exploration and send-to-palette workflow.
3. Library and catalogs
   - Import/remove catalog flows.
   - Catalog metadata and base URL management.
4. Workflows lifecycle
   - Workflow list with import/remove.
   - Pre-run input collection (including manual entry path).
   - Run view with status, step outputs, and controls (pause/resume/cancel).
5. Plugin and MCP surfaces
   - Plugin editor with validate/save.
   - MCP HTTP server start/stop and auto-start controls.
6. Logs and inspection
   - Log filtering, details modal, copy behavior, and pretty/raw JSON toggle.

## CLI fallback features (document as secondary path)

1. Direct command execution via generated CLI command tree.
2. Import command with source auto-detection:
   - catalog vs workflow
   - local path vs URL
3. Workflow command group:
   - `workflow list`
   - `workflow preview`
   - `workflow run` (with input overrides)
4. Structured JSON output options for automation.

## Progressive disclosure candidates (advanced sections)

1. Provider-backed input resolution and dependency semantics (`depends_on`) in workflow authoring.
2. Workflow validation and structured failure diagnostics.
3. Deterministic orchestration controls:
   - repeat/poll semantics
   - in-flight run controls (pause/resume/cancel)
4. MCP-assisted workflow authoring and command discovery:
   - output metadata usage (`output_fields` / `output_schema`) for step chaining.
5. Secrets and environment strategy for local + CI execution patterns.
6. Multi-vendor workflow composition patterns and guardrails.

## Docs page planning notes (with screenshot affordances)

Each page should reserve explicit screenshot slots and call out what to capture.

### Quick Start

- Focus:
  - Launch TUI
  - Import catalog
  - Discover and run command
  - Run workflow
- Screenshot affordances:
  - nav and command runner visible
  - command browser interaction
  - workflow list and run view status

### Learn: Getting Oriented

- Focus:
  - Layout model (nav, content, logs, hints)
  - keyboard and mouse affordances
  - focus traversal expectations
- Screenshot affordances:
  - full-screen layout with callouts for each pane
  - logs open vs closed state

### Learn: Search and Run Commands

- Focus:
  - palette completion cycle and acceptance flow
  - browser-to-palette handoff
  - command help usage (`F1`)
- Screenshot affordances:
  - suggestion list open
  - browser with inline help panel
  - executed command result state

### Learn: Library and Catalogs

- Focus:
  - import/remove catalog
  - enabled state toggling
  - base URL selection management
- Screenshot affordances:
  - library table with selected catalog
  - base URL list controls

### Learn: Workflows Basics

- Focus:
  - list -> input collection -> run flow
  - run status transitions and step details
  - limitations called out (resume/re-run gap)
- Screenshot affordances:
  - workflow list and selected item
  - input collector state
  - run view with step table and controls

### Learn: Plugins and MCP Server

- Focus:
  - plugin creation/validation/save flow
  - MCP HTTP server operations
- Screenshot affordances:
  - plugin editor form + validation messaging
  - MCP server status panel

### Guides (task-driven)

- Focus:
  - import catalog guide
  - run first workflow guide
  - provider-backed inputs guide
  - NL human-in-the-loop guide
- Screenshot affordances:
  - one end-to-end screenshot sequence per guide step
  - include pre-action and post-action states

### Reference

- Focus:
  - CLI command patterns (fallback)
  - TUI interactions and keybindings
  - config/environment references
- Screenshot affordances:
  - optional compact UI snapshots next to keybinding tables

## Pre-write checklist for each docs page

1. Confirm source files to audit for the page scope.
2. Confirm foundational behavior vs advanced behavior split.
3. Capture feature limitations/tradeoffs for honest framing.
4. Reserve screenshot slots with named capture targets.
5. Include "What you'll learn" card and next-step links.
