//! Status types for MCP plugins.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Status of a plugin.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PluginStatus {
    /// Plugin is running and healthy.
    Running,
    /// Plugin is stopped.
    Stopped,
    /// Plugin has a warning (e.g., slow response).
    Warning,
    /// Plugin has an error.
    Error,
    /// Plugin is starting up.
    Starting,
    /// Plugin is stopping.
    Stopping,
}

impl PluginStatus {
    /// Get the display icon for this status.
    pub fn icon(&self) -> &'static str {
        match self {
            PluginStatus::Running => "✓",
            PluginStatus::Stopped => "✗",
            PluginStatus::Warning => "!",
            PluginStatus::Error => "✗",
            PluginStatus::Starting => "⏳",
            PluginStatus::Stopping => "⏳",
        }
    }

    /// Get the display text for this status.
    pub fn display(&self) -> &'static str {
        match self {
            PluginStatus::Running => "Running",
            PluginStatus::Stopped => "Stopped",
            PluginStatus::Warning => "Warning",
            PluginStatus::Error => "Error",
            PluginStatus::Starting => "Starting",
            PluginStatus::Stopping => "Stopping",
        }
    }

    /// Check if the plugin is in a running state.
    pub fn is_running(&self) -> bool {
        matches!(self, PluginStatus::Running)
    }

    /// Check if the plugin is in an error state.
    pub fn is_error(&self) -> bool {
        matches!(self, PluginStatus::Error)
    }

    /// Check if the plugin is in a transitional state.
    pub fn is_transitional(&self) -> bool {
        matches!(self, PluginStatus::Starting | PluginStatus::Stopping)
    }
}

/// Health status of a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Whether the plugin is healthy.
    pub healthy: bool,

    /// Last health check time.
    pub last_check: Option<std::time::SystemTime>,

    /// Start time of the plugin.
    pub start_time: Option<std::time::SystemTime>,

    /// Handshake latency in milliseconds.
    pub handshake_latency: Option<u64>,

    /// Number of consecutive failures.
    pub failure_count: u32,

    /// Last error message.
    pub last_error: Option<String>,

    /// Transport-specific status.
    pub transport_status: TransportStatus,
}

impl Default for HealthStatus {
    fn default() -> Self {
        Self {
            healthy: false,
            last_check: None,
            start_time: None,
            handshake_latency: None,
            failure_count: 0,
            last_error: None,
            transport_status: TransportStatus::Disconnected,
        }
    }
}

impl HealthStatus {
    /// Create a new health status.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark the plugin as healthy.
    pub fn mark_healthy(&mut self) {
        self.healthy = true;
        self.failure_count = 0;
        self.last_error = None;
        self.last_check = Some(std::time::SystemTime::now());
    }

    /// Mark the plugin as unhealthy with an error.
    pub fn mark_unhealthy(&mut self, error: String) {
        self.healthy = false;
        self.failure_count += 1;
        self.last_error = Some(error);
        self.last_check = Some(std::time::SystemTime::now());
    }

    /// Check if the plugin is healthy.
    pub fn is_healthy(&self) -> bool {
        self.healthy
    }

    /// Get the uptime of the plugin.
    pub fn uptime(&self) -> Option<Duration> {
        self.start_time.map(|start| start.elapsed().unwrap_or_default())
    }

    /// Get the time since the last health check.
    pub fn time_since_last_check(&self) -> Option<Duration> {
        self.last_check.map(|check| check.elapsed().unwrap_or_default())
    }
}

/// Transport-specific status information.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransportStatus {
    /// Transport is connected and working.
    Connected,
    /// Transport is disconnected.
    Disconnected,
    /// Transport is connecting.
    Connecting,
    /// Transport has an error.
    Error,
    /// Transport is not applicable (e.g., for stopped plugins).
    NotApplicable,
}

impl TransportStatus {
    /// Get the display text for this transport status.
    pub fn display(&self) -> &'static str {
        match self {
            TransportStatus::Connected => "Connected",
            TransportStatus::Disconnected => "Disconnected",
            TransportStatus::Connecting => "Connecting",
            TransportStatus::Error => "Error",
            TransportStatus::NotApplicable => "N/A",
        }
    }

    /// Check if the transport is connected.
    pub fn is_connected(&self) -> bool {
        matches!(self, TransportStatus::Connected)
    }

    /// Check if the transport has an error.
    pub fn is_error(&self) -> bool {
        matches!(self, TransportStatus::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_status_icons() {
        assert_eq!(PluginStatus::Running.icon(), "✓");
        assert_eq!(PluginStatus::Stopped.icon(), "✗");
        assert_eq!(PluginStatus::Warning.icon(), "!");
        assert_eq!(PluginStatus::Error.icon(), "✗");
    }

    #[test]
    fn test_plugin_status_checks() {
        assert!(PluginStatus::Running.is_running());
        assert!(!PluginStatus::Stopped.is_running());
        assert!(PluginStatus::Error.is_error());
        assert!(!PluginStatus::Running.is_error());
        assert!(PluginStatus::Starting.is_transitional());
        assert!(!PluginStatus::Running.is_transitional());
    }

    #[test]
    fn test_health_status() {
        let mut health = HealthStatus::new();
        assert!(!health.is_healthy());

        health.mark_healthy();
        assert!(health.is_healthy());
        assert_eq!(health.failure_count, 0);

        health.mark_unhealthy("Test error".to_string());
        assert!(!health.is_healthy());
        assert_eq!(health.failure_count, 1);
        assert_eq!(health.last_error, Some("Test error".to_string()));
    }

    #[test]
    fn test_transport_status() {
        assert!(TransportStatus::Connected.is_connected());
        assert!(!TransportStatus::Disconnected.is_connected());
        assert!(TransportStatus::Error.is_error());
        assert!(!TransportStatus::Connected.is_error());
    }
}
