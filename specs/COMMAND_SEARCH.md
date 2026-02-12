# COMMAND_SEARCH.md

As-built specification for command search behavior.

## Scope

Primary implementation files:
- `/Users/justinwilaby/Development/next-gen-cli/crates/registry/src/search.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/mcp/src/server/core.rs`

## Search implementation

Implemented behavior:
- Search is in-memory and queries the live command registry directly.
- No SeekStorm or external index backend is used.
- Search uses fuzzy token scoring with additional ranking bonuses:
  - token coverage score
  - canonical-id contains query bonus
  - canonical-id prefix bonus
- Empty/whitespace queries return empty results.

## Search haystack fields

The score input includes:
- canonical command id
- command summary
- positional arg names/help
- flag names/descriptions
- catalog/vendor metadata when available

## Result shape

Search returns structured `SearchResult` entries including:
- `canonical_id`
- `summary`
- `execution_type`
- `http_method` (if applicable)

For MCP `search_commands`, optional metadata enrichment is supported through `include_inputs`:
- `none` (default): no input/output metadata augmentation.
- `required_only`: adds required input metadata and compact `output_fields` when output schema can be projected.
- `full`: adds full positional/flag metadata, `output_schema`, and compact `output_fields`.

`output_fields` are a compact projection intended for workflow chaining:
- object outputs: top-level property keys.
- array-of-object outputs: top-level keys prefixed as `[].<field>`.

## MCP tool exposure

MCP `search_commands` delegates to the same search handle.

Recommended MCP lookup pattern:
- Use `search_commands` for candidate discovery.
- Use `get_command` with a selected `canonical_id` for deterministic, exact schema/details.
- Reserve `include_inputs=full` for cases that need complete argument/flag/output schema inspection.

## Correctness notes

- This is as-built behavior only.
- Keep aligned with `crates/registry/src/search.rs` and `search_commands` handlers in MCP core.


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/COMMANDS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/AUTOCOMPLETE.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/MCP_CATALOG_TOOLS.md`
