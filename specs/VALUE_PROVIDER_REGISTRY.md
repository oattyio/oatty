# Value Provider Registry (As-Built)

## Scope
The provider registry is implemented in `crates/engine/src/provider/registry.rs`.

## Responsibilities
- Resolve provider fetches from provider identifiers.
- Maintain cached provider values with TTL.
- Deduplicate in-flight fetch work for the same cache key.
- Expose contract lookups for provider metadata.
- Build field suggestions through the shared `ValueProvider` trait.

## Data Flow
1. UI requests suggestions for `(command, field, partial, inputs)`.
2. Provider registry evaluates cache status.
3. If cached, suggestions are returned immediately.
4. If uncached, a pending fetch plan is returned (and dispatched by caller when applicable).
5. Completed fetch results are cached and used in subsequent suggestions.

## Registry Inputs
- Command registry (`Arc<Mutex<CommandRegistry>>`)
- Provider fetcher implementation (`ProviderValueFetcher`)
- cache TTL

## Current Constraints
- Suggestion quality depends on available provider metadata/contracts and inferred bindings from generation.
- No secondary index service is used for provider lookups.

## Source Alignment
- `crates/engine/src/provider/registry.rs`
- `crates/engine/src/provider/value_provider.rs`
- `crates/engine/src/provider/suggestion_builder.rs`
- `crates/engine/src/provider/contract_store.rs`


## Related specs

- `/Users/justinwilaby/Development/next-gen-cli/specs/VALUE_PROVIDERS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/COMMANDS.md`
- `/Users/justinwilaby/Development/next-gen-cli/specs/WORKFLOW_TUI.md`
