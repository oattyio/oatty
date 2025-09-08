//! Stdio transport implementation for MCP clients.

use crate::client::{HealthCheckResult, McpConnection, McpTransport};
use crate::config::McpServer;
use rmcp::{
    ErrorData as McpError, ServiceExt, service::RoleClient, service::RunningService, transport::TokioChildProcess,
};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// Stdio transport for MCP clients.
pub struct StdioTransport {
    /// Server configuration.
    server: McpServer,

    /// Command timeout for health checks.
    health_check_timeout: Duration,
}

/// Stdio connection wrapper.
pub struct StdioClient {
    /// The running MCP service.
    service: RunningService<RoleClient, ()>,

    /// Child stderr handle for potential log streaming.
    #[allow(dead_code)]
    stderr: Option<tokio::process::ChildStderr>,
}

impl StdioTransport {
    /// Create a new stdio transport.
    pub fn new(server: McpServer) -> Self {
        Self {
            server,
            health_check_timeout: Duration::from_secs(20),
        }
    }

    /// Create a new stdio transport with custom timeout.
    pub fn with_timeout(server: McpServer, timeout: Duration) -> Self {
        Self {
            server,
            health_check_timeout: timeout,
        }
    }

    /// Build the command for the MCP server.
    fn build_command(&self) -> Result<Command, McpError> {
        let command = self
            .server
            .command
            .as_ref()
            .ok_or_else(|| McpError::invalid_request("No command specified for stdio transport", None))?;

        let mut cmd = Command::new(command);

        // Set arguments
        if let Some(args) = &self.server.args {
            cmd.args(args);
        }

        // Set environment variables
        if let Some(env) = &self.server.env {
            for (key, value) in env {
                cmd.env(key, value);
            }
        }

        // Set working directory
        if let Some(cwd) = &self.server.cwd {
            cmd.current_dir(cwd);
        }

        // Configure stdio
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        Ok(cmd)
    }
}

#[async_trait::async_trait]
impl McpTransport for StdioTransport {
    async fn connect(&self) -> Result<Box<dyn McpConnection>, McpError> {
        let cmd = self.build_command()?;

        // Create the MCP service using rmcp's builder
        // Capture the real child process handle for proper lifecycle management
        let (transport, child_stderr) = TokioChildProcess::builder(cmd)
            .spawn()
            .map_err(|e| McpError::invalid_request(format!("Failed to spawn process: {}", e), None))?;

        // Start the service with default client handler
        let service = ()
            .serve(transport)
            .await
            .map_err(|e| McpError::invalid_request(format!("Failed to start MCP service: {}", e), None))?;

        Ok(Box::new(StdioClient {
            service,
            stderr: child_stderr,
        }))
    }

    async fn health_check(&self) -> Result<HealthCheckResult, McpError> {
        let start = std::time::Instant::now();

        // Try to connect and perform a basic handshake
        match timeout(self.health_check_timeout, self.connect()).await {
            Ok(Ok(connection)) => {
                let latency = start.elapsed().as_millis() as u64;

                // Close the connection
                connection.close().await.ok();

                Ok(HealthCheckResult {
                    healthy: true,
                    latency_ms: Some(latency),
                    error: None,
                })
            }
            Ok(Err(error)) => Ok(HealthCheckResult {
                healthy: false,
                latency_ms: None,
                error: Some(error.to_string()),
            }),
            Err(_) => Ok(HealthCheckResult {
                healthy: false,
                latency_ms: None,
                error: Some("Health check timeout".to_string()),
            }),
        }
    }

    fn transport_type(&self) -> &'static str {
        "stdio"
    }

    fn server_config(&self) -> &McpServer {
        &self.server
    }
}

#[async_trait::async_trait]
impl McpConnection for StdioClient {
    fn peer(&self) -> &rmcp::service::Peer<RoleClient> {
        self.service.peer()
    }

    async fn is_alive(&self) -> bool {
        // For now, we'll assume the service is alive if it exists
        // In a real implementation, we'd check the actual process status

        // Try to perform a simple operation to check if the service is responsive
        // This is a basic check - in a real implementation, you might want to
        // send a ping or list tools to verify the connection is working
        true
    }

    async fn close(self: Box<Self>) -> Result<(), McpError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::McpServer;

    #[test]
    fn test_stdio_transport_creation() {
        let mut server = McpServer::default();
        server.command = Some("echo".to_string());
        server.args = Some(vec!["hello".to_string()]);

        let transport = StdioTransport::new(server);
        assert_eq!(transport.transport_type(), "stdio");
    }

    #[test]
    fn test_build_command() {
        let mut server = McpServer::default();
        server.command = Some("node".to_string());
        server.args = Some(vec!["-e".to_string(), "console.log('test')".to_string()]);

        let transport = StdioTransport::new(server);
        let cmd = transport.build_command().unwrap();

        // We can't easily test the command without spawning it,
        // but we can verify it was created successfully
        assert!(cmd.as_std().get_program() == "node");
    }
}
