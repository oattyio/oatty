//! McpClient: lifecycle and connections for rmcp-backed plugins.

use std::{process::Stdio, sync::Arc};

use anyhow::Result;
use rmcp::{
    RoleClient,
    service::{RunningService, ServiceExt as _},
    transport::{SseClientTransport, TokioChildProcess, sse_client::SseClientConfig},
};

use crate::{
    config::McpServer,
    logging::LogManager,
    types::{HealthStatus, PluginStatus},
};

use super::{
    health::HealthCheckResult,
    http::{build_http_client_with_auth, build_sse_url},
    stdio::{build_stdio_command, spawn_stderr_logger},
};

/// rmcp-backed MCP client lifecycle wrapper.
pub struct McpClient {
    /// Logical plugin name.
    pub(crate) name: String,
    /// Server configuration for this client.
    pub(crate) server: McpServer,
    /// Current plugin status.
    pub(crate) status: PluginStatus,
    /// Aggregated health info for UI.
    pub(crate) health: HealthStatus,
    /// Underlying rmcp running service when connected.
    pub(crate) service: Option<RunningService<RoleClient, ()>>,
    /// Shared log manager for capturing plugin logs (e.g., stderr).
    pub(crate) log_manager: Arc<LogManager>,
}

impl McpClient {
    /// Construct a new client from a name and server configuration.
    pub fn new(name: String, server: McpServer, log_manager: Arc<LogManager>) -> Self {
        Self {
            name,
            server,
            status: PluginStatus::Stopped,
            health: HealthStatus::default(),
            service: None,
            log_manager,
        }
    }

    /// Connect using rmcp via stdio or http transport.
    pub async fn connect(&mut self) -> Result<()> {
        self.status = PluginStatus::Starting;
        let start_time = std::time::Instant::now();

        let service: RunningService<RoleClient, ()> = if self.server.is_stdio() {
            self.connect_stdio().await?
        } else if self.server.is_http() {
            self.connect_http().await?
        } else {
            anyhow::bail!("unsupported transport for plugin '{}': must be stdio or http", self.name)
        };

        self.service = Some(service);
        self.status = PluginStatus::Running;
        self.health.mark_healthy();
        self.health.handshake_latency = Some(start_time.elapsed().as_millis() as u64);
        Ok(())
    }

    /// Disconnect from the server and mark the client as stopped.
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(running) = self.service.take() {
            let _ = running.cancel().await; // ignore
        }
        self.status = PluginStatus::Stopped;
        self.health.mark_unhealthy("Disconnected".to_string());
        Ok(())
    }

    /// Current plugin status.
    pub fn status(&self) -> PluginStatus {
        self.status
    }
    /// Current health snapshot.
    pub fn health(&self) -> &HealthStatus {
        &self.health
    }

    /// Lightweight health probe used by the TUI refresh.
    pub async fn health_check(&mut self) -> Result<HealthCheckResult> {
        let healthy = matches!(self.status, PluginStatus::Running);
        Ok(HealthCheckResult {
            healthy,
            latency_ms: self.health.handshake_latency,
            error: (!healthy).then(|| "not running".to_string()),
        })
    }

    /// Connect via stdio using a spawned child process.
    async fn connect_stdio(&self) -> Result<RunningService<RoleClient, ()>> {
        let command = build_stdio_command(&self.server)?;
        // Use builder to capture stderr for logging
        let (transport, stderr_opt) = TokioChildProcess::builder(command).stderr(Stdio::piped()).spawn()?;

        if let Some(stderr) = stderr_opt {
            spawn_stderr_logger(self.name.clone(), self.log_manager.clone(), stderr);
        }

        Ok(().serve(transport).await?)
    }

    /// Connect via HTTP/SSE using rmcp's reqwest transport.
    async fn connect_http(&self) -> Result<RunningService<RoleClient, ()>> {
        let sse_url = build_sse_url(&self.server);
        let http_client = build_http_client_with_auth(&self.server).await?;
        let cfg = SseClientConfig {
            sse_endpoint: sse_url.as_str().into(),
            ..Default::default()
        };
        let transport = SseClientTransport::start_with_client(http_client, cfg)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        Ok(().serve(transport).await?)
    }
}
