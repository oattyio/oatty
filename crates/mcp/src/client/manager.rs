//! Client manager for MCP plugins.
//!
//! This module provides the `McpClientManager`, which is responsible for managing
//! the lifecycle of all MCP clients (plugins). It handles starting, stopping,
//! and restarting plugins based on the application's configuration.
//!
//! ## Responsibilities
//!
//! - **Lifecycle Management**: The manager can start and stop plugins, ensuring that
//!   the underlying transport is connected or disconnected correctly.
//! - **Configuration**: It uses an `McpConfig` to discover and configure the
//!   available MCP servers.
//! - **Health Monitoring**: The manager runs a background task to periodically
//!   perform health checks on all running plugins. It uses a `HealthMonitor`

use crate::client::{HealthMonitor, HttpTransport, McpClient, McpTransport, StdioTransport};
use crate::config::McpConfig;
use crate::logging::LogManager;
use crate::types::PluginStatus;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{debug, error, warn};

/// Manages the lifecycle and health monitoring of MCP clients.
#[derive(Clone)]
pub struct McpClientManager {
    /// A map of active clients, keyed by plugin name.
    clients: Arc<Mutex<HashMap<String, Arc<Mutex<McpClient>>>>>,

    /// A data store for the health status of all plugins.
    health_monitor: HealthMonitor,

    /// The log manager for audit logging.
    log_manager: Arc<LogManager>,

    /// The application's MCP configuration.
    config: McpConfig,

    /// The interval at which to perform health checks.
    health_check_interval: Duration,

    /// A handle to the background health check task.
    health_check_task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl McpClientManager {
    /// Creates a new `McpClientManager`.
    ///
    /// Initializes the manager with the given configuration, but does not start
    /// any plugins or health checks.
    pub fn new(config: McpConfig) -> anyhow::Result<Self> {
        let log_manager = Arc::new(LogManager::new()?);
        let health_monitor = HealthMonitor::new();

        Ok(Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            health_monitor,
            log_manager,
            config,
            health_check_interval: Duration::from_secs(30),
            health_check_task: Arc::new(Mutex::new(None)),
        })
    }

    /// Starts the client manager and associated background tasks.
    ///
    /// This will:
    /// 1. Start all auto-enabled plugins defined in the configuration.
    /// 2. Spawn a background task to perform periodic health checks.
    pub async fn start(&self) -> Result<(), ClientManagerError> {
        // Start auto-enabled plugins
        for (name, server) in &self.config.mcp_servers {
            if !server.is_disabled() {
                if let Err(e) = self.start_plugin(name).await {
                    warn!("Failed to start plugin {}: {}", name, e);
                }
            }
        }

        // Spawn the periodic health check task
        let mut task_handle = self.health_check_task.lock().await;
        if task_handle.is_none() {
            let this = self.clone();
            let handle = tokio::spawn(async move {
                this.run_health_checks().await;
            });
            *task_handle = Some(handle);
        }

        debug!("Client manager started");
        Ok(())
    }

    /// Runs the periodic health check loop.
    ///
    /// This method is intended to be run in a background task.
    async fn run_health_checks(&self) {
        let mut ticker = tokio::time::interval(self.health_check_interval);
        loop {
            ticker.tick().await;

            // Snapshot current client names to avoid holding the lock during checks
            let names: Vec<String> = {
                let clients = self.clients.lock().await;
                clients.keys().cloned().collect()
            };

            for name in names {
                let client_opt = {
                    let clients = self.clients.lock().await;
                    clients.get(&name).cloned()
                };

                if let Some(client_arc) = client_opt {
                    let mut client = client_arc.lock().await;
                    let result = match client.health_check().await {
                        Ok(result) => result,
                        Err(e) => crate::client::HealthCheckResult {
                            healthy: false,
                            latency_ms: None,
                            error: Some(e.to_string()),
                        },
                    };
                    self.health_monitor.update_health(&name, result).await;
                }
            }
        }
    }

    /// Stops the client manager and all managed plugins.
    ///
    /// This will gracefully disconnect all clients and stop the health monitoring task.
    pub async fn stop(&self) -> Result<(), ClientManagerError> {
        // Stop the health check task
        if let Some(handle) = self.health_check_task.lock().await.take() {
            handle.abort();
        }

        // Stop all clients
        let mut clients = self.clients.lock().await;
        for (name, client) in clients.iter() {
            let mut client = client.lock().await;
            if let Err(e) = client.disconnect().await {
                warn!("Error stopping client {}: {}", name, e);
            }
        }
        clients.clear();

        debug!("Client manager stopped");
        Ok(())
    }

    /// Starts a single plugin by name.
    ///
    /// This creates the appropriate transport, connects the client, and registers
    /// it for health monitoring.
    pub async fn start_plugin(&self, name: &str) -> Result<(), ClientManagerError> {
        let server = self
            .config
            .mcp_servers
            .get(name)
            .ok_or_else(|| ClientManagerError::ClientNotFound { name: name.to_string() })?;

        // Ensure client is not already running
        {
            let clients = self.clients.lock().await;
            if clients.contains_key(name) {
                return Err(ClientManagerError::ClientAlreadyExists { name: name.to_string() });
            }
        }

        // Create the transport based on configuration
        let transport: Box<dyn McpTransport> = if server.is_stdio() {
            Box::new(StdioTransport::new(server.clone()))
        } else if server.is_http() {
            Box::new(
                HttpTransport::new(server.clone())
                    .map_err(|e| ClientManagerError::TransportError { message: e.to_string() })?,
            )
        } else {
            return Err(ClientManagerError::TransportError {
                message: "Unknown transport type".to_string(),
            });
        };

        let mut client = McpClient::new(transport);
        client
            .connect()
            .await
            .map_err(|e| ClientManagerError::ConnectionError { message: e.to_string() })?;

        // Register with the health monitor and store the client
        self.health_monitor.register_plugin(name.to_string()).await;
        {
            let mut clients = self.clients.lock().await;
            clients.insert(name.to_string(), Arc::new(Mutex::new(client)));
        }

        self.log_manager
            .log_audit(crate::logging::AuditEntry::plugin_start(
                name.to_string(),
                serde_json::Map::new(),
            ))
            .await
            .map_err(|e| ClientManagerError::ConnectionError { message: e.to_string() })?;

        debug!("Started plugin: {}", name);
        Ok(())
    }

    /// Stops a single plugin by name.
    pub async fn stop_plugin(&self, name: &str) -> Result<(), ClientManagerError> {
        let client_arc = {
            let mut clients = self.clients.lock().await;
            clients.remove(name)
        };

        if let Some(client_arc) = client_arc {
            let mut client = client_arc.lock().await;
            client
                .disconnect()
                .await
                .map_err(|e| ClientManagerError::ConnectionError { message: e.to_string() })?;

            self.health_monitor.unregister_plugin(name).await;

            self.log_manager
                .log_audit(crate::logging::AuditEntry::plugin_stop(
                    name.to_string(),
                    serde_json::Map::new(),
                ))
                .await
                .map_err(|e| ClientManagerError::ConnectionError { message: e.to_string() })?;

            debug!("Stopped plugin: {}", name);
        }

        Ok(())
    }

    /// Restarts a single plugin by name.
    pub async fn restart_plugin(&self, name: &str) -> Result<(), ClientManagerError> {
        self.stop_plugin(name).await?;
        self.start_plugin(name).await?;
        debug!("Restarted plugin: {}", name);
        Ok(())
    }

    /// Gets the current status of a plugin.
    pub async fn get_plugin_status(&self, name: &str) -> Result<PluginStatus, ClientManagerError> {
        let clients = self.clients.lock().await;
        if let Some(client) = clients.get(name) {
            let client = client.lock().await;
            Ok(client.status())
        } else {
            Ok(PluginStatus::Stopped)
        }
    }

    /// Gets the current health of a plugin.
    pub async fn get_plugin_health(&self, name: &str) -> Option<crate::types::HealthStatus> {
        self.health_monitor.get_health(name).await
    }

    /// Lists the names of all currently managed plugins.
    pub async fn list_plugins(&self) -> Vec<String> {
        let clients = self.clients.lock().await;
        clients.keys().cloned().collect()
    }

    /// Checks if a plugin is currently in the `Running` state.
    pub async fn is_plugin_running(&self, name: &str) -> bool {
        matches!(self.get_plugin_status(name).await, Ok(PluginStatus::Running))
    }

    /// Returns a handle to a client for direct interaction.
    pub async fn get_client(&self, name: &str) -> Option<Arc<Mutex<McpClient>>> {
        let clients = self.clients.lock().await;
        clients.get(name).cloned()
    }

    /// Returns a reference to the `HealthMonitor`.
    pub fn health_monitor(&self) -> &HealthMonitor {
        &self.health_monitor
    }

    /// Returns a reference to the `LogManager`.
    pub fn log_manager(&self) -> &LogManager {
        &self.log_manager
    }

    /// Updates the manager's configuration.
    ///
    /// Note: This is a simplified implementation that stops all clients.
    /// A more advanced version would handle this more gracefully.
    pub async fn update_config(&self, _config: McpConfig) -> Result<(), ClientManagerError> {
        // Stop all existing clients
        let mut clients = self.clients.lock().await;
        for (name, client) in clients.iter() {
            let mut client = client.lock().await;
            if let Err(e) = client.disconnect().await {
                warn!("Failed to stop client {} during config update: {}", name, e);
            }
        }
        clients.clear();

        debug!("Configuration updated");
        Ok(())
    }
}

/// Errors that can occur in the client manager.
#[derive(Debug, thiserror::Error)]
pub enum ClientManagerError {
    #[error("Client not found: {name}")]
    ClientNotFound { name: String },

    #[error("Client already exists: {name}")]
    ClientAlreadyExists { name: String },

    #[error("Connection error: {message}")]
    ConnectionError { message: String },

    #[error("Health check failed: {message}")]
    HealthCheckFailed { message: String },

    #[error("Transport error: {message}")]
    TransportError { message: String },

    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::McpConfig;

    #[tokio::test]
    async fn test_client_manager_creation() {
        let config = McpConfig::default();
        let manager = McpClientManager::new(config).unwrap();

        let plugins = manager.list_plugins().await;
        assert!(plugins.is_empty());
    }

    #[tokio::test]
    async fn test_client_manager_start_stop() {
        let config = McpConfig::default();
        let manager = McpClientManager::new(config).unwrap();

        manager.start().await.unwrap();
        // Give the health check task a moment to start
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(manager.health_check_task.lock().await.is_some());

        manager.stop().await.unwrap();
        assert!(manager.health_check_task.lock().await.is_none());
    }
}
