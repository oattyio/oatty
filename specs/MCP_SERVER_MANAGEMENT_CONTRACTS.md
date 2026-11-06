# MCP Server Management — Refined Type & Concurrency Contracts

This document augments `MCP_SERVER_MANAGEMENT_PLAN.md` with concrete type signatures and concurrency contracts needed before implementation delegation. All types target `crates/mcp/src/config/mutation.rs` or `crates/mcp/src/client/gateway.rs` unless noted.

---

## 1. Patch Types

### `ClearField` — typed field-path enum

Prevents invalid clear operations at compile time rather than relying on string matching.

```rust
/// A field that can be explicitly cleared (set to absent/None) during a mutation.
///
/// This enum uses an internal representation so that the clear-field validator can
/// check path containment without string-matching magic values.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClearField {
    // Scalar field clears
    Command,
    Args,
    Cwd,
    BaseUrl,
    // Collection clears (sets to empty)
    Env,
    Headers,
    Tags,
    // Parent auth object clear
    Auth,
    // Auth child field clears (must not be mixed with Auth clear)
    AuthUsername,
    AuthPassword,
    AuthToken,
    AuthHeaderName,
    AuthInteractive,
}
```

### `McpServerConfigurationPatch` — optional scalar/map fields for upsert

Distinguishes "field not mentioned" from "field explicitly null" using `Option<T>`.
The `serde(skip_serializing_if = "Option::is_none")` attribute means omitted fields
are not serialized; the patch service inspects `Some(value)` as "explicitly set" and
`None` as "not mentioned" (preserve persisted value).

```rust
/// Partial configuration for a single server.
///
/// Omitted fields preserve persisted values. An explicit `Some(value)` replaces
/// the persisted value; an absent field (`None`) leaves it unchanged.
/// `null` is not an implicit delete — use `clear_fields` to remove entries.
#[derive(Debug, Clone, Default)]
pub struct McpServerConfigurationPatch {
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<IndexMap<String, String>>,
    pub cwd: Option<String>,
    pub base_url: Option<String>,
    pub headers: Option<IndexMap<String, String>>,
    pub auth: Option<McpAuthPatch>,
    pub disabled: Option<bool>,
    pub tags: Option<Vec<String>>,
}
```

### `McpAuthPatch` — partial auth configuration

```rust
/// Partial authorization configuration for patching.
///
/// Each field is independently optional. An explicit `Some(value)` replaces the
/// persisted value; `None` preserves it. This is distinct from the `Auth` variant
/// in `McpServerPatch` (which clears the entire auth object).
#[derive(Debug, Clone, Default)]
pub struct McpAuthPatch {
    pub scheme: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub token: Option<String>,
    pub header_name: Option<String>,
    pub interactive: Option<bool>,
}
```

### `McpServerPatch` — patch + clear + remove operations combined

The complete mutation input combining configuration upsert, clear operations, and
removal operations. The shared config service validates conflict rules before
applying any mutations.

```rust
/// Complete patch for a single server including upsert, clear, and remove operations.
///
/// The mutation service applies operations in this order:
/// 1. Clear fields (clear_fields)
/// 2. Remove entries from maps (remove_env, remove_headers)
/// 3. Apply scalar/array/map upserts (configuration)
/// 4. Clear the server's diagnostic err field
///
/// Conflict validation rejects:
/// - Supplying a field that is simultaneously cleared
/// - Upserting and removing the same env/header entry
/// - Clearing a parent field and a child field simultaneously
/// - Clearing auth and supplying auth
/// - Clearing a required transport field for the resulting transport
#[derive(Debug, Clone, Default)]
pub struct McpServerPatch {
    pub configuration: McpServerConfigurationPatch,
    pub clear_fields: Vec<ClearField>,
    pub remove_env: Vec<String>,
    pub remove_headers: Vec<String>,
}
```

### Validation invariant

```rust
/// Validate that clear, remove, and upsert operations are not in conflict.
///
/// Returns a list of conflicting field paths. An empty list means the patch
/// is safe to apply (pending typed deserialization of the complete candidate).
pub fn validate_patch_operations(patch: &McpServerPatch) -> Vec<String>;
```

**Conflict rules** (one function, no early return on first conflict):

| Conflict | Rule |
|----------|------|
| Clear `Command` + supply `configuration.command` | Reject |
| Clear `Auth` + supply `configuration.auth` | Reject |
| Clear `Env` + remove `KEY` | Reject |
| Clear `Headers` + remove `KEY` | Reject |
| Header upsert `Authorization` + remove `authorization` | Reject (case-insensitive) |
| Clear `AuthPassword` + supply `auth.username` | Allowed (distinct children) |
| Clear `AuthPassword` + supply `auth.password` | Reject |
| Clear parent (`Auth`) + clear child (`AuthPassword`) | Reject (redundant overlapping) |
| Clear `BaseUrl` + supply a non-empty `command` | Allowed only when existing HTTP-only fields are explicitly cleared |
| Clear `Command` + supply a valid `baseUrl` | Allowed only when existing stdio-only fields are explicitly cleared |
| Supply `baseUrl` while retaining a non-empty `command` | Invalid resulting transport; neither field wins |

---

## 2. `McpConfigMutation` — bounded mutation variants

```rust
/// A mutation targeting the MCP configuration file.
///
/// Bounded variants prevent the tool layer from invoking TUI-only operations.
#[derive(Debug, Clone)]
pub enum McpConfigMutation {
    /// Configure a named server using the patch contract.
    ConfigureServer {
        name: String,
        patch: McpServerPatch,
        mode: ConfigureMode,
    },
    /// Replace a server by name (used by TUI replace/rename path).
    ReplaceServer {
        old_name: String,
        new_name: String,
        server: McpServer,
    },
    /// Remove a server by name (used by TUI delete path).
    RemoveServer(String),
    /// Set the local HTTP server autostart boolean (TUI-only).
    SetHttpAutostart { auto_start: bool },
}

/// Whether the operation is adding or editing an existing server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigureMode {
    Add,
    Edit,
}

/// Private typed mirror used by the raw-JSON patch helper.
#[derive(Debug, Clone)]
struct ConfigureServerMutation {
    name: String,
    patch: McpServerPatch,
    mode: ConfigureMode,
}
```

---

## 3. `McpConfigMutationOutcome` — per-server result projection

```rust
/// Outcome of a configuration mutation, including persistence and runtime stages.
#[derive(Debug, Clone)]
pub struct McpConfigMutationOutcome {
    pub server_name: String,
    pub operation: String, // "configure", "replace", "remove", "set_autostart"
    pub changed_fields: Vec<String>, // names of fields that changed
    pub persisted: bool,
    pub runtime_applied: bool,
    pub connection_attempted: bool,
    pub status: ServerStatus,
    pub error: Option<SafeError>,
}

/// Safe error that never contains raw credentials or debug dumps.
#[derive(Debug, Clone)]
pub struct SafeError {
    pub error_code: ErrorCategory, // connection_failed, validation_failed, etc.
    pub stage: ErrorStage,       // read, parse, patch, save, runtime_load, runtime_apply, connect
    pub next_step: String,       // actionable guidance
}

/// A category of error for structured reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCategory {
    ReadFailed,
    ParseFailed,
    PatchConflict,
    ValidationFailed,
    TokenizationFailed,
    SaveFailed,
    RuntimeLoadFailed,
    RuntimeApplyFailed,
    ConnectionFailed,
}

/// The stage at which an error occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorStage {
    Read,
    Parse,
    Patch,
    Save,
    RuntimeLoad,
    RuntimeApply,
    Connect,
}

/// The current lifecycle status of a server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerStatus {
    Running,
    Stopped,
    Starting,
    Stopping,
    Error,
    Unknown,
}
```

---

## 4. `McpConfigMutationError` — stage-distinguished error types

```rust
/// Errors specific to configuration mutations.
///
/// Distinguishes stages so callers can report appropriate next steps.
#[derive(Debug, thiserror::Error)]
pub enum McpConfigMutationError {
    #[error("Failed to read configuration file: {0}")]
    ReadFailed(std::io::Error),

    #[error("Configuration file contains invalid JSON: {0}")]
    ParseFailed(#[from] serde_json::Error),

    #[error("Root document is not an object with 'mcpServers' key")]
    InvalidRootShape,

    #[error("Patch conflict detected: {0}")]
    PatchConflict(Vec<String>), // list of conflicting field paths

    #[error("Invalid candidate after patch: {0}")]
    ValidationFailed(String),

    #[error("Failed to tokenize configuration: {0}")]
    TokenizationFailed(String),

    #[error("Failed to save configuration file: {0}")]
    SaveFailed(std::io::Error),

    #[error("Failed to reload configuration after save: {0}")]
    RuntimeLoadFailed(String),

    #[error("Failed to apply configuration at runtime: {0}")]
    RuntimeApplyFailed(String),
}
```

---

## 5. `ReloadPolicy` — reload behavior selector

```rust
/// Policy controlling how the shared mutation service applies changes at runtime.
#[derive(Debug, Clone)]
pub enum ReloadPolicy {
    /// Restart all servers that were running before the mutation.
    /// Used by: TUI replace/rename, remove, save.
    RestorePreviouslyRunning,

    /// Connect or disconnect the specified server according to `connect`.
    /// Used by: `mcp_server_configure` tool.
    ConfigureTarget {
        name: String,
        connect: bool,
    },
}
```

---

## 6. Operation Guard — `McpClientGateway` storage and accessor

`PluginEngine` stores `McpClientGateway` directly (not `Arc<McpClientGateway>`):

```rust
// From plugin/engine.rs lines 26-54
pub struct PluginEngine {
    client_manager: McpClientGateway,  // stored directly, not Arc-wrapped
    // ...
}
```

The guard must be owned by `McpClientGateway` and exposed as a **crate-private** method
returning an **opaque permit** — callers cannot manipulate the raw mutex.

**Tokio guard (corrected)**: `std::sync::MutexGuard` is `!Send` and blocks the
executor across `.await`, so a permit wrapping it would make every guarded async
path `!Send` (it would not even compile in the autostart path, where `gateway.rs:71`
spawns a `Send` task) and invites deadlock. Use `Arc<tokio::sync::Mutex<()>>` and
a permit that owns a `tokio::sync::OwnedMutexGuard<()>`. That guard is `Send` and
`'static`, so the permit crosses `await` points without a borrow lifetime and the
cancellation-safe owned guarded worker (plan §"Shared targeted reload") can hold it
after the engine borrow ends. `McpClientGateway` derives `Clone` (`gateway.rs:26`);
the `Arc` field preserves one shared instance across engine and gateway clones
(plan L352).

```rust
// In McpClientGateway (crate: oatty-mcp, module: client)
pub struct McpClientGateway {
    // existing fields ...
    operation_guard: Arc<tokio::sync::Mutex<()>>,  // NEW: shared across gateway/engine clones
}

/// An opaque permit for the shared operation guard.
///
/// This struct cannot be constructed outside of `McpClientGateway` and
/// provides no public methods. The permit must be held (not dropped) for the
/// duration of the guarded operation. The permit is `Send` and `'static`
/// (`OwnedMutexGuard` is `Send`), so it can cross `await` points and outlive
/// the `&McpClientGateway` borrow. It is not `Clone`.
#[derive(Debug)]
pub struct McpOperationGuard {
    _guard: tokio::sync::OwnedMutexGuard<()>,
}

impl McpClientGateway {
    /// Acquire the shared operation guard. Async because
    /// `tokio::sync::Mutex::lock_owned` is async.
    ///
    /// # Scope
    ///
    /// - Shared mutation/TUI save: acquire before reading config, release after runtime apply
    /// - PluginEngine::update_config: acquire before old snapshot, release after report
    /// - PluginEngine::start/stop/restart: acquire before status read, retain through cleanup
    /// - Autostart task: acquire before checking revision, retain through startup
    /// - Inventory: acquire briefly for coherent snapshot, no lifecycle side effects
    ///
    /// # Lock ordering
    ///
    /// The operation guard is always acquired FIRST; subordinate locks
    /// (`config`, `active_clients`, `starting`, `stopping` — all tokio mutexes,
    /// `gateway.rs:29-37`) are taken only while the guard is held, never the
    /// reverse. `tokio::sync::Mutex` is non-reentrant: internal helpers take
    /// `&McpOperationGuard` and never reacquire the guard. The engine status
    /// listener and ordinary read/tool paths never acquire the guard
    /// (plan L362), so no cycle exists.
    ///
    /// # Shutdown
    ///
    /// Engine/gateway-wide shutdown must not hold the guard while joining
    /// autostart tasks that await it (plan L368); follow the shutdown ordering
    /// contract in the plan.
    pub(crate) async fn acquire_operation_guard(&self) -> McpOperationGuard {
        let guard = Arc::clone(&self.operation_guard).lock_owned().await;
        McpOperationGuard { _guard: guard }
    }
}
```

**Usage**: Callers receive `McpOperationGuard` (owned, `'static`). The permit
cannot be cloned, constructed, or inspected. It releases the mutex when dropped.

---

## 7. PluginEngine accessor for the guard

Since `PluginEngine` stores `McpClientGateway` directly (not `Arc`), the accessor
is a method on `PluginEngine` that delegates to `client_manager`:

```rust
// In PluginEngine
use crate::client::McpOperationGuard;

impl PluginEngine {
    /// Acquire the shared operation guard from the client gateway.
    ///
    /// This is the only way callers (including the shared mutation service)
    /// obtain the guard permit. The permit must be held for the duration of
    /// the guarded operation.
    pub(crate) async fn acquire_operation_guard(&self) -> McpOperationGuard {
        self.client_manager.acquire_operation_guard().await
    }
}
```

---

## 8. Per-Server Revision Counter

A per-server generation map in `McpClientGateway` increments on every guarded
configuration mutation (configure, remove, replace) and explicit start/stop/restart
(plan L366). Entries are never removed: a removed server keeps its tombstone, so a
later re-addition cannot reuse a revision held by a stale autostart task. The map is
bounded by the number of distinct names ever configured.

**Storage**: `Arc<std::sync::RwLock<HashMap<String, u64>>>`. `McpClientGateway`
derives `Clone`, so the `Arc` is required to keep one revision map shared across all
gateway clones. Revisions are read or incremented only in short synchronous lock
sections; never hold this lock across an `await`. Increments occur under the shared
async operation guard.

```rust
// In McpClientGateway
pub struct McpClientGateway {
    // existing fields ...
    operation_guard: Arc<tokio::sync::Mutex<()>>,
    server_revisions: Arc<std::sync::RwLock<HashMap<String, u64>>>,
}

impl McpClientGateway {
    /// Return the current generation, with zero as the initial generation.
    /// Capture this while holding the operation guard when scheduling autostart.
    pub(crate) fn current_server_revision(&self, name: &str) -> u64 {
        self.server_revisions.read().unwrap().get(name).copied().unwrap_or(0)
    }

    /// Increment the generation under the operation guard and return its new value.
    /// Entries are retained as tombstones across removal and re-addition.
    pub(crate) fn increment_server_revision(&self, name: &str) -> u64 {
        let mut revisions = self.server_revisions.write().unwrap();
        let next = revisions.get(name).copied().unwrap_or(0) + 1;
        revisions.insert(name.to_string(), next);
        next
    }

    /// Check the expected generation after acquiring the operation guard.
    /// A never-mutated server has generation zero and must match a captured zero.
    pub(crate) fn check_server_revision(&self, name: &str, expected: u64) -> bool {
        self.server_revisions.read().unwrap().get(name).copied().unwrap_or(0) == expected
    }
}
```

When scheduling autostart, acquire the operation guard, read the current generation,
and capture it in the queued task. The task reacquires the guard before reading
current config or beginning startup, then compares the captured generation. Skip
startup if it differs. Explicit start/stop/restart and every affected configuration
mutation increment the generation, including no-op stop and `connect=false`, so
queued intent is retired when those operations complete. The valid autostart holds
the guard through connection/cleanup; no competing mutation can change the generation
mid-handshake. Do not add a removal API that deletes a map entry: a stale task across
remove/re-add must observe a strictly newer tombstone generation.

---

## 9. Barrier Envelope — Acknowledgement Mechanism

A private, single-consumer FIFO channel carries lifecycle events and barriers to
the engine status listener. An acknowledged barrier proves that the listener has
applied all earlier events before metadata/cache replacement proceeds. The existing
public `ClientGatewayEvent` broadcast channel and `subscribe()` API remain unchanged.

```rust
// Internal types: NOT added to the public ClientGatewayEvent enum.
#[derive(Debug)]
enum InternalEvent {
    Lifecycle(ClientGatewayEvent),
    Barrier(AckFence),
    Reconcile {
        snapshot: EngineStateSnapshot,
        acknowledge: oneshot::Sender<Result<(), String>>,
    },
}

#[derive(Debug)]
struct AckFence {
    sequence: usize,
    acknowledge: oneshot::Sender<Result<(), BarrierLagError>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum BarrierLagError {
    #[error("engine status listener is unavailable")]
    ListenerUnavailable,
    #[error("engine status listener state is dirty and requires reconciliation")]
    ListenerDirty,
    #[error("engine status listener did not acknowledge before the deadline")]
    AcknowledgementTimeout,
}

/// Authoritative state used to repair listener-owned projections.
#[derive(Debug)]
struct EngineStateSnapshot {
    config: McpConfig,
    servers: HashMap<String, ServerRuntimeSnapshot>,
}

#[derive(Debug)]
struct ServerRuntimeSnapshot {
    status: ServerStatus,
    tools: Arc<Vec<McpToolMetadata>>,
}
```

**Channel choice and ownership**: use `tokio::sync::mpsc::UnboundedSender<InternalEvent>`
plus exactly one `UnboundedReceiver<InternalEvent>`. `InternalEvent` contains a
non-`Clone` oneshot sender, so it cannot be carried by Tokio broadcast, whose
messages must be `Clone`. The gateway is `Clone`; share the sender, receiver slot,
listener state, and sequence counter through `Arc`s. Store the receiver in a
one-shot `Arc<tokio::sync::Mutex<Option<UnboundedReceiver<InternalEvent>>>>` slot;
the engine takes it exactly once when starting its status listener. The channel is
created in `McpClientGateway::new`. Engine lifecycle operations must prepare/start
the listener before emitting internal lifecycle events. Before the
engine listener is active, standalone gateway operations publish only on the public
channel and do not queue unused internal events.

```rust
pub struct McpClientGateway {
    // existing fields ...
    event_tx: broadcast::Sender<ClientGatewayEvent>, // public, unchanged
    internal_tx: mpsc::UnboundedSender<InternalEvent>,
    internal_rx: Arc<tokio::sync::Mutex<Option<mpsc::UnboundedReceiver<InternalEvent>>>>,
    internal_listener_active: Arc<AtomicBool>,
    barrier_counter: Arc<AtomicUsize>,
}
```

Initialization creates both channels and retains the internal receiver until the
engine listener takes it:

```rust
let (event_tx, _) = broadcast::channel(64);
let (internal_tx, internal_rx) = mpsc::unbounded_channel();
let internal_rx = Arc::new(tokio::sync::Mutex::new(Some(internal_rx)));
```

The gateway exposes a one-time receiver handoff to the engine; the engine marks the
listener active only after acquiring the receiver and immediately before spawning:

```rust
pub(crate) async fn take_internal_receiver(
    &self,
) -> Result<mpsc::UnboundedReceiver<InternalEvent>, BarrierLagError> {
    self.internal_rx
        .lock()
        .await
        .take()
        .ok_or(BarrierLagError::ListenerUnavailable)
}
```

The listener takes the receiver once and marks itself active immediately before
spawning its task. `PluginEngine` shares `listener_dirty: Arc<AtomicBool>` with the
task; the task owns a small drop guard that marks the listener inactive on normal
exit, abort, or panic. A later attempt to take an already-consumed receiver
returns `ListenerUnavailable`. Shutdown is terminal for that engine/gateway instance;
a subsequent engine start uses a newly constructed engine and gateway. Application
errors do not terminate the listener; the explicit reconciliation control event below
repairs dirty listener state without replacing the receiver.

**Lifecycle publication**: every gateway lifecycle event is published to the
existing public broadcast channel. When the engine listener is active, publish the
same event to the internal mpsc channel in FIFO order. A standalone gateway with no
engine listener publishes only to the public channel and does not require an internal
consumer. If the active listener's receiver has closed, internal send fails and the
engine operation reports `ListenerUnavailable`; it must not continue claiming runtime
application succeeded.

Engine operations that require completion enqueue all lifecycle events in order and
then enqueue one barrier. These two steps are serialized by the shared operation
guard, so another guarded operation cannot interleave events between the terminal
event and its barrier. A barrier is not itself a lifecycle event and is never sent to
the public channel.

```rust
pub(crate) fn emit_lifecycle(&self, event: ClientGatewayEvent) -> Result<(), BarrierLagError> {
    let _ = self.event_tx.send(event.clone());
    if self.internal_listener_active.load(Ordering::Acquire) {
        self.internal_tx
            .send(InternalEvent::Lifecycle(event))
            .map_err(|_| BarrierLagError::ListenerUnavailable)?;
    }
    Ok(())
}

pub(crate) fn emit_barrier(
    &self,
) -> Result<oneshot::Receiver<Result<(), BarrierLagError>>, BarrierLagError> {
    if !self.internal_listener_active.load(Ordering::Acquire) {
        return Err(BarrierLagError::ListenerUnavailable);
    }
    let (acknowledge, ack) = oneshot::channel();
    let fence = AckFence {
        sequence: self.barrier_counter.fetch_add(1, Ordering::Relaxed) + 1,
        acknowledge,
    };
    self.internal_tx
        .send(InternalEvent::Barrier(fence))
        .map_err(|_| BarrierLagError::ListenerUnavailable)?;
    Ok(ack)
}
```

The guarded operation awaits the returned acknowledgement receiver with the
configured lifecycle timeout:

```rust
pub(crate) async fn wait_for_barrier_ack(
    acknowledgement: oneshot::Receiver<Result<(), BarrierLagError>>,
    timeout: Duration,
) -> Result<(), BarrierLagError> {
    match tokio::time::timeout(timeout, acknowledgement).await {
        Ok(Ok(result)) => result,
        Ok(Err(_dropped)) => Err(BarrierLagError::ListenerUnavailable),
        Err(_elapsed) => Err(BarrierLagError::AcknowledgementTimeout),
    }
}
```

The explicit `Result` distinguishes a dirty interval from a dropped listener: the
listener sends `Err(ListenerDirty)` when it reaches a dirty barrier, while a dropped
oneshot sender means the listener is unavailable. Do not wrap the non-blocking oneshot
`send` in a timeout.

**Listener loop**:

```rust
loop {
    match internal_rx.recv().await {
        Some(InternalEvent::Lifecycle(event)) => {
            if apply_lifecycle_event(&event).is_err() {
                listener_dirty.store(true, Ordering::Release);
            }
        }
        Some(InternalEvent::Barrier(fence)) => {
            let result = if listener_dirty.load(Ordering::Acquire) {
                Err(BarrierLagError::ListenerDirty)
            } else {
                Ok(())
            };
            let _ = fence.acknowledge.send(result);
        }
        Some(InternalEvent::Reconcile { snapshot, acknowledge }) => {
            let result = rebuild_listener_state(
                snapshot,
                &mut registry,
                &tool_cache,
                &mut synthetic_specs,
                &command_registry,
            )
            .await;
            if result.is_ok() {
                listener_dirty.store(false, Ordering::Release);
            }
            let _ = acknowledge.send(result.map_err(|error| error.to_string()));
        }
        None => break,
    }
}
```

The internal mpsc channel has no broadcast lag condition; an `UnboundedSender::send`
failure means the receiver is closed. Any lifecycle application error marks the
shared `listener_dirty: Arc<AtomicBool>` state. Dirty state covers every subsequent
barrier and is never cleared by a barrier alone. The operation whose terminal barrier
returns `ListenerDirty` reports partial runtime progress truthfully and leaves the
dirty flag set. Before the next guarded mutation or lifecycle operation changes state, the engine
checks that flag and calls the reconciliation helper below. It sends
`InternalEvent::Reconcile` with an authoritative snapshot of current configuration,
lifecycle state, and active client tool sets. The listener rebuilds its plugin
metadata, tool cache, synthetic command map, and registry commands from that snapshot
in FIFO order. It acknowledges reconciliation only after the full rebuild succeeds;
then it clears the shared dirty flag. If reconciliation fails, retain dirty state,
return `ListenerDirty`, and fail the guarded operation before further mutation. Do not
hold a synchronous registry lock across an `await`. The reconciliation request itself
is handled by the same listener task and must not acquire the operation guard.

```rust
pub(crate) async fn reconcile_status_listener_under_guard(
    &self,
    _guard: &McpOperationGuard,
) -> Result<(), PluginEngineError>;
```

The engine sends the reconciliation request and awaits its oneshot result using the
same configured lifecycle timeout as barriers. The listener alone clears the dirty
flag after a successful rebuild. Every guarded operation checks `listener_dirty`
after acquiring the operation guard and invokes this helper before performing any
new mutation when the flag is set.

**Shutdown**: engine shutdown aborts the status listener before gateway shutdown
(plan L368). Aborting the task drops its receiver and any pending barrier senders;
waiting operations fail rather than being awaited by shutdown. The listener's drop
guard clears the active flag. Gateway shutdown retires queued revisions and completes
or cancels autostart workers without waiting for workers blocked behind a guard held
by shutdown itself.

**Standalone gateway calls** do not wait for engine-event acknowledgement and do not
require an engine listener (plan L362). Engine wrappers and shared mutation/tool
operations use barriers and must fail safely if the listener is unavailable, dirty,
closed, or times out. Public event variants and payloads remain unchanged.

---

## 10. Cancellation-safe guarded workers

Any operation whose caller cancellation could otherwise release the operation guard
while detached lifecycle work can still publish state must move the owned permit into
an owned worker. For configuration mutations, the entry point clones/owns the engine
`Arc`, path, mutation, and reload policy in a Tokio task; the worker acquires the
operation guard before reading persisted configuration and retains it through
validation, save, runtime application, barriers, and terminal cleanup. The caller
awaits the worker's join result. If the caller is cancelled, dropping its join handle
detaches the worker; it continues to a terminal result or performs bounded cleanup
before dropping the permit. The worker, not the caller future, is authoritative for
persistence/runtime/connection progress metadata.

Lifecycle start/stop/restart workers similarly own a cloned gateway and the guard
permit across handshakes, disconnects, event acknowledgement, revision retirement,
and cleanup of `starting`/`stopping` state. Cancellation or timeout must not leave a
server permanently transitional or allow a superseded client to register after the
operation boundary is released. Shutdown explicitly aborts/cancels these workers and
retires their intent without awaiting a worker blocked on shutdown-held coordination.

No worker may reacquire the operation guard recursively. Worker internals receive the
owned permit (or a borrow of it) and call permit-taking gateway/engine helpers.

---

## 11. `apply_config_mutation` — shared service entry point

`McpToolServices` must carry the config path used by the shared mutation service.
Initialize it from `default_config_path()` in production, and provide a narrow
internal/test constructor that injects a temporary path. Do not make tests read or
write the user's config directory, and do not resolve the environment override after
services have been created.

```rust
/// Apply a configuration mutation with shared operation guard semantics.
///
/// This is the single entry point for both tool and TUI mutation paths.
/// It acquires the guard once and retains it across read/merge/save/reload.
///
/// # Arguments
/// * `engine` — The PluginEngine that owns the shared gateway directly.
/// * `path` — The configuration file path.
/// * `mutation` — The mutation to apply.
/// * `policy` — The reload behavior for runtime application.
///
/// # Returns
/// A `McpConfigMutationOutcome` with per-stage success/failure metadata.
pub async fn apply_config_mutation(
    engine: Arc<PluginEngine>,
    path: PathBuf,
    mutation: McpConfigMutation,
    policy: ReloadPolicy,
) -> Result<McpConfigMutationOutcome, McpConfigMutationError>;
```

**Note**: `PluginEngine` stores `McpClientGateway` directly, not in an `Arc`. The
service owns an `Arc<PluginEngine>` so its cancellation-safe worker can retain the
engine while awaiting. The worker acquires the guard once, then passes its permit to
guarded helpers without recursive acquisition.

---

## 12. `update_config_under_guard` — guarded engine reload

The internal report distinguishes structural apply failure from per-server
connection outcomes. Connection failures after configuration installation are data
in the report, not a false claim that persistence failed:

```rust
pub(crate) struct McpConfigApplyOutcome {
    pub applied_servers: Vec<McpServerApplyOutcome>,
}

pub(crate) struct McpServerApplyOutcome {
    pub name: String,
    pub status: ServerStatus,
    pub runtime_applied: bool,
    pub connection_attempted: bool,
    pub connection_error: Option<PluginEngineError>, // internal cause; never serialize directly
}
```

The tool/TUI mutation adapter maps internal failures into safe categories and
progress metadata; it never exposes `connection_error` text directly.

```rust
// In PluginEngine
impl PluginEngine {
    /// Apply a configuration update under the shared operation guard.
    ///
    /// This is the internal reload method. The public `update_config` wraps it
    /// with `ReloadPolicy::RestorePreviouslyRunning`. The shared mutation service
    /// passes its caller-selected policy.
    ///
    /// Returns `McpConfigApplyOutcome` with per-server connection results.
    pub(crate) async fn update_config_under_guard(
        &self,
        config: McpConfig,
        policy: &ReloadPolicy,
        _guard: &McpOperationGuard,  // borrowed, not moved
    ) -> Result<McpConfigApplyOutcome, PluginEngineError>;
}
```

---

## 13. `remove_commands_for_plugin` — ownership-based removal

Following the existing `remove_commands` pattern (`models.rs:574`):

```rust
impl CommandRegistry {
    /// Remove commands owned by a specific plugin.
    ///
    /// Uses the same `extract_if` pattern as `remove_commands`.
    /// Emits `CommandsRemoved` with exactly the specs actually removed.
    pub fn remove_commands_for_plugin(&mut self, plugin_name: &str) -> Vec<CommandSpec> {
        let commands: Vec<_> = self.commands.extract_if(.., |c| {
            c.mcp().is_some_and(|meta| meta.plugin_name == plugin_name)
        }).collect();
        if let Some(tx) = self.event_tx.as_ref() {
            let _ = tx.send(CommandRegistryEvent::CommandsRemoved(Arc::from(commands.clone())));
        }
        commands
    }
}
```

**Existing reference** (`models.rs:574`):

```rust
pub fn remove_commands(&mut self, command_ids: Vec<String>) {
    let set: HashSet<String> = command_ids.into_iter().collect();
    let commands: Vec<_> = self.commands.extract_if(.., |c| set.contains(&c.canonical_id())).collect();
    if let Some(tx) = self.event_tx.as_ref() {
        let _ = tx.send(CommandRegistryEvent::CommandsRemoved(Arc::from(commands)));
    }
}
```

The new helper follows the same pattern with `c.mcp().is_some_and(|meta| meta.plugin_name == plugin_name)` as the predicate.

---

## 14. Shared configuration service — module tree

```text
crates/mcp/src/
  config/
    io.rs                 existing I/O APIs + unresolved document reader
    mutation.rs           NEW: patch/mutation/policy types, orchestration entry
    interpolation.rs      clear diagnostics before each fresh interpolation
    model.rs              existing persisted/runtime model; no agent-only store
    mod.rs                export shared mutation types/functions
  plugin/engine.rs        incremental classification/application; compatible public wrapper
  client/gateway.rs       shared operation guard, guarded lifecycle helpers, autostart revision checks
  server/
    mcp_server_tools.rs   NEW: safe adapters and inventory projection
    schemas.rs            three tool request schemas; reuse shared patch field types
    core.rs               register/delegate three new handlers
    mod.rs                register private adapter module
```

---

## Summary of All New Types and Methods

| Type / Method | Module | Purpose |
|---------------|--------|---------|
| `ClearField` | `config/mutation.rs` | Typed enum for clear operations |
| `McpServerConfigurationPatch` | `config/mutation.rs` | Optional scalar/map fields for upsert |
| `McpAuthPatch` | `config/mutation.rs` | Partial auth config for patching |
| `McpServerPatch` | `config/mutation.rs` | Complete patch (upsert + clear + remove) |
| `validate_patch_operations` | `config/mutation.rs` | Conflict validation function |
| `McpConfigMutation` | `config/mutation.rs` | Bounded mutation enum |
| `ConfigureMode` | `config/mutation.rs` | Add vs edit mode |
| `McpConfigMutationOutcome` | `config/mutation.rs` | Per-server result projection |
| `SafeError` | `config/mutation.rs` | Safe error with code/stage/next_step |
| `ErrorCategory` | `config/mutation.rs` | Structured error category enum |
| `ErrorStage` | `config/mutation.rs` | Stage enum for error reporting |
| `ServerStatus` | `config/mutation.rs` | Server lifecycle status enum |
| `McpConfigMutationError` | `config/mutation.rs` | Stage-distinguished errors |
| `ReloadPolicy` | `config/mutation.rs` | Reload behavior selector |
| `ConfigureServerMutation` | `config/mutation.rs` | Private typed mirror used by raw-entry patch helper |
| `McpConfigApplyOutcome` | `plugin/engine.rs` | Applied-server and connection outcomes for shared reload |
| `McpOperationGuard` | `client/gateway.rs` | Opaque guard permit (Send, 'static, !Clone) |
| `BarrierLagError` | `client/gateway.rs` | Typed unavailable/dirty/timeout barrier result |
| `McpClientGateway::acquire_operation_guard()` | `client/gateway.rs` | Guard accessor (async) |
| `McpClientGateway::take_internal_receiver()` | `client/gateway.rs` | One-time handoff of the internal FIFO receiver |
| `server_revisions: Arc<RwLock<HashMap<String, u64>>>` | `client/gateway.rs` | Shared per-server generations; tombstones retained |
| `McpClientGateway::current_server_revision()` | `client/gateway.rs` | Capture generation, defaulting to zero |
| `McpClientGateway::increment_server_revision()` | `client/gateway.rs` | Increment + return generation (tombstone stays) |
| `McpClientGateway::check_server_revision()` | `client/gateway.rs` | Check generation against expected, defaulting to zero |
| `barrier_counter: Arc<AtomicUsize>` | `client/gateway.rs` | Shared barrier sequence across gateway clones |
| `InternalEvent` | `client/gateway.rs` | Private FIFO mpsc envelope for lifecycle, barrier, and reconcile |
| `AckFence` | `client/gateway.rs` | Barrier with oneshot ack (listener holds Sender) |
| `EngineStateSnapshot` | `plugin/engine.rs` | Authoritative config, lifecycle, and tool state for reconciliation |
| `ServerRuntimeSnapshot` | `plugin/engine.rs` | Per-server lifecycle status and active tool set |
| `McpClientGateway::emit_lifecycle()` | `client/gateway.rs` | Publish lifecycle events to public/internal channels |
| `McpClientGateway::emit_barrier()` | `client/gateway.rs` | Enqueue barrier, return acknowledgement Receiver |
| `PluginEngine::listener_dirty: Arc<AtomicBool>` | `plugin/engine.rs` | Persistent dirty-state signal shared with listener task |
| `PluginEngine::reconcile_status_listener_under_guard()` | `plugin/engine.rs` | Rebuild dirty listener-owned state from an authoritative snapshot |
| `McpToolServices::config_path: PathBuf` | `server/core.rs` | Production config path with test-only injection |
| `wait_for_barrier_ack()` | `client/gateway.rs` | Wait for barrier ack with timeout |
| Cancellation-safe guarded worker | `config/mutation.rs`, `client/gateway.rs` | Owns permit through terminal cleanup if caller cancels |
| `apply_config_mutation()` | `config/mutation.rs` | Shared service entry point |
| `PluginEngine::update_config_under_guard()` | `plugin/engine.rs` | Guarded reload method |
| `PluginEngine::acquire_operation_guard()` | `plugin/engine.rs` | PluginEngine guard accessor (async) |
| `CommandRegistry::remove_commands_for_plugin()` | `registry/models.rs` | Ownership-based removal |
