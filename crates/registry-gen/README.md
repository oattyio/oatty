# Heroku CLI Command Registry (Generator)

This crate parses the Heroku JSON Hyper‑Schema and generates a compact command registry used by the CLI/TUI. It outputs a binary manifest via `bincode` (and optionally JSON) so frontends can load commands quickly, render help, autocomplete flags/args, and now infer ValueProviders for dynamic value completion.

## Features

- JSON Hyper‑Schema → `CommandSpec` generation (walks `links`, `$ref`, `anyOf/oneOf/allOf`).
- Compact binary manifest with `bincode`; optional pretty JSON output.
- Structured flags, positional args (with help), ranges (for pagination), and summaries.
- ValueProvider inference: maps flags/positionals to provider commands like `apps:list`.
- Clear errors via `anyhow`; small surface area for consumers.

## Installation

This crate is internal to the workspace; consumers generally depend on the generated manifest via `heroku-registry`.

## Usage

### CLI

This crate can be invoked as a CLI to generate a manifest from a JSON Hyper-Schema.

- Binary (bincode) output:

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
use heroku_registry_gen::write_manifest;
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
4. Serialize the commands to `commands.bin` using `bincode`.
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
    pub providers: Vec<ProviderBinding> // inferred ValueProviders (see below)
}

pub struct PositionalArgument { pub name: String, pub help: Option<String> }

pub struct CommandFlag {
    pub name: String,
    pub short_name: Option<String>,
    pub required: bool,
    pub r#type: String,
    pub enum_values: Vec<String>,
    pub default_value: Option<String>,
    pub description: Option<String>,
}

pub enum ProviderParamKind { Flag, Positional }
pub enum ProviderConfidence { High, Medium, Low }

pub struct ProviderBinding {
    pub kind: ProviderParamKind,        // flag or positional
    pub name: String,                   // e.g., "app"
    pub provider_id: String,            // e.g., "apps:list"
    pub confidence: ProviderConfidence, // High/Medium/Low
}
```

### ValueProvider Inference

The generator performs a second pass to infer providers for flags/positionals:

- Build an index of available list commands by inspecting every `CommandSpec`’s `method+path` (classified into group+action where action == `list`).
- Positional args: Walk `spec.path` and, for each `{arg}`, bind a provider from the immediately preceding concrete segment, e.g. `/addons/{addon}/config` → `{addon}` → `addons:list` (confidence: High), if that list command exists.
- Flags: Map flag name using a conservative synonyms table (e.g., `app→apps`, `pipeline→pipelines`). If the group has a list command, attach `provider_id = "<group>:list"` (confidence: Medium). If only careful pluralization is used, confidence is Low.
- Only attach providers when a matching list command exists.

Examples:

- `addons config:update` (path `/addons/{addon}/config`) → positional `addon` → `addons:list` (High)
- Any command with `--app` flag and a known `apps:list` → flag `app` → `apps:list` (Medium)

### Notes on Workflows

This generator focuses on API-derived commands. If workflow features are enabled elsewhere, they may be added as synthetic commands by the caller or a feature module.

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
- `bincode`: For binary serialization of the command registry.
- `heck`: For kebab-case conversion of command names.
- `percent-encoding`: For decoding URL-encoded placeholders.
- `serde` and `serde_json`: For JSON hyper-schema serialization/deserialization.

## Contributing

Contributions are welcome! Please submit pull requests or open issues on the repository. Ensure code follows Rust conventions and includes tests for new functionality.

## License

This crate is licensed under the MIT License. See the `LICENSE` file for details.
