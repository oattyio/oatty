# Agent management of upstream MCP servers

Status: implementation plan, not as-built behavior. Decisions approved on 2026-10-11; review contracts clarified on 2026-10-12. Runtime implementation has not started.

## Outcome and approved decisions

Agents can add or repair an upstream MCP server through Oatty's own MCP tools, connect it, and inspect the configured servers without receiving credentials. The implementation shares the TUI's configuration and lifecycle functionality.

- Support add and edit. Edits preserve omitted fields, including credentials; removals are explicit.
- Connect immediately by default after configuration. Disabled servers remain disconnected.
- Reload only affected servers. Preserve unrelated connections, commands, caches, status, and lifecycle state.
- Include inexpensive independent start/stop controls. Keep inventory read-only and include stopped/disabled servers.
- Accept raw credentials through existing storage behavior. Existing environment/secret references remain valid inputs, but are not required.

The tool names and payloads below are proposed implementation contracts. The approved decisions above govern their behavior.

## Existing integration points

| Responsibility | Existing code | Reuse |
| --- | --- | --- |
| Core tool registration and shared engine | `crates/mcp/src/server/core.rs`, `McpToolServices` | Thin tool handlers using the existing `Arc<PluginEngine>`; add a `config_path: PathBuf` initialized from `default_config_path()` and a narrow constructor for injecting a temporary test path |
| TUI save orchestration | `crates/tui/src/cmd.rs`, `execute_plugins_save` | Extract persistence/reload work; keep form parsing and UI outcomes in the TUI |
| Configuration types and validation | `crates/mcp/src/config/{model,validation}.rs` | `McpConfig`, `McpServer`, server-name and transport validation |
| Persistence and interpolation | `crates/mcp/src/config/{io,interpolation}.rs` | Default path, tokenization, serialization, runtime interpolation |
| Runtime reload | `crates/mcp/src/plugin/engine.rs`, `update_config` | Make the shared implementation incremental |
| Independent lifecycle | `crates/mcp/src/client/gateway.rs` | Existing per-server start/stop and connection behavior |
| Registry and discovery | `PluginRegistry`, engine status listener, `CommandRegistry` | Update affected metadata and synthetic commands |
| HTTP tool logs | `core.rs` finalizers and `server/log_payload.rs` | Pass safe request/result projections to existing finalizers |

Current behavior needs explicit handling:

- `update_config` stops every registered server, clears every tool cache, rebuilds the registry, and restarts previously running servers. New servers are not automatically connected by this reload method.
- The TUI save path reads unresolved configuration before saving. Runtime configuration snapshots contain resolved values and must not be used as persistence or response payloads.
- `config/io.rs::load_config_from_path` returns `Ok(McpConfig::default())` when `path.exists()` is false, but propagates file-read, JSON parsing, typed deserialization, and validation errors for an existing file. TUI `execute_plugins_save` instead uses `read_to_string` and `from_str::<McpConfig>(...).unwrap_or_default()`: read errors, invalid JSON syntax, and JSON objects that fail typed deserialization all become an empty configuration. Objects that deserialize but fail `validate_config` are rejected later; they do not take the deserialization fallback.
- Keep `load_config_from_path` and its public signature. Preserve default configuration for an actually missing file; add an unresolved-document reader and replace the TUI fallback reads. Existing unreadable/invalid content must never be replaced with defaults. The exact loader boundary is specified below.
- Interpolation failure records `err` and forces runtime `disabled = true`. Keep the user's persisted disabled setting separate from this effective runtime state.
- Synthetic commands are currently removed by canonical command ID. Different servers or an HTTP catalog can share an ID, so targeted cleanup must use MCP server ownership.
- Tool finalizers retain request payloads in HTTP logs. Masking only the response is insufficient.

## Proposed MCP tools

### `mcp_server_configure`

Add or edit one named upstream server and apply its configuration immediately.

| Input | Meaning |
| --- | --- |
| `name` | Existing server-name validation rules |
| `mode` | Required `add` or `edit`; reject an existing name on add and a missing name on edit |
| `configuration` | Partial configuration using existing names: `command`, `args`, `env`, `cwd`, `baseUrl`, `headers`, `auth`, `disabled`, `tags` |
| `clear_fields` | Optional list explicitly clearing supported optional fields/collections |
| `remove_env` | Optional environment keys to remove |
| `remove_headers` | Optional header names to remove |
| `connect` | Optional boolean, default `true` |

Keep `configuration` typed and its generated JSON schema explicit. Reject unknown fields and conflicting operations before mutation. Do not accept runtime `err`, a filesystem destination, or the local Oatty `httpServer` settings through this tool.

Merge rules:

- Omitted fields preserve persisted values. `null` is not an implicit delete; use the removal inputs.
- Supplied scalar fields replace that field. Supplied arrays replace that array; `args` is an array of exact argument strings, not a shell command to split.
- `env` and `headers` accept key/value maps and upsert supplied entries while preserving other entries. Normalize header identity case-insensitively and reject duplicate normalized names. If a legacy selected entry already contains case-insensitive duplicate headers, reject it without mutation; remove all variants with `remove_headers` first, then upsert the corrected value in a later patch.
- A supplied `auth` object patches explicitly supplied existing-model fields; clear individual optional auth fields through documented `clear_fields` paths. Preserve omitted auth fields.
- Reject attempts to both set and clear a field, or upsert and remove the same entry. Reject attempts to clear required fields for the resulting transport.
- Require exactly one transport after merging. A transport switch must explicitly clear the old incompatible connection fields; do not silently discard omitted settings.
- Reuse existing auth support without adding an authentication scheme or OAuth login flow. The current TUI uses environment variables and headers; the tool must not promise capabilities absent from the runtime.
- Runtime diagnostic fields are recomputed, never copied from an agent request or preserved as a stale failure for a repaired server.

### Deterministic merge and conflict rules

Validate request operations before editing a candidate, tokenizing values, saving, or changing lifecycle state. There is no last-operation-wins behavior.

Environment key identity is exact, case-sensitive string equality; keys are not trimmed or case-folded. Header identity is ASCII case-insensitive (`to_ascii_lowercase()`), without trimming. Field paths are exact, case-sensitive schema names. Reject duplicate normalized removal entries and duplicate normalized header upserts. Existing key/name validation still runs on the resulting candidate.

Allowed `clear_fields`: `command`, `args`, `cwd`, `baseUrl`, `env`, `headers`, `auth`, `tags`, and `auth.username`, `auth.password`, `auth.token`, `auth.headerName`, `auth.interactive`. Reject other paths, including `disabled`, `auth.scheme`, and `err`.

| Request combination | Result before any mutation |
| --- | --- |
| `configuration.env.KEY` and `remove_env=["KEY"]` | `patch_conflict`; exact environment-key equality |
| Header upsert `Authorization` and removal `authorization` | `patch_conflict`; normalized header-key equality |
| Clear a field and supply that field | `patch_conflict`, including an empty supplied array/map |
| Clear `auth` and supply any `configuration.auth` object | `patch_conflict` |
| Clear `auth.password` and supply `auth.username` only | Allowed; distinct child fields |
| Clear `auth.password` and supply `auth.password` | `patch_conflict` |
| Clear a parent plus a child, or clear `env`/`headers` plus remove an entry from that collection | `patch_conflict`; redundant overlapping operations are rejected |
| Clear absent `auth`/optional field, or remove a missing key | Successful no-op, subject to final candidate validation |
| Clear `command` and supply `baseUrl` | Allowed switch only if the final candidate is valid HTTP and obsolete stdio-only fields are explicitly cleared |
| Supply `baseUrl` while retaining a nonempty `command` | Invalid resulting transport; neither field wins |

After conflict validation, apply operations to a clone in this order: clear fields → remove map entries → apply scalar/array replacements and map/auth upserts → clear the selected server's diagnostic `err` → deserialize/validate the complete candidate. The order is deterministic but does not make overlapping operations valid.

Clearing an optional field sets it to absent/`None`; clearing `env` or `headers` yields an empty collection. Empty arrays supplied for `args`/`tags` are explicit replacement values. A missing auth parent with only child clears stays absent; an empty auth patch is a no-op. Creating auth from a nonempty patch requires a valid resulting `scheme`.

Transport validation occurs after the merge: exactly one of nonempty `command` or valid HTTP(S) `baseUrl`. For a switch to HTTP, clear `command` and any existing `args`, `cwd`, or `env`; for a switch to stdio, clear `baseUrl` and any existing `headers` or `auth`. Do not fail early merely because a required field is being cleared during a valid switch. Reject incompatible fields remaining in the final candidate instead of silently dropping them. This stricter switch rule applies to the new configure contract, without making the runtime loader reject previously accepted mixed configuration files globally.

Example repair that preserves existing credentials:

```json
{
  "name": "project-tools",
  "mode": "edit",
  "configuration": { "baseUrl": "https://tools.example.com/mcp" }
}
```

Return a structured result containing `name`, `operation`, `changed_fields`, `persisted`, `runtime_applied`, `connection_attempted`, current `status`, and a safe error/next step when applicable. Use the inventory projection if returning configuration details. Never echo submitted credentials.

This is a mutating, open-world tool: connecting stdio executes the configured program; connecting HTTP contacts the configured endpoint. Document those effects and use the existing MCP annotation conventions. Do not label configuration as read-only or guarantee idempotent external effects.

### `mcp_servers_list`

Read-only inventory, with an optional exact `name` filter. Return all configured servers by default, sorted by configured name, including disabled, stopped, and failed servers. An unknown exact name returns a structured not-found result.

Build an explicit response type from unresolved persisted configuration plus runtime status. Do not serialize `McpConfig` or `PluginDetail` directly, and do not call a detail getter that refreshes upstream tools as a side effect of inventory.

Include configured name, transport, configured disabled state, effective runtime disabled state when known, lifecycle status, cached tool count, and masked configuration. Distinguish a configured server absent from the live registry from an empty inventory. Expose configuration/runtime mismatch or unavailable status explicitly.

Masking contract:

- Mask every environment, header, and auth value, regardless of its name, source metadata, or recognizable token format. Return field/key names and configured presence so an agent can patch a value without reading it.
- Never resolve secret/environment references merely to generate inventory. Do not expose backing secret identifiers.
- Mask argument values and arbitrary command/cwd strings in the initial inventory contract; return configured presence and argument count. Agents can replace these fields when the user supplies corrected values.
- For remote endpoints, expose a connection origin derived structurally from the URL. Omit userinfo, path, query, and fragment from this public summary; keep the full configured URL masked. Credentials can occur outside conventional query parameter names.
- Return error categories and actionable guidance, not raw upstream errors, stderr, logs, tool descriptions, or diagnostic strings that might contain credentials.

Concrete response contract: the result is `{ "servers": [...] }`, including for an exact-name lookup. `disabled` is the persisted setting; `effective_disabled` is the runtime setting or `null` if unavailable. `status` uses lowercase lifecycle names, and `tool_count` is the cached count or `null` if unknown. The fixed masking token is `<masked>`, which also indicates that the field is present; absent optional configuration fields are omitted. Every present auth leaf, including username and scheme, uses that same token. The shown origin is structurally derived, never the full URL.

```json
{
  "servers": [
    {
      "name": "project-tools",
      "transport": "http",
      "disabled": false,
      "effective_disabled": false,
      "runtime_available": true,
      "status": "running",
      "tool_count": 4,
      "masked_config": {
        "baseUrl": "<masked>",
        "connection_origin": "https://tools.example.com",
        "headers": { "Authorization": "<masked>" },
        "auth": {
          "scheme": "<masked>",
          "username": "<masked>",
          "password": "<masked>"
        }
      }
    },
    {
      "name": "local-tools",
      "transport": "stdio",
      "disabled": true,
      "effective_disabled": true,
      "runtime_available": true,
      "status": "stopped",
      "tool_count": 0,
      "masked_config": {
        "command": "<masked>",
        "args": ["<masked>", "<masked>"],
        "argument_count": 2,
        "env": { "PROJECT_TOKEN": "<masked>" },
        "cwd": "<masked>"
      }
    }
  ]
}
```

For a configured server absent from runtime, use `runtime_available=false`, `effective_disabled=null`, `status="unknown"`, and `tool_count=null`. A missing file returns `{ "servers": [] }`; an existing unreadable or invalid file returns a safe error, not an empty list. Omit raw `err`/health errors from each item; use a typed category such as `interpolation_failed` when available.

Example configure failure after a successful save and runtime installation, with no raw upstream text:

```json
{
  "name": "project-tools",
  "operation": "edit",
  "persisted": true,
  "runtime_applied": true,
  "connection_attempted": true,
  "status": "error",
  "error": {
    "error_code": "connection_failed",
    "stage": "connect",
    "next_step": "Repair the connection settings or credentials, then retry configuration or start."
  }
}
```

Inventory snapshots acquire the same operation guard briefly for a coherent persisted/runtime read. They do not resolve credentials or emit lifecycle operations. Configuration mismatch is only asserted from known revision information; if its provenance is unavailable, do not infer a match from masked values.

Inventory never starts, stops, connects, or refreshes a server. Existing `search_commands`, `get_command`, and appropriate `run_*` tools continue to discover and invoke connected upstream tools. This feature adds no separate upstream invocation proxy.

### `mcp_server_set_running`

Inputs: `name` and `running` boolean. Reuse existing lifecycle operations for one server.

- Starting an already running server and stopping an already stopped server are successful no-ops.
- Starting a persistently disabled server returns a clear error directing the agent to explicitly enable it through configuration.
- Start/stop changes runtime state only. It does not modify the persisted disabled setting or startup policy.
- Stopping one server removes only its discoverable commands. Starting it restores its tools through existing discovery events.
- Report actual completion/status and sanitized failures. Apply the same lifecycle coordination as configuration changes.

## Shared configuration service

Extract a small shared mutation service in `crates/mcp/src/config/mutation.rs`, and place tool-specific request/result shaping in `crates/mcp/src/server/mcp_server_tools.rs`. Keep persistence rules outside MCP handlers and TUI components.

Scope comment for the service: server mutations target upstream entries in `mcp.json.mcpServers`, not the local `McpHttpServer` TUI component; the TUI-only HTTP autostart variant shares persistence without controlling the local listener's lifecycle.

### Loader compatibility and extraction boundary

Do not replace `load_config_from_path` with the patch service or make it return unresolved configuration. It remains the typed, interpolated runtime loader with the existing `anyhow::Result<McpConfig>` contract.

Add `io.rs::load_raw_config_document_from_path`, returning unresolved `serde_json::Value`. Share this file-read/JSON-parse helper with the runtime loader and mutation service. Use `fs::read_to_string` and match `ErrorKind::NotFound` rather than `path.exists()`, so a permissions/other I/O error cannot masquerade as an absent file. For a missing file return the serialized default document. An existing document must have an object root and an object `mcpServers`; reject malformed JSON and invalid root shape without substitution.

| File/candidate condition | Runtime loader | Shared mutation/TUI save |
| --- | --- | --- |
| File missing (`NotFound`) | `Ok(McpConfig::default())` semantics preserved | Default raw document; add may create the file, edit returns not-found |
| Unreadable, invalid JSON syntax, invalid root shape | Error | Error before save/tokenization/lifecycle changes |
| Parseable document, selected entry contains a typed-invalid field | Deserialization error | Patch raw selected entry first; accept only if the resulting complete candidate deserializes and validates |
| Invalid unrelated server remains after patch | Error | Candidate error; preserve existing file rather than dropping the server |
| Valid candidate | Deserialize → reset runtime diagnostics → interpolate → validate | Validate unresolved candidate → save → reload via the same runtime loader → guarded apply |

Keep `default_config_path`, `load_config`, and `save_config_to_path` in `io.rs` with their existing public signatures. The mutation service calls them; it does not duplicate their persistence/tokenization behavior. Replace fallback reads in `execute_plugins_save`, `execute_plugins_delete`, and `update_mcp_http_autostart` in TUI `cmd.rs` with the shared service. Form parsing and its presentation-specific validation remain in `cmd.rs`.

For `update_mcp_http_autostart`, a missing file explicitly means accept `McpConfig::default()`, set `httpServer.autoStart` to the requested boolean, and create the configuration file through the existing save path. This applies to either toggle value on first use; do not require an upstream server to exist. Preserve the default bind address and an empty `mcpServers` map. An existing unreadable/invalid document returns an error without overwriting it. Update `app.mcp_http_server.auto_start` only after successful persistence; changing this startup preference does not start or stop the currently running local listener. Route this TUI-only mutation through the shared operation guard and `RestorePreviouslyRunning` policy.

### Planned module tree and interfaces

```text
crates/mcp/src/
  config/
    io.rs                 existing I/O APIs + unresolved document reader
    mutation.rs           NEW shared patch/mutation/policy types, outcomes/errors, orchestration
    interpolation.rs      clear diagnostics before each fresh interpolation
    model.rs              existing persisted/runtime model; no agent-only store
    mod.rs                export shared mutation types/functions
  plugin/engine.rs         incremental classification/application; compatible public wrapper
  client/gateway.rs        shared operation guard, guarded lifecycle helpers, autostart revision checks
  server/
    mcp_server_tools.rs    NEW safe adapters and inventory projection
    schemas.rs            three tool request schemas; reuse shared patch field types
    core.rs               register/delegate three new handlers
    mod.rs                register private adapter module
```

Proposed signatures, to be implemented with documented typed errors and outcomes:

```rust
// config/io.rs: public existing entry point retained; new reader exported to the service.
pub fn load_config_from_path(path: &Path) -> anyhow::Result<McpConfig>;
pub fn load_raw_config_document_from_path(path: &Path) -> Result<Value, McpConfigMutationError>;

// config/mutation.rs: shared TUI/tool entry point; acquires the operation guard once.
pub async fn apply_config_mutation(
    engine: Arc<PluginEngine>,
    path: PathBuf,
    mutation: McpConfigMutation,
    policy: ReloadPolicy,
) -> Result<McpConfigMutationOutcome, McpConfigMutationError>;

// Focused internal helpers; no I/O or lifecycle side effects.
fn validate_patch_operations(patch: &McpServerPatch) -> Vec<String>;
fn apply_server_patch(document: &mut Value, mutation: &ConfigureServerMutation)
    -> Result<(), McpConfigMutationError>;
fn deserialize_validated_candidate(document: Value) -> Result<McpConfig, McpConfigMutationError>;
```

`McpConfigMutation` has bounded variants: configure a server using the new patch contract; replace/rename a server using an existing TUI-built `McpServer`; remove a server; set local HTTP autostart. The TUI replacement variant preserves its current form/rename semantics instead of silently changing it into agent-style patching. Tool schemas cannot invoke the TUI-only variants.

Define `McpServerConfigurationPatch`, `McpServerPatch` (configuration plus clear/remove operations), `ConfigureServerMutation`, `ReloadPolicy`, and outcome/error types in `config/mutation.rs`, exporting the needed domain types through `config/mod.rs`. `server/schemas.rs` composes/reuses those typed fields for MCP requests; the shared config service must not depend on private server/schema modules. In particular, the `configuration` field uses `McpServerConfigurationPatch`, while conflict validation takes the complete `McpServerPatch` so it sees both upserts and removals.

`McpConfigMutationOutcome` carries changed field/server names, persistence/runtime stages, and per-server connection outcomes. `McpConfigMutationError` distinguishes read, JSON/root shape, patch conflict, candidate validation, tokenization/save, runtime load, and runtime apply stages, with progress metadata. `validate_patch_operations` returns every conflict field path; its caller wraps a nonempty list in `McpConfigMutationError::PatchConflict`. `ConfigureServerMutation` is a private typed mirror of the configure variant used by the raw-entry helper. Internally retain causes for diagnosis; adapters render only safe categories/field paths. A connection failure after runtime installation is an outcome, not a false claim that saving failed.

The new orchestration entry point owns an `Arc<PluginEngine>` and `PathBuf` so its guarded worker can finish or clean up safely if the caller stops awaiting it. This does not change existing borrowed engine method signatures. The TUI and core already hold an engine `Arc` and clone it when dispatching the mutation. `McpToolServices` carries the selected config path; production initializes it from `default_config_path()`, while tests inject a temporary path through a narrow internal constructor.

The mutation sequence is:

1. Acquire the shared operation guard specified below; retain it continuously across read/merge/save/reload/application and terminal-event acknowledgement.
2. Read and parse the existing JSON without interpolation. A missing file means a new configuration; unreadable/malformed content is an error.
3. Merge the requested changes into the selected raw server entry, then deserialize and validate the candidate. Applying the patch before deserialization allows repair of a parseable JSON entry containing an invalid URL or transport field.
4. Run existing whole-candidate `validate_config`; apply the configure tool's additional exactly-one-transport, nonempty command, incompatible-field, and normalized header rules only to its selected entry. Report field paths without echoing values. Preserve other servers and local HTTP server settings without retroactively imposing the new patch contract on unrelated legacy entries.
5. Set the selected persisted candidate's `server.err = None` before saving. Preserve its persisted `disabled` setting. Save through existing tokenization/persistence behavior, then load/interpolate the runtime candidate.
6. Apply the shared incremental reload and honor the target's requested connection state. Report persistence, runtime application, and connection outcomes separately.

TUI integration delegates its existing save/delete and local HTTP setting writes to the common mutation/persistence boundary where applicable, so in-process tool and TUI writes cannot overwrite one another. Keep TUI form parsing, selection, modal dismissal, and `ExecOutcome` conversion local. Preserve existing TUI rename/delete behavior; those operations are not new agent tools in this scope.

Use existing serialization types and error patterns where practical. Do not introduce a separate agent-owned configuration store, new transport implementation, new secret backend, or a generalized configuration framework. Offload blocking filesystem/keychain work using the project's established async conventions.

This guard coordinates one Oatty process. It does not promise transactional coordination with external file editors or a second Oatty process.

### Explicit diagnostic reset on repair

In `interpolate_config`, set each freshly loaded `server.err = None` immediately before `interpolate_server(server)`. On success leave it `None`; on failure set the new error and force effective runtime `disabled=true` as today. The fresh candidate's initial `disabled` comes from persisted configuration, never the previous runtime snapshot. Successful repair therefore clears a stale diagnostic and restores the user's configured enabled state; explicitly persisted `disabled=true` remains disabled. Do not implement recovery by blindly setting every repaired server to enabled. The configure patch rejects `err` as input and clears any legacy persisted `err` on its selected entry before save.

## Shared targeted reload

Improve `PluginEngine::update_config` rather than adding an agent-only reload path. Break its current responsibilities into focused helpers.

Retain its public signature and use a policy/report-aware internal implementation under the same guard:

```rust
pub async fn update_config(&self, config: McpConfig) -> Result<(), PluginEngineError>;

pub(crate) async fn update_config_under_guard(
    &self,
    config: McpConfig,
    policy: &ReloadPolicy,
    operation: &McpOperationGuard,
) -> Result<McpConfigApplyOutcome, PluginEngineError>;
```

The public wrapper acquires the guard and selects `ReloadPolicy::RestorePreviouslyRunning`. The shared mutation service already owns the guard and passes its caller-selected policy to the internal method, avoiding recursive acquisition. Agent configure uses `ReloadPolicy::ConfigureTarget { name, connect }`; TUI replace/rename, removal, and local HTTP autostart mutations use `RestorePreviouslyRunning`. For the TUI this preserves current policy: an existing same-name running server reconnects, a newly added server stays stopped, and a rename does not automatically start the new name. The service rejects a configure-target policy that disagrees with the selected mutation name. `McpConfigApplyOutcome` records applied servers and their connection outcomes; the wrapper maps structural application failures to its existing error type. The detailed configure adapter uses the report instead of discarding failed connections. These are new internal interfaces, not a change to existing callers' argument/return types.

Split application into these concrete steps: acknowledge earlier lifecycle events → classify old/new config into a `McpConfigDelta` → record affected clients' running state → retire affected queued startup intent → stop affected active clients → acknowledge stop/tool-removal events → invalidate affected commands/cache and apply config/metadata → restart/start policy-selected enabled clients → acknowledge terminal discovery/status events → produce the report. Use focused helpers for classification, affected-client stop, metadata/cache application, and policy-selected start. Metadata-only and unchanged entries do not enter the stop/rebuild branch.

Classify each server as unchanged, metadata-only change, connection change, added, or removed. Compare actual transport inputs and resolved environment/header/auth values; runtime diagnostics and display-only metadata must not create unnecessary reconnects. A newly resolved credential value is a connection change even if its stored reference is unchanged.

| Change | Runtime behavior |
| --- | --- |
| Unchanged, running | Preserve connection, tools, status, and lifecycle state |
| Tags/display metadata only | Update metadata without restarting a running connection |
| Connection inputs changed | Stop affected client, invalidate its commands/cache, update metadata/config, reconnect according to the caller's connection policy |
| Added | Register it; configuration tool connects immediately unless disabled or `connect=false` |
| Removed through existing TUI flow | Stop it and remove only its metadata, lifecycle state, commands, and cache |
| Disabled | Stop it and keep it configured with no active tools |
| Identical tool edit, enabled but stopped | `connect=true` still attempts connection; a configuration no-op is not a lifecycle no-op |

Ordinary shared reload callers can retain existing running-state restoration semantics. The configure tool explicitly supplies its immediate-connect policy for the selected server, including recovery from a previous connection failure. `connect=false` leaves a connection-changing edit disconnected; it need not stop an unchanged running client.

Implementation requirements:

- Replace global registry/cache clearing with updates scoped to the affected server set. Do not reset unrelated lifecycle counters or metadata.
- Ownership is verified in the current model: `CommandSpec::mcp()` returns `Option<&McpCommandSpec>` and `McpCommandSpec` has `plugin_name: String`. `CommandRegistry` currently has `remove_commands(Vec<String>)`; there is no `remove_commands_for_plugin`. Add `pub fn remove_commands_for_plugin(&mut self, plugin_name: &str)` in `crates/registry/src/models.rs`, with predicate `command.mcp().is_some_and(|metadata| metadata.plugin_name == plugin_name)`. Emit `CommandRegistryEvent::CommandsRemoved` with exactly the removed commands, following the existing method. Leave the general ID-based method unchanged.
- Remove the affected server's previous synthetic commands before inserting its replacement tool set, including when tool names/groups change. Do not rely on insertion deduplication to replace stale MCP commands.
- Coordinate pending startup, shutdown, and configuration operations in the existing lifecycle/gateway boundary. TUI lifecycle calls and autostart must participate; a lock used only by MCP handlers is insufficient.
- Ensure a superseded startup cannot register an old client after reconfiguration. Preserve event ordering for old stop/tool-removal and new start/tool-discovery updates. Do not hold a synchronous registry lock across an await.
- The status listener's `ToolsUpdated` handler calls the ownership removal helper before every replacement insertion, including nonempty tool sets; empty updates remove only that plugin. Apply the explicit guard/revision/event completion contract below rather than leaving coordination to implementation guesswork.
- Bound lifecycle operations using existing timeout behavior, handle cancellation without leaving permanent starting/stopping state, and propagate affected-server failure instead of treating a failed reconnect as success.
- Reuse the existing TUI plugin refresh flow and registry/search synchronization. Verify that the visible list refreshes after tool-driven changes without rebuilding UI state or introducing file watching.

### `CommandsRemoved` consumer audit

Audited the current repository's Rust sources for `CommandsRemoved`, `CommandRegistryEvent`, registry receiver types, and structural `.subscribe(...)` calls. `crates/registry/src/models.rs` defines the event and emits it from `remove_commands`; `CommandRegistry::subscribe` exposes the channel, but there are no in-repository calls to that registry subscription and no `CommandsRemoved` listeners. The other subscription calls belong to the separate MCP client-gateway event stream.

`crates/registry/src/search.rs::SearchHandle` reads the shared registry directly. `get_or_build_search_index` compares `compute_registry_search_fingerprint`, which includes `McpCommandSpec.plugin_name` and `tool_name`, so removing a plugin's actual commands invalidates the cached index without an event handler change.

No existing in-repository consumer needs additional logic for ownership-based removal. Preserve `CommandsRemoved(Arc<[CommandSpec]>)` and its meaning: the payload contains exactly the specifications actually removed, regardless of the selection predicate. External consumers are outside this audit; the public payload contract remains unchanged. Add regression evidence that a test subscriber receives only A's removed specifications and that an existing search handle rebuilds after removal while retaining B/HTTP commands with colliding canonical IDs. Re-run the audit during implementation if new listeners have appeared.

### Operation guard, queued work, and event completion

Use one shared `Arc<tokio::sync::Mutex<()>>` owned by `McpClientGateway` and accessible to its owning engine. The cloneable gateway and engine must use this same instance. Represent ownership with a crate-private `McpOperationGuard` permit, passed to internal guarded methods. The mutation service obtains it through an engine accessor; neither TUI nor tool handlers allocate their own lock.

| Entry point | Guard scope |
| --- | --- |
| Shared mutation/TUI save, delete, HTTP setting write | Acquire before reading persisted config; release after runtime application and event acknowledgement |
| Public `PluginEngine::update_config` | Acquire before taking the old runtime snapshot; release after report/event completion |
| Public engine or gateway start/stop/restart | Acquire before reading config/status; retain through handshake/disconnect and final cleanup; restart acquires once for both phases |
| Autostart task | Acquire before checking captured revision and reading current config; retain through startup/cleanup |
| Inventory | Acquire only while making the coherent unresolved/runtime snapshot; no lifecycle side effects |

Introduce permit-taking internal gateway helpers for start, stop, restart, and configuration assignment. Public wrappers acquire once and call these helpers. Engine guarded methods also call helpers, never public wrappers that would reacquire the lock. TUI `spawn_execute_plugin_action` delegates to the engine's guarded public start/stop/restart wrappers; preserve its `ExecOutcome` and messages. Standalone gateway calls also acquire the same guard for client state, but do not require an engine listener to exist. Engine wrappers and tool mutation results additionally wait for engine-event completion. Gateway autostart uses the guarded scheduling contract. The status listener does not acquire this operation guard, so it can process acknowledgements while an operation waits for it. Within guarded mutation/lifecycle methods, acquire subordinate config/client/cache/registry locks after the operation guard. The listener and ordinary read/tool paths retain their own short locks without acquiring the operation guard. No synchronous registry lock is held across await.

The current start path clones config and checks `is_disabled`; current stop reads/removes `active_clients` and does not check disabled. Under the guard, a pending start finishes or is cleaned up before stop/reconfigure examines clients. The same guard protects both engine and gateway config assignments, so an external start cannot observe a partially applied candidate.

A lock alone does not cancel stale scheduled intent. Add a per-server operation revision in the gateway. Autostart captures its revision when scheduled under the guard, then rechecks it after obtaining the guard; skip if a configuration mutation, explicit start/stop/restart, or removal has retired that revision. This includes `connect=false` and a no-op explicit stop, so a queued autostart cannot reconnect a server after either operation returns. Retire revisions only for affected servers; do not invalidate unrelated queued starts.

Engine/gateway-wide shutdown must retire queued intent and finish/cancel scheduled workers without awaiting a worker that is blocked on a guard held by shutdown itself. Engine shutdown cannot wait for a status-listener acknowledgement after it has aborted that listener. Preserve the existing public shutdown surface while giving this terminal path explicit cleanup ordering.

Use an acknowledged FIFO barrier on the gateway-to-engine event flow, consumed/acknowledged only by the engine status listener. Preserve public `ClientGatewayEvent` variants and `subscribe()` by introducing a crate-private event envelope/channel containing `Lifecycle(ClientGatewayEvent)` and `Barrier(...)`. One gateway emit helper publishes lifecycle events to the existing public channel and this internal channel; engine subscription consumes the internal channel. Barrier control messages are internal only. This avoids adding a variant to a public enum and breaking downstream exhaustive matches.

A barrier follows all events emitted by the guarded operation; the listener acknowledges it only after applying preceding status/tool updates successfully. Track lag/application-error state for the whole interval covered by a barrier: any dropped update in that interval makes completion fail, even when the barrier itself survives. Do not reset a dirty listener state merely because a later barrier arrives; require an explicit reconciliation or fail the operation with `runtime_applied`/connection progress reported truthfully. Prepare the listener before waiting, and fail safely if it is unavailable, closes, drops events, encounters an application error, or acknowledgement times out. A stop/reconfigure waits for its old events before rebuilding metadata, then waits for new terminal events before reporting completion. This prevents old queued removals/status from arriving after replacement tools become visible. No transport messages or public event payloads change.

Cancellation/timeout must finish affected startup/shutdown bookkeeping and retire queued intent before releasing the operation boundary. If the caller future is cancelled mid-mutation, keep an owned guarded worker responsible for completing or cleaning up that operation; cancellation cannot merely drop the guard while detached work can still publish an old client. Define completion/error metadata from that worker's terminal result. Add deterministic tests for cancellation and acknowledgement failure.

This deliberately serializes in-process configuration/lifecycle operations, including unrelated new start requests, for a small auditable boundary. It does not stop unrelated running clients or place ordinary upstream tool calls behind this guard. Per-server concurrent mutation is outside the initial feature.

## Credentials, logging, and failure behavior

Raw credentials use the existing `save_config_to_path` / tokenization behavior. The agent does not need separate keychain access. Existing placeholder inputs are still accepted.

This does not make storage stronger than it is today: tokenization examines environment/header values using secret detection; arbitrary secrets are not guaranteed to be identified, `auth` fields are not tokenized by that helper, and the environment secrets backend can retain inline values. File/keychain updates are not one atomic transaction. Document these existing limits instead of claiming that every raw credential is stored in the keychain.

Use the explicit inventory projection for responses and configuration-related logging. Log only the selected server, operation, changed field names, safe status, and error category. Mask or omit configuration values before passing a request to core finalizers; their standard and parsed-response log payloads both retain the supplied request.

Use structured errors with safe `error_code`, field path where useful, stage, `persisted`, `runtime_applied`, and `next_step`. Do not expose raw parse/interpolation/keychain/network error strings or debug dumps of requests/configurations at this boundary.

- Validation and unresolved-file read failures: no configuration file mutation and no lifecycle change.
- Tokenization/save failure: no successful-save claim; acknowledge that existing keychain operations may already have occurred.
- Saved configuration but failed runtime load/application: explicitly report partial success. Preserve the saved configuration and support correction/retry through the same edit tool.
- Failed target connection: retain the saved configuration, report the failure, and allow repair or an independent start retry. Unrelated connections remain active.
- Inventory read/status failure: return an error or explicit unavailable state, never a fabricated empty inventory.

Keep error/log protections scoped to these new operations and their shared reload diagnostics. A repository-wide logging rewrite is outside this feature.

## Implementation sequence and expected diff

1. **Shared configuration mutation contract:** add the unresolved reader in `config/io.rs`; create `config/mutation.rs` with typed requests/outcomes/errors and pure conflict/merge/candidate helpers; export through `config/mod.rs`; clear diagnostics in `interpolation.rs`. Acceptance: missing/invalid-file matrix, each conflict-table row, and successful diagnostic repair are tested in `oatty-mcp`. Stage the orchestration signature before wiring callers; no TUI switch until step 2 provides the guarded apply interface.
2. **Incremental engine and guard:** implement shared permit ownership and guarded gateway helpers first; add autostart revisions and the private event-envelope/acknowledgement channel; create the registry ownership helper and listener replacement cleanup; then extract classify/stop/apply/start/report helpers beneath the compatible `update_config` wrapper. Wire `apply_config_mutation` to that guarded path, then migrate TUI save/delete/HTTP writes and lifecycle calls in `cmd.rs`. Acceptance: A changes while B retains its session/tools, no canonical-ID collateral deletion, no late autostart, no recursive lock deadlock, and unchanged TUI outcomes.
3. **Tool adapters:** add `McpServerConfigureRequest`, `McpServersListRequest`, and `McpServerSetRunningRequest` in `server/schemas.rs`, reusing the shared configuration patch/removal types. Create `server/mcp_server_tools.rs` for the three adapters and masked response types; add `mod mcp_server_tools` in `server/mod.rs`. Register three thin handlers in the core's default tool router and pass only safe request projections to existing finalizers. Acceptance: tools/list schemas, JSON examples, structured errors, and both emitted log forms conform to this plan.
4. **End-to-end validation and documentation:** run the unit/fixture/router/TUI checks listed below; update `specs/PLUGINS.md` and `crates/mcp/README.md` to as-built behavior only after implementation. Existing catalog, command execution, and workflow tool handlers keep their contracts; they are not rewritten for upstream management.

Step 2 has two separately testable boundaries while remaining one integrated delivery:

- **2a, gateway operation boundary:** shared permit acquisition, guarded lifecycle/config assignment helpers, pending-start cleanup, and autostart revision invalidation. Test these in `client/gateway.rs` with controlled startup gates before involving incremental engine reload.
- **2b, engine incremental application:** delta classification, ownership-based command cleanup, metadata/cache preservation, internal event acknowledgement, and policy-driven reconnection. Test these in `plugin/engine.rs` using the completed 2a boundary and local A/B fixtures; then wire TUI/service callers.

Splitting the test phases isolates gateway races from engine state errors and makes failures easier to diagnose. Shipping or assigning them as independent concurrent changes risks mismatched permit ownership, nested acquisition, and uncoordinated old reload calls. Keep one owner for the coupled engine/gateway contract, and do not treat 2a alone as a releasable targeted-reload feature.

Planned adapter signatures (crate-private, with safe JSON results passed to existing core finalizers):

```rust
pub(crate) async fn configure_server(
    engine: &Arc<PluginEngine>, path: &Path, request: &McpServerConfigureRequest,
) -> Result<Value, ErrorData>;
pub(crate) async fn list_servers(
    engine: &PluginEngine, path: &Path, request: &McpServersListRequest,
) -> Result<Value, ErrorData>;
pub(crate) async fn set_server_running(
    engine: &Arc<PluginEngine>, request: &McpServerSetRunningRequest,
) -> Result<Value, ErrorData>;
```

The orchestrator owns contracts, `core.rs`/`schemas.rs` integration, and final acceptance. Useful delegation boundaries are read-only contract/security review; pure patch tests/helpers in `config/mutation.rs`; and registry ownership tests in `crates/registry/src/models.rs`. Engine/gateway coordination is one coupled assignment, not concurrent edits to the two halves. Sequence TUI/tool adapters after the shared signatures settle. Never assign overlapping edits to `engine.rs`, `gateway.rs`, `cmd.rs`, or `core.rs`; provide bounded file permissions and require command/results plus unresolved concerns at handoff.

Expected production touch points are `config/`, `plugin/engine.rs`, `client/gateway.rs`, `registry/models.rs`, TUI `cmd.rs`, and server `core.rs`, `schemas.rs`, `mod.rs` plus one focused adapter module. Avoid editing unrelated components or transport implementations.

The approved targeted-reload increment is estimated at 200–350 production lines and 250–400 test lines across approximately 4–6 files, around 2–3 engineering days including verification. This estimate excludes the baseline configuration/inventory tools, shared save extraction, and credential projection. It is a planning range, not a measured diff. Preserve existing uncommitted catalog/OpenAPI changes when implementing.

## Validation and acceptance criteria

Use temporary configuration files, synthetic credentials, and local test servers; no production endpoints or user keychain contents. Add business-behavior tests rather than tests mirroring helper implementations.

Configuration and security coverage:

- Add stdio/HTTP configurations; preserve unrelated servers and local HTTP settings.
- Repair one field while preserving omitted credentials; explicitly remove entries/fields; preserve exact argument strings; reject conflicting patches and ambiguous transports.
- Repair a parseable invalid target entry. Reject an unreadable/malformed document without replacing it with defaults.
- Cover interpolation/connection failure and verify partial-success fields. Inventory distinguishes configured disabled from effective runtime disabled.
- Test arbitrary unrecognized credential values in environment/header/auth fields, URL credentials/path/query, arguments, and command strings. Assert absence from response text, structured content, both HTTP log payload forms, and operation error payloads.

Approximately 8–10 focused targeted-reload scenarios should prove:

1. Adding A starts it by default and preserves running B's client identity/session.
2. Editing A reconnects A while B remains callable with unchanged commands and lifecycle state.
3. Identical and metadata-only edits do not reconnect running clients.
4. Disabling/removing A removes only A's tools and preserves B/HTTP commands with colliding canonical IDs.
5. Changed tool names/groups replace A's old commands without duplicates or stale discovery entries.
6. Failed A startup reports failure while B remains usable; correcting A allows recovery.
7. A pending startup cannot complete with superseded settings after an edit or stop request.
8. Concurrent in-process TUI/tool configuration mutations preserve both intended updates.
9. Start/stop is idempotent, rejects disabled starts, and leaves persisted startup policy unchanged.
10. Tool-driven updates appear through the existing TUI refresh and command-discovery paths.

### Test ownership and fixture setup

| Location/package | Evidence |
| --- | --- |
| `crates/mcp/src/config/io.rs` tests, `oatty-mcp` | Missing file returns defaults; malformed syntax, invalid root, typed-invalid entries, and unreadable inputs return distinct errors |
| `crates/mcp/src/config/mutation.rs` tests, `oatty-mcp` | Every merge conflict/no-op row, raw-entry repair before typed validation, unchanged file on failure, retained credentials/settings, and `err` reset |
| `crates/mcp/src/plugin/engine.rs` and `client/gateway.rs` tests, `oatty-mcp` | Shared guard, affected reload, startup/autostart races, cancellation, event acknowledgement, and shutdown ordering |
| `crates/registry/src/models.rs` and `search.rs` tests, `oatty-registry` | Ownership removal preserves HTTP and other-plugin commands with identical canonical IDs; correct subscriber payload and existing search-cache invalidation |
| `crates/mcp/src/server/mcp_server_tools.rs` and `core.rs` tests, `oatty-mcp` | Masked examples, tool schemas/router registration, safe errors, and request/response log projections |
| `crates/tui/src/cmd.rs` tests, `oatty-tui` | Shared save/delete/HTTP-write delegation preserves outcomes and rejects fallback overwrite; first autostart toggle creates defaults for either boolean, invalid existing content is preserved, and toggling does not change the local listener's running state; lifecycle routing uses engine wrappers |

Use `#[cfg(test)]` support in `crates/mcp/src/test_support.rs` to share a `LocalMcpFixture` across inline modules. Serve a minimal rmcp `ServerHandler` over Streamable HTTP on `127.0.0.1:0` with `StreamableHttpService`/`LocalSessionManager` and the existing Axum/Tokio dependencies. Provide one deterministic tool, a connection counter, a changed-tool-set variant, and channels/`Notify` to block and release the initialization handshake. Create two independent fixtures A/B. Assert B's `Arc` client identity, connection count, and callable result before/after A's reload; release blocked handshakes explicitly instead of using timing sleeps. Close fixtures and engine workers at teardown with bounded waits.

Use `tempfile::TempDir` for configuration and audit paths. Current default `LogManager::new` uses the user's config directory; provide narrow test-only/internal constructor injection for `LogManager`, engine, and gateway so fixtures use `AuditLogger::with_settings` at a temporary path rather than changing process HOME. Credential interpolation tests use the environment backend and scoped synthetic values; environment-sensitive scenarios must be serialized or subprocess-isolated, not raced in async tests. Keychain interaction can be a separately documented opt-in check with disposable synthetic accounts; it is not required for ordinary fixture tests and never reads user accounts.

Add explicit regressions for: clearing absent auth; exact env vs case-folded header conflict; a valid transport switch after clearing its old required field; stale persisted `err` after successful repair; queued autostart after `connect=false`/stop; listener unavailable/barrier timeout; a dropped tool/status update with its later barrier still delivered; event-application failure; and cancellation while startup or save/application is pending. Exercise both public reload compatibility wrappers and guarded service paths, plus TUI same-name replacement, add, rename, removal, and local HTTP write policy selection.

Implementation verification commands:

```text
cargo fmt --all
cargo fmt --all -- --check
cargo test -p oatty-mcp --locked
cargo test -p oatty-registry --locked
cargo test -p oatty-tui --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --locked --all-targets
cargo doc --workspace --no-deps
```

Manually configure/repair a local test server through MCP, inspect masked inventory, invoke its discovered tool, start/stop it, and confirm the TUI reflects the result while a second server stays connected. Capture terminal or MCP output for review using synthetic data.

Completion means both TUI and agent configuration paths use the shared persistence/reload functionality, targeted updates preserve unrelated servers, all tool results/log projections satisfy the masking contract, and the relevant checks pass. This plan does not implement the feature or claim those checks have run.
