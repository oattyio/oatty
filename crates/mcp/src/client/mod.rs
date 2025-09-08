//! MCP client framework and transport management.
//!
//! This module provides the core components for creating and managing clients that
//! communicate with MCP (Managed Component Protocol) servers. It defines a generic
//! framework with traits for different communication methods (transports) and a
//! client struct that manages the state of a connection.
//!
//! ## Key Components
//!
//! - `McpTransport`: A trait for transport implementations (e.g., `StdioTransport`,
//!   `HttpTransport`). It defines the interface for connecting to a server and
//!   performing health checks.
//! - `McpConnection`: A trait representing an active, established connection to a
//!   server. It provides access to the underlying `rmcp` service peer and methods
//!   to check the connection's liveness and to close it.
//! - `McpClient`: A state machine for a single client instance. It wraps a
//!   transport and manages the connection lifecycle (e.g., connecting,
//!   disconnecting, tracking status).
//! - `McpClientManager`: (In the `manager` submodule) A higher-level component
//!   that manages a collection of `McpClient` instances, handles their lifecycle,
//!   and performs periodic health monitoring.

mod health;
mod http;
mod manager;
mod stdio;

pub use crate::types::HealthStatus;
pub use health::{HealthCheckResult, HealthMonitor};
pub use http::{HttpClient, HttpTransport};
pub use manager::McpClientManager;
pub use stdio::{StdioClient, StdioTransport};

use crate::config::McpServer;
use crate::types::{PluginStatus, TransportStatus};
use rmcp::{ErrorData as McpError, service::RoleClient};

/// A trait for MCP transport implementations.
///
/// A transport is responsible for establishing a connection to an MCP server
/// and performing transport-specific health checks.
#[async_trait::async_trait]
pub trait McpTransport: Send + Sync {
    /// Establishes a connection to the MCP server.
    ///
    /// On success, returns a `Box<dyn McpConnection>` representing the active
    /// connection.
    async fn connect(&self) -> Result<Box<dyn McpConnection>, McpError>;

    /// Performs a health check on the MCP server.
    ///
    /// The implementation is transport-specific. For example, an HTTP transport
    /// might send a GET request, while a stdio transport might spawn the process.
    async fn health_check(&self) -> Result<HealthCheckResult, McpError>;

    /// Returns a string slice identifying the transport type (e.g., "stdio", "http").
    fn transport_type(&self) -> &'static str;

    /// Returns a reference to the server configuration.
    fn server_config(&self) -> &McpServer;
}

/// A trait for an active MCP connection.
///
/// This represents an established connection to an MCP server and provides the
/// means to interact with it.
#[async_trait::async_trait]
pub trait McpConnection: Send + Sync {
    /// Returns a reference to the `rmcp` service peer for making RPC calls.
    fn peer(&self) -> &rmcp::service::Peer<RoleClient>;

    /// Checks if the connection is still active.
    async fn is_alive(&self) -> bool;

    /// Closes the connection and cleans up any associated resources.
    async fn close(self: Box<Self>) -> Result<(), McpError>;
}

/// A client that manages a connection to a single MCP server.
///
/// `McpClient` acts as a state machine, wrapping a specific `McpTransport` and
/// managing the lifecycle of the connection. It tracks the connection's status,
/// health, and any errors.
#[derive(Debug)]
pub struct McpClient {
    /// The transport used to establish and check the connection.
    transport: Box<dyn McpTransport>,

    /// The active connection, if one is established.
    connection: Option<Box<dyn McpConnection>>,

    /// The overall status of the plugin (e.g., Running, Stopped).
    status: PluginStatus,

    /// The status of the underlying transport connection (e.g., Connected, Disconnected).
    transport_status: TransportStatus,

    /// The current health status of the client.
    health: HealthStatus,

    /// The last error message, if the client is in an error state.
    last_error: Option<String>,
}

impl McpClient {
    /// Creates a new `McpClient` with the given transport.
    ///
    /// The client starts in a `Stopped` state.
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

    /// Connects to the MCP server using the configured transport.
    ///
    /// This method updates the client's status and health based on the outcome.
    pub async fn connect(&mut self) -> Result<(), McpError> {
        self.status = PluginStatus::Starting;
        self.transport_status = TransportStatus::Connecting;
        self.last_error = None;

        match self.transport.connect().await {
            Ok(connection) => {
                self.connection = Some(connection);
                self.status = PluginStatus::Running;
                self.transport_status = TransportStatus::Connected;
                self.health.mark_healthy();
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

    /// Disconnects from the MCP server.
    ///
    /// This closes the active connection and updates the client's status.
    pub async fn disconnect(&mut self) -> Result<(), McpError> {
        self.status = PluginStatus::Stopping;

        if let Some(connection) = self.connection.take() {
            connection.close().await?;
        }

        self.status = PluginStatus::Stopped;
        self.transport_status = TransportStatus::Disconnected;
        self.health.mark_unhealthy("Disconnected".to_string());

        Ok(())
    }

    /// Returns `true` if the client is currently connected and running.
    pub fn is_connected(&self) -> bool {
        self.connection.is_some() && self.status.is_running()
    }

    /// Returns the current `PluginStatus` of the client.
    pub fn status(&self) -> PluginStatus {
        self.status
    }

    /// Returns the current `TransportStatus` of the client's connection.
    pub fn transport_status(&self) -> TransportStatus {
        self.transport_status
    }

    /// Returns a reference to the client's current `HealthStatus`.
    pub fn health(&self) -> &HealthStatus {
        &self.health
    }

    /// Returns the last error message, if any.
    pub fn last_error(&self) -> Option<&String> {
        self.last_error.as_ref()
    }

    /// Performs a health check using the client's transport.
    ///
    /// This updates the client's internal health state based on the result.
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

    /// Returns a reference to the `rmcp` service peer if the client is connected.
    ///
    /// This can be used to make direct RPC calls to the MCP server.
    pub fn peer(&self) -> Option<&rmcp::service::Peer<RoleClient>> {
        self.connection.as_ref().map(|conn| conn.peer())
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
        assert!(client.last_error().is_none());
    }
}
