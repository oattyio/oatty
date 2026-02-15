# Workflow TUI (As-Built)

## Scope
This spec documents the currently implemented workflow UX in the TUI.

## Route Flow
- Workflow list route (`Route::Workflows`)
- Input collection route (`Route::WorkflowInputs`)
- Run route (`Route::WorkflowRun`)
- Selector/manual entry modals via `Modal::WorkflowCollector` and `Modal::ManualEntry`

## Implemented Components
- Workflow list: `workflows_component`
- Input list/status: `input/input_component`
- Provider selector + inline manual override: `collector/collector_component`
- Manual entry modal: `collector/manual_entry/manual_entry_component`
- Run timeline/status: `run/run_component`

## Workflow List UX
- Search input with cursor-aware editing.
- Filtered list with keyboard and mouse navigation.
- Enter opens input session for selected workflow.
- Workflow import checks `requires.catalogs[]` before persisting imported manifests.
- When required catalogs are missing and importable metadata exists, a confirmation modal prompts to install dependencies first.
- Confirmed installation stages catalog imports through existing catalog import effects, then proceeds with workflow import.

## Input Collection UX
- Required/unresolved tracking based on run state.
- Input defaults applied before selection (including history defaults where configured).
- Provider-backed selection when provider metadata exists.
- Manual entry path available for direct value entry.
- Manual entry can surface per-input guidance (`hint`) and concrete sample values (`example`) when defined.

## Selector UX
- Filterable selector table.
- Selection staging and apply/cancel actions.
- Inline manual override input is always available in selector mode.
- Apply source is explicit (`table` vs `manual`) and follows last interaction.
- Manual override value remains visible even after table selection; status line indicates what will be applied.
- Refresh and provider error handling (`manual`/`cached`/`fail` behavior surfaced in state).
- Empty provider result fallback for workflow inputs remains in collector and shifts focus to manual override.
- Manual override supports JSON file selection (`Ctrl+O`) via shared file picker (`.json`) and returns to collector with loaded content.

## Run UX
- Run session state and lifecycle updates are rendered in run view.
- Step statuses and logs are updated from workflow run events.
- Run control messages (pause/resume/cancel) are wired through workflow state and engine control channels.

## Source Alignment
- `crates/tui/src/ui/components/workflows/workflows_component.rs`
- `crates/tui/src/ui/components/workflows/state.rs`
- `crates/tui/src/ui/components/workflows/input/input_component.rs`
- `crates/tui/src/ui/components/workflows/collector/collector_component.rs`
- `crates/tui/src/ui/components/workflows/collector/manual_entry/manual_entry_component.rs`
- `crates/tui/src/ui/components/workflows/run/run_component.rs`


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/WORKFLOWS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/VALUE_PROVIDERS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/MCP_WORKFLOWS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/TABLES.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/UX_GUIDELINES.md`
