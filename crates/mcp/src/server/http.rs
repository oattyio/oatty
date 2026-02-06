//! Local MCP HTTP server host utilities.

use std::net::{IpAddr, SocketAddr};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use anyhow::{Result, anyhow};
use axum::Router;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::PluginEngine;
use crate::server::core::{McpToolServices, OattyMcpCore};
use oatty_registry::{CommandRegistry, spawn_search_engine_thread};
use std::sync::Mutex;

/// Log entry emitted by the local MCP HTTP server.
#[derive(Debug, Clone)]
pub struct McpHttpLogEntry {
    /// Human-readable summary for list display.
    pub message: String,
    /// Optional structured payload for detail inspection.
    pub payload: Option<Value>,
}

impl McpHttpLogEntry {
    /// Create a new MCP HTTP log entry.
    pub fn new(message: String, payload: Option<Value>) -> Self {
        Self { message, payload }
    }
}

/// Host configuration for a local MCP HTTP server instance.
#[derive(Debug, Clone)]
pub struct McpHttpServer {
    bind_address: SocketAddr,
    log_sender: Option<UnboundedSender<McpHttpLogEntry>>,
    services: Arc<McpToolServices>,
}

impl McpHttpServer {
    /// Create a new MCP HTTP server bound to the provided address.
    pub fn new(bind_address: SocketAddr, command_registry: Arc<Mutex<CommandRegistry>>, plugin_engine: Arc<PluginEngine>) -> Self {
        let search_handle = spawn_search_engine_thread(Arc::clone(&command_registry));
        let services = Arc::new(McpToolServices::new(command_registry, plugin_engine, search_handle));
        Self {
            bind_address,
            log_sender: None,
            services,
        }
    }

    /// Attach a log sender to stream request/response events to the caller.
    pub fn with_log_sender(mut self, log_sender: UnboundedSender<McpHttpLogEntry>) -> Self {
        self.log_sender = Some(log_sender);
        self
    }

    /// Start the server and return a handle for runtime inspection and shutdown.
    pub async fn start(self) -> Result<RunningMcpHttpServer> {
        let cancellation_token = CancellationToken::new();
        let session_manager = Arc::new(LocalSessionManager::default());
        let client_counter = Arc::new(AtomicUsize::new(0));
        let monitor_handle = spawn_session_monitor(
            Arc::clone(&session_manager),
            Arc::clone(&client_counter),
            cancellation_token.child_token(),
        );

        let log_sender = self.log_sender.clone();
        let services = Arc::clone(&self.services);
        let service: StreamableHttpService<OattyMcpCore, LocalSessionManager> = StreamableHttpService::new(
            move || Ok(OattyMcpCore::new(log_sender.clone(), Arc::clone(&services))),
            Arc::clone(&session_manager),
            StreamableHttpServerConfig {
                stateful_mode: true,
                sse_keep_alive: None,
                cancellation_token: cancellation_token.child_token(),
                ..Default::default()
            },
        );

        let router = Router::new().nest_service("/mcp", service);
        let listener = tokio::net::TcpListener::bind(self.bind_address).await?;
        let bound_address = listener.local_addr()?;

        let server_handle = tokio::spawn({
            let shutdown = cancellation_token.child_token();
            async move {
                let _ = axum::serve(listener, router)
                    .with_graceful_shutdown(async move {
                        shutdown.cancelled().await;
                    })
                    .await;
            }
        });

        Ok(RunningMcpHttpServer {
            bind_address: bound_address,
            cancellation_token,
            server_handle,
            monitor_handle,
            client_counter,
        })
    }
}

/// Runtime handle for a running MCP HTTP server.
#[derive(Debug)]
pub struct RunningMcpHttpServer {
    bind_address: SocketAddr,
    cancellation_token: CancellationToken,
    server_handle: JoinHandle<()>,
    monitor_handle: JoinHandle<()>,
    client_counter: Arc<AtomicUsize>,
}

impl RunningMcpHttpServer {
    /// Return the bound socket address for the running server.
    pub fn bound_address(&self) -> SocketAddr {
        self.bind_address
    }

    /// Return the most recently observed client count.
    pub fn connected_clients(&self) -> usize {
        self.client_counter.load(Ordering::Relaxed)
    }

    /// Stop the server and wait for background tasks to finish.
    pub async fn stop(self) -> Result<()> {
        self.cancellation_token.cancel();
        self.monitor_handle
            .await
            .map_err(|error| anyhow!("MCP HTTP monitor task failed: {error}"))?;
        self.server_handle
            .await
            .map_err(|error| anyhow!("MCP HTTP server task failed: {error}"))?;
        Ok(())
    }
}

/// Resolve a safe local bind address for the MCP HTTP server.
pub fn resolve_bind_address(bind_address: Option<&str>) -> Result<SocketAddr> {
    let address = bind_address.unwrap_or("127.0.0.1:0");
    let parsed: SocketAddr = address
        .parse()
        .map_err(|error| anyhow!("invalid MCP HTTP bind address '{address}': {error}"))?;
    if !is_loopback(parsed.ip()) {
        return Err(anyhow!("MCP HTTP server must bind to a loopback address"));
    }
    Ok(parsed)
}

fn is_loopback(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => ip.is_loopback(),
        IpAddr::V6(ip) => ip.is_loopback(),
    }
}

fn spawn_session_monitor(
    session_manager: Arc<LocalSessionManager>,
    client_counter: Arc<AtomicUsize>,
    cancellation_token: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_millis(500));
        loop {
            tokio::select! {
                _ = cancellation_token.cancelled() => break,
                _ = ticker.tick() => {
                    let count = session_manager.sessions.read().await.len();
                    client_counter.store(count, Ordering::Relaxed);
                }
            }
        }
    })
}
