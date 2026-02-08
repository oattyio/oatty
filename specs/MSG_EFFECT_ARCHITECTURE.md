# MSG_EFFECT_ARCHITECTURE.md

As-built specification for TUI state/event/effect flow.

## Scope

Primary implementation files:
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/app.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/cmd.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/runtime.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/types/src/lib.rs`

## Runtime flow

1. Input events are dispatched to components.
2. Components mutate local state and may return `Vec<Effect>`.
3. `Msg` events are processed by `App::update` and component `handle_message` hooks.
4. Effects are translated by `run_from_effects` into `Cmd` values (or local side operations).
5. `run_cmds` executes command side effects (sync and async).
6. Async completions are converted back to `Msg::ExecCompleted`/related messages.

## Message role

`Msg` represents inbound events that require centralized handling, such as:
- ticks
- resize
- async execution outcomes
- higher-level UI control messages

`App::update` is the authoritative state reducer for these messages.

## Effect role

`Effect` represents intent to perform actions beyond local component mutation, including:
- command execution
- clipboard writes
- routing/modal changes
- plugin and workflow run controls
- catalog/workflow operations

Effects are intentionally declarative; execution occurs in `cmd.rs`.

## Command role

`Cmd` is the executable layer used by `run_cmds`.
Implemented command categories include:
- HTTP/MCP execution dispatch
- provider fetch dispatch
- registry/library mutation operations
- plugin operations
- workflow run control plumbing

## Architectural characteristics

- Local-first component mutation is used for many UI interactions.
- Message/effect/command separation is used for cross-cutting or side-effecting operations.
- Asynchronous work is routed through join handles and returns `ExecOutcome` values.
- Logging is integrated at app level and side-effect boundaries.

## Known pragmatic deviations

- Some effect handling performs immediate local state updates in addition to command translation.
- Not all component interactions are pure TEA-style reducers; this is intentional for TUI ergonomics.

## Correctness notes

- This file is as-built. Update it when message enums, effect routing, or command execution topology changes.
- Keep examples consistent with `types::Msg`, `types::Effect`, and `cmd.rs`.


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/FOCUS_MANAGEMENT.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/UX_GUIDELINES.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/WORKFLOW_TUI.md`
