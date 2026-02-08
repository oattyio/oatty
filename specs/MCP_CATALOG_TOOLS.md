# MCP_CATALOG_TOOLS.md

As-built specification for MCP catalog-management tools.

## Scope

Primary implementation files:
- `/Users/justinwilaby/Development/next-gen-cli/crates/mcp/src/server/core.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/mcp/src/server/catalog.rs`
- `/Users/justinwilaby/Development/next-gen-cli/crates/mcp/src/server/schemas.rs`

## Implemented catalog tools

Catalog-related MCP tools exposed from server core include:
- `catalog_validate_openapi`
- `catalog_preview_import`
- `catalog_import_openapi`
- `catalog_set_enabled`
- `catalog_remove`

## Source handling

Source loading supports:
- local path
- URL (`http`/`https`)

`source_type` can be explicit; otherwise inferred from source string.

## Validation and preview

- `catalog_validate_openapi` parses + preflight-validates without mutation.
- `catalog_preview_import` validates and returns preview metadata, optionally including command preview.

## Import and runtime mutation

- `catalog_import_openapi` imports into runtime config/registry using shared registry import pipeline.
- `catalog_set_enabled` toggles enabled state and persists config.
- `catalog_remove` removes runtime catalog entry (with optional manifest removal) and persists config.

## Error contract

Catalog tools return structured MCP errors with actionable `error_code`/violation payloads for parse/validation/runtime failures.

## Correctness notes

- This is as-built behavior only.
- Keep aligned with `server/core.rs` tool definitions and `server/catalog.rs` handlers.


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/OPENAPI_IMPORT.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/COMMAND_SEARCH.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/MCP_WORKFLOWS.md`
