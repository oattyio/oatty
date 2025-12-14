# Oatty Engine

The Oatty Engine is a robust workflow execution engine that provides comprehensive support for defining, validating, and executing automation workflows. It's designed with a focus on developer experience, type safety, and extensibility.

## Features

### 🚀 **Core Workflow Engine**
- **Multi-format Support**: Parse workflows from YAML and JSON with automatic format detection
- **Template Interpolation**: Dynamic value substitution using `${{ ... }}` syntax
- **Conditional Execution**: Step-level conditional logic with expression evaluation
- **Input Validation**: Declarative input specifications with provider integration
- **Legacy Compatibility**: Backward compatibility with older workflow formats

### 🔧 **Provider System**
- **Dynamic Value Resolution**: Providers supply runtime values for workflow inputs
- **Contract-based Design**: Metadata-driven provider interfaces for validation and UI generation
- **Extensible Architecture**: Easy to implement custom providers for specific use cases
- **Registry Pattern**: Centralized provider management and discovery

### 📊 **Data Model**
- **Type-safe Structures**: Comprehensive data models with full serialization support
- **Input Specifications**: Rich input definitions with validation rules and defaults
- **Step Definitions**: Flexible step configuration with conditional execution
- **Output Contracts**: Structured output definitions for automatic value mapping

## Architecture

The engine is organized into several focused modules:

```
crates/engine/
├── src/
│   ├── lib.rs          # Main library interface (modern workflows)
│   ├── model.rs        # Core data structures and workflow specifications
│   ├── resolve.rs      # Template interpolation and expression evaluation
│   └── provider.rs     # Provider registry and value resolution
├── Cargo.toml          # Dependencies and build configuration
└── README.md           # This documentation
```

### Module Responsibilities

- **`lib.rs`**: Main entry point and workflow parsing
- **`model.rs`**: Data structures for workflows, steps, inputs, and outputs
- **`resolve.rs`**: Template expression resolution and conditional evaluation
- **`provider.rs`**: Provider interface definitions and registry abstractions

## Usage

### Basic Workflow Parsing

```rust
use oatty_engine::parse_workflow_file;

let workflow_bundle = parse_workflow_file("workflow.yaml")?;
for (name, spec) in &workflow_bundle.workflows {
    println!("Workflow: {}", name);
    println!("Steps: {}", spec.steps.len());
}
```

### Prepare + Execute (Noop Runner)

```rust
use oatty_engine::{parse_workflow_file, RunContext, execute_workflow};

let bundle = parse_workflow_file("crates/engine/workflows/create_app_and_db.yaml")?;
let spec = bundle.workflows.values().next().unwrap();

let mut ctx = RunContext::default();
ctx.inputs.insert("app_name".into(), serde_json::json!("myapp"));
ctx.inputs.insert("region".into(), serde_json::json!("us"));
ctx.inputs.insert("addon_plan".into(), serde_json::json!("heroku-postgresql:hobby-dev"));

// Uses the Noop runner by default; safe for previews/tests
let results = execute_workflow(spec, &mut ctx)?;
assert!(!results.is_empty());
```

### Execute with Real Oatty API

```rust
use oatty_engine::{parse_workflow_file, RunContext, execute_workflow_with_runner, RegistryCommandRunner};

// Requires HEROKU_API_KEY in the environment
let runner = RegistryCommandRunner::from_env()?;
let bundle = parse_workflow_file("crates/engine/workflows/collaborator_lifecycle.yaml")?;
let spec = bundle.workflows.values().next().unwrap();

let mut ctx = RunContext::default();
ctx.inputs.insert("app".into(), serde_json::json!("myapp"));
ctx.inputs.insert("user".into(), serde_json::json!("alice@example.com"));
ctx.inputs.insert("permissions".into(), serde_json::json!(["view", "deploy"]));

let results = execute_workflow_with_runner(spec, &mut ctx, &runner)?;
for r in results { println!("{} -> {:?}", r.id, r.status); }
```

### Template Interpolation

```rust
use oatty_engine::resolve::{RunContext, interpolate_value};
use serde_json::json;

let mut context = RunContext::default();
context.environment_variables.insert("APP_NAME".into(), "myapp".into());
context.inputs.insert("environment".into(), json!("production"));

let value = json!({
    "name": "${{ env.APP_NAME }}",
    "env": "${{ inputs.environment }}"
});

let interpolated = interpolate_value(&value, &context);
```

### Provider Integration

```rust
use oatty_engine::provider::{ProviderRegistry, NullProvider};

let registry: Box<dyn ProviderRegistry> = Box::new(NullProvider);
let values = registry.fetch_values("apps:list", &serde_json::Map::new())?;
```

## Workflow Format

The engine supports multiple workflow formats:

### Modern Format (Recommended)

```yaml
workflow: "deploy-app"
inputs:
  app_name:
    description: "Application to deploy"
    type: "string"
    provider: "apps:list"
steps:
  - id: "deploy"
    run: "apps:deploy"
    with:
      app: "${{ inputs.app_name }}"
    if: "inputs.environment == 'production'"
```

### Multi-workflow Bundle

```yaml
workflows:
  deploy:
    workflow: "deploy-app"
    steps: [...]
  rollback:
    workflow: "rollback-app"
    steps: [...]
```

<!-- Legacy formats have been removed. Use the modern format only. -->

## Template Expressions

The engine supports rich template expressions for dynamic value resolution:

### Environment Variables
```yaml
region: "${{ env.REGION }}"
api_key: "${{ env.HEROKU_API_KEY }}"
```

### Workflow Inputs
```yaml
app_name: "${{ inputs.application }}"
team_id: "${{ inputs.team.id }}"
```

### Step Outputs
```yaml
app_id: "${{ steps.create.output.id }}"
status: "${{ steps.deploy.status }}"
```

### Conditional Logic
```yaml
if: "inputs.environment == 'production'"
if: "steps.validate.status == 'success'"
if: "[\"succeeded\",\"failed\"].includes(steps.build.status)"   # array helper
if: "inputs.permissions.includes(\"deploy\")"                       # array helper on input arrays
```

### Array Helpers
- `array.includes(value)`: Returns true if `value` is present in `array`.
  - Supports JSON array literals and resolved arrays from `inputs.*` or `steps.*`.
  - Examples:
    - `"[\"a\",\"b\"].includes(inputs.choice)"`
    - `"inputs.permissions.includes(\"deploy\")"`
    - `"[\"succeeded\",\"failed\"].includes(steps.build.status)"`

## Provider System

Providers enable dynamic value resolution for workflow inputs:

### Built-in Providers
- **`apps:list`**: List available applications
- **`teams:list`**: List team memberships
- **`regions:list`**: List available regions

### Custom Provider Implementation

```rust
use oatty_engine::provider::{ProviderRegistry, ProviderContract};

pub struct CustomProvider;

impl ProviderRegistry for CustomProvider {
    fn fetch_values(&self, provider_id: &str, args: &serde_json::Map<String, Value>) -> anyhow::Result<Vec<Value>> {
        // Implementation here
        Ok(vec![json!({"id": "value", "name": "display"})])
    }

    fn get_contract(&self, provider_id: &str) -> Option<ProviderContract> {
        // Return provider contract
        Some(ProviderContract::default())
    }
}
```

## Error Handling

The engine uses `anyhow::Result` for comprehensive error handling:

```rust
use anyhow::{Context, Result};

let workflow = parse_workflow_file("workflow.yaml")
    .with_context(|| "Failed to parse workflow file")?;

let interpolated = interpolate_value(&value, &context)
    .with_context(|| "Failed to interpolate template values")?;
```

## Testing

The engine includes comprehensive test coverage:

```bash
# Run all tests
cargo test

# Run tests with logging
RUST_LOG=debug cargo test

# Run specific module tests
cargo test --lib resolve
```

## Performance Considerations

- **Lazy Evaluation**: Template expressions are resolved only when needed
- **Efficient Parsing**: Optimized YAML/JSON parsing with minimal allocations
- **Smart Caching**: Provider contracts and metadata are cached when possible
- **Memory Efficient**: Uses references and avoids unnecessary cloning

## Security Features

- **Expression Sandboxing**: Template expressions are limited to safe operations
- **No Code Execution**: Templates cannot execute arbitrary code
- **Input Validation**: All inputs are validated against their specifications
- **Provider Isolation**: Providers run in isolated contexts

## Contributing

When contributing to the engine:

1. **Follow Rust Conventions**: Use idiomatic Rust with comprehensive documentation
2. **Expand Abbreviations**: Use descriptive names instead of abbreviations
3. **Add Tests**: Include unit tests for all new functionality
4. **Update Documentation**: Keep examples and documentation current
5. **Run Checks**: Ensure `cargo fmt` and `cargo clippy` pass

## Dependencies

- **`serde`**: Serialization and deserialization
- **`serde_json`**: JSON support
- **`serde_yaml`**: YAML support
- **`anyhow`**: Error handling
- **`tracing`**: Logging and diagnostics
- **`reqwest`**: HTTP client (API runner)
- **`percent-encoding`**: Safe path templating

## License

This project is licensed under the same terms as the parent repository.
