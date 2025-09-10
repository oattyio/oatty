//! Adapters for ultrafast-mcp transports, behind the `mcp-ultrafast` feature.

// ultrafast-mcp adapters (rmcp removed)

use crate::McpError;
use crate::client::{HealthCheckResult, McpConnection, McpTransport};
use crate::config::McpServer;

/// HTTP transport backed by ultrafast-mcp's streamable HTTP client.
pub struct UltraHttpTransport {
    server: McpServer,
}

impl UltraHttpTransport {
    pub fn new(server: McpServer) -> Result<Self, McpError> {
        Ok(Self { server })
    }
}

/// Stdio transport backed by ultrafast-mcp's stdio transport.
pub struct UltraStdioTransport {
    server: McpServer,
}

impl UltraStdioTransport {
    pub fn new(server: McpServer) -> Result<Self, McpError> {
        Ok(Self { server })
    }
}

#[async_trait::async_trait]
impl McpTransport for UltraHttpTransport {
    async fn connect(&self) -> Result<Box<dyn McpConnection>, McpError> {
        // TODO: build ultrafast streamable HTTP client with Basic auth middleware
        Err(McpError::invalid_request(
            "ultrafast HTTP transport not yet implemented",
            None,
        ))
    }

    async fn health_check(&self) -> Result<HealthCheckResult, McpError> {
        Ok(HealthCheckResult {
            healthy: false,
            latency_ms: None,
            error: Some("not implemented".into()),
        })
    }

    fn transport_type(&self) -> &'static str {
        "http"
    }
    fn server_config(&self) -> &McpServer {
        &self.server
    }
}

#[async_trait::async_trait]
impl McpTransport for UltraStdioTransport {
    async fn connect(&self) -> Result<Box<dyn McpConnection>, McpError> {
        // TODO: create_transport(TransportConfig::Stdio{...}) and install ClientElicitationHandler for Basic
        Err(McpError::invalid_request(
            "ultrafast stdio transport not yet implemented",
            None,
        ))
    }

    async fn health_check(&self) -> Result<HealthCheckResult, McpError> {
        Ok(HealthCheckResult {
            healthy: false,
            latency_ms: None,
            error: Some("not implemented".into()),
        })
    }

    fn transport_type(&self) -> &'static str {
        "stdio"
    }
    fn server_config(&self) -> &McpServer {
        &self.server
    }
}

/// Minimal connection placeholder until ultrafast client is wired.
struct UltraConn;

#[async_trait::async_trait]
impl McpConnection for UltraConn {
    async fn is_alive(&mut self) -> bool {
        false
    }
    async fn close(self: Box<Self>) -> Result<(), McpError> {
        Ok(())
    }
}
