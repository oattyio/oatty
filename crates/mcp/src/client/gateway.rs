//! McpClientGateway: registry and lifecycle management.

use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use anyhow::Result;
use rmcp::model::InitializeResult;
use serde_json::Map as JsonMap;
use thiserror::Error;
use tokio::{
    sync::{Mutex, OwnedMutexGuard, RwLock as TokioRwLock, broadcast, mpsc, oneshot},
    task::JoinHandle,
    time::{Duration, timeout},
};

use crate::{
    config::McpConfig,
    logging::{AuditEntry, LogManager},
    types::{HealthStatus, McpToolMetadata, PluginStatus},
};

use super::core::McpClient;

/// Registry and lifecycle manager for MCP clients.
#[derive(Clone, Debug)]
pub struct McpClientGateway {
    /// Active client handles keyed by plugin name.
    active_clients: Arc<Mutex<HashMap<String, Arc<Mutex<McpClient>>>>>,
    /// Names currently in the process of starting to avoid races and report transitional status.
    starting: Arc<Mutex<HashSet<String>>>,
    /// Names currently in the process of stopping for transitional status reporting.
    stopping: Arc<Mutex<HashSet<String>>>,
    /// Join handles for autostart tasks that are still executing.
    autostart_tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
    /// Parsed MCP configuration guarded for runtime updates.
    config: Arc<TokioRwLock<McpConfig>>,
    /// Centralized logging manager (shared with engine/TUI).
    log_manager: Arc<LogManager>,
    /// Broadcast channel for lifecycle events emitted to interested listeners.
    event_tx: broadcast::Sender<ClientGatewayEvent>,
    /// Shared async mutex serializing in-process configuration and lifecycle operations.
    operation_guard: Arc<Mutex<()>>,
    /// Per-server generation counters used to retire stale queued intent.
    server_revisions: Arc<RwLock<HashMap<String, u64>>>,
    /// Internal FIFO sender for engine status listener events.
    internal_tx: mpsc::UnboundedSender<InternalEvent>,
    /// One-time receiver handoff slot consumed by the engine status listener.
    internal_rx: Arc<Mutex<Option<mpsc::UnboundedReceiver<InternalEvent>>>>,
    /// Whether the engine status listener has taken the internal receiver.
    internal_listener_active: Arc<AtomicBool>,
    /// Monotonic barrier sequence counter.
    #[allow(dead_code)]
    barrier_counter: Arc<AtomicUsize>,
}

impl McpClientGateway {
    /// Create a new manager from config.
    pub fn new(config: McpConfig) -> Result<Self> {
        Self::new_with_log_manager(config, Arc::new(LogManager::new()?))
    }

    /// Create a new manager with an injected log manager.
    pub(crate) fn new_with_log_manager(config: McpConfig, log_manager: Arc<LogManager>) -> Result<Self> {
        let (event_tx, _rx) = broadcast::channel(64);
        let (internal_tx, internal_rx) = mpsc::unbounded_channel();

        Ok(Self {
            active_clients: Arc::new(Mutex::new(HashMap::new())),
            starting: Arc::new(Mutex::new(HashSet::new())),
            stopping: Arc::new(Mutex::new(HashSet::new())),
            autostart_tasks: Arc::new(Mutex::new(Vec::new())),
            config: Arc::new(TokioRwLock::new(config)),
            log_manager,
            event_tx,
            operation_guard: Arc::new(Mutex::new(())),
            server_revisions: Arc::new(RwLock::new(HashMap::new())),
            internal_tx,
            internal_rx: Arc::new(Mutex::new(Some(internal_rx))),
            internal_listener_active: Arc::new(AtomicBool::new(false)),
            barrier_counter: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// Start the manager and autostart all non-disabled plugins from config.
    ///
    /// Plugin startup is scheduled onto the async runtime so the caller does
    /// not block on potentially slow handshake operations. Errors are logged
    /// but do not stop initialization.
    pub async fn start(&self) -> Result<(), ClientGatewayError> {
        let config_snapshot = self.config.read().await.clone();
        for (name, server) in &config_snapshot.mcp_servers {
            if server.is_disabled() {
                continue;
            }
            let manager = self.clone();
            let plugin_name = name.clone();
            // Capture the current generation while holding the operation guard so a
            // later mutation can retire this queued intent.
            let revision = {
                let _guard = self.acquire_operation_guard().await;
                self.current_server_revision(&plugin_name)
            };
            let autostart_handle = tokio::spawn(async move {
                let guard = manager.acquire_operation_guard().await;
                if !manager.check_server_revision(&plugin_name, revision) {
                    tracing::debug!(plugin = %plugin_name, "Autostart skipped: revision retired");
                    return;
                }
                if let Err(err) = manager.guarded_start_plugin(&plugin_name, &guard).await {
                    tracing::warn!(plugin = %plugin_name, error = %err, "Autostart failed");
                }
            });
            self.autostart_tasks.lock().await.push(autostart_handle);
        }
        Ok(())
    }

    /// Subscribe to lifecycle events emitted by this manager.
    pub fn subscribe(&self) -> broadcast::Receiver<ClientGatewayEvent> {
        self.event_tx.subscribe()
    }

    /// Acquire the shared operation guard.
    pub(crate) async fn acquire_operation_guard(&self) -> McpOperationGuard {
        let guard = Arc::clone(&self.operation_guard).lock_owned().await;
        McpOperationGuard { _guard: guard }
    }

    /// Return the current generation for a server, defaulting to zero.
    pub(crate) fn current_server_revision(&self, name: &str) -> u64 {
        self.server_revisions.read().unwrap().get(name).copied().unwrap_or(0)
    }

    /// Increment the generation for a server and return the new value.
    /// Tombstones are retained across removal and re-addition.
    pub(crate) fn increment_server_revision(&self, name: &str) -> u64 {
        let mut revisions = self.server_revisions.write().unwrap();
        let next = revisions.get(name).copied().unwrap_or(0) + 1;
        revisions.insert(name.to_string(), next);
        next
    }

    /// Check whether a server's current generation matches the captured value.
    pub(crate) fn check_server_revision(&self, name: &str, expected: u64) -> bool {
        self.current_server_revision(name) == expected
    }

    /// Take the internal FIFO receiver exactly once for the engine status listener.
    pub(crate) async fn take_internal_receiver(&self) -> Result<mpsc::UnboundedReceiver<InternalEvent>, BarrierLagError> {
        self.internal_rx.lock().await.take().ok_or(BarrierLagError::ListenerUnavailable)
    }

    /// Mark the internal listener active or inactive.
    pub(crate) fn mark_listener_active(&self, active: bool) {
        self.internal_listener_active.store(active, Ordering::Release);
    }

    /// Disconnect all clients and clear the registry.
    pub async fn stop(&self) -> Result<(), ClientGatewayError> {
        // Retire any queued autostart intent before joining tasks so a superseded
        // startup cannot register an old client after shutdown returns.
        {
            let revisions = self.server_revisions.read().unwrap();
            let names: Vec<String> = revisions.keys().cloned().collect();
            drop(revisions);
            for name in names {
                self.increment_server_revision(&name);
            }
        }

        let autostart_handles = {
            let mut handle_store = self.autostart_tasks.lock().await;
            handle_store.drain(..).collect::<Vec<JoinHandle<()>>>()
        };

        for handle in autostart_handles {
            if let Err(join_error) = handle.await {
                tracing::warn!("Autostart task join failed: {}", join_error);
            }
        }

        let guard = self.acquire_operation_guard().await;
        let clients_drain: Vec<(String, Arc<Mutex<McpClient>>)> = {
            let mut clients = self.active_clients.lock().await;
            clients.drain().collect()
        };

        let mut finished = vec![];
        for (name, handle) in clients_drain {
            self.guarded_emit_lifecycle(ClientGatewayEvent::Stopping { name: name.clone() });
            {
                let mut client = handle.lock().await;
                let _ = client.disconnect().await;
            }
            self.guarded_emit_lifecycle(ClientGatewayEvent::Stopped { name: name.clone() });
            self.guarded_emit_lifecycle(ClientGatewayEvent::ToolsUpdated {
                name: name.clone(),
                tools: Arc::new(Vec::new()),
            });
            finished.push(name);
        }

        if !finished.is_empty() {
            let mut stopping = self.stopping.lock().await;
            for stopped in &finished {
                stopping.remove(stopped);
            }
        }

        drop(guard);
        Ok(())
    }

    /// Start a plugin by name using its configuration.
    pub async fn start_plugin(&self, name: &str) -> Result<(), ClientGatewayError> {
        let guard = self.acquire_operation_guard().await;
        self.increment_server_revision(name);
        self.guarded_start_plugin(name, &guard).await
    }

    /// Guarded implementation of `start_plugin`. The caller must hold the operation guard.
    pub(crate) async fn guarded_start_plugin(&self, name: &str, _guard: &McpOperationGuard) -> Result<(), ClientGatewayError> {
        let server = {
            let config = self.config.read().await;
            config
                .mcp_servers
                .get(name)
                .cloned()
                .ok_or_else(|| ClientGatewayError::ClientNotFound { name: name.into() })?
        };

        if server.is_disabled() {
            return Err(ClientGatewayError::PluginDisabled { name: name.into() });
        }

        let plugin_name = name.to_string();

        {
            let active_guard = self.active_clients.lock().await;
            if active_guard.contains_key(name) {
                return Err(ClientGatewayError::ClientAlreadyExists { name: plugin_name.clone() });
            }
        }

        {
            let mut starting_guard = self.starting.lock().await;
            if !starting_guard.insert(plugin_name.clone()) {
                return Err(ClientGatewayError::ClientAlreadyExists { name: plugin_name.clone() });
            }
        }

        self.guarded_emit_lifecycle(ClientGatewayEvent::Starting { name: plugin_name.clone() });

        let connect_result = async {
            let mut client = McpClient::new(plugin_name.clone(), server, self.log_manager.clone());
            client
                .connect()
                .await
                .map(|tools| (client, tools))
                .map_err(|e| ClientGatewayError::ConnectionError { message: e.to_string() })
        }
        .await;

        {
            let mut starting = self.starting.lock().await;
            starting.remove(name);
        }

        match connect_result {
            Ok((client, tools)) => {
                self.active_clients
                    .lock()
                    .await
                    .insert(plugin_name.clone(), Arc::new(Mutex::new(client)));

                self.guarded_emit_lifecycle(ClientGatewayEvent::Started { name: plugin_name.clone() });
                self.guarded_emit_lifecycle(ClientGatewayEvent::ToolsUpdated {
                    name: plugin_name.clone(),
                    tools,
                });

                let _ = self
                    .log_manager
                    .log_audit(AuditEntry::plugin_start(plugin_name.clone(), serde_json::Map::new()))
                    .await;
                Ok(())
            }
            Err(err) => {
                let error_message = err.to_string();
                self.guarded_emit_lifecycle(ClientGatewayEvent::StartFailed {
                    name: plugin_name.clone(),
                    error: error_message,
                });
                Err(err)
            }
        }
    }

    /// Stop a running plugin by name.
    pub async fn stop_plugin(&self, name: &str) -> Result<(), ClientGatewayError> {
        let guard = self.acquire_operation_guard().await;
        self.increment_server_revision(name);
        self.guarded_stop_plugin(name, &guard).await
    }

    /// Guarded implementation of `stop_plugin`. The caller must hold the operation guard.
    pub(crate) async fn guarded_stop_plugin(&self, name: &str, _guard: &McpOperationGuard) -> Result<(), ClientGatewayError> {
        let handle_opt = {
            let mut clients = self.active_clients.lock().await;
            clients.remove(name)
        };

        if let Some(handle) = handle_opt {
            {
                let mut stopping = self.stopping.lock().await;
                stopping.insert(name.to_string());
            }
            self.guarded_emit_lifecycle(ClientGatewayEvent::Stopping { name: name.to_string() });

            let disconnect_result = {
                let mut client = handle.lock().await;
                client.disconnect().await
            };

            disconnect_result.map_err(|e| ClientGatewayError::ConnectionError { message: e.to_string() })?;

            {
                let mut stopping = self.stopping.lock().await;
                stopping.remove(name);
            }
            self.guarded_emit_lifecycle(ClientGatewayEvent::Stopped { name: name.to_string() });
            self.guarded_emit_lifecycle(ClientGatewayEvent::ToolsUpdated {
                name: name.to_string(),
                tools: Arc::new(Vec::new()),
            });
        }
        // Audit stop event (best-effort)
        let _ = self
            .log_manager
            .log_audit(AuditEntry::plugin_stop(name.to_string(), serde_json::Map::new()))
            .await;
        Ok(())
    }

    /// Restart a plugin by stopping and re-starting it.
    pub async fn restart_plugin(&self, name: &str) -> Result<(), ClientGatewayError> {
        let guard = self.acquire_operation_guard().await;
        self.increment_server_revision(name);
        self.guarded_restart_plugin(name, &guard).await
    }

    /// Guarded implementation of `restart_plugin`. The caller must hold the operation guard.
    pub(crate) async fn guarded_restart_plugin(&self, name: &str, guard: &McpOperationGuard) -> Result<(), ClientGatewayError> {
        self.guarded_stop_plugin(name, guard).await?;
        self.guarded_start_plugin(name, guard).await
    }

    /// Return current status for a plugin.
    pub async fn get_plugin_status(&self, name: &str) -> Result<PluginStatus, ClientGatewayError> {
        let config = self.config.read().await;
        if !config.mcp_servers.contains_key(name) {
            return Ok(PluginStatus::Unknown);
        }

        {
            let starting = self.starting.lock().await;
            if starting.contains(name) {
                return Ok(PluginStatus::Starting);
            }
        }

        {
            let stopping = self.stopping.lock().await;
            if stopping.contains(name) {
                return Ok(PluginStatus::Stopping);
            }
        }

        let map = self.active_clients.lock().await;
        let maybe_handle = map.get(name).cloned();
        drop(map);
        if let Some(handle) = maybe_handle {
            let client = handle.lock().await;
            Ok(client.status())
        } else {
            Ok(PluginStatus::Stopped)
        }
    }

    /// Return the current health snapshot for a plugin if known.
    pub async fn get_plugin_health(&self, name: &str) -> Option<HealthStatus> {
        let map = self.active_clients.lock().await;
        let maybe_handle = map.get(name).cloned();
        drop(map);
        if let Some(handle) = maybe_handle {
            let guard = handle.lock().await;
            Some(guard.health().clone())
        } else {
            None
        }
    }

    /// Get an `Arc<Mutex<_>>` handle to a running client.
    pub async fn get_client(&self, name: &str) -> Option<Arc<Mutex<McpClient>>> {
        let map = self.active_clients.lock().await;
        map.get(name).cloned()
    }

    /// List names of all currently running plugins.
    pub async fn list_plugins(&self) -> Vec<String> {
        let map = self.active_clients.lock().await;
        map.keys().cloned().collect()
    }

    /// Get information about all currently active clients.
    pub async fn get_active_client_infos(&self) -> Option<Vec<InitializeResult>> {
        let map = self.active_clients.lock().await;
        let mut results: Vec<InitializeResult> = Vec::new();
        let values = map.values().cloned().collect::<Vec<_>>();
        drop(map);
        for handle in values {
            let guard = handle.lock().await;
            if let Some(result) = guard.get_info() {
                results.push(result.clone());
            }
        }
        Some(results)
    }

    /// Update configuration reference for future lifecycle operations.
    pub async fn update_config(&self, config: McpConfig) -> Result<(), ClientGatewayError> {
        let guard = self.acquire_operation_guard().await;
        self.guarded_update_config(config, &guard).await
    }

    /// Guarded implementation of `update_config`. The caller must hold the operation guard.
    pub(crate) async fn guarded_update_config(&self, config: McpConfig, _guard: &McpOperationGuard) -> Result<(), ClientGatewayError> {
        let mut guard = self.config.write().await;
        *guard = config;
        Ok(())
    }

    /// Access the centralized log manager.
    pub fn log_manager(&self) -> &LogManager {
        &self.log_manager
    }

    /// Invoke a tool exposed by a running plugin.
    pub async fn call_tool(
        &self,
        name: &str,
        tool: &str,
        arguments: &JsonMap<String, serde_json::Value>,
    ) -> Result<rmcp::model::CallToolResult, ClientGatewayError> {
        let handle = self
            .get_client(name)
            .await
            .ok_or_else(|| ClientGatewayError::ClientNotFound { name: name.to_string() })?;

        let client = handle.lock().await;
        client
            .call_tool(tool, arguments)
            .await
            .map_err(|err| ClientGatewayError::ConnectionError { message: err.to_string() })
    }

    /// Emit a lifecycle event to the public broadcast channel and, when the engine
    /// listener is active, to the internal FIFO channel.
    ///
    /// Standalone gateway calls do not fail if the engine listener is unavailable;
    /// engine operations rely on barrier acknowledgement to detect listener loss.
    pub(crate) fn guarded_emit_lifecycle(&self, event: ClientGatewayEvent) {
        let _ = self.event_tx.send(event.clone());
        if self.internal_listener_active.load(Ordering::Acquire) {
            let _ = self.internal_tx.send(InternalEvent::Lifecycle(event));
        }
    }

    /// Enqueue a barrier and return a receiver that resolves when the listener has
    /// applied all preceding events.
    #[allow(dead_code)]
    pub(crate) fn emit_barrier(&self) -> Result<oneshot::Receiver<Result<(), BarrierLagError>>, BarrierLagError> {
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

    /// Wait for a barrier acknowledgement, mapping a dropped sender to listener unavailable
    /// and an elapsed deadline to acknowledgement timeout.
    #[allow(dead_code)]
    pub(crate) async fn wait_for_barrier_ack(
        acknowledgement: oneshot::Receiver<Result<(), BarrierLagError>>,
        timeout_duration: Duration,
    ) -> Result<(), BarrierLagError> {
        match timeout(timeout_duration, acknowledgement).await {
            Ok(Ok(result)) => result,
            Ok(Err(_dropped)) => Err(BarrierLagError::ListenerUnavailable),
            Err(_elapsed) => Err(BarrierLagError::AcknowledgementTimeout),
        }
    }

    /// Enqueue a reconciliation event and return a receiver that resolves when the listener
    /// has rebuilt its state from the authoritative snapshot.
    #[allow(dead_code)]
    pub(crate) fn emit_reconcile(&self, snapshot: EngineStateSnapshot) -> Result<oneshot::Receiver<Result<(), String>>, BarrierLagError> {
        if !self.internal_listener_active.load(Ordering::Acquire) {
            return Err(BarrierLagError::ListenerUnavailable);
        }
        let (acknowledge, ack) = oneshot::channel();
        self.internal_tx
            .send(InternalEvent::Reconcile { snapshot, acknowledge })
            .map_err(|_| BarrierLagError::ListenerUnavailable)?;
        Ok(ack)
    }
}

/// Lifecycle events emitted by [`McpClientGateway`] for plugin state transitions.
#[derive(Debug, Clone)]
pub enum ClientGatewayEvent {
    /// A plugin has begun its startup sequence.
    Starting { name: String },
    /// A plugin finished connecting successfully.
    Started { name: String },
    /// The available tool list for a plugin changed.
    ToolsUpdated { name: String, tools: Arc<Vec<McpToolMetadata>> },
    /// A plugin failed to start.
    StartFailed { name: String, error: String },
    /// A plugin is in the process of shutting down.
    Stopping { name: String },
    /// A plugin has fully stopped.
    Stopped { name: String },
}

/// Errors from the client manager lifecycle APIs.
#[derive(Debug, Error)]
pub enum ClientGatewayError {
    /// The plugin name was not found in the configuration.
    #[error("Plugin not found in configuration: {name}")]
    ClientNotFound { name: String },
    /// The plugin is already running or starting.
    #[error("Plugin is already running: {name}")]
    ClientAlreadyExists { name: String },
    /// Failed to connect or disconnect a client.
    #[error("Connection error: {message}")]
    ConnectionError { message: String },
    /// Action is not allowed: the plugin is disabled.
    #[error("Not allowed: the plugin is disabled: {name}")]
    PluginDisabled { name: String },
}

/// Opaque permit for the shared operation guard.
///
/// The permit is `Send` and `'static` so it can cross await points and be owned
/// by cancellation-safe workers. It releases the mutex when dropped.
#[derive(Debug)]
pub(crate) struct McpOperationGuard {
    _guard: OwnedMutexGuard<()>,
}

/// Errors that can occur while waiting for a barrier acknowledgement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[allow(dead_code)]
pub(crate) enum BarrierLagError {
    /// The engine status listener is not available.
    #[error("engine status listener is unavailable")]
    ListenerUnavailable,
    /// The engine status listener state is dirty and requires reconciliation.
    #[error("engine status listener state is dirty and requires reconciliation")]
    ListenerDirty,
    /// The engine status listener did not acknowledge before the deadline.
    #[error("engine status listener did not acknowledge before the deadline")]
    AcknowledgementTimeout,
}

/// FIFO envelope for events sent from the gateway to the engine status listener.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum InternalEvent {
    Lifecycle(ClientGatewayEvent),
    Barrier(AckFence),
    Reconcile {
        snapshot: EngineStateSnapshot,
        acknowledge: oneshot::Sender<Result<(), String>>,
    },
}

/// Barrier control message with a oneshot acknowledgement.
#[derive(Debug)]
pub(crate) struct AckFence {
    #[allow(dead_code)]
    sequence: usize,
    pub(crate) acknowledge: oneshot::Sender<Result<(), BarrierLagError>>,
}

/// Authoritative state used to repair listener-owned projections.
#[derive(Debug, Clone)]
pub(crate) struct EngineStateSnapshot {
    /// Current runtime configuration.
    pub config: McpConfig,
    /// Per-server runtime status and discovered tools.
    pub servers: HashMap<String, ServerRuntimeSnapshot>,
}

/// Per-server runtime snapshot used during reconciliation.
#[derive(Debug, Clone)]
pub(crate) struct ServerRuntimeSnapshot {
    /// Current lifecycle status.
    pub status: PluginStatus,
    /// Cached or live tool metadata.
    pub tools: Arc<Vec<McpToolMetadata>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::McpConfig;

    #[tokio::test]
    async fn operation_guard_is_shared_across_clones() {
        let gateway = McpClientGateway::new(McpConfig::default()).unwrap();
        let clone = gateway.clone();

        let guard = gateway.acquire_operation_guard().await;
        // The clone should not be able to acquire the guard while the original holds it.
        let try_guard = clone.operation_guard.try_lock();
        assert!(try_guard.is_err(), "cloned gateway must share the same operation guard");
        drop(guard);
        let try_guard = clone.operation_guard.try_lock();
        assert!(try_guard.is_ok(), "guard releases after drop");
    }

    #[tokio::test]
    async fn revisions_default_to_zero_and_tombstones_preserved() {
        let gateway = McpClientGateway::new(McpConfig::default()).unwrap();
        assert_eq!(gateway.current_server_revision("alpha"), 0);
        assert!(gateway.check_server_revision("alpha", 0));

        gateway.increment_server_revision("alpha");
        assert_eq!(gateway.current_server_revision("alpha"), 1);
        assert!(!gateway.check_server_revision("alpha", 0));

        // Removal does not delete the tombstone; re-addition continues from it.
        gateway.increment_server_revision("alpha");
        assert_eq!(gateway.current_server_revision("alpha"), 2);
    }

    #[tokio::test]
    async fn explicit_start_stop_and_connect_false_increment_revision() {
        let gateway = McpClientGateway::new(McpConfig::default()).unwrap();
        let initial = gateway.current_server_revision("alpha");

        // No-op stop still increments.
        gateway.stop_plugin("alpha").await.unwrap();
        assert_eq!(gateway.current_server_revision("alpha"), initial + 1);

        // Starting an unknown server increments before failing.
        let _ = gateway.start_plugin("alpha").await;
        assert_eq!(gateway.current_server_revision("alpha"), initial + 2);
    }

    #[tokio::test]
    async fn internal_receiver_can_only_be_taken_once() {
        let gateway = McpClientGateway::new(McpConfig::default()).unwrap();
        let rx1 = gateway.take_internal_receiver().await.unwrap();
        drop(rx1);
        let rx2 = gateway.take_internal_receiver().await;
        assert_eq!(rx2.unwrap_err(), BarrierLagError::ListenerUnavailable);
    }

    #[tokio::test]
    async fn barrier_acks_after_preceding_lifecycle_events() {
        let gateway = McpClientGateway::new(McpConfig::default()).unwrap();
        let mut rx = gateway.take_internal_receiver().await.unwrap();
        gateway.mark_listener_active(true);

        let listener = tokio::spawn(async move {
            let mut event_count = 0;
            loop {
                match rx.recv().await {
                    Some(InternalEvent::Lifecycle(_)) => event_count += 1,
                    Some(InternalEvent::Barrier(AckFence { acknowledge, .. })) => {
                        let result = if event_count == 2 {
                            Ok(())
                        } else {
                            Err(BarrierLagError::ListenerDirty)
                        };
                        let _ = acknowledge.send(result);
                    }
                    _ => break,
                }
            }
        });

        gateway.guarded_emit_lifecycle(ClientGatewayEvent::Starting { name: "a".into() });
        gateway.guarded_emit_lifecycle(ClientGatewayEvent::Started { name: "a".into() });
        let ack = gateway.emit_barrier().unwrap();
        let result = McpClientGateway::wait_for_barrier_ack(ack, Duration::from_secs(1)).await;
        assert_eq!(result, Ok(()));
        listener.abort();
    }

    #[tokio::test]
    async fn barrier_returns_dirty_when_listener_reports_dirty() {
        let gateway = McpClientGateway::new(McpConfig::default()).unwrap();
        let mut rx = gateway.take_internal_receiver().await.unwrap();
        gateway.mark_listener_active(true);

        let listener = tokio::spawn(async move {
            while let Some(InternalEvent::Barrier(AckFence { acknowledge, .. })) = rx.recv().await {
                let _ = acknowledge.send(Err(BarrierLagError::ListenerDirty));
            }
        });

        let ack = gateway.emit_barrier().unwrap();
        let result = McpClientGateway::wait_for_barrier_ack(ack, Duration::from_secs(1)).await;
        assert_eq!(result, Err(BarrierLagError::ListenerDirty));
        listener.abort();
    }

    #[tokio::test]
    async fn barrier_returns_unavailable_when_listener_drops() {
        let gateway = McpClientGateway::new(McpConfig::default()).unwrap();
        let rx = gateway.take_internal_receiver().await.unwrap();
        gateway.mark_listener_active(true);
        drop(rx);

        let result = gateway.emit_barrier();
        assert_eq!(result.unwrap_err(), BarrierLagError::ListenerUnavailable);
    }

    #[tokio::test]
    async fn barrier_returns_timeout_when_listener_hangs() {
        let gateway = McpClientGateway::new(McpConfig::default()).unwrap();
        let mut rx = gateway.take_internal_receiver().await.unwrap();
        gateway.mark_listener_active(true);

        let _listener = tokio::spawn(async move {
            let mut held_fences: Vec<AckFence> = Vec::new();
            loop {
                match rx.recv().await {
                    Some(InternalEvent::Barrier(fence)) => {
                        // Hold the fence without acknowledging to simulate a hung listener.
                        held_fences.push(fence);
                    }
                    Some(_) => {}
                    None => break,
                }
            }
        });

        let ack = gateway.emit_barrier().unwrap();
        let result = McpClientGateway::wait_for_barrier_ack(ack, Duration::from_millis(100)).await;
        assert_eq!(result, Err(BarrierLagError::AcknowledgementTimeout));
    }
}
