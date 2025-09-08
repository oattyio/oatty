//! Client manager for MCP plugins.

use crate::client::{HealthMonitor, HttpTransport, McpClient, McpTransport, StdioTransport};
use crate::config::McpConfig;
use crate::logging::LogManager;
use crate::types::PluginStatus;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, warn};

/// Manager for MCP clients.
#[derive(Clone)]
pub struct McpClientManager {
    /// Active clients.
    clients: Arc<Mutex<HashMap<String, Arc<Mutex<McpClient>>>>>,

    /// Health monitor.
    health_monitor: HealthMonitor,

    /// Log manager.
    log_manager: Arc<LogManager>,

    /// Configuration.
    config: McpConfig,
}

impl McpClientManager {
    /// Create a new client manager.
    pub fn new(config: McpConfig) -> anyhow::Result<Self> {
        let log_manager = Arc::new(LogManager::new()?);
        let health_monitor = HealthMonitor::new();

        Ok(Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            health_monitor,
            log_manager,
            config,
        })
    }

    /// Start the client manager.
    pub async fn start(&self) -> Result<(), ClientManagerError> {
        // Start health monitoring
        self.health_monitor.start().await;

        // Start auto-enabled plugins
        for (name, server) in &self.config.mcp_servers {
            if !server.is_disabled()
                && let Err(e) = self.start_plugin(name).await
            {
                warn!("Failed to start plugin {}: {}", name, e);
            }
        }

        // Spawn periodic health checks using the real client health_check
        let this = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(this.health_monitor.check_interval());
            loop {
                ticker.tick().await;

                // Stop if monitoring turned off
                if !this.health_monitor.is_monitoring().await {
                    break;
                }

                // Snapshot current client names
                let names: Vec<String> = {
                    let clients = this.clients.lock().await;
                    clients.keys().cloned().collect()
                };

                for name in names {
                    // Get client handle
                    let client_opt = {
                        let clients = this.clients.lock().await;
                        clients.get(&name).cloned()
                    };

                    if let Some(client_arc) = client_opt {
                        let mut client = client_arc.lock().await;
                        let hc = client.health_check().await;

                        match hc {
                            Ok(result) => {
                                this.health_monitor.update_health(&name, result).await;
                            }
                            Err(e) => {
                                this.health_monitor
                                    .update_health(
                                        &name,
                                        crate::client::HealthCheckResult {
                                            healthy: false,
                                            latency_ms: None,
                                            error: Some(e.to_string()),
                                        },
                                    )
                                    .await;
                            }
                        }
                    }
                }
            }
        });

        debug!("Client manager started");
        Ok(())
    }

    /// Stop the client manager.
    pub async fn stop(&self) -> Result<(), ClientManagerError> {
        // Stop all clients
        let mut clients = self.clients.lock().await;
        for (name, client) in clients.iter() {
            let mut client = client.lock().await;
            if let Err(e) = client.disconnect().await {
                warn!("Failed to stop client {}: {}", name, e);
            }
        }
        clients.clear();

        // Stop health monitoring
        self.health_monitor.stop().await;

        debug!("Client manager stopped");
        Ok(())
    }

    /// Start a plugin.
    pub async fn start_plugin(&self, name: &str) -> Result<(), ClientManagerError> {
        let server = self
            .config
            .mcp_servers
            .get(name)
            .ok_or_else(|| ClientManagerError::ClientNotFound { name: name.to_string() })?;

        // Check if client already exists
        {
            let clients = self.clients.lock().await;
            if clients.contains_key(name) {
                return Err(ClientManagerError::ClientAlreadyExists { name: name.to_string() });
            }
        }

        // Create transport
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

        // Create client
        let mut client = McpClient::new(transport);

        // Connect
        client
            .connect()
            .await
            .map_err(|e| ClientManagerError::ConnectionError { message: e.to_string() })?;

        // Register for health monitoring
        self.health_monitor.register_plugin(name.to_string()).await;

        // Store client
        {
            let mut clients = self.clients.lock().await;
            clients.insert(name.to_string(), Arc::new(Mutex::new(client)));
        }

        // Log the start event
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

    /// Stop a plugin.
    pub async fn stop_plugin(&self, name: &str) -> Result<(), ClientManagerError> {
        let client = {
            let mut clients = self.clients.lock().await;
            clients.remove(name)
        };

        if let Some(client) = client {
            let mut client = client.lock().await;
            client
                .disconnect()
                .await
                .map_err(|e| ClientManagerError::ConnectionError { message: e.to_string() })?;

            // Unregister from health monitoring
            self.health_monitor.unregister_plugin(name).await;

            // Log the stop event
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

    /// Restart a plugin.
    pub async fn restart_plugin(&self, name: &str) -> Result<(), ClientManagerError> {
        self.stop_plugin(name).await?;
        self.start_plugin(name).await?;
        debug!("Restarted plugin: {}", name);
        Ok(())
    }

    /// Get plugin status.
    pub async fn get_plugin_status(&self, name: &str) -> Result<PluginStatus, ClientManagerError> {
        let clients = self.clients.lock().await;
        if let Some(client) = clients.get(name) {
            let client = client.lock().await;
            Ok(client.status())
        } else {
            Ok(PluginStatus::Stopped)
        }
    }

    /// Get plugin health.
    pub async fn get_plugin_health(&self, name: &str) -> Option<crate::types::HealthStatus> {
        self.health_monitor.get_health(name).await
    }

    /// List all plugins.
    pub async fn list_plugins(&self) -> Vec<String> {
        let clients = self.clients.lock().await;
        clients.keys().cloned().collect()
    }

    /// Check if a plugin is running.
    pub async fn is_plugin_running(&self, name: &str) -> bool {
        matches!(self.get_plugin_status(name).await, Ok(PluginStatus::Running))
    }

    /// Get client for a plugin.
    pub async fn get_client(&self, name: &str) -> Option<Arc<Mutex<McpClient>>> {
        let clients = self.clients.lock().await;
        clients.get(name).cloned()
    }

    /// Get the health monitor.
    pub fn health_monitor(&self) -> &HealthMonitor {
        &self.health_monitor
    }

    /// Get the log manager.
    pub fn log_manager(&self) -> &LogManager {
        &self.log_manager
    }

    /// Update configuration.
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

        // Update configuration
        // Note: This is a simplified implementation
        // In practice, you'd want to handle configuration updates more gracefully

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
        manager.stop().await.unwrap();
    }
}
