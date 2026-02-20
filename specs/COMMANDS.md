# COMMANDS.md

As-built specification for command definition, discovery, and execution.

This document describes currently implemented behavior only.

## Scope

Primary implementation files:

- `/Users/justinwilaby/Development/next-gen-cli/crates/types/src/lib.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/registry/src/models.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/registry/src/search.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/registry/src/openapi_import.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/palette/suggestion_engine.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/palette/state.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/cmd.rs`

## Command identity

- Canonical user-facing command form is two tokens:
    - `<group> <name>`
- The `name` token may contain `:` for nested actions.
    - Example: `apps addons:list`
- Palette and command execution resolve a command from the first two shell-like tokens.

## Command model

Commands are represented by `CommandSpec` and include:

- `group`
- `name`
- `summary`
- `positional_args`
- `flags`
- `execution` (`HTTP` or `MCP`)

Execution variants:

- HTTP: method/path/base URL metadata and optional output schema.
- MCP: plugin/tool execution metadata and optional output schema/render hints.

## Registry as source of truth

- Runtime commands are held in `CommandRegistry`.
- Catalog manifests are loaded from registry config.
- MCP tool-derived commands are merged into the same registry model.
- OpenAPI imports use shared service logic in `openapi_import.rs` and persist into registry config + catalog manifest
  storage.

## Palette autocomplete integration

`SuggestionEngine` + `PaletteState` implement command-time suggestion behavior:

- unresolved command fuzzy suggestions
- flag suggestions (required to be surfaced before optional)
- positional suggestions
- enum and provider-backed value suggestions
- provider loading sentinel (`loading more…`) and failure handling

Accepted suggestions update input contextually:

- command suggestions set/replace command portion
- flag/value suggestions replace or append based on token context
- positional suggestions replace current positional token or append in positional slot

## Parsing and validation path

On run (`Effect::Run`), `cmd.rs` executes:

1. tokenize shell-like input
2. require at least two tokens (`group` + `name`)
3. resolve command via registry lookup
4. parse arguments via `CommandSpec::parse_arguments`
5. persist pending execution + history context
6. dispatch based on execution type

Validation errors are surfaced back to palette as user-facing errors.

## Dispatch semantics

### HTTP commands

- Enqueued as `Cmd::ExecuteHttp`.
- Execution occurs asynchronously.
- Result emitted as HTTP `ExecOutcome` and routed back to app state/logs.

### MCP commands

- Parsed user args/flags are assembled into a JSON object.
- Enqueued as `Cmd::ExecuteMcp`.
- Execution delegated to MCP plugin engine.
- Result emitted as MCP `ExecOutcome` and routed back to app state/logs.

## Search implementation

- Command search is in-memory (no external index dependency).
- Implemented by `SearchHandle` in `/Users/justinwilaby/Development/next-gen-cli/crates/registry/src/search.rs`.
- Uses `oatty_util::fuzzy_score` over a synthesized haystack containing:
    - canonical id
    - summary
    - positional/flag names and descriptions
    - catalog metadata (title/description/vendor) when available
- Base search results include canonical id, summary, execution type, and optional HTTP method.
- MCP `search_commands` can enrich result payloads with `include_inputs`:
    - `required_only`: required input metadata and compact `output_fields`.
    - `full`: full positional/flag metadata, `output_schema`, and compact `output_fields`.
- `output_fields` provide a compact, chain-friendly list of top-level output keys for object and array-of-object
  schemas.

## TUI integration points

- `Effect::SendToPalette` injects a command identifier into palette input and positions cursor.
- `Effect::Run` is the entry point from palette to command execution pipeline.
- Successful runs are recorded in palette history and persisted via `HistoryStore`.

## Constraints (current implementation)

- Command resolution requires first two tokens to map to a known command.
- Suggestion ranking is fuzzy-score driven; no semantic reranker layer is implemented.
- Provider-backed suggestions depend on provider availability and may be temporarily loading.
- HTTP and MCP share the same initial parse/validation entry path, then diverge at dispatch.

## Correctness notes

- This file is as-built. Update it in the same PR when command model or execution behavior changes.
- Keep planned/future command features in separate planning specs, not here.

## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/AUTOCOMPLETE.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/COMMAND_SEARCH.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/OPENAPI_IMPORT.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/VALUE_PROVIDERS.md`
