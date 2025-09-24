//! McpClientManager: registry and lifecycle management.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::Result;
use tokio::sync::{Mutex, broadcast};

use crate::{
    config::McpConfig,
    logging::{AuditEntry, LogManager},
    types::{HealthStatus, PluginStatus},
};

use super::client::McpClient;

/// Registry and lifecycle manager for MCP clients.
#[derive(Clone)]
pub struct McpClientManager {
    /// Active client handles keyed by plugin name.
    active_clients: Arc<Mutex<HashMap<String, Arc<Mutex<McpClient>>>>>,
    /// Names currently in the process of starting to avoid races and report transitional status.
    starting: Arc<Mutex<HashSet<String>>>,
    /// Names currently in the process of stopping for transitional status reporting.
    stopping: Arc<Mutex<HashSet<String>>>,
    /// Parsed MCP configuration.
    config: McpConfig,
    /// Centralized logging manager (shared with engine/TUI).
    log_manager: Arc<LogManager>,
    /// Broadcast channel for lifecycle events emitted to interested listeners.
    event_tx: broadcast::Sender<ClientManagerEvent>,
}

impl McpClientManager {
    /// Create a new manager from config.
    pub fn new(config: McpConfig) -> Result<Self> {
        let (event_tx, _rx) = broadcast::channel(64);

        Ok(Self {
            active_clients: Arc::new(Mutex::new(HashMap::new())),
            starting: Arc::new(Mutex::new(HashSet::new())),
            stopping: Arc::new(Mutex::new(HashSet::new())),
            config,
            log_manager: Arc::new(LogManager::new()?),
            event_tx,
        })
    }

    /// Start the manager and autostart all non-disabled plugins from config.
    ///
    /// Plugin startup is scheduled onto the async runtime so the caller does
    /// not block on potentially slow handshake operations. Errors are logged
    /// but do not stop initialization.
    pub async fn start(&self) -> Result<(), ClientManagerError> {
        for (name, server) in &self.config.mcp_servers {
            if !server.is_disabled() {
                let manager = self.clone();
                let plugin_name = name.clone();
                tokio::spawn(async move {
                    if let Err(err) = manager.start_plugin(&plugin_name).await {
                        tracing::warn!("Autostart '{}' failed: {}", plugin_name, err);
                    }
                });
            }
        }
        Ok(())
    }

    /// Subscribe to lifecycle events emitted by this manager.
    pub fn subscribe(&self) -> broadcast::Receiver<ClientManagerEvent> {
        self.event_tx.subscribe()
    }

    /// Disconnect all clients and clear the registry.
    pub async fn stop(&self) -> Result<(), ClientManagerError> {
        let mut clients = self.active_clients.lock().await;
        let clients_drain:Vec<(String, Arc<Mutex<McpClient>>)> = clients.drain().collect();
        drop(clients);
        let mut finished = vec![];
        for (name, handle) in clients_drain {
            {
                let mut stopping = self.stopping.lock().await;
                stopping.insert(name.clone());
            }
            let _ = self.event_tx.send(ClientManagerEvent::Stopping { name: name.clone() });
            {
                let mut client = handle.lock().await;
                let _ = client.disconnect().await;
            }
            finished.push(name);
        }

        if !finished.is_empty() {
            let mut stopping = self.stopping.lock().await;
            for stopped in &finished {
                stopping.remove(stopped);
                let _ = self.event_tx.send(ClientManagerEvent::Stopped { name: stopped.clone() });
            }
        }
        Ok(())
    }

    /// Start a plugin by name using its configuration.
    pub async fn start_plugin(&self, name: &str) -> Result<(), ClientManagerError> {
        let server = self
            .config
            .mcp_servers
            .get(name)
            .cloned()
            .ok_or_else(|| ClientManagerError::ClientNotFound { name: name.into() })?;

        // Prevent duplicates: if already running or in progress, bail
        if self.active_clients.lock().await.contains_key(name) {
            return Err(ClientManagerError::ClientAlreadyExists { name: name.into() });
        }
        {
            let mut starting = self.starting.lock().await;
            if !starting.insert(name.to_string()) {
                return Err(ClientManagerError::ClientAlreadyExists { name: name.into() });
            }
            
        }

        // Connect outside of global locks
        let connect_result = async {
            let mut client = McpClient::new(name.to_string(), server, self.log_manager.clone());
            client
                .connect()
                .await
                .map(|_| client)
                .map_err(|e| ClientManagerError::ConnectionError { message: e.to_string() })
        }
        .await;

        // Always clear reservation
        {
            let mut starting = self.starting.lock().await;
            starting.remove(name);
        }

        match connect_result {
            Ok(client) => {
                self.active_clients
                    .lock()
                    .await
                    .insert(name.to_string(), Arc::new(Mutex::new(client)));

                let _ = self.event_tx.send(ClientManagerEvent::Started { name: name.to_string() });

                // Audit start event (best-effort)
                let _ = self
                    .log_manager
                    .log_audit(AuditEntry::plugin_start(name.to_string(), serde_json::Map::new()))
                    .await;
                Ok(())
            }
            Err(err) => {
                let error_message = err.to_string();
                let _ = self.event_tx.send(ClientManagerEvent::StartFailed {
                    name: name.to_string(),
                    error: error_message,
                });
                Err(err)
            }
        }
    }

    /// Stop a running plugin by name.
    pub async fn stop_plugin(&self, name: &str) -> Result<(), ClientManagerError> {
        let handle_opt = {
            let mut clients = self.active_clients.lock().await;
            clients.remove(name)
        };

        if let Some(handle) = handle_opt {
            {
                let mut stopping = self.stopping.lock().await;
                stopping.insert(name.to_string());
            }
            let _ = self.event_tx.send(ClientManagerEvent::Stopping { name: name.to_string() });

            let disconnect_result = {
                let mut client = handle.lock().await;
                client.disconnect().await
            };
            
            disconnect_result.map_err(|e| ClientManagerError::ConnectionError { message: e.to_string() })?;

            {
                let mut stopping = self.stopping.lock().await;
                stopping.remove(name);
                let _ = self.event_tx.send(ClientManagerEvent::Stopped { name: name.to_string() });
            }
        }
        // Audit stop event (best-effort)
        let _ = self
            .log_manager
            .log_audit(AuditEntry::plugin_stop(name.to_string(), serde_json::Map::new()))
            .await;
        Ok(())
    }

    /// Restart a plugin by stopping and re-starting it.
    pub async fn restart_plugin(&self, name: &str) -> Result<(), ClientManagerError> {
        self.stop_plugin(name).await?;
        self.start_plugin(name).await
    }

    /// Return current status for a plugin.
    ///
    /// If the plugin is not defined in the configuration, `PluginStatus::Unknown`
    /// is returned to signal that the request does not map to a known plugin.
    /// When the plugin exists but is not running, `PluginStatus::Stopped` is
    /// returned.
    pub async fn get_plugin_status(&self, name: &str) -> Result<PluginStatus, ClientManagerError> {
        if !self.config.mcp_servers.contains_key(name) {
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

    /// Return current health snapshot for a plugin if known.
    pub async fn get_plugin_health(&self, name: &str) -> Option<HealthStatus> {
        let map = self.active_clients.lock().await;
        let maybe_handle =  map.get(name).cloned();
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

    /// Update configuration. For now this is a no-op; callers control restarts.
    pub async fn update_config(&self, _config: McpConfig) -> Result<(), ClientManagerError> {
        Ok(())
    }

    /// Access the centralized log manager.
    pub fn log_manager(&self) -> &LogManager {
        &self.log_manager
    }
}

/// Lifecycle events emitted by [`McpClientManager`] for plugin state transitions.
#[derive(Debug, Clone)]
pub enum ClientManagerEvent {
    /// A plugin has begun its startup sequence.
    Starting { name: String },
    /// A plugin finished connecting successfully.
    Started { name: String },
    /// A plugin failed to start.
    StartFailed { name: String, error: String },
    /// A plugin is in the process of shutting down.
    Stopping { name: String },
    /// A plugin has fully stopped.
    Stopped { name: String },
}

/// Errors from the client manager lifecycle APIs.
#[derive(Debug, thiserror::Error)]
pub enum ClientManagerError {
    /// The plugin name was not found in the configuration.
    #[error("Plugin not found in configuration: {name}")]
    ClientNotFound { name: String },
    /// The plugin is already running or starting.
    #[error("Plugin is already running: {name}")]
    ClientAlreadyExists { name: String },
    /// Failed to connect or disconnect a client.
    #[error("Connection error: {message}")]
    ConnectionError { message: String },
}
