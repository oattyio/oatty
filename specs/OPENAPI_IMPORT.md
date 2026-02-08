# OPENAPI_IMPORT.md

As-built specification for OpenAPI import into runtime catalog state.

## Scope

Primary implementation files:
- `/Users/justinwilaby/Development/next-gen-cli/crates/registry/src/openapi_import.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/registry-gen/src/openapi.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/mcp/src/server/catalog.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/ui/components/library/library_component.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/tui/src/cmd.rs`

## Shared import pipeline

Implemented import flow:
1. Parse source as JSON or YAML.
2. Run OpenAPI preflight validation.
3. Generate catalog/manifest via registry-gen.
4. Apply overrides (title/vendor/base_url/enabled).
5. Insert into registry (with optional overwrite semantics).
6. Persist registry config.

## Validation behavior

Preflight validation checks are required before generation.
Violations are returned as structured validation metadata where supported (notably MCP endpoints).

## Overwrite behavior

When overwrite is enabled and catalog id exists:
- Existing catalog entry is removed/replaced through shared overwrite path.

## MCP catalog tools integration

MCP catalog endpoints reuse the same import logic for:
- validate
- preview
- import
- enable/disable
- remove

## TUI/Library integration

Library import and mutation flows are expected to refresh library projections from the updated runtime registry state.

## Correctness notes

- This is as-built behavior only.
- Keep aligned with `registry/openapi_import.rs`, `registry-gen/openapi.rs`, and `mcp/server/catalog.rs`.


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/MCP_CATALOG_TOOLS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/LIBRARY.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/COMMANDS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/VALUE_PROVIDERS.md`
