# Value Provider Registry Architecture

## Overview

The Value Provider Registry is a sophisticated system that enables dynamic, context-aware suggestions for Oatty CLI commands. It bridges the gap between static command definitions and dynamic runtime data by providing intelligent autocomplete suggestions for flags, positional arguments, and command values.

## Purpose

The Value Provider Registry serves several key purposes:

1. **Dynamic Autocomplete**: Provides real-time suggestions based on actual Oatty API data
2. **Context Awareness**: Understands command relationships and suggests relevant values
3. **Intelligent Binding**: Automatically maps command parameters to appropriate data sources
4. **Performance Optimization**: Implements caching and background fetching to maintain responsiveness
5. **User Experience Enhancement**: Reduces typing and prevents errors in command construction

## Core Components

### 1. Schema-Driven Provider Resolution (two-pass)

The generator infers and verifies providers in a second pass and embeds them directly on fields (`CommandFlag.provider` and `PositionalArgument.provider`). Providers include input bindings (`binds: Vec<Bind>`) so runtimes can resolve provider inputs from consumer inputs.

#### Key Functions (updated)

- Build command index keyed by the canonical `<group> <name>` identifier for every command (legacy colon forms are still recognized during manifest generation but are no longer emitted into runtime identifiers).
- Identify list-capable groups (presence of `<group> list`)
- Resolve providers:
  - Positionals: find best provider by checking both simple (`<group> list`) and scoped (`<earlier_segment> <group> list`) providers, preferring those with successful bindings
  - Flags: map flag name to plural group via synonym/pluralization → `<group> list` when present
  - Bindings: bind provider path placeholders from earlier consumer positionals; bind required provider flags only from a safe set and only from consumer required flags or earlier positionals.

#### Provider Resolution Logic

```rust
pub fn resolve_and_infer_providers(commands: &mut [CommandSpec]) {
    let index = build_command_index(commands);
    let list_groups = groups_with_list(&index);
    for spec in commands.iter_mut() {
        // Flags
        apply_flag_providers(&mut spec.flags, &list_groups, &index);
        // Positionals
        apply_positional_providers(&mut spec.positional_args, &spec.path, &list_groups, &index);
    }
}
```

### 2. ProviderRegistry (`engine/provider/registry.rs`)

The shared engine `ProviderRegistry` now implements the `ValueProvider` trait directly. This unifies provider lookup, caching, and suggestion ranking for both the workflow engine and the TUI palette.

#### Architecture Features

- **Shared Runtime**: One registry instance serves workflow execution and the palette.
- **Asynchronous Fetching**: Background API calls prevent UI stalls while new data loads.
- **Intelligent Caching**: TTL-based cache keyed by provider ID + arguments with deduplicated in-flight fetches.
- **Binding Awareness**: Reuses schema-inferred binds to supply path/query arguments automatically.
- **Fuzzy Ranking**: Scores results with `fuzzy_score` before returning `SuggestionItem`s.

#### Core Implementation (shape)

```rust
pub struct ProviderRegistry {
    registry: Arc<Mutex<CommandRegistry>>,
    fetcher: Arc<dyn ProviderValueFetcher>,
    cache_ttl: Duration,
    cache: Arc<Mutex<HashMap<String, CacheEntry>>>,
    choices: Arc<Mutex<HashMap<String, FieldSelection>>>,
    active_fetches: Arc<Mutex<HashSet<String>>>,
    runtime: Arc<Runtime>,
}

impl ValueProvider for ProviderRegistry {
    fn suggest(
        &self,
        commands: &[CommandSpec],
        command_key: &str,
        field: &str,
        partial: &str,
        inputs: &HashMap<String, String>,
    ) -> Vec<SuggestionItem> { /* ... */ }
}
```

### 3. Palette Integration (`palette.rs`)

The command palette component integrates the provider system with the user interface.

#### Key Features

- **Real-time Suggestions**: Updates suggestions as users type
- **Contextual Help**: Integrates with help system for command guidance
- **Smart Navigation**: Handles suggestion selection and command completion
- **Error Handling**: Displays validation errors and provider status

#### Suggestion Flow

```rust
fn handle_tab_press(&self, app: &mut app::App) {
    let SharedCtx { providers, .. } = &app.ctx;
    app.palette.apply_build_suggestions(providers, &*app.ctx.theme);
    let open = app.palette.suggestions_len() > 0 || app.palette.is_provider_loading();
    app.palette.set_is_suggestions_open(open);
}
```

## Data Flow Architecture

### 1. Command Registration Flow

```mermaid
graph TD
    A[OpenAPI Schema] --> B[Schema Parser]
    B --> C[Command Derivation]
    C --> D[Provider Binding Inference]
    D --> E[Registry Population]
    E --> F[Command Registry]
    
    subgraph "Provider Binding Logic"
        G[Path Analysis] --> H[Flag Mapping]
        H --> I[Group Identification]
        I --> J[Provider Assignment]
    end
    
    D --> G
```

### 2. Runtime Suggestion Flow

```mermaid
sequenceDiagram
    participant U as User
    participant P as Palette
    participant R as Registry
    participant V as ValueProvider
    participant C as Cache
    participant A as API
    
    U->>P: Type in field
    P->>R: Get command spec
    R->>P: Return spec with per-field providers
    P->>V: Request suggestions
    V->>C: Check cache
    alt Cache hit
        C->>V: Return cached values
    else Cache miss
        V->>A: Fetch from API
        A->>V: Return data
        V->>C: Update cache
    end
    V->>P: Return suggestions
    P->>U: Display suggestions
```

### 3. Provider Resolution Heuristics

```mermaid
graph TD
    A[Command Path] --> B[Segment Analysis]
    B --> C{Has Placeholders?}
    C -->|Yes| D[Extract Parameter Names]
    C -->|No| E[No Positional Providers]
    
    D --> F[Collect All Concrete Segments]
    F --> G[Find Provider Candidates]
    G --> H[Simple Provider: group:list]
    G --> I[Scoped Provider: earlier_segment:group:list]
    
    H --> J{Can Bind?}
    I --> K{Can Bind?}
    
    J -->|Yes| L[Use Simple Provider with Bindings]
    J -->|No| M[Use Simple Provider without Bindings]
    K -->|Yes| N[Use Scoped Provider with Bindings]
    K -->|No| O[Skip Scoped Provider]
    
    L --> P[Field.provider]
    N --> P
    M --> P
    O --> P
    
    Q[Command Flags] --> R[Flag Name Analysis]
    R --> S[Map to Resource Group]
    S --> T{Group Has List Command?}
    T -->|Yes| U[Create Flag Provider]
    T -->|No| V[No Provider Binding]
    
    U --> P
    V --> P
```

#### Scoped Provider Resolution

The system now supports **scoped providers** that can use earlier path segments to create more contextually appropriate providers. For example:

- Path: `/apps/{app}/addons/{addon}`
- For `{addon}` placeholder:
  - Simple provider: `addons:list` (no context)
  - Scoped provider: `apps:addons:list` (scoped by app)
  - **Preference**: Scoped provider with successful binding to earlier `app` argument

This enables commands like `heroku apps addons:info <app> <addon>` to use the scoped `apps:addons:list` provider, which can filter addons by the specified app.

## Implementation Details

### Provider Attachment Types

Providers are attached directly to fields:

1. **Positional Arguments**: Inferred from URL path parameters and verified
2. **Flag Values**: Inferred via naming conventions and verified

Note: Confidence scoring and a separate providers vector are removed; providers are only attached when verifiably resolvable. Bindings are embedded on the field’s provider so runtimes can resolve provider inputs from consumer inputs.

### Caching Strategy

The caching system implements several optimizations:

- **TTL-based Expiration**: Configurable cache lifetime
- **Background Refresh**: Non-blocking API updates
- **Deduplication**: Prevents concurrent fetches for the same provider
- **Memory Efficiency**: Automatic cleanup of expired entries

### Error Handling

The system gracefully handles various failure scenarios:

- **API Failures**: Falls back to cached data or empty suggestions
- **Network Issues**: Continues operation with existing cache
- **Schema Errors**: Logs issues without breaking the UI
- **Provider Mismatches**: Graceful degradation to basic autocomplete

## Usage Examples

### 1. Basic Provider Usage

```rust
// Create a provider with 60-second cache TTL
let provider = RegistryBackedProvider::new(
    Arc::new(registry), 
    Duration::from_secs(60)
);

// Get suggestions for an app field
let suggestions = provider.suggest("config:update", "app", "my");
// Returns: ["my-app-1", "my-app-2", "my-app-production"]
```

### 2. Custom Provider Integration

```rust
// Implement custom ValueProvider trait
impl ValueProvider for CustomProvider {
    fn suggest(&self, command_key: &str, field: &str, partial: &str) -> Vec<SuggestionItem> {
        // Custom suggestion logic
        vec![]
    }
}

// Register with the system
let providers = vec![
    Box::new(RegistryBackedProvider::new(registry, ttl)),
    Box::new(CustomProvider::new()),
];
```

### 3. Schema-Driven Binding

```yaml
# OpenAPI schema automatically generates bindings
paths:
  /apps/{app}/config:
    get:
      parameters:
        - name: app
          in: path
          required: true
          schema:
            type: string
      # Automatically infers: app parameter → apps:list provider
```

### 4. Scoped Provider Resolution

```yaml
# OpenAPI schema automatically generates scoped providers with bindings
paths:
  /apps/{app}/addons/{addon}:
    get:
      parameters:
        - name: app
          in: path
          required: true
          schema:
            type: string
        - name: addon
          in: path
          required: true
          schema:
            type: string
      # Automatically infers:
      # - app parameter → apps:list provider
      # - addon parameter → apps:addons:list provider with binding: app → app
```

**Generated Provider Configuration:**
```json
{
  "positional_args": [
    {
      "name": "app",
      "provider": {
        "Command": {
          "command_id": "apps:list",
          "binds": []
        }
      }
    },
    {
      "name": "addon",
      "provider": {
        "Command": {
          "command_id": "apps:addons:list",
          "binds": [
            {
              "provider_key": "app",
              "from": "app"
            }
          ]
        }
      }
    }
  ]
}
```

## Performance Considerations

### Optimization Strategies

1. **Lazy Loading**: Providers only fetch data when needed
2. **Background Processing**: API calls don't block the UI
3. **Smart Caching**: TTL-based expiration with background refresh
4. **Request Deduplication**: Prevents redundant API calls
5. **Fuzzy Matching**: Efficient suggestion filtering

### Memory Management

- **Cache Size Limits**: Configurable maximum cache entries
- **Automatic Cleanup**: Expired entries are removed
- **Arc-based Sharing**: Efficient memory sharing across components
- **Lazy Initialization**: Providers are created on-demand

## Security Considerations

### Data Protection

- **API Key Management**: Secure handling of Oatty API credentials
- **Request Validation**: Input sanitization and validation
- **Rate Limiting**: Respects API rate limits
- **Error Masking**: Sensitive information is not exposed in logs

### Access Control

- **Provider Isolation**: Each provider operates independently
- **Permission Checking**: Respects user's Oatty account permissions
- **Audit Logging**: Tracks provider usage for debugging

## Testing Strategy

### Unit Tests

- **Provider Logic**: Individual provider behavior testing
- **Binding Inference**: Schema parsing and provider assignment
- **Cache Operations**: Cache hit/miss scenarios
- **Error Handling**: Various failure modes

### Integration Tests

- **End-to-End Flows**: Complete suggestion workflows
- **API Integration**: Real Oatty API interactions
- **Performance Testing**: Cache efficiency and response times
- **Concurrency Testing**: Multiple simultaneous requests

### Test Data

```rust
#[test]
fn test_provider_binding_inference() {
    let json = r#"{
        "links": [
            { "href": "/apps", "method": "GET", "title": "List apps" },
            { "href": "/apps/{app}/config", "method": "PATCH", "title": "Update config" }
        ]
    }"#;
    
    let commands = derive_commands_from_schema(&json).unwrap();
    let config_cmd = commands.iter().find(|c| c.path.contains("config")).unwrap();
    
    assert!(config_cmd.providers.iter().any(|p| 
        p.name == "app" && p.provider_id == "apps:list"
    ));
}
```

## Future Enhancements

### Planned Features

1. **Multi-Provider Support**: Combine suggestions from multiple sources
2. **Context-Aware Suggestions**: Consider command history and user preferences
3. **Intelligent Ranking**: Machine learning-based suggestion ordering
4. **Offline Mode**: Local caching for offline operation
5. **Provider Plugins**: Extensible provider system for third-party integrations

### Scalability Improvements

1. **Distributed Caching**: Redis-based shared cache
2. **Provider Load Balancing**: Multiple provider instances
3. **Async Streaming**: Real-time suggestion updates
4. **Batch Processing**: Efficient bulk data fetching

## Conclusion

The Value Provider Registry represents a sophisticated approach to command-line autocomplete that goes beyond simple text matching. By combining schema analysis, intelligent binding inference, and efficient caching, it provides a seamless user experience while maintaining performance and reliability.

The architecture's modular design allows for easy extension and customization, making it suitable for both current needs and future enhancements. The integration with the command palette creates a cohesive user experience that significantly improves CLI usability.
