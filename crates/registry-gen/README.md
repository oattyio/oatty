# Oatty CLI Command Registry (Generator)

This crate parses the Oatty JSON Hyper‑Schema and generates a compact command registry used by the CLI/TUI. It outputs a binary manifest via `bincode` (and optionally JSON) so frontends can load commands quickly, render help, autocomplete flags/args, and now infer ValueProviders for dynamic value completion.

## Features

- JSON Hyper‑Schema → `CommandSpec` generation (walks `links`, `$ref`, `anyOf/oneOf/allOf`).
- Compact binary manifest with `postcard`; optional pretty JSON output.
- Structured flags, positional args (with help), ranges (for pagination), and summaries.
- ValueProvider inference: maps flags/positionals to provider commands like `apps:list`,
  attaching input bindings when the provider can be satisfied by earlier consumer inputs.
- Clear errors via `anyhow`; small surface area for consumers.

## Installation

This crate is internal to the workspace; consumers generally depend on the generated manifest via `heroku-registry`.

## Usage

### CLI

This crate can be invoked as a CLI to generate a manifest from a JSON Hyper-Schema.

- Binary (postcard) output:

```bash
cargo run -p heroku-registry-gen -- path/to/schema.json target/manifest.bin
```

- JSON output via `--json` flag:

```bash
cargo run -p heroku-registry-gen -- --json path/to/schema.json target/manifest.json
```

The CLI will create parent directories for the output path if needed.

### Generating a Command Manifest (library)

The primary function, `write_manifest`, reads a JSON hyper-schema file, generates command specifications, and writes them to a binary manifest file.

```rust
use oatty_registry_gen::write_manifest;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let input = PathBuf::from("schema.json");
    let output = PathBuf::from("commands.bin");
    write_manifest(input, output)?;
    Ok(())
}
```

This will:
1. Read the JSON hyper-schema from `schema.json`.
2. Parse it to generate `CommandSpec` entries.
3. Add synthetic workflow commands if `FEATURE_WORKFLOWS` is enabled.
4. Serialize the commands to `commands.bin` using `postcard`.
5. Create parent directories for the output file if they don't exist.

### Command Specification Structure

Types live in `crates/types`. The important ones are shown here (trimmed):

```rust
pub struct CommandSpec {
    pub group: String,                  // e.g., "apps"
    pub name: String,                   // e.g., "list", "config:update"
    pub summary: String,                // human summary
    pub positional_args: Vec<PositionalArgument>,
    pub flags: Vec<CommandFlag>,
    pub method: String,                 // e.g., "GET", "POST"
    pub path: String,                   // e.g., "/apps", "/addons/{addon}/config"
    pub ranges: Vec<String>,            // supported range fields
}

pub struct PositionalArgument { pub name: String, pub help: Option<String>, pub provider: Option<ValueProvider> }

pub struct CommandFlag {
    pub name: String,
    pub short_name: Option<String>,
    pub required: bool,
    pub r#type: String,
    pub enum_values: Vec<String>,
    pub default_value: Option<String>,
    pub description: Option<String>,
    pub provider: Option<ValueProvider>,
}

// Providers are now embedded directly on fields via:
// pub enum ValueProvider { Command { command_id: String, binds: Vec<Bind> } }
// pub struct Bind { provider_key: String, from: String }
```

### ValueProvider Inference & Bindings

The generator performs a second pass to infer providers and embeds them directly on fields:

- Build an index of available list commands keyed by `<group>:<name>`.
- Positional args: Walk `spec.path` and bind the provider from the immediately preceding concrete segment (e.g., `/addons/{addon}/config` → `{addon}` → `addons:list`) when that command exists.
- Flags: Map flag names to plural resource groups via a small synonym map (and conservative pluralization) and bind `<group>:list` when present.
- Input bindings (high-reliability only):
  - Bind provider path placeholders to earlier consumer positionals of the same/synonym name.
  - Bind required provider flags only when they are among a safe set (app/app_id, addon/addon_id, pipeline, team/team_name, space/space_id, region, stack), and only from consumer required flags (same/synonym name) or earlier consumer positionals.
  - If any required provider placeholder/flag cannot be satisfied, the provider isn’t attached.
- Only attach providers when the referenced command exists (verified at 100%).

Examples:

 - `addons config:update` (path `/addons/{addon}/config`) → positional `addon` → provider `ValueProvider::Command { command_id: "addons:list" }`
 - Any command with `--app` and a known `apps:list` → flag `app` → provider `ValueProvider::Command { command_id: "apps:list" }`
  - `addons info <app> <addon>` with provider `addons:list` at `/apps/{app}/addons` → positional `addon` → provider binds `{ app ← app }` so suggestions are app‑scoped.

### Notes on Workflows

The generator can now bundle authored workflows alongside command specs. Pass the `--workflows <dir>` flag when invoking the CLI, or supply `Some(PathBuf::from("workflows"))` when calling `write_manifest*` programmatically. Every `.yaml`, `.yml`, or `.json` file found in the directory tree is deserialized into the strongly typed structures defined in `heroku-types::workflow::WorkflowDefinition`.

### JSON Hyper-Schema Requirements

The input JSON hyper-schema must conform to the JSON Hyper-Schema specification and include a `links` array with entries containing:

- `href`: The API endpoint path (e.g., `/apps/{app_id}/config-vars`).
- `method`: The HTTP method (e.g., `GET`, `POST`, `PATCH`, `DELETE`).
- `title`: Optional command title.
- `description`: Optional command description.
- `schema`: Optional schema for flags, including `properties`, `required`, and `$ref` references to other schema parts.

The crate processes these to generate commands with appropriate groups, names, and flags. For example:

```json
{
  "links": [
    {
      "href": "/apps/{app_id}/config-vars",
      "method": "GET",
      "title": "List config vars",
      "description": "Retrieve all config vars for an app",
      "schema": {
        "required": ["app_id"],
        "properties": {
          "app_id": { "type": "string", "description": "Application ID" }
        }
      }
    }
  ]
}
```

This generates a `config:list` command with a positional argument and the usual flags. If a matching list provider exists (e.g., `apps:list`), a `ProviderBinding` is attached.

## Development

### Building

```bash
cargo build
```

### Testing

Add tests to verify hyper-schema parsing and command generation:

```bash
cargo test
```

### Dependencies

- `anyhow`: For error handling with context.
- `postcard`: For binary serialization of the command registry.
- `heck`: For kebab-case conversion of command names.
- `percent-encoding`: For decoding URL-encoded placeholders.
- `serde` and `serde_json`: For JSON hyper-schema serialization/deserialization.

## Contributing

Contributions are welcome! Please submit pull requests or open issues on the repository. Ensure code follows Rust conventions and includes tests for new functionality.

## License

This crate is licensed under the MIT License. See the `LICENSE` file for details.
