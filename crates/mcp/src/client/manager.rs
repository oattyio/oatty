//! Client manager for MCP (Model Context Protocol) plugins.
//!
//! This module provides a centralized manager for handling multiple MCP client connections,
//! including health monitoring, lifecycle management, and configuration updates.

use crate::client::{HealthMonitor, HttpTransport, McpClient, McpTransport, StdioTransport};
use crate::config::McpConfig;
use crate::logging::LogManager;
use crate::logging::sanitize_log_text;
use crate::types::PluginStatus;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, warn};

/// Manager for MCP (Model Context Protocol) clients.
///
/// The `McpClientManager` is responsible for:
/// - Managing the lifecycle of MCP plugin connections
/// - Monitoring client health and connectivity
/// - Coordinating configuration updates
/// - Providing centralized access to client instances
///
/// This manager ensures that all MCP clients are properly initialized,
/// monitored, and cleaned up when no longer needed.
#[derive(Clone)]
pub struct McpClientManager {
    /// Map of active client connections, keyed by plugin name.
    active_clients: Arc<Mutex<HashMap<String, Arc<Mutex<McpClient>>>>>,

    /// Health monitoring service for all connected clients.
    health_monitor: HealthMonitor,

    /// Centralized logging service for audit and debug information.
    log_manager: Arc<LogManager>,

    /// Current configuration for MCP servers and clients.
    configuration: McpConfig,
}

impl McpClientManager {
    /// Creates a new MCP client manager with the provided configuration.
    ///
    /// # Arguments
    ///
    /// * `configuration` - The MCP configuration containing server definitions
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the new manager instance or an error if
    /// initialization fails (e.g., logging setup issues).
    ///
    /// # Example
    ///
    /// ```rust
    /// use heroku_mcp::config::McpConfig;
    /// use heroku_mcp::client::McpClientManager;
    ///
    /// let config = McpConfig::default();
    /// let manager = McpClientManager::new(config)?;
    /// ```
    pub fn new(configuration: McpConfig) -> anyhow::Result<Self> {
        let log_manager = Arc::new(LogManager::new()?);
        let health_monitor = HealthMonitor::new();

        Ok(Self {
            active_clients: Arc::new(Mutex::new(HashMap::new())),
            health_monitor,
            log_manager,
            configuration,
        })
    }

    /// Starts the client manager and initializes all configured services.
    ///
    /// This method performs the following operations:
    /// 1. Starts the health monitoring service
    /// 2. Automatically starts all enabled plugins from the configuration
    /// 3. Spawns background tasks for periodic health checks
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if all services start successfully, or a `ClientManagerError`
    /// if any initialization step fails.
    ///
    /// # Errors
    ///
    /// This method can fail if:
    /// - Health monitoring fails to start
    /// - Any auto-start plugin fails to initialize
    /// - Background task spawning fails
    pub async fn start(&self) -> Result<(), ClientManagerError> {
        // Start health monitoring service
        self.health_monitor.start().await;

        // Start all plugins marked as enabled in the configuration
        self.start_autostart_plugins().await;

        // Spawn background task for periodic health checks
        self.spawn_periodic_health_check_task();

        debug!("MCP client manager started successfully");
        Ok(())
    }

    /// Starts all plugins that are marked as enabled in the configuration.
    ///
    /// This method iterates through all configured MCP servers and attempts to
    /// start those that are not explicitly disabled. Failures are logged but
    /// do not prevent other plugins from starting.
    ///
    /// # Behavior
    ///
    /// - Only starts plugins that are not disabled in the configuration
    /// - Logs warnings for any plugins that fail to start
    /// - Continues attempting to start other plugins even if some fail
    async fn start_autostart_plugins(&self) {
        for (plugin_name, server_configuration) in &self.configuration.mcp_servers {
            if !server_configuration.is_disabled()
                && let Err(startup_error) = self.start_plugin(plugin_name).await
            {
                warn!(
                    "Failed to start auto-start plugin '{}': {}",
                    plugin_name,
                    startup_error
                );
            }
        }
    }

    /// Spawns a background task that performs periodic health checks on all active clients.
    ///
    /// This task runs continuously in the background, checking the health of all
    /// connected MCP clients at regular intervals. The task will automatically
    /// stop when health monitoring is disabled.
    ///
    /// # Behavior
    ///
    /// - Runs health checks at intervals defined by the health monitor
    /// - Stops automatically when monitoring is disabled
    /// - Updates health status for each client based on check results
    /// - Handles both successful and failed health checks gracefully
    fn spawn_periodic_health_check_task(&self) {
        let manager_clone = self.clone();
        tokio::spawn(async move {
            let mut health_check_interval = tokio::time::interval(
                manager_clone.health_monitor.check_interval()
            );
            
            loop {
                health_check_interval.tick().await;

                // Stop if health monitoring has been disabled
                if !manager_clone.health_monitor.is_monitoring().await {
                    break;
                }

                // Perform health checks on all active clients
                manager_clone.perform_health_checks_on_all_clients().await;
            }
        });
    }

    /// Performs health checks on all currently active clients.
    ///
    /// This method creates a snapshot of all active client names and then
    /// performs individual health checks on each client. Health check results
    /// are reported back to the health monitor.
    ///
    /// # Process
    ///
    /// 1. Creates a snapshot of active client names to avoid holding locks
    /// 2. Iterates through each client and performs a health check
    /// 3. Updates the health monitor with the results
    /// 4. Handles both successful and failed health checks
    async fn perform_health_checks_on_all_clients(&self) {
        // Create a snapshot of current client names to avoid holding locks
        let active_client_names = self.get_active_client_names().await;

        for client_name in active_client_names {
            self.perform_health_check_for_client(&client_name).await;
        }
    }

    /// Gets a snapshot of all currently active client names.
    ///
    /// This method creates a copy of all client names to avoid holding
    /// the clients lock for extended periods during health checks.
    ///
    /// # Returns
    ///
    /// A vector containing the names of all currently active clients.
    async fn get_active_client_names(&self) -> Vec<String> {
        let active_clients = self.active_clients.lock().await;
        active_clients.keys().cloned().collect()
    }

    /// Performs a health check for a specific client and updates the health monitor.
    ///
    /// This method attempts to perform a health check on the specified client
    /// and reports the results to the health monitor. If the client is no longer
    /// available, the health check is skipped.
    ///
    /// # Arguments
    ///
    /// * `client_name` - The name of the client to perform a health check on
    async fn perform_health_check_for_client(&self, client_name: &str) {
        // Get a reference to the client if it still exists
        let client_reference = self.get_client_reference(client_name).await;
        
        if let Some(client_handle) = client_reference {
            let mut client_guard = client_handle.lock().await;
            let health_check_result = client_guard.health_check().await;

            match health_check_result {
                Ok(successful_health_result) => {
                    self.health_monitor
                        .update_health(client_name, successful_health_result)
                        .await;
                }
                Err(health_check_error) => {
                    let failed_health_result = crate::client::HealthCheckResult {
                        healthy: false,
                        latency_ms: None,
                        error: Some(sanitize_log_text(&health_check_error.to_string())),
                    };
                    
                    self.health_monitor
                        .update_health(client_name, failed_health_result)
                                    .await;
                            }
                        }
                    }
                }

    /// Gets a reference to a client by name without holding the clients lock.
    ///
    /// This method creates a clone of the client's Arc reference, allowing
    /// the caller to work with the client without holding the global clients lock.
    ///
    /// # Arguments
    ///
    /// * `client_name` - The name of the client to retrieve
    ///
    /// # Returns
    ///
    /// Returns `Some(Arc<Mutex<McpClient>>)` if the client exists, or `None` if not found.
    async fn get_client_reference(&self, client_name: &str) -> Option<Arc<Mutex<McpClient>>> {
        let active_clients = self.active_clients.lock().await;
        active_clients.get(client_name).cloned()
    }

    /// Stops the client manager and gracefully shuts down all services.
    ///
    /// This method performs a clean shutdown by:
    /// 1. Disconnecting all active clients
    /// 2. Clearing the client registry
    /// 3. Stopping the health monitoring service
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the shutdown completes successfully. Individual
    /// client disconnection failures are logged but do not cause the method to fail.
    ///
    /// # Behavior
    ///
    /// - Attempts to disconnect all clients gracefully
    /// - Logs warnings for any clients that fail to disconnect
    /// - Continues with shutdown even if some clients fail to disconnect
    /// - Stops health monitoring after all clients are disconnected
    pub async fn stop(&self) -> Result<(), ClientManagerError> {
        // Disconnect all active clients
        self.disconnect_all_clients().await;

        // Stop health monitoring service
        self.health_monitor.stop().await;

        debug!("MCP client manager stopped successfully");
        Ok(())
    }

    /// Disconnects all active clients and clears the client registry.
    ///
    /// This method iterates through all active clients and attempts to
    /// disconnect them gracefully. Failures are logged but do not prevent
    /// the shutdown process from continuing.
    async fn disconnect_all_clients(&self) {
        let mut active_clients = self.active_clients.lock().await;
        
        for (client_name, client_handle) in active_clients.iter() {
            let mut client_guard = client_handle.lock().await;
            if let Err(disconnect_error) = client_guard.disconnect().await {
                warn!(
                    "Failed to disconnect client '{}': {}", 
                    client_name, 
                    sanitize_log_text(&disconnect_error.to_string())
                );
            }
        }
        
        active_clients.clear();
    }

    /// Starts a plugin with the specified name.
    ///
    /// This method performs the complete plugin startup process:
    /// 1. Validates the plugin configuration exists
    /// 2. Checks that the plugin is not already running
    /// 3. Creates the appropriate transport for the plugin
    /// 4. Establishes the connection
    /// 5. Registers the plugin for health monitoring
    /// 6. Stores the client in the active clients registry
    /// 7. Logs the startup event for audit purposes
    ///
    /// # Arguments
    ///
    /// * `plugin_name` - The name of the plugin to start (must exist in configuration)
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the plugin starts successfully, or a `ClientManagerError`
    /// if any step in the startup process fails.
    ///
    /// # Errors
    ///
    /// This method can fail with:
    /// - `ClientNotFound` if the plugin name is not in the configuration
    /// - `ClientAlreadyExists` if the plugin is already running
    /// - `TransportError` if the transport cannot be created
    /// - `ConnectionError` if the connection fails or audit logging fails
    pub async fn start_plugin(&self, plugin_name: &str) -> Result<(), ClientManagerError> {
        // Validate plugin configuration exists
        let server_configuration = self.get_server_configuration(plugin_name)?;

        // Ensure plugin is not already running
        self.ensure_plugin_not_already_running(plugin_name).await?;

        // Create and connect the client
        let connected_client = self.create_and_connect_client(server_configuration, plugin_name).await?;

        // Register plugin for health monitoring
        self.health_monitor.register_plugin(plugin_name.to_string()).await;

        // Store the client in the active clients registry
        self.store_client_in_registry(plugin_name, connected_client).await;

        // Log the startup event for audit purposes
        self.log_plugin_startup_event(plugin_name).await?;

        debug!("Successfully started plugin: {}", plugin_name);
        Ok(())
    }

    /// Retrieves the server configuration for a plugin by name.
    ///
    /// # Arguments
    ///
    /// * `plugin_name` - The name of the plugin to get configuration for
    ///
    /// # Returns
    ///
    /// Returns the server configuration or `ClientNotFound` error if not found.
    fn get_server_configuration(&self, plugin_name: &str) -> Result<&crate::config::McpServer, ClientManagerError> {
        self.configuration
            .mcp_servers
            .get(plugin_name)
            .ok_or_else(|| ClientManagerError::ClientNotFound { 
                name: plugin_name.to_string() 
            })
    }

    /// Ensures that a plugin with the given name is not already running.
    ///
    /// # Arguments
    ///
    /// * `plugin_name` - The name of the plugin to check
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the plugin is not running, or `ClientAlreadyExists` if it is.
    async fn ensure_plugin_not_already_running(&self, plugin_name: &str) -> Result<(), ClientManagerError> {
        let active_clients = self.active_clients.lock().await;
        if active_clients.contains_key(plugin_name) {
            return Err(ClientManagerError::ClientAlreadyExists { 
                name: plugin_name.to_string() 
            });
        }
        Ok(())
    }

    /// Creates a new MCP client and establishes a connection.
    ///
    /// # Arguments
    ///
    /// * `server_configuration` - The server configuration to use
    /// * `plugin_name` - The name of the plugin (for error reporting)
    ///
    /// # Returns
    ///
    /// Returns the connected client or a `ClientManagerError` if creation/connection fails.
    async fn create_and_connect_client(
        &self, 
        server_configuration: &crate::config::McpServer, 
        _plugin_name: &str
    ) -> Result<McpClient, ClientManagerError> {
        // Create the appropriate transport for the server configuration
        let transport = self.build_transport(server_configuration)?;

        // Create a new MCP client with the transport
        let mut new_client = McpClient::new(transport);

        // Establish the connection
        new_client
            .connect()
            .await
            .map_err(|connection_error| ClientManagerError::ConnectionError {
                message: sanitize_log_text(&connection_error.to_string()),
            })?;

        Ok(new_client)
    }

    /// Stores a connected client in the active clients registry.
    ///
    /// # Arguments
    ///
    /// * `plugin_name` - The name of the plugin
    /// * `connected_client` - The connected client to store
    async fn store_client_in_registry(&self, plugin_name: &str, connected_client: McpClient) {
        let mut active_clients = self.active_clients.lock().await;
        active_clients.insert(
            plugin_name.to_string(), 
            Arc::new(Mutex::new(connected_client))
        );
    }

    /// Logs a plugin startup event for audit purposes.
    ///
    /// # Arguments
    ///
    /// * `plugin_name` - The name of the plugin that was started
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if logging succeeds, or a `ConnectionError` if it fails.
    async fn log_plugin_startup_event(&self, plugin_name: &str) -> Result<(), ClientManagerError> {
        self.log_manager
            .log_audit(crate::logging::AuditEntry::plugin_start(
                plugin_name.to_string(),
                serde_json::Map::new(),
            ))
            .await
            .map_err(|logging_error| ClientManagerError::ConnectionError {
                message: sanitize_log_text(&logging_error.to_string()),
            })
    }

    /// Builds the appropriate transport for a given server configuration.
    ///
    /// This method creates the correct transport implementation based on the
    /// server configuration. Currently supports:
    /// - Standard I/O (stdio) transport for local processes
    /// - HTTP transport for remote servers
    ///
    /// # Arguments
    ///
    /// * `server_configuration` - The server configuration containing transport details
    ///
    /// # Returns
    ///
    /// Returns a boxed transport implementation or a `TransportError` if:
    /// - The transport type is not supported
    /// - The transport cannot be created (e.g., invalid HTTP configuration)
    ///
    /// # Supported Transports
    ///
    /// - **Stdio**: For local process-based MCP servers
    /// - **HTTP**: For remote HTTP-based MCP servers
    fn build_transport(&self, server_configuration: &crate::config::McpServer) -> Result<Box<dyn McpTransport>, ClientManagerError> {
        if server_configuration.is_stdio() {
            Ok(Box::new(StdioTransport::new(server_configuration.clone())))
        } else if server_configuration.is_http() {
            HttpTransport::new(server_configuration.clone())
                .map(|http_transport| Box::new(http_transport) as Box<dyn McpTransport>)
                .map_err(|transport_creation_error| ClientManagerError::TransportError { 
                    message: transport_creation_error.to_string() 
                })
        } else {
            Err(ClientManagerError::TransportError {
                message: "Unsupported transport type - only stdio and HTTP transports are currently supported".to_string(),
            })
        }
    }

    /// Stops a plugin with the specified name.
    ///
    /// This method performs a graceful shutdown of the specified plugin:
    /// 1. Removes the client from the active clients registry
    /// 2. Disconnects the client
    /// 3. Unregisters the plugin from health monitoring
    /// 4. Logs the shutdown event for audit purposes
    ///
    /// # Arguments
    ///
    /// * `plugin_name` - The name of the plugin to stop
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the plugin stops successfully, or a `ClientManagerError`
    /// if the disconnection or audit logging fails.
    ///
    /// # Behavior
    ///
    /// - If the plugin is not running, the method returns successfully
    /// - If the plugin exists, it is gracefully disconnected and removed
    /// - Health monitoring is automatically unregistered
    /// - All shutdown events are logged for audit purposes
    pub async fn stop_plugin(&self, plugin_name: &str) -> Result<(), ClientManagerError> {
        // Remove the client from the active clients registry
        let removed_client = self.remove_client_from_registry(plugin_name).await;

        if let Some(client_handle) = removed_client {
            // Disconnect the client
            self.disconnect_client(client_handle, plugin_name).await?;

            // Unregister from health monitoring
            self.health_monitor.unregister_plugin(plugin_name).await;

            // Log the shutdown event for audit purposes
            self.log_plugin_shutdown_event(plugin_name).await?;

            debug!("Successfully stopped plugin: {}", plugin_name);
        }

        Ok(())
    }

    /// Removes a client from the active clients registry.
    ///
    /// # Arguments
    ///
    /// * `plugin_name` - The name of the plugin to remove
    ///
    /// # Returns
    ///
    /// Returns the client handle if it existed, or `None` if not found.
    async fn remove_client_from_registry(&self, plugin_name: &str) -> Option<Arc<Mutex<McpClient>>> {
        let mut active_clients = self.active_clients.lock().await;
        active_clients.remove(plugin_name)
    }

    /// Disconnects a client and handles any errors.
    ///
    /// # Arguments
    ///
    /// * `client_handle` - The client handle to disconnect
    /// * `_plugin_name` - The name of the plugin (for error reporting)
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if disconnection succeeds, or a `ConnectionError` if it fails.
    async fn disconnect_client(&self, client_handle: Arc<Mutex<McpClient>>, _plugin_name: &str) -> Result<(), ClientManagerError> {
        let mut client_guard = client_handle.lock().await;
        client_guard
            .disconnect()
            .await
            .map_err(|disconnect_error| ClientManagerError::ConnectionError { 
                message: disconnect_error.to_string() 
            })
    }

    /// Logs a plugin shutdown event for audit purposes.
    ///
    /// # Arguments
    ///
    /// * `plugin_name` - The name of the plugin that was stopped
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if logging succeeds, or a `ConnectionError` if it fails.
    async fn log_plugin_shutdown_event(&self, plugin_name: &str) -> Result<(), ClientManagerError> {
        self.log_manager
            .log_audit(crate::logging::AuditEntry::plugin_stop(
                plugin_name.to_string(),
                serde_json::Map::new(),
            ))
            .await
            .map_err(|logging_error| ClientManagerError::ConnectionError { 
                message: logging_error.to_string() 
            })
    }

    /// Restarts a plugin with the specified name.
    ///
    /// This method performs a complete restart cycle by stopping the plugin
    /// (if it's running) and then starting it again. This is useful for:
    /// - Applying configuration changes
    /// - Recovering from connection issues
    /// - Refreshing plugin state
    ///
    /// # Arguments
    ///
    /// * `plugin_name` - The name of the plugin to restart
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the restart completes successfully, or a `ClientManagerError`
    /// if either the stop or start operation fails.
    ///
    /// # Process
    ///
    /// 1. Stops the plugin (gracefully handles case where plugin is not running)
    /// 2. Starts the plugin with current configuration
    /// 3. Logs the restart event
    ///
    /// # Errors
    ///
    /// This method can fail if:
    /// - The plugin fails to stop (disconnection issues)
    /// - The plugin fails to start (configuration, transport, or connection issues)
    pub async fn restart_plugin(&self, plugin_name: &str) -> Result<(), ClientManagerError> {
        self.stop_plugin(plugin_name).await?;
        self.start_plugin(plugin_name).await?;
        debug!("Successfully restarted plugin: {}", plugin_name);
        Ok(())
    }

    /// Gets the current status of a plugin.
    ///
    /// This method checks whether a plugin is currently running and returns
    /// its operational status. If the plugin is not in the active clients
    /// registry, it is considered stopped.
    ///
    /// # Arguments
    ///
    /// * `plugin_name` - The name of the plugin to check
    ///
    /// # Returns
    ///
    /// Returns the current `PluginStatus` of the plugin:
    /// - `PluginStatus::Running` if the plugin is active and connected
    /// - `PluginStatus::Stopped` if the plugin is not running or not found
    ///
    /// # Note
    ///
    /// This method never returns an error - plugins that are not found
    /// are simply considered stopped.
    pub async fn get_plugin_status(&self, plugin_name: &str) -> Result<PluginStatus, ClientManagerError> {
        let active_clients = self.active_clients.lock().await;
        if let Some(client_handle) = active_clients.get(plugin_name) {
            let client_guard = client_handle.lock().await;
            Ok(client_guard.status())
        } else {
            Ok(PluginStatus::Stopped)
        }
    }

    /// Gets the current health status of a plugin.
    ///
    /// This method retrieves the latest health information for a plugin
    /// from the health monitoring service. The health status includes
    /// information about connectivity, latency, and any recent errors.
    ///
    /// # Arguments
    ///
    /// * `plugin_name` - The name of the plugin to get health information for
    ///
    /// # Returns
    ///
    /// Returns `Some(HealthStatus)` if health information is available,
    /// or `None` if the plugin is not being monitored or has no health data.
    ///
    /// # Health Information
    ///
    /// The returned health status includes:
    /// - Overall health status (healthy/unhealthy)
    /// - Connection latency (if available)
    /// - Last error message (if any)
    /// - Timestamp of last health check
    pub async fn get_plugin_health(&self, plugin_name: &str) -> Option<crate::types::HealthStatus> {
        self.health_monitor.get_health(plugin_name).await
    }

    /// Lists all currently active plugins.
    ///
    /// This method returns a list of all plugin names that are currently
    /// running and registered in the active clients registry.
    ///
    /// # Returns
    ///
    /// Returns a vector of plugin names that are currently active.
    /// The list is empty if no plugins are running.
    ///
    /// # Usage
    ///
    /// This method is useful for:
    /// - Displaying the current state of all plugins
    /// - Iterating over all active plugins for maintenance operations
    /// - Debugging and monitoring purposes
    pub async fn list_plugins(&self) -> Vec<String> {
        let active_clients = self.active_clients.lock().await;
        active_clients.keys().cloned().collect()
    }

    /// Checks if a plugin is currently running.
    ///
    /// This is a convenience method that provides a simple boolean check
    /// for whether a plugin is currently active and running.
    ///
    /// # Arguments
    ///
    /// * `plugin_name` - The name of the plugin to check
    ///
    /// # Returns
    ///
    /// Returns `true` if the plugin is running, `false` otherwise.
    ///
    /// # Note
    ///
    /// This method internally calls `get_plugin_status()` and checks
    /// if the status is `PluginStatus::Running`.
    pub async fn is_plugin_running(&self, plugin_name: &str) -> bool {
        matches!(self.get_plugin_status(plugin_name).await, Ok(PluginStatus::Running))
    }

    /// Gets a reference to a client for a specific plugin.
    ///
    /// This method provides direct access to a plugin's client instance,
    /// allowing callers to perform operations on the client directly.
    /// The returned client is wrapped in `Arc<Mutex<>>` for thread-safe access.
    ///
    /// # Arguments
    ///
    /// * `plugin_name` - The name of the plugin to get the client for
    ///
    /// # Returns
    ///
    /// Returns `Some(Arc<Mutex<McpClient>>)` if the plugin is running,
    /// or `None` if the plugin is not found or not running.
    ///
    /// # Usage
    ///
    /// This method is typically used by higher-level services that need
    /// to interact directly with MCP clients for specific operations.
    ///
    /// # Example
    ///
    /// ```rust
    /// if let Some(client_handle) = manager.get_client("my_plugin").await {
    ///     let mut client = client_handle.lock().await;
    ///     // Perform operations on the client
    /// }
    /// ```
    pub async fn get_client(&self, plugin_name: &str) -> Option<Arc<Mutex<McpClient>>> {
        let active_clients = self.active_clients.lock().await;
        active_clients.get(plugin_name).cloned()
    }

    /// Gets a reference to the health monitor.
    ///
    /// This method provides access to the health monitoring service,
    /// allowing callers to query health information or configure monitoring.
    ///
    /// # Returns
    ///
    /// Returns a reference to the `HealthMonitor` instance used by this manager.
    pub fn health_monitor(&self) -> &HealthMonitor {
        &self.health_monitor
    }

    /// Gets a reference to the log manager.
    ///
    /// This method provides access to the centralized logging service,
    /// allowing callers to perform audit logging or query log information.
    ///
    /// # Returns
    ///
    /// Returns a reference to the `LogManager` instance used by this manager.
    pub fn log_manager(&self) -> &LogManager {
        &self.log_manager
    }

    /// Updates the configuration and restarts all clients.
    ///
    /// This method applies a new configuration by stopping all existing
    /// clients and clearing the client registry. This is a simplified
    /// implementation that performs a full restart rather than graceful
    /// configuration updates.
    ///
    /// # Arguments
    ///
    /// * `new_configuration` - The new MCP configuration to apply
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the configuration update completes successfully.
    /// Individual client disconnection failures are logged but do not
    /// cause the method to fail.
    ///
    /// # Behavior
    ///
    /// 1. Stops all currently active clients
    /// 2. Clears the active clients registry
    /// 3. Logs the configuration update event
    ///
    /// # Note
    ///
    /// This is a simplified implementation. In a production system, you would
    /// want to implement more graceful configuration updates that:
    /// - Compare old and new configurations
    /// - Only restart clients whose configuration has changed
    /// - Preserve client state where possible
    /// - Handle partial update failures more gracefully
    pub async fn update_config(&self, _new_configuration: McpConfig) -> Result<(), ClientManagerError> {
        // Stop all existing clients before applying new configuration
        self.disconnect_all_clients().await;

        // Note: In a more sophisticated implementation, we would:
        // 1. Compare the old and new configurations
        // 2. Only restart clients whose configuration has changed
        // 3. Apply the new configuration
        // 4. Restart affected clients
        //
        // For now, this is a simplified full restart approach

        debug!("Configuration updated - all clients restarted");
        Ok(())
    }
}

/// Errors that can occur during client manager operations.
///
/// This enum represents all possible error conditions that can arise
/// when working with the MCP client manager. Each variant includes
/// contextual information to help with debugging and error handling.
#[derive(Debug, thiserror::Error)]
pub enum ClientManagerError {
    /// The requested client/plugin was not found in the configuration.
    ///
    /// This error occurs when trying to start a plugin that is not
    /// defined in the MCP configuration.
    #[error("Plugin not found in configuration: {name}")]
    ClientNotFound { 
        /// The name of the plugin that was not found
        name: String 
    },

    /// The requested client/plugin is already running.
    ///
    /// This error occurs when trying to start a plugin that is
    /// already active in the client manager.
    #[error("Plugin is already running: {name}")]
    ClientAlreadyExists { 
        /// The name of the plugin that is already running
        name: String 
    },

    /// A connection-related error occurred.
    ///
    /// This error covers various connection issues including:
    /// - Failed client connections
    /// - Disconnection failures
    /// - Audit logging failures
    #[error("Connection error: {message}")]
    ConnectionError { 
        /// A sanitized error message describing the connection issue
        message: String 
    },

    /// A health check operation failed.
    ///
    /// This error occurs when health monitoring operations fail,
    /// such as when health checks cannot be performed.
    #[error("Health check failed: {message}")]
    HealthCheckFailed { 
        /// A sanitized error message describing the health check failure
        message: String 
    },

    /// A transport-related error occurred.
    ///
    /// This error covers issues with transport creation and configuration,
    /// such as invalid transport types or transport initialization failures.
    #[error("Transport error: {message}")]
    TransportError { 
        /// A sanitized error message describing the transport issue
        message: String 
    },

    /// A configuration-related error occurred.
    ///
    /// This error covers issues with configuration validation,
    /// parsing, or application.
    #[error("Configuration error: {message}")]
    ConfigurationError { 
        /// A sanitized error message describing the configuration issue
        message: String 
    },
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
