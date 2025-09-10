//! Stdio transport implementation for MCP clients.

use crate::McpError;
use crate::client::{HealthCheckResult, McpConnection, McpTransport};
use crate::config::McpServer;
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
    /// Child process handle.
    child: tokio::process::Child,

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

        Self::apply_args_env_cwd(&mut cmd, &self.server);
        Self::configure_stdio(&mut cmd);

        Ok(cmd)
    }

    /// Apply args, environment variables, and cwd settings to the command.
    fn apply_args_env_cwd(cmd: &mut Command, server: &McpServer) {
        // Start with a minimal, clean environment to avoid inheriting secrets.
        cmd.env_clear();
        // Provide a minimal PATH if needed (avoid command searches using broad PATHs).
        #[cfg(unix)]
        {
            cmd.env("PATH", "/usr/bin:/bin");
        }
        #[cfg(windows)]
        {
            // Keep Windows minimal PATH; if the command is absolute, PATH is not used.
            if std::env::var_os("PATH").is_some() {
                cmd.env("PATH", std::env::var("PATH").unwrap_or_default());
            }
        }

        if let Some(args) = &server.args {
            cmd.args(args);
        }
        if let Some(env) = &server.env {
            for (key, value) in env {
                cmd.env(key, value);
            }
        }
        if let Some(cwd) = &server.cwd {
            cmd.current_dir(cwd);
        }
    }

    /// Configure stdio pipes for the child process.
    fn configure_stdio(cmd: &mut Command) {
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        // Drop privileges and harden the process on Unix before exec.
        #[cfg(unix)]
        {
            use libc::{getgid, getuid, setgid, setuid};
            unsafe {
                cmd.pre_exec(|| {
                    // Disable core dumps for the child process in a portable way
                    #[cfg(target_os = "linux")]
                    {
                        use libc::{PR_SET_DUMPABLE, prctl};
                        let _ = prctl(PR_SET_DUMPABLE, 0, 0, 0, 0);
                    }
                    #[cfg(all(unix, not(target_os = "linux")))]
                    {
                        use libc::{RLIMIT_CORE, rlimit, setrlimit};
                        let lim = rlimit {
                            rlim_cur: 0,
                            rlim_max: 0,
                        };
                        let _ = setrlimit(RLIMIT_CORE, &lim);
                    }

                    // Drop to invoking user's uid/gid (no-op if already non-root)
                    let uid = getuid();
                    let gid = getgid();
                    if setgid(gid) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    if setuid(uid) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }
    }
}

#[async_trait::async_trait]
impl McpTransport for StdioTransport {
    async fn connect(&self) -> Result<Box<dyn McpConnection>, McpError> {
        let mut cmd = self.build_command()?;
        let mut child = cmd
            .spawn()
            .map_err(|e| McpError::invalid_request(format!("Failed to spawn process: {}", e), None))?;
        let stderr = child.stderr.take();

        Ok(Box::new(StdioClient { child, stderr }))
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
    async fn is_alive(&mut self) -> bool {
        match self.child.id() {
            Some(_) => match self.child.try_wait() {
                Ok(Some(_status)) => false,
                Ok(None) => true,
                Err(_) => false,
            },
            None => false,
        }
    }

    async fn close(mut self: Box<Self>) -> Result<(), McpError> {
        // Terminate the child process if still running
        let _ = self.child.kill().await;
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
