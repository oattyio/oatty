//! MCP client management for different transport types.
//!
//! This module exposes transports (`stdio`, `http`), a managed `McpClient`, and a
//! `McpClientManager` to orchestrate multiple plugins with health monitoring.

mod health;
mod http;
mod manager;
mod stdio;
mod ultrafast;

pub use crate::types::HealthStatus;
pub use health::{HealthCheckResult, HealthMonitor};
pub use http::{HttpClient, HttpTransport};
pub use manager::McpClientManager;
pub use stdio::{StdioClient, StdioTransport};

use crate::McpError;
use crate::config::McpServer;
use crate::types::{PluginStatus, TransportStatus};

/// Trait for MCP transport implementations.
#[async_trait::async_trait]
pub trait McpTransport: Send + Sync {
    /// Connect to the MCP server.
    async fn connect(&self) -> Result<Box<dyn McpConnection>, McpError>;

    /// Perform a health check.
    async fn health_check(&self) -> Result<HealthCheckResult, McpError>;

    /// Get the transport type.
    fn transport_type(&self) -> &'static str;

    /// Get the server configuration.
    fn server_config(&self) -> &McpServer;
}

/// Trait for MCP connections.
#[async_trait::async_trait]
pub trait McpConnection: Send + Sync {
    /// Check if the connection is alive.
    async fn is_alive(&mut self) -> bool;

    /// Close the connection.
    async fn close(self: Box<Self>) -> Result<(), McpError>;
}

/// A managed MCP client.
pub struct McpClient {
    /// The transport for this client.
    transport: Box<dyn McpTransport>,

    /// The current connection (if any).
    connection: Option<Box<dyn McpConnection>>,

    /// Current status of the client.
    status: PluginStatus,

    /// Transport-specific status.
    transport_status: TransportStatus,

    /// Health information.
    health: HealthStatus,

    /// Last error (if any).
    last_error: Option<String>,
}

impl McpClient {
    /// Create a new MCP client.
    pub fn new(transport: Box<dyn McpTransport>) -> Self {
        Self {
            transport,
            connection: None,
            status: PluginStatus::Stopped,
            transport_status: TransportStatus::Disconnected,
            health: HealthStatus::default(),
            last_error: None,
        }
    }

    /// Connect to the MCP server.
    pub async fn connect(&mut self) -> Result<(), McpError> {
        self.status = PluginStatus::Starting;
        self.transport_status = TransportStatus::Connecting;

        match self.transport.connect().await {
            Ok(connection) => {
                self.connection = Some(connection);
                self.status = PluginStatus::Running;
                self.transport_status = TransportStatus::Connected;
                self.health.mark_healthy();
                self.last_error = None;
                Ok(())
            }
            Err(error) => {
                self.status = PluginStatus::Error;
                self.transport_status = TransportStatus::Error;
                self.health.mark_unhealthy(error.to_string());
                self.last_error = Some(error.to_string());
                Err(error)
            }
        }
    }

    /// Disconnect from the MCP server.
    pub async fn disconnect(&mut self) -> Result<(), McpError> {
        self.status = PluginStatus::Stopping;

        if let Some(connection) = self.connection.take() {
            // Guard against hangs during shutdown
            match tokio::time::timeout(std::time::Duration::from_secs(5), connection.close()).await {
                Ok(res) => res?,
                Err(_) => {
                    // Timed out
                    self.last_error = Some("Disconnect timeout".to_string());
                }
            }
        }

        self.status = PluginStatus::Stopped;
        self.transport_status = TransportStatus::Disconnected;
        self.health.mark_unhealthy("Disconnected".to_string());

        Ok(())
    }

    /// Check if the client is connected.
    pub fn is_connected(&self) -> bool {
        self.connection.is_some() && self.status.is_running()
    }

    /// Get the current status.
    pub fn status(&self) -> PluginStatus {
        self.status
    }

    /// Get the transport status.
    pub fn transport_status(&self) -> TransportStatus {
        self.transport_status
    }

    /// Get the health status.
    pub fn health(&self) -> &HealthStatus {
        &self.health
    }

    /// Get the last error.
    pub fn last_error(&self) -> Option<&String> {
        self.last_error.as_ref()
    }

    /// Perform a health check.
    pub async fn health_check(&mut self) -> Result<HealthCheckResult, McpError> {
        let result = self.transport.health_check().await?;

        if result.healthy {
            self.health.mark_healthy();
            if let Some(latency) = result.latency_ms {
                self.health.handshake_latency = Some(latency);
            }
        } else {
            self.health.mark_unhealthy(result.error.clone().unwrap_or_default());
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::McpServer;

    #[test]
    fn test_mcp_client_creation() {
        let server = McpServer::default();
        let transport = Box::new(StdioTransport::new(server));
        let client = McpClient::new(transport);

        assert_eq!(client.status(), PluginStatus::Stopped);
        assert_eq!(client.transport_status(), TransportStatus::Disconnected);
        assert!(!client.is_connected());
    }
}
