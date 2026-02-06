//! McpClient: lifecycle and connections for rmcp-backed plugins.

use std::{process::Stdio, sync::Arc, time::Duration};

use anyhow::Result;
use rmcp::model::InitializeResult;
use rmcp::{
    RoleClient,
    model::CallToolRequestParams,
    service::{RunningService, ServiceExt as _},
    transport::{StreamableHttpClientTransport, TokioChildProcess, streamable_http_client::StreamableHttpClientTransportConfig},
};
use tokio::time::timeout;

use crate::{
    config::McpServer,
    logging::LogManager,
    types::{HealthStatus, McpToolMetadata, PluginStatus},
};

use super::{
    health::HealthCheckResult,
    http::{build_http_client_with_auth, resolve_streamable_endpoint},
    stdio::{build_stdio_command, spawn_stderr_logger},
};

/// rmcp-backed MCP client lifecycle wrapper.
#[derive(Debug)]
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
    /// Last known list of tools exposed by the plugin.
    pub(crate) tools: Arc<Vec<McpToolMetadata>>,
}

/// Maximum amount of time to wait for a tool invocation before returning a timeout error.
const TOOL_INVOCATION_TIMEOUT: Duration = Duration::from_secs(30);

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
            tools: Arc::new(Vec::new()),
        }
    }

    /// Connect using rmcp via stdio or http transport.
    pub async fn connect(&mut self) -> Result<Arc<Vec<McpToolMetadata>>> {
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
        let tools = self.refresh_tools().await?;
        self.status = PluginStatus::Running;
        self.health.mark_healthy();
        self.health.handshake_latency = Some(start_time.elapsed().as_millis() as u64);
        Ok(tools)
    }

    /// Disconnect from the server and mark the client as stopped.
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(running) = self.service.take() {
            let _ = running.cancel().await; // ignore
        }
        self.status = PluginStatus::Stopped;
        self.health.mark_unhealthy("Disconnected".to_string());
        self.tools = Arc::new(Vec::new());
        Ok(())
    }

    /// Latest discovered tools for this client.
    pub fn tools(&self) -> Arc<Vec<McpToolMetadata>> {
        Arc::clone(&self.tools)
    }

    /// Current plugin status.
    pub fn status(&self) -> PluginStatus {
        self.status
    }
    /// Current health snapshot.
    pub fn health(&self) -> &HealthStatus {
        &self.health
    }

    /// Latest result of the Initialize handshake, if any.
    pub fn get_info(&self) -> Option<&InitializeResult> {
        self.service.as_ref().and_then(|service| service.peer_info())
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

    /// Connect via Streamable HTTP using rmcp's reqwest transport.
    async fn connect_http(&self) -> Result<RunningService<RoleClient, ()>> {
        let endpoint = resolve_streamable_endpoint(&self.server)?;
        let http_client = build_http_client_with_auth(&self.server).await?;
        let config = StreamableHttpClientTransportConfig::with_uri(endpoint);
        let transport = StreamableHttpClientTransport::with_client(http_client, config);
        Ok(().serve(transport).await?)
    }

    /// Fetch the current tool list from the active service and update the local snapshot.
    async fn refresh_tools(&mut self) -> Result<Arc<Vec<McpToolMetadata>>> {
        let service = self
            .service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("plugin '{}' is not connected", self.name))?;

        let tools = service
            .list_all_tools()
            .await
            .map_err(|err| anyhow::anyhow!("list_tools failed for '{}': {err}", self.name))?
            .into_iter()
            .map(McpToolMetadata::from)
            .collect::<Vec<_>>();

        let snapshot = Arc::new(tools);
        self.tools = Arc::clone(&snapshot);
        Ok(snapshot)
    }

    /// Invoke a tool provided by this plugin and return the raw MCP response payload.
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<rmcp::model::CallToolResult> {
        let service = self
            .service
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("plugin '{}' is not connected", self.name))?;

        let call_future = service.call_tool(CallToolRequestParams {
            name: tool_name.to_string().into(),
            arguments: Some(arguments.clone()),
            task: None,
            meta: None,
        });

        match timeout(TOOL_INVOCATION_TIMEOUT, call_future).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(err)) => Err(anyhow::anyhow!("tool '{}' failed: {err}", tool_name)),
            Err(_) => Err(anyhow::anyhow!(
                "tool '{}' timed out after {:?}",
                tool_name,
                TOOL_INVOCATION_TIMEOUT
            )),
        }
    }
}
