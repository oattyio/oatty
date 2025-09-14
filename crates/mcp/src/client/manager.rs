//! McpClientManager: registry and lifecycle management.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::Result;
use tokio::sync::Mutex;

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
    /// Names currently in the process of starting to avoid races.
    starting: Arc<Mutex<HashSet<String>>>,
    /// Parsed MCP configuration.
    config: McpConfig,
    /// Centralized logging manager (shared with engine/TUI).
    log_manager: Arc<LogManager>,
}

impl McpClientManager {
    /// Create a new manager from config.
    pub fn new(config: McpConfig) -> Result<Self> {
        Ok(Self {
            active_clients: Arc::new(Mutex::new(HashMap::new())),
            starting: Arc::new(Mutex::new(HashSet::new())),
            config,
            log_manager: Arc::new(LogManager::new()?),
        })
    }

    /// Start the manager and autostart all non-disabled plugins from config.
    pub async fn start(&self) -> Result<(), ClientManagerError> {
        for (name, server) in &self.config.mcp_servers {
            if !server.is_disabled() {
                if let Err(err) = self.start_plugin(name).await {
                    tracing::warn!("Autostart '{}' failed: {}", name, err);
                }
            }
        }
        Ok(())
    }

    /// Disconnect all clients and clear the registry.
    pub async fn stop(&self) -> Result<(), ClientManagerError> {
        let mut clients = self.active_clients.lock().await;
        for (_name, handle) in clients.drain() {
            let mut client = handle.lock().await;
            let _ = client.disconnect().await;
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

        let client = connect_result?;

        self.active_clients
            .lock()
            .await
            .insert(name.to_string(), Arc::new(Mutex::new(client)));

        // Audit start event (best-effort)
        let _ = self
            .log_manager
            .log_audit(AuditEntry::plugin_start(name.to_string(), serde_json::Map::new()))
            .await;
        Ok(())
    }

    /// Stop a running plugin by name.
    pub async fn stop_plugin(&self, name: &str) -> Result<(), ClientManagerError> {
        let mut clients = self.active_clients.lock().await;
        if let Some(handle) = clients.remove(name) {
            let mut client = handle.lock().await;
            client
                .disconnect()
                .await
                .map_err(|e| ClientManagerError::ConnectionError { message: e.to_string() })?
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

    /// Return current status for a plugin (or Stopped if not running).
    pub async fn get_plugin_status(&self, name: &str) -> Result<PluginStatus, ClientManagerError> {
        let map = self.active_clients.lock().await;
        if let Some(handle) = map.get(name) {
            let client = handle.lock().await;
            Ok(client.status())
        } else {
            Ok(PluginStatus::Stopped)
        }
    }

    /// Return current health snapshot for a plugin if known.
    pub async fn get_plugin_health(&self, name: &str) -> Option<HealthStatus> {
        let map = self.active_clients.lock().await;
        if let Some(h) = map.get(name) {
            let guard = h.lock().await;
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
