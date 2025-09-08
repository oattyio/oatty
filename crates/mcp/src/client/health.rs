//! Health monitoring for MCP clients.
//!
//! This module provides the `HealthMonitor`, a thread-safe data store for tracking
//! the health status of multiple MCP plugins. It is designed to be a passive
//! component, holding health information that is updated by an external actor,
//! such as the `McpClientManager`.
//!
//! ## Design
//!
//! - `HealthCheckResult`: A simple struct to represent the outcome of a single health check.
//! - `HealthMonitor`: A container for the health status of all registered plugins. It
//!   provides methods to register/unregister plugins, update their health, and query
//!   their current status.
//!
//! The `HealthMonitor` itself does not perform any health checks; it only stores the
//! results. The `McpClientManager` is responsible for scheduling and executing the
//! health checks periodically.

use crate::types::HealthStatus;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::debug;

/// Represents the result of a single health check.
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// Whether the service is considered healthy.
    pub healthy: bool,

    /// The latency of the health check in milliseconds, if successful.
    pub latency_ms: Option<u64>,

    /// An error message if the service is unhealthy.
    pub error: Option<String>,
}

/// A thread-safe data store for tracking the health of multiple plugins.
///
/// The `HealthMonitor` is a passive component that holds the health status for
/// any number of plugins, identified by unique string names. It is intended to be
/// updated by an active monitoring system, like the `McpClientManager`.
#[derive(Clone, Debug)]
pub struct HealthMonitor {
    /// A map from plugin names to their current health status.
    health_status: Arc<Mutex<HashMap<String, HealthStatus>>>,
}

impl HealthMonitor {
    /// Creates a new, empty `HealthMonitor`.
    pub fn new() -> Self {
        Self {
            health_status: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Registers a new plugin for health monitoring.
    ///
    /// This will add the plugin to the store with a default, unhealthy status.
    /// If the plugin is already registered, its status will be reset.
    pub async fn register_plugin(&self, plugin_name: String) {
        let mut status = self.health_status.lock().await;
        status.insert(plugin_name.clone(), HealthStatus::new());
        debug!("Registered plugin for health monitoring: {}", plugin_name);
    }

    /// Unregisters a plugin from health monitoring.
    ///
    /// If the plugin was registered, it is removed from the store.
    pub async fn unregister_plugin(&self, plugin_name: &str) {
        let mut status = self.health_status.lock().await;
        if status.remove(plugin_name).is_some() {
            debug!("Unregistered plugin from health monitoring: {}", plugin_name);
        }
    }

    /// Updates the health status for a registered plugin.
    ///
    /// If the plugin is found, its `HealthStatus` is updated based on the
    /// provided `HealthCheckResult`.
    pub async fn update_health(&self, plugin_name: &str, result: HealthCheckResult) {
        let mut status = self.health_status.lock().await;

        if let Some(health) = status.get_mut(plugin_name) {
            if result.healthy {
                health.mark_healthy();
                if let Some(latency) = result.latency_ms {
                    health.handshake_latency = Some(latency);
                }
            } else {
                health.mark_unhealthy(result.error.unwrap_or_default());
            }
        }
    }

    /// Retrieves the health status for a specific plugin.
    ///
    /// Returns `None` if the plugin is not registered.
    pub async fn get_health(&self, plugin_name: &str) -> Option<HealthStatus> {
        let status = self.health_status.lock().await;
        status.get(plugin_name).cloned()
    }

    /// Retrieves the health status for all registered plugins.
    pub async fn get_all_health(&self) -> HashMap<String, HealthStatus> {
        let status = self.health_status.lock().await;
        status.clone()
    }
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_monitor_registration() {
        let monitor = HealthMonitor::new();

        // Register a plugin
        monitor.register_plugin("test-plugin".to_string()).await;

        // Check initial health
        let health = monitor.get_health("test-plugin").await;
        assert!(health.is_some(), "Plugin should be registered");
        assert!(!health.unwrap().is_healthy(), "Initial status should be unhealthy");
    }

    #[tokio::test]
    async fn test_health_monitor_update() {
        let monitor = HealthMonitor::new();
        monitor.register_plugin("test-plugin".to_string()).await;

        // Update health to healthy
        let healthy_result = HealthCheckResult {
            healthy: true,
            latency_ms: Some(100),
            error: None,
        };
        monitor.update_health("test-plugin", healthy_result).await;
        let health = monitor.get_health("test-plugin").await.unwrap();
        assert!(health.is_healthy(), "Status should be healthy");
        assert_eq!(health.handshake_latency, Some(100));

        // Update health to unhealthy
        let unhealthy_result = HealthCheckResult {
            healthy: false,
            latency_ms: None,
            error: Some("Connection failed".to_string()),
        };
        monitor.update_health("test-plugin", unhealthy_result).await;
        let health = monitor.get_health("test-plugin").await.unwrap();
        assert!(!health.is_healthy(), "Status should be unhealthy");
        assert_eq!(health.last_error, Some("Connection failed".to_string()));
    }

    #[tokio::test]
    async fn test_health_monitor_unregistration() {
        let monitor = HealthMonitor::new();
        monitor.register_plugin("test-plugin".to_string()).await;

        // Unregister plugin
        monitor.unregister_plugin("test-plugin").await;

        // Check health
        let health = monitor.get_health("test-plugin").await;
        assert!(health.is_none(), "Plugin should be unregistered");
    }

    #[tokio::test]
    async fn test_get_all_health() {
        let monitor = HealthMonitor::new();
        monitor.register_plugin("plugin-1".to_string()).await;
        monitor.register_plugin("plugin-2".to_string()).await;

        let all_health = monitor.get_all_health().await;
        assert_eq!(all_health.len(), 2);
        assert!(all_health.contains_key("plugin-1"));
        assert!(all_health.contains_key("plugin-2"));
    }
}
