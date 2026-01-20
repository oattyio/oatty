# Oatty CLI Command Registry (Generator)

This crate parses OpenAPI v3 documents and generates a compact command registry
used by the CLI/TUI. It outputs a binary manifest via `postcard` (and optionally
JSON) so frontends can load commands quickly, render help, autocomplete
flags/args, and infer ValueProviders for dynamic value completion.

## Features

- OpenAPI v3 → `CommandSpec` generation from paths and operations.
- Command base URLs derived from OpenAPI `servers`.
- Structured flags, positional args (with help), and summaries.
- ValueProvider inference: maps flags/positionals to provider commands like
  `apps:list`, attaching input bindings when the provider can be satisfied by
  earlier consumer inputs.
- Compact binary manifest with `postcard`; optional pretty JSON output.

## Installation

This crate is internal to the workspace; consumers generally depend on the
generated manifest via `oatty-registry`.

## Usage

### Generating a Command Manifest (library)

The primary functions, `write_manifest` and `write_manifest_json`, read OpenAPI
documents, generate command specifications, and write them to a manifest file.

```rust
use std::path::PathBuf;
use oatty_registry_gen::{io::ManifestInput, write_manifest};

fn main() -> anyhow::Result<()> {
    let schema = PathBuf::from("schemas/samples/render-public-api.json");
    let input = ManifestInput::new(Some(schema), None, None);
    let workflows = Some(PathBuf::from("workflows"));
    let output = PathBuf::from("commands.bin");
    write_manifest(input, workflows, output)?;
    Ok(())
}
```

This will:
1. Read the OpenAPI document from `schemas/samples/render-public-api.json`.
2. Parse it to generate `CommandSpec` entries.
3. Load workflows from the optional workflow root directory (YAML/JSON) and embed them into the manifest.
4. Serialize the manifest to `commands.bin` using `postcard`.
5. Create parent directories for the output file if they don't exist.

### Command Specification Structure

Types live in `crates/types`. The important ones are shown here (trimmed):

```rust
pub struct CommandSpec {
    pub group: String,                  // e.g., "apps"
    pub name: String,                   // e.g., "list", "config:update"
    pub catalog_identifier: Option<String>, // derived from manifest path
    pub summary: String,                // human summary
    pub positional_args: Vec<PositionalArgument>,
    pub flags: Vec<CommandFlag>,
    pub execution: CommandExecution,
}

pub struct HttpCommandSpec {
    pub method: String,                 // e.g., "GET", "POST"
    pub path: String,                   // e.g., "/apps", "/addons/{addon}/config"
    pub ranges: Vec<String>,            // supported range fields
    pub base_url: String,               // resolved from OpenAPI servers
}
```

### ValueProvider Inference & Bindings

The generator performs a second pass to infer providers and embeds them directly
on fields:

- Build an index of available list commands keyed by `<group> <name>`.
- Positional args: Walk `spec.path` and bind the provider from the immediately
  preceding concrete segment (e.g., `/addons/{addon}/config` → `{addon}` →
  `addons list`) when that command exists.
- Flags: Map flag names to plural resource groups via conservative pluralization
  and bind `<group> list` when present.
- Input bindings (high-reliability only):
  - Bind provider path placeholders to earlier consumer positionals of the
    same name.
  - Bind required provider flags only when exact matches exist in consumer
    required flags or earlier consumer positionals.
  - If any required provider placeholder/flag cannot be satisfied, the provider
    isn’t attached.
- Only attach providers when the referenced command exists (verified at 100%).

### OpenAPI Requirements

The input OpenAPI document must include:

- `paths` with HTTP operations (GET, POST, PATCH, PUT, DELETE).
- `servers` with the base URL for the API (document, path, or operation level).
- `parameters` and `requestBody` definitions for flags/arguments.
- Response schemas (optional) for output schema metadata.

## Development

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

### Dependencies

- `anyhow`: For error handling with context.
- `postcard`: For binary serialization of the command registry.
- `heck`: For kebab-case conversion of command names.
- `serde_json` and `serde_yaml`: For OpenAPI parsing.

## Contributing

Contributions are welcome! Please submit pull requests or open issues on the
repository. Ensure code follows Rust conventions and includes tests for new
functionality.

## License

This crate is licensed under the MIT License. See the `LICENSE` file for
details.
