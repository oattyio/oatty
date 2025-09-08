//! Health monitoring for MCP clients.

use crate::types::HealthStatus;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::Mutex;
use tokio::time::{interval, sleep};
use tracing::{debug, warn};

/// Health check result.
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// Whether the service is healthy.
    pub healthy: bool,

    /// Latency in milliseconds.
    pub latency_ms: Option<u64>,

    /// Error message if unhealthy.
    pub error: Option<String>,
}

/// Health monitor for tracking plugin health.
#[derive(Clone)]
pub struct HealthMonitor {
    /// Health status for each plugin.
    health_status: Arc<Mutex<HashMap<String, HealthStatus>>>,

    /// Health check interval.
    check_interval: Duration,

    /// Health check timeout.
    check_timeout: Duration,

    /// Whether monitoring is active.
    monitoring: Arc<Mutex<bool>>,
}

impl HealthMonitor {
    /// Create a new health monitor.
    pub fn new() -> Self {
        Self {
            health_status: Arc::new(Mutex::new(HashMap::new())),
            check_interval: Duration::from_secs(30),
            check_timeout: Duration::from_secs(10),
            monitoring: Arc::new(Mutex::new(false)),
        }
    }

    /// Create a new health monitor with custom settings.
    pub fn with_settings(check_interval: Duration, check_timeout: Duration) -> Self {
        Self {
            health_status: Arc::new(Mutex::new(HashMap::new())),
            check_interval,
            check_timeout,
            monitoring: Arc::new(Mutex::new(false)),
        }
    }

    /// Start health monitoring.
    pub async fn start(&self) {
        let mut monitoring = self.monitoring.lock().await;
        if *monitoring {
            return; // Already monitoring
        }
        *monitoring = true;
        drop(monitoring);

        let health_status = Arc::clone(&self.health_status);
        let monitoring = Arc::clone(&self.monitoring);
        let check_interval = self.check_interval;
        let check_timeout = self.check_timeout;

        tokio::spawn(async move {
            let mut interval = interval(check_interval);

            loop {
                interval.tick().await;

                // Check if we should still be monitoring
                let should_monitor = {
                    let monitoring = monitoring.lock().await;
                    *monitoring
                };

                if !should_monitor {
                    break;
                }

                // Perform health checks for all registered plugins
                let plugin_names = {
                    let status = health_status.lock().await;
                    status.keys().cloned().collect::<Vec<_>>()
                };

                for plugin_name in plugin_names {
                    if let Err(e) = Self::perform_health_check(&health_status, &plugin_name, check_timeout).await {
                        warn!("Health check failed for plugin {}: {}", plugin_name, e);
                    }
                }
            }
        });

        debug!("Health monitoring started");
    }

    /// Stop health monitoring.
    pub async fn stop(&self) {
        let mut monitoring = self.monitoring.lock().await;
        *monitoring = false;
        debug!("Health monitoring stopped");
    }

    /// Register a plugin for health monitoring.
    pub async fn register_plugin(&self, plugin_name: String) {
        let name_clone = plugin_name.clone();
        let mut status = self.health_status.lock().await;
        status.insert(plugin_name, HealthStatus::new());
        debug!("Registered plugin for health monitoring: {}", name_clone);
    }

    /// Unregister a plugin from health monitoring.
    pub async fn unregister_plugin(&self, plugin_name: &str) {
        let mut status = self.health_status.lock().await;
        status.remove(plugin_name);
        debug!("Unregistered plugin from health monitoring: {}", plugin_name);
    }

    /// Update health status for a plugin.
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

    /// Get health status for a plugin.
    pub async fn get_health(&self, plugin_name: &str) -> Option<HealthStatus> {
        let status = self.health_status.lock().await;
        status.get(plugin_name).cloned()
    }

    /// Get health status for all plugins.
    pub async fn get_all_health(&self) -> HashMap<String, HealthStatus> {
        let status = self.health_status.lock().await;
        status.clone()
    }

    /// Perform a health check for a specific plugin.
    async fn perform_health_check(
        health_status: &Arc<Mutex<HashMap<String, HealthStatus>>>,
        plugin_name: &str,
        _timeout: Duration,
    ) -> Result<(), String> {
        // This is a placeholder implementation
        // In practice, you would call the actual health check method
        // for the specific plugin's transport

        let start = SystemTime::now();

        // Simulate a health check
        sleep(Duration::from_millis(100)).await;

        let latency = start.elapsed().unwrap_or_default().as_millis() as u64;

        // Update the health status
        let mut status = health_status.lock().await;
        if let Some(health) = status.get_mut(plugin_name) {
            health.mark_healthy();
            health.handshake_latency = Some(latency);
        }

        Ok(())
    }

    /// Check if monitoring is active.
    pub async fn is_monitoring(&self) -> bool {
        let monitoring = self.monitoring.lock().await;
        *monitoring
    }

    /// Get the check interval.
    pub fn check_interval(&self) -> Duration {
        self.check_interval
    }

    /// Get the check timeout.
    pub fn check_timeout(&self) -> Duration {
        self.check_timeout
    }

    /// Set the check interval.
    pub fn set_check_interval(&mut self, interval: Duration) {
        self.check_interval = interval;
    }

    /// Set the check timeout.
    pub fn set_check_timeout(&mut self, timeout: Duration) {
        self.check_timeout = timeout;
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
    async fn test_health_monitor() {
        let monitor = HealthMonitor::new();

        // Register a plugin
        monitor.register_plugin("test-plugin".to_string()).await;

        // Check initial health
        let health = monitor.get_health("test-plugin").await;
        assert!(health.is_some());
        assert!(!health.unwrap().is_healthy());

        // Update health
        let result = HealthCheckResult {
            healthy: true,
            latency_ms: Some(100),
            error: None,
        };

        monitor.update_health("test-plugin", result).await;

        let health = monitor.get_health("test-plugin").await;
        assert!(health.unwrap().is_healthy());

        // Unregister plugin
        monitor.unregister_plugin("test-plugin").await;

        let health = monitor.get_health("test-plugin").await;
        assert!(health.is_none());
    }

    #[tokio::test]
    async fn test_health_monitor_start_stop() {
        let monitor = HealthMonitor::new();

        assert!(!monitor.is_monitoring().await);

        monitor.start().await;
        assert!(monitor.is_monitoring().await);

        monitor.stop().await;
        assert!(!monitor.is_monitoring().await);
    }
}
