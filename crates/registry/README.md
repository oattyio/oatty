# Oatty CLI Registry

This crate provides the core functionality for managing Oatty CLI command definitions. It loads, organizes, and generates command-line interface (CLI) commands from Oatty API schemas, enabling the creation of a structured command tree using the `clap` library.

## Overview

The `oatty-registry` crate is designed to:
- Load command definitions from registry manifests generated from OpenAPI specs.
- Organize commands by resource groups (e.g., `apps`, `services`).
- Generate a `clap`-based command tree for argument parsing and help generation.
- Support feature flags, such as workflows, controlled via environment variables.

The crate is built with extensibility in mind, allowing for easy integration of new commands and features while maintaining a robust and maintainable CLI structure.

## Benefits of Using a Precompiled Manifest

The crate uses a precompiled JSON manifest (`registry-manifest.json`) instead of parsing remote data at runtime. This approach provides:
- **Improved Performance**: Startup avoids expensive OpenAPI parsing.
- **Reduced Overhead**: Loading from a manifest is lighter than full schema traversal.
- **Reliability**: The schema is validated and compiled during the build process, ensuring consistency.

## Features

- **Manifest Loading**: Loads command specifications from a precompiled JSON manifest (`registry-manifest.json`) during the build process.
- **Command Grouping**: Organizes commands by resource type (e.g., `apps:list`, `apps:create`) for intuitive CLI navigation.
- **Clap Integration**: Builds a hierarchical `clap` command tree with support for global flags (`--json`, `--verbose`) and command-specific arguments.
- **Feature Gating**: Supports enabling/disabling features like workflows via environment variables (e.g., `FEATURE_WORKFLOWS`).
- **Provider Contracts**: Exposes argument and return metadata for provider commands so UIs and engines can drive auto-mapping heuristics.
- **Error Handling**: Uses `anyhow` for robust error management during schema loading and command processing.

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
oatty-registry = { path = "../registry" }
```

Ensure the required dependencies (`clap`, `postcard`, `serde`, `anyhow`, etc.) are also included in your project.

## Usage

### Loading the Registry

The `CommandRegistry` struct is the central component for managing command specifications. You can load it from the registry configuration as follows:

```rust
use oatty_registry::CommandRegistry;

fn main() -> anyhow::Result<()> {
    let registry = CommandRegistry::from_config()?;
    println!("Loaded {} commands", registry.commands.len());
    Ok(())
}
```

### Finding a Command

You can search for a specific command by its group and name:

```rust
use oatty_registry::{CommandRegistry, find_by_group_and_cmd};

fn main() -> anyhow::Result<()> {
    let registry = CommandRegistry::from_config()?;
    let apps_list = find_by_group_and_cmd(&registry.commands, "apps", "list")?;
    println!("Found command: {}", apps_list.name);
    Ok(())
}
```

### Inspecting Provider Contracts

Provider argument and return metadata are available on the registry via the `provider_contracts`
map, keyed by `<group> <name>` command identifiers:

```rust
use oatty_registry::CommandRegistry;

fn main() -> anyhow::Result<()> {
    let registry = CommandRegistry::from_config()?;
    if let Some(contract) = registry.provider_contracts.get("apps list") {
        println!("apps list returns {} fields", contract.returns.fields.len());
    }
    Ok(())
}
```

### Building the Clap Command Tree

To generate a `clap` command tree for argument parsing:

```rust
use oatty_registry::{CommandRegistry, build_clap};
use std::sync::{Arc, Mutex};
use clap::Parser;

fn main() -> anyhow::Result<()> {
    let registry = Arc::new(Mutex::new(CommandRegistry::from_config()?));
    let clap_command = build_clap(Arc::clone(&registry));
    let matches = clap_command.get_matches();
    Ok(())
}
```

This creates a command tree with global flags (`--json`, `--verbose`) and grouped subcommands (e.g., `example apps list`, `example services restart`).

### Feature Gating

Check if the workflows feature is enabled:

```rust
use oatty_registry::feature_workflows;

fn main() {
    if feature_workflows() {
        println!("Workflows are enabled");
    } else {
        println!("Workflows are disabled");
    }
}
```

Set the `FEATURE_WORKFLOWS` environment variable to `"1"` or `"true"` to enable workflows.

## Project Structure

The crate is organized into several modules:

- **`models.rs`**: Defines the `CommandRegistry` struct and methods for loading and querying command specifications.
- **`clap_builder.rs`**: Implements functions to build a `clap` command tree from the registry.
- **`feat_gate.rs`**: Provides feature-gating functionality via environment variables.
- **`lib.rs`**: Exports core functionality and includes tests for the registry.
- **`build.rs`**: Handles the build process, generating the `registry-manifest.json` from the sample OpenAPI schemas.

## Build Process

The crate uses a custom build script (`build.rs`) to process OpenAPI documents (for example, `schemas/samples/render-public-api.json`) and generate a JSON manifest (`registry-manifest.json`). This manifest is loaded at runtime by `CommandRegistry::from_config`.

To rebuild the manifest when the schema changes, the build script monitors the schema file:

```rust
println!("cargo:rerun-if-changed={}", schema_path.display());
```

## Testing

The crate includes tests to ensure the embedded manifest is valid and contains unique command names:

```rust
#[test]
fn manifest_non_empty_and_unique_names() {
    let registry = CommandRegistry::from_config().expect("load registry from manifest");
    assert!(!registry.commands.is_empty(), "registry commands should not be empty");
    let mut seen = HashSet::new();
    for c in &*registry.commands {
        assert!(seen.insert(&c.name), "duplicate command name detected: {}", c.name);
    }
}
```

Run tests with:

```bash
cargo test
```

## Contributing

Contributions are welcome! To contribute:

1. Fork the repository.
2. Create a new branch (`git checkout -b feature/your-feature`).
3. Make your changes and commit (`git commit -am 'Add new feature'`).
4. Push to the branch (`git push origin feature/your-feature`).
5. Create a pull request.

Please ensure your code follows the existing style and includes tests where applicable.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Contact

For questions or support, please contact the Oatty CLI team or open an issue on the repository.
