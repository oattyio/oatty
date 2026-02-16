# Value Providers (As-Built)

## Scope
Implemented value-provider behavior across:
- `crates/engine/src/provider`
- `crates/registry-gen/src/openapi.rs`
- palette/workflow input consumers in `crates/tui`

## What Exists
- Provider metadata is attached to command fields (`CommandFlag` / `PositionalArgument`) during OpenAPI generation and provider inference.
- Runtime suggestions are produced by `ProviderRegistry` via `ValueProvider::suggest`.
- Suggestion responses can include:
  - ready items
  - a pending fetch plan for async completion
- Provider fetches are cached with TTL and deduplicated for in-flight requests.

## Provider Inputs and Bindings
- Suggestion calls accept resolved `inputs` so provider arguments can be bound from already-entered command/workflow values.
- Binding and identifier canonicalization are handled by engine/provider helper modules.

## Where Providers Are Used
- Palette suggestions (`suggestion_engine`)
- Workflow collector/provider selector flow

## Workflow Collector Behavior (Current)
- Provider-backed input selection and manual override share the same collector modal.
- Empty provider result sets for workflow inputs do not force a separate modal; collector remains open and focuses manual override.
- When provider mappings are ambiguous or value extraction fails, users can manually override directly in collector.
- Manual override supports JSON file loading through the shared file picker (`Ctrl+O`, `.json`) and resumes collector flow with parsed content/error messaging.

## Current Constraints
- Provider behavior is driven by inferred metadata and available contracts; no separate external provider registry file is required.
- Advanced provider chaining semantics beyond current binding resolution are not separately implemented.

## Source Alignment
- `crates/engine/src/provider/value_provider.rs`
- `crates/engine/src/provider/registry.rs`
- `crates/registry-gen/src/openapi.rs`
- `crates/tui/src/ui/components/palette/suggestion_engine.rs`
- `crates/tui/src/ui/components/workflows/collector/collector_component.rs`


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/VALUE_PROVIDER_REGISTRY.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/WORKFLOWS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/WORKFLOW_TUI.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/AUTOCOMPLETE.md`
