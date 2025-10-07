# Workflow TUI Implementation Plan

## Summary
- Ground work in `specs/WORKFLOWS.md` §§1–6 for workflow authoring semantics (inputs, steps, dependent providers, output contracts).
- Align UI behaviors with `specs/WORKFLOW_TUI.md` §§2.1–2.4 (workflow picker, provider-backed input collection, selector layouts) and `specs/WORKFLOW_VALUE_PROVIDERS_UX.md` §§1–4 (provider declaration forms, dropdown interactions, error fallbacks).
- Targeted code touchpoints span registry generation (`crates/registry-gen`), shared types (`crates/types`), runtime orchestration (`crates/engine`), and Ratatui components (`crates/tui`).

## Work Unit 1: Workflow Schema Ingestion and Registry Exposure
- **Audit workflow sources**: Inspect `workflows/` YAML files and confirm coverage of scenarios described in `specs/WORKFLOWS.md` §§2–3; document gaps for follow-up samples.
- **Define workflow types**: Introduce strongly typed workflow structures (inputs, select configuration, defaults, steps, output contracts) in `crates/types` (likely a new `workflow` module) with serde derives matching §1 authoring rules.
- **Extend registry generator**: Update `crates/registry-gen/src/lib.rs` and `io.rs` to load workflow YAML, validate against the new types, and surface errors with actionable context (yaml path, offending key).
- **Embed in manifest**: Adjust the build pipeline so `Registry` (see `crates/registry/src/models.rs`) carries workflows alongside `CommandSpec`, ensuring gating via `FEATURE_WORKFLOWS` as flagged in repo guidelines.

## Work Unit 2: Provider Metadata and Auto-Mapping Infrastructure
- **Provider schema enrichment**: Model `select`, `mode`, `join`, `provider_args`, `placeholder`, and `on_error` fields per `specs/WORKFLOWS.md` §§1.1–1.7 and `specs/WORKFLOW_VALUE_PROVIDERS_UX.md` §1 within the new workflow types.
- **Registry metadata**: Extend `crates/registry-gen/src/provider_resolver.rs` to emit provider argument contracts and return field descriptors per `specs/WORKFLOWS.md` §6 (requires/returns tags) and add serialization into the manifest.
- **Shared provider types**: Expand `crates/types/src/provider` (or new module) so runtime consumers can distinguish command-backed providers, workflow outputs, and manual fallback metadata; include caching policies (`cache_ttl_sec`) and default sources.
- **Auto-mapping heuristics**: Design and stub resolver APIs in `crates/engine` (probably a new `providers` module) implementing steps from `specs/WORKFLOWS.md` §5.3 (explicit path, heuristics, picker fallback) and the tag matching rules using registry contracts.
- **Cache layer**: Specify caching interface (probably leveraging `tokio::sync::RwLock` + timestamps) that honors TTL and manual refresh triggers noted in `specs/WORKFLOW_TUI.md` §2.2 (status badges like “loaded 24s ago”).

## Work Unit 3: Workflow Runtime Orchestration
- **Execution model**: Extend `crates/engine` (likely new `workflow` module) to interpret workflow definitions, resolve inputs, execute steps sequentially, and surface `Msg`/`Effect` events compatible with existing runner patterns (`heroku_types::Msg`, `Effect`).
- **Dependent provider binding**: Implement evaluation of `${{ inputs.* }}` and `${{ steps.*.output.* }}` expressions, respecting `on_missing` policies from `specs/WORKFLOWS.md` §5.4 and persisting user overrides during a session.
- **Repeat/loop support**: Model `repeat` directives (see `specs/WORKFLOWS.md` §2.2) with cancellation hooks and status propagation back into the TUI (update `ExecOutcome` or introduce workflow-specific progress struct).
- **Telemetry hooks**: Define structured logs or analytics (reusing `crates/util`) to record provider usage, manual overrides, and outcome statuses for future UX insights, aligning with resiliency goals in §1 Principles.

## Work Unit 4: TUI Workflow Surfaces
- **Workflow picker view**: Create a `workflows` component folder under `crates/tui/src/ui/components/` and implement `WorkflowPickerComponent` that renders the layout in `specs/WORKFLOW_TUI.md` §2.1, integrating search, selection, and nav instructions.
- **State integration**: Extend `crates/tui/src/app.rs` to host `WorkflowState` (modal visibility, selected workflow, cached provider results) and wire focus flags via `FocusBuilder`, mirroring existing `BrowserState` patterns.
- **Navigation wiring**: Update `crates/tui/src/lib.rs` nav routing to add a “Workflows” entry to `VerticalNavBarState`, ensuring Tab cycling, ESC handling, and route transitions reuse existing component trait hooks.
- **Guided input collector**: Implement `WorkflowInputComponent` rendering provider-backed inputs per §2.2 (collapsible sections, refresh keybinding, manual override button), coordinating with provider cache and auto-mapping status badges.

## Work Unit 5: Selector and Detail Enhancements
- **Table upgrades**: Enhance `crates/tui/src/ui/components/table` to support selection markers, multi-select chips, TTL badges, and inline provider metadata per `specs/WORKFLOW_TUI.md` §§2.3–2.4; expose configuration hooks that `WorkflowInputComponent` can toggle.
- **Detail pane integration**: Extend Key/Value viewer (likely `crates/tui/src/ui/components/browser` or dedicated detail view) to highlight schema tags (`app_id`, `enum`) and enumerated literals as described in both workflow specs.
- **Field picker modal**: Add a modal component for the JSON field picker defined in `specs/WORKFLOWS.md` §5.2, reusing tree navigation patterns from the browser component and persisting selections into workflow state.
- **Error and fallback UX**: Implement `on_error` handling surfaces (manual entry prompts, cached badges) consistent with `specs/WORKFLOW_VALUE_PROVIDERS_UX.md` §2 and §3, ensuring accessibility cues (focus, shortcuts) follow existing theme helpers.

## Work Unit 6: Validation, Tooling, and Documentation
- **Unit and integration tests**: Add serde round-trip tests for workflow types (`crates/types`), parser tests in `crates/registry-gen/tests`, and TUI state reducers/unit tests for provider caching logic; follow testing guidance in repo instructions.
- **Feature flagging**: Gate TUI entry points and engine hooks behind `FEATURE_WORKFLOWS`, providing graceful no-op behavior when disabled (update CLI entry in `crates/cli/src/main.rs`).
- **Documentation pass**: Update `specs/WORKFLOWS.md` with any clarifications discovered (or annotate separate CHANGELOG), add developer-focused README paragraphs in `workflows/README.md`, and ensure `ARCHITECTURE.md` cross-links new modules.
- **Operational checklist**: Document validation commands (`cargo fmt`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`) and manual UX verification steps (run `cargo run -p heroku-cli` with `FEATURE_WORKFLOWS=1`, set `HEROKU_LOG=debug`).

## Open Questions and Dependencies
- Confirm whether workflow execution should reuse existing `cmd::run_cmds` pipeline or introduce a dedicated executor; coordinate with future `heroku_mcp` integration.
- Determine persistence scope for provider selection overrides (per session vs. persisted config) before finalizing cache storage strategy.
- Validate manifest size impact when embedding workflows and provider contracts; may require compression adjustments in `crates/registry-gen`.
- Align keyboard shortcuts with global policy (check `specs/UX_GUIDELINES.md`) to avoid conflicts with existing components.
