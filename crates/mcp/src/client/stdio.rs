//! Stdio transport implementation for MCP clients.
//!
//! This module provides an `McpTransport` that communicates with an MCP server
//! by spawning it as a child process and interacting with it over `stdin` and
//! `stdout`.
//!
//! ## Design
//!
//! - `StdioTransport`: Responsible for spawning the child process based on the
//!   server configuration (`command`, `args`, etc.).
//! - `StdioClient`: Represents an active connection to the child process. It wraps
//!   the `rmcp` `RunningService` to manage the connection and communication.
//! - **Process Management**: The transport handles the details of setting up the
//!   `tokio::process::Command`, including piping `stdin`, `stdout`, and `stderr`.
//!   The `StdioClient` ensures that the child process is properly terminated when
//!   the connection is closed.

use crate::client::{HealthCheckResult, McpConnection, McpTransport};
use crate::config::McpServer;
use rmcp::{
    ErrorData as McpError, ServiceExt,
    service::{RoleClient, RunningService},
    transport::TokioChildProcess,
};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// An `McpTransport` that spawns a child process and communicates over stdio.
pub struct StdioTransport {
    /// The server configuration, specifying the command and arguments to execute.
    server: McpServer,

    /// The timeout for health checks, which involves spawning a new process.
    health_check_timeout: Duration,
}

/// An `McpConnection` for a stdio-based transport.
///
/// This wraps the `rmcp` service that manages the running child process.
pub struct StdioClient {
    /// The running `rmcp` service, which handles the RPC protocol.
    service: RunningService<RoleClient, ()>,

    /// The `stderr` handle of the child process, which can be used for logging.
    #[allow(dead_code)]
    stderr: Option<tokio::process::ChildStderr>,
}

impl StdioTransport {
    /// Creates a new `StdioTransport`.
    pub fn new(server: McpServer) -> Self {
        Self {
            server,
            health_check_timeout: Duration::from_secs(20),
        }
    }

    /// Creates a new `StdioTransport` with a custom timeout for health checks.
    #[allow(dead_code)]
    pub fn with_timeout(server: McpServer, timeout: Duration) -> Self {
        Self {
            server,
            health_check_timeout: timeout,
        }
    }

    /// Builds a `tokio::process::Command` from the server configuration.
    ///
    /// This sets up the command, arguments, environment variables, and working
    /// directory, and configures the stdio pipes.
    fn build_command(&self) -> Result<Command, McpError> {
        let command = self
            .server
            .command
            .as_ref()
            .ok_or_else(|| McpError::invalid_request("No command specified for stdio transport", None))?;

        let mut cmd = Command::new(command);

        if let Some(args) = &self.server.args {
            cmd.args(args);
        }
        if let Some(env) = &self.server.env {
            cmd.envs(env.clone());
        }
        if let Some(cwd) = &self.server.cwd {
            cmd.current_dir(cwd);
        }

        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());

        Ok(cmd)
    }
}

#[async_trait::async_trait]
impl McpTransport for StdioTransport {
    async fn connect(&self) -> Result<Box<dyn McpConnection>, McpError> {
        let cmd = self.build_command()?;

        let (transport, child_stderr) = TokioChildProcess::builder(cmd)
            .spawn()
            .map_err(|e| McpError::transport(format!("Failed to spawn process: {}", e), None))?;

        let service = ()
            .serve(transport)
            .await
            .map_err(|e| McpError::internal(format!("Failed to start MCP service: {}", e), None))?;

        Ok(Box::new(StdioClient {
            service,
            stderr: child_stderr,
        }))
    }

    async fn health_check(&self) -> Result<HealthCheckResult, McpError> {
        let start = std::time::Instant::now();

        // A health check for a stdio transport involves spawning the process and
        // ensuring the service can start.
        match timeout(self.health_check_timeout, self.connect()).await {
            Ok(Ok(connection)) => {
                let latency = start.elapsed().as_millis() as u64;
                // We successfully connected, so close it immediately.
                let _ = connection.close().await;
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

    /// Checks if the connection to the child process is still active.
    ///
    /// This is determined by checking if the underlying `rmcp` service is closed.
    async fn is_alive(&self) -> bool {
        !self.service.is_closed()
    }

    /// Closes the connection and terminates the child process.
    async fn close(mut self: Box<Self>) -> Result<(), McpError> {
        self.service.close().await;
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

    #[tokio::test]
    async fn test_build_command() {
        let mut server = McpServer::default();
        server.command = Some("node".to_string());
        server.args = Some(vec!["-e".to_string(), "console.log('test')".to_string()]);

        let transport = StdioTransport::new(server);
        let cmd = transport.build_command().unwrap();

        assert_eq!(cmd.as_std().get_program(), "node");
        let mut args = cmd.as_std().get_args();
        assert_eq!(args.next().unwrap(), "-e");
        assert_eq!(args.next().unwrap(), "console.log('test')");
    }
}
