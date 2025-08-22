Heroku Registry Generator
=========================

**Overview**
- Purpose: build-time generator that converts the Heroku JSON Hyper-Schema into a compact manifest consumed by `heroku-registry` at runtime.
- Output: a JSON manifest with the same shape as `Registry` (commands only). Runtime deserializes it in O(n) without walking the Hyper-Schema.
- Scope: focuses solely on schema→manifest generation. Workflow commands are added at runtime by `heroku-registry` and are not part of the manifest.

**Manifest Format**
- Structure: JSON object with a `commands` array; each item is a command specification.
- `commands`: list where each entry contains:
  - `name`: qualified name like `apps:list`, `releases:list`, `apps:app:create`.
  - `summary`: short human-readable description.
  - `positional_args`: ordered list of positional parameter names that appear in the path template.
  - `positional_help`: map from positional name to description text (when derivable from schema refs).
  - `flags`: array of flags (non-positional inputs) with:
    - `name`, `required`, `type`, `enum_values`, `default_value`, `description`.
  - `method`: HTTP method such as `GET`, `POST`, `PATCH`, `DELETE`.
  - `path`: normalized path template (e.g., `/apps/{app}/releases`).

**Library API**
- Crate: `heroku-registry-gen`
- Function: `generate_manifest(schema_json: &str) -> anyhow::Result<String>`
  - Input: raw Hyper-Schema string.
  - Output: pretty-printed JSON manifest string matching the format above.
- Derivation highlights:
  - Walks all objects containing `links`.
  - Classifies command action from `(href, method)` and builds stable names.
  - Extracts positional args from path placeholders; resolves help text from `$ref`/`anyOf`.
  - Resolves flag definitions from `schema.properties`, following `$ref`, and merges `type`, `enum`, `default`, `description`.
  - Deduplicates commands by `(name, method, path)` while preserving distinct names.

**CLI Usage**
- Binary: `schema-to-manifest`
- Run:
  - `cargo run -p heroku-registry-gen --bin schema-to-manifest -- schemas/heroku-schema.json /tmp/heroku-manifest.json`
- Behavior:
  - Reads input schema file, generates the manifest, ensures output directory exists, writes JSON.

**Build Integration**
- Primary consumer: `crates/registry/build.rs`
- Build step:
  - Reads `schemas/heroku-schema.json`.
  - Calls `heroku_registry_gen::generate_manifest`.
  - Writes `OUT_DIR/heroku-manifest.json` and marks the schema as a `rerun-if-changed` dependency.
- Runtime ingestion:
  - `heroku-registry` embeds the manifest with `include_str!` and deserializes it on startup.

**Development**
- Test locally:
  - `cargo run -p heroku-registry-gen --bin schema-to-manifest -- schemas/heroku-schema.json /tmp/manifest.json`
  - Inspect or diff the produced manifest as needed.
- Coding style:
  - Keep `use` imports at the top of files.
  - Avoid adding runtime dependencies to the generator unless necessary; keep it lean.
- Performance:
  - Generator runs at build-time, so prioritizing correctness and clarity is acceptable; runtime stays fast.

**Troubleshooting**
- Empty or missing commands:
  - Ensure the input schema path is correct and the schema is valid JSON.
- Missing enum/default/description on flags:
  - Confirm the schema uses `$ref` or inline properties; the generator merges missing fields from referenced definitions.
- Build not regenerating:
  - Verify the build script prints `cargo:rerun-if-changed=schemas/heroku-schema.json` and that you changed that file.

**Examples**

Manifest snippet
```json
{
  "commands": [
    {
      "name": "apps:list",
      "summary": "List apps for the current user",
      "positional_args": [],
      "positional_help": {},
      "flags": [],
      "method": "GET",
      "path": "/apps"
    },
    {
      "name": "apps:app:create",
      "summary": "Create a new app",
      "positional_args": [],
      "positional_help": {},
      "flags": [
        { "name": "name", "required": false, "type": "string", "enum_values": [], "default_value": null, "description": "Unique app name" },
        { "name": "region", "required": false, "type": "string", "enum_values": ["us", "eu"], "default_value": "us", "description": "Region where app will run" }
      ],
      "method": "POST",
      "path": "/apps"
    }
  ]
}
```

End-to-end (manual) generation and run
- Generate a manifest to a temporary location:
  - `cargo run -p heroku-registry-gen --bin schema-to-manifest -- schemas/heroku-schema.json target/tmp/heroku-manifest.json`
- Inspect the result (optional):
  - `head -n 40 target/tmp/heroku-manifest.json`
- Build the workspace (build.rs will also generate and embed the manifest for the registry):
  - `cargo build --workspace`
- Run a command using the registry-backed CLI (no runtime schema parsing):
  - `cargo run -p heroku-cli -- apps list --dry-run`
