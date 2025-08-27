# Heroku CLI Command Registry

This Rust crate generates and manages a registry of commands for a Heroku CLI by processing JSON hyper-schemas (conforming to the JSON Hyper-Schema specification) and augmenting them with synthetic workflow commands. It produces a compact binary manifest using `bincode` to reduce storage and runtime overhead, ensuring efficient command loading for terminal user interfaces (TUIs) or CLI frontends. The crate automates command derivation, supports extensible workflow functionality, and provides structured, type-safe command specifications with robust error handling.

## Features

- **JSON Hyper-Schema Parsing**: Generates command specifications (`CommandSpec`) from a JSON hyper-schema, leveraging `links` arrays and schema references.
- **Workflow Commands**: Adds synthetic commands (`workflow:list`, `workflow:preview`, `workflow:run`) when the `FEATURE_WORKFLOWS` environment variable is enabled.
- **Compact Binary Serialization**: Uses `bincode` to encode commands into a binary manifest file.
- **Reduced Runtime Overhead**: Pre-processes hyper-schemas to minimize runtime parsing, improving CLI startup and responsiveness.
- **Structured and Extensible Command Model**: Supports command groups, positional arguments, flags, and help text for rich CLI features.
- **Error Handling**: Leverages `anyhow` for clear, contextual error messages.
- **Flexible CLI Integration**: Outputs a binary manifest optimized for TUI or CLI frontends, enabling autocompletion and help documentation.

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
heroku-cli-registry = { path = "./path/to/this/crate" }
```

Ensure the following dependencies are included in your project:

```toml
anyhow = "1.0"
bincode = "2.0.0-rc.3"
heck = "0.5"
percent-encoding = "2.3"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## Usage

### Generating a Command Manifest

The primary function, `write_manifest`, reads a JSON hyper-schema file, generates command specifications, and writes them to a binary manifest file.

```rust
use heroku_cli_registry::write_manifest;
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

The `CommandSpec` struct defines a command with the following fields:

```rust
pub struct CommandSpec {
    pub group: String,                    // Command group (e.g., "config", "workflow")
    pub name: String,                     // Command name (e.g., "config:list", "workflow:run")
    pub summary: String,                  // Short description of the command
    pub positional_args: Vec<String>,     // List of positional argument names
    pub positional_help: HashMap<String, String>, // Help text for positional arguments
    pub flags: Vec<CommandFlag>,          // Command flags/options
    pub method: String,                   // HTTP method (e.g., "GET", "POST", or "INTERNAL")
    pub path: String,                     // API path or internal placeholder
}
```

The `CommandFlag` struct defines command flags:

```rust
pub struct CommandFlag {
    pub name: String,                     // Flag name (e.g., "--file")
    pub required: bool,                   // Whether the flag is required
    pub r#type: String,                   // Data type (e.g., "string", "boolean")
    pub enum_values: Vec<String>,         // Allowed values for enum flags
    pub default_value: Option<String>,    // Default value, if any
    pub description: Option<String>,      // Description of the flag
}
```

### Workflow Commands

When the `FEATURE_WORKFLOWS` environment variable is set, the crate adds the following synthetic commands:

- **`workflow:list`**: Lists workflows in the `workflows/` directory.
- **`workflow:preview`**: Previews a workflow plan. Supports optional `--file` and `--name` flags.
- **`workflow:run`**: Executes a workflow. Supports optional `--file` and `--name` flags.

These commands use placeholder `method` ("INTERNAL") and `path` ("__internal__") as they do not correspond to HTTP API calls.

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

This generates a `config:list` command with a required `app_id` positional argument.

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

### Environment Variables

- `FEATURE_WORKFLOWS`: When set, enables synthetic workflow commands.

## Contributing

Contributions are welcome! Please submit pull requests or open issues on the repository. Ensure code follows Rust conventions and includes tests for new functionality.

## License

This crate is licensed under the MIT License. See the `LICENSE` file for details.