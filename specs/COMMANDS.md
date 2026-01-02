# Commands Architecture

## Purpose
The command system powers both the keyboard-driven palette and automation features that run Oatty API or MCP-backed operations. This document captures the behavior as implemented across the registry, TUI palette, and execution pipeline, and serves as the canonical reference for identifier syntax, argument semantics, and execution behavior.

## Identifier Syntax

- User-facing everywhere (palette input, workflows `run`, providers): `group name`
  - Examples: `apps list`, `apps addons:create`, `pipelines couplings:create`, `teams apps:collaborators:info`
- The `name` portion can itself include `:` for nested resources/actions; only the first separator between `group` and `name` is a space.
- Quoting and whitespace follow shell-like rules in the palette; see Parsing below.

Implementation notes:
- The underlying manifest stores `group` and `name` as separate fields; the combined identifier is reconstructed by pairing them.

## Command Definitions

- **Source of truth:** `oatty_registry::CommandRegistry` loads command metadata from `registry-manifest.json` files (see `crates/registry/src/models.rs`). Synthetic MCP tools are merged through `CommandRegistry::insert_synthetic`, then deduplicated.
- **CommandSpec:** (`crates/types/src/lib.rs:364`) captures the user-facing command surfaces.
  - `group`: primary resource bucket (e.g., `"apps"`).
  - `name`: subcommand token paired with the group in palette input (e.g., `"list"`, `"addons:list"`). The CLI always references commands as `<group> <name>`.
  - `summary`: short description for palette/tooling copy.
  - `positional_args`: ordered `PositionalArgument`s; individual entries can reference a `ValueProvider`.
  - `flags`: list of `CommandFlag`s enumerating required/optional switches, their types, enum values, defaults, descriptions, and optional providers.
  - `execution`: discriminated union (`CommandExecution`) describing the backend.
- **Execution metadata:**
- `CommandExecution::Http(HttpCommandSpec)` includes method, raw path template (with `{placeholder}` slots), required `base_url`, declared pagination ranges, and optional output schema.
  - `CommandExecution::Mcp(McpCommandSpec)` stores plugin/tool identifiers, optional authentication summary, output schema, and render hint.
  - Default execution is HTTP; helper constructors `CommandSpec::new_http` and `CommandSpec::new_mcp` wire the appropriate variant.
- **Provider contracts:** Manifest entries also include `provider_contracts` keyed by command id. The TUI consumes these via the registry for palette value providers and workflow collectors.

### Supporting Types
- `CommandFlag` (`crates/types/src/lib.rs:215`) tracks long/short names, requirement, declared type, enum values, defaults, optional description, and provider binding.
- `PositionalArgument` (`crates/types/src/lib.rs:286`) records the name, optional help text, and optional provider reference.
- `SchemaProperty` (`crates/types/src/lib.rs:303`) summarizes output schemas for downstream rendering.

### Naming Derivation (registry-gen)
- `crates/registry-gen/src/openapi.rs` (`derive_command_group_and_name`) produces the `(group, name)` pair. The first concrete path segment becomes the group, while remaining segments join with `:` and append the HTTP-derived action (`list`, `info`, `create`, `update`, `delete`).
- Examples:
  - `/apps` + `GET` → group `apps`, name `list`.
  - `/apps/{app}/addons` + `GET` → group `apps`, name `addons:list`.
  - `/pipelines/{pipeline}/couplings` + `POST` → group `pipelines`, name `couplings:create`.
- The manifest therefore never stores `"apps:list"` as a single field; consumers reconstruct that identifier by pairing `group` and `name` when needed.

## Palette Interaction Model

- **State container:** `PaletteState` (`crates/tui/src/ui/components/palette/state.rs`) manages input text, cursor position, focus flags, ghost hints, suggestion lists, provider loading indicator, history buffer, and transient errors.
- **Input parsing:** User text is tokenized with shell-like lexing (`lex_shell_like` / `lex_shell_like_ranged`) so quoting and escaping match CLI expectations.
- **Command resolution:** Until two tokens match a known command, suggestions come from fuzzy-ranked `CommandSpec` entries (`SuggestionEngine::suggest_when_unresolved`).
- **Suggestion engine:** `SuggestionEngine::build` orchestrates contextual suggestions:
  - Resolves the active command once `<group> <command>` tokens align with registry metadata.
  - Detects pending flag values and requests value suggestions (enum literals plus provider-backed results) while tracking asynchronous provider loading.
  - Supplies positional argument suggestions for the next unmet positional, including provider results, and suppresses duplicates of the user’s current token.
  - Surfaces required flag suggestions before optional ones; when the user is typing a flag token suggestions switch to flag mode immediately.
  - Emits an inline `"--"` hint when more flags remain, otherwise prompts `" press Enter to run"` via ghost text when requirements are satisfied.
- **Value providers:** Providers implement `ValueProvider::suggest` and are invoked for both positional and flag contexts. When a provider is registered but has not yet returned data, the palette shows a `"loading more…"` placeholder and marks the provider as loading to keep the popup visible.
- **History & focus:** Completed commands are pushed into `PaletteState::push_history_if_needed`. Focus management uses `rat_focus::FocusFlag` so the palette integrates with the global focus ring. History browsing preserves a draft of the in-progress input while paging through previous commands.

## Command Execution Semantics

- **Trigger path:** Hitting `Enter` (or routing through other components) emits `Effect::Run`, which `run_from_effects` forwards to `start_palette_execution` (`crates/tui/src/cmd.rs:903`).
- **Validation pipeline (`validate_command`):**
  1. Trim and tokenize the palette input; require at least `<group> <command>`.
  2. Resolve the `CommandSpec` from the registry (`find_by_group_and_cmd`).
  3. Split trailing tokens into flags/positionals via `parse_command_arguments`.
  4. Enforce positional count and required flag/value presence through `validate_command_arguments`.
  5. Coerce flag values into a JSON body (`build_request_body`), respecting declared types and enum constraints.
  6. Persist execution context (`persist_execution_context`) storing last spec/body, initial range header, pagination history, and command history entry.
- **Dispatch (`execute_command`):**
  - **HTTP:** Resolve templated path segments with positional arguments (`oatty_util::resolve_path`), then enqueue `Cmd::ExecuteHttp`. Pagination headers are derived from request body via `build_range_header_from_body`.
  - **MCP:** Inject positional arguments into the body map and enqueue `Cmd::ExecuteMcp`.
- **Command dispatcher (`run_cmds`):**
  - Converts `Cmd` variants into side effects, spawning asynchronous tasks for network/MCP execution (`spawn_execute_http` / `spawn_execute_mcp`) and returning `ExecOutcome`s once tasks complete.
  - Clipboard writes, palette errors, and plugin management commands execute immediately and log via `App::logs`.
  - Pagination effects reuse the stashed `last_spec`/`last_body`, mutating the stored Range overrides before reissuing the HTTP command.
- **Outcomes:** Background handles resolve to `ExecOutcome` variants (`crates/types/src/lib.rs:604`), which update UI panels (tables, logs, detail modals). HTTP outcomes capture serialized body, pagination metadata, and whether table rendering is recommended; MCP outcomes return log summaries plus structured payloads.

## Palette Integration with Other Surfaces

- `Effect::SendToPalette` populates the palette with a formatted command (`"group name"`) when, for example, a workflow browser row is activated. The handler sets the input, moves the cursor to the end, rebuilds suggestions, and leaves the palette focused for immediate edits.
- The palette is the default route (`Route::Palette`) when the application starts (`crates/tui/src/app.rs:171`). Other components can switch back via `Effect::SwitchTo(Route::Palette)` to keep command editing consistent.
- Logs capture every executed palette command (`LogEntry::Text`), providing historical context within the TUI.

## Known Constraints (Implementation Reality)

- Command ranking hinges on fuzzy scores and manifest ordering; there is no additional heuristic weighting beyond what `fuzzy_score` provides.
- Provider-backed suggestions display a loading sentinel while awaiting results; the palette does not currently cache provider values between commands.
- MCP commands share the same validation pipeline as HTTP commands; richer schema-driven validation of MCP arguments remains future work.

## Argument & Request Semantics

- Positionals → path placeholders
  - Each `PositionalArgument` maps by name to a `{placeholder}` in the HTTP path.
  - At runtime, any matching `with` entry is consumed into the path map first.
- Remaining `with` vs `body`
  - For GET/DELETE: remaining `with` entries become query parameters.
  - For POST/PUT/PATCH: if a JSON `body` is supplied, that is used; otherwise the remaining `with` entries are serialized as the request body object.
- Flags
  - Flags in `CommandSpec.flags` define types (string, number, boolean, enum, array, object), requirement, defaults, and optional providers.
  - The palette validates required flags and coercions before dispatch. Enum flags surface enum values in suggestions.
- Providers on arguments
  - Both positional arguments and flags may declare `provider` metadata. The palette and workflow collectors call providers to populate suggestions/selectors.
- Range headers
  - When a body contains pagination hints, `build_range_header_from_body` derives a `Range` header and `strip_range_body_fields` removes those pagination fields from the JSON body before sending.

## Parsing & Quoting Rules

- Palette input is tokenized with shell-like lexing (`lex_shell_like`, `lex_shell_like_ranged`). Use quotes to include spaces in argument values.
- Identifier is resolved from the first two tokens only: `<group> <name>`; any additional tokens are parsed as flags/positionals.

## HTTP vs MCP Execution

- HTTP (default)
  - Uses `HttpCommandSpec { method, path, base_url, pagination, output_schema }`.
  - Path placeholders are resolved, query/body are shaped as above, request is executed via `OattyClient`.
  - Results are packaged into an HTTP `ExecOutcome` including status code, optional `Content-Range`, and parsed JSON payload.
- MCP
  - Uses `McpCommandSpec { plugin_name, tool_name, auth_summary, output_schema, render_hint }`.
  - MCP-backed commands run through the MCP pipeline; palette validation is identical, but the backend call is delegated to the MCP client.

## Errors & Validation

- Identifier errors: unparseable identifiers produce `invalid run/provider identifier: <text>`.
- Resolution errors: unknown `<group> <name>` result in a lookup error from `find_by_group_and_cmd`.
- Argument validation errors include:
  - Missing required positional/flag
  - Type coercion failures (e.g., expecting number but got string)
  - Enum mismatches (value not in declared set)
- Provider errors are surfaced inline in the palette/workflow collector; long-running fetches display a loading sentinel.

## Examples

- Simple list
  - Identifier: `apps list`
  - GET /apps → no positionals; optional query flags become `?key=value`.
- Nested resource
  - Identifier: `apps addons:list`
  - GET /apps/{app}/addons → positional `app` maps to path; remaining flags become query.
- Create with body
  - Identifier: `apps builds:create`
  - POST /apps/{app}/builds → positional `app` maps to path; JSON body includes `source_blob` fields.

## Migration & Compatibility Notes

- Canonical user-facing form: space-separated `group name`.
- The legacy colon form (`group:name`) is no longer accepted in this release. Update any workflows, scripts, or docs to the space-separated form.
