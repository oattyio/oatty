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
- `catalog_apply_patch`
- `catalog_set_enabled`
- `catalog_set_base_url`
- `catalog_edit_headers`
- `catalog_get_masked_headers`
- `catalog_remove`

## Source handling

Source loading supports:
- local path
- URL (`http`/`https`)
- inline JSON/YAML document text passed directly in `source`

`source_type` accepts `path`, `url`, or `inline`. An explicit type takes precedence.
When omitted, HTTP(S) URLs and structured JSON/YAML documents are detected;
other strings are treated as paths. Use `source_type: "inline"` for malformed
YAML or scalar input to receive document validation errors rather than path errors.

All three OpenAPI tools (`catalog_validate_openapi`, `catalog_preview_import`,
and `catalog_import_openapi`) accept inline content. An agent-authored document
does not need an intermediate source file. For example, validation accepts:

```json
{
  "source": "openapi: 3.0.3\ninfo:\n  title: Example API\n  version: 1.0.0\npaths:\n  /items:\n    get:\n      responses:\n        '200':\n          description: Success\n",
  "source_type": "inline"
}
```

Preview and import accept the same source parameters plus `catalog_title` and
their existing options. Inline previews identify their source as `inline`
instead of echoing the document. Import persists the generated catalog and
registry configuration normally, records `import_source_type: "inline"`, and
omits source-location provenance rather than storing document text there.

## Validation and preview

- `catalog_validate_openapi` parses + preflight-validates without mutation.
- `catalog_preview_import` validates and returns preview metadata, optionally including command preview.

## Import and runtime mutation

- `catalog_import_openapi` imports into runtime config/registry using shared registry import pipeline.
- `catalog_apply_patch` applies deterministic command-level patch operations to an existing catalog manifest.
  - Supports strict command matching (`group`, `name`, `http_method`, `http_path`).
  - Supports policy overrides (`fail_on_missing`, `fail_on_ambiguous`, `overwrite`).
  - Persists patched catalog through registry save path and returns operation-level results.
- `catalog_set_enabled` toggles enabled state and persists config.
- `catalog_set_base_url` updates selected base URL for an existing catalog without re-import.
- `catalog_edit_headers` mutates catalog headers via `upsert|remove|replace_all`.
- `catalog_get_masked_headers` returns masked header view plus selected base URL metadata.
- `catalog_remove` removes runtime catalog entry (with optional manifest removal) and persists config.

## Catalog patch semantics

- Patch operations replace a matched command with a provided `replacement_command` payload.
- Matching can fail as:
  - target not found
  - target ambiguous
  - overwrite required
- Errors are returned with structured MCP payloads and actionable next-step guidance.
- Patch execution is explicit and deterministic: no silent mutation outside requested operations.

## Error contract

Catalog tools return structured MCP errors with actionable `error_code`/violation payloads for parse/validation/runtime failures.

## Correctness notes

- This is as-built behavior only.
- Keep aligned with `server/core.rs` tool definitions and `server/catalog.rs` handlers.


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/OPENAPI_IMPORT.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/COMMAND_SEARCH.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/MCP_WORKFLOWS.md`
