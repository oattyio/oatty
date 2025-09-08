//! HTTP/SSE transport implementation for MCP clients.
//!
//! This module provides an `McpTransport` implementation that communicates with
//! an MCP server over HTTP and Server-Sent Events (SSE).
//!
//! ## Design
//!
//! - `HttpTransport`: The main transport struct, responsible for creating connections
//!   and performing health checks.
//! - `HttpClient`: Represents an active connection. It manages an HTTP client and a
//!   background task for listening to SSE events.
//! - **RPC over HTTP/SSE**: The client can send requests (e.g., to call a tool)
//!   via an HTTP POST. The server is expected to process the request and send the
//!   response back as an SSE event. A correlation mechanism using a unique `id`
//!   in the request and response is used to match them up.
//! - **SSE Listener**: A background task (`sse::spawn_sse_listener`) is responsible
//!   for maintaining a persistent connection to the server's SSE endpoint, handling
//!   reconnects with backoff, and dispatching incoming messages.

mod sse;

use crate::client::{HealthCheckResult, McpConnection, McpTransport};
use crate::config::McpServer;
use reqwest::Client;
use rmcp::{
    ErrorData as McpError, ServiceExt,
    service::{RoleClient, RunningService},
};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;
use tokio::time::timeout;

/// An `McpTransport` implementation that uses HTTP for requests and Server-Sent
/// Events (SSE) for receiving asynchronous responses.
pub struct HttpTransport {
    /// The server configuration, including base URL and headers.
    server: McpServer,

    /// The timeout for health checks.
    health_check_timeout: Duration,
}

/// An `McpConnection` implementation for the HTTP transport.
///
/// This struct holds the state for an active connection, including the HTTP client,
/// the background SSE listener task, and a map of pending requests waiting for
/// a response.
pub struct HttpClient {
    /// The underlying `rmcp` service.
    /// Note: In this transport, this is a placeholder as the actual communication
    /// happens over HTTP/SSE, not the `rmcp` transport layer.
    service: RunningService<RoleClient, ()>,

    /// The `reqwest` client used for making HTTP requests.
    client: Client,

    /// The base URL of the MCP server.
    base_url: url::Url,

    /// A handle to the background SSE listener task.
    sse_task: Option<JoinHandle<()>>,

    /// A map of pending requests, keyed by a correlation ID. The `oneshot::Sender`
    /// is used to send the response back to the waiting caller.
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<JsonValue>>>>,

    /// A monotonic counter for generating unique request IDs.
    id_counter: AtomicU64,
}

impl HttpTransport {
    /// Creates a new `HttpTransport`.
    ///
    /// Returns an error if the server configuration is missing a `base_url`.
    pub fn new(server: McpServer) -> Result<Self, McpError> {
        if server.base_url.is_none() {
            return Err(McpError::invalid_request(
                "No baseUrl specified for HTTP transport",
                None,
            ));
        }
        Ok(Self {
            server,
            health_check_timeout: Duration::from_secs(20),
        })
    }

    /// Creates a new `HttpTransport` with a custom timeout for the underlying client.
    #[allow(dead_code)]
    pub fn with_timeout(server: McpServer, timeout: Duration) -> Result<Self, McpError> {
        if server.base_url.is_none() {
            return Err(McpError::invalid_request(
                "No baseUrl specified for HTTP transport",
                None,
            ));
        }
        Ok(Self {
            server,
            health_check_timeout: timeout,
        })
    }

    /// Extracts the base URL from the server configuration.
    fn get_base_url(&self) -> Result<url::Url, McpError> {
        self.server
            .base_url
            .as_ref()
            .cloned()
            .ok_or_else(|| McpError::invalid_request("No baseUrl specified for HTTP transport", None))
    }

    /// Builds the HTTP headers from the server configuration.
    fn build_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(config_headers) = &self.server.headers {
            for (key, value) in config_headers {
                if let (Ok(key), Ok(value)) = (
                    reqwest::header::HeaderName::try_from(key.as_str()),
                    reqwest::header::HeaderValue::try_from(value.as_str()),
                ) {
                    headers.insert(key, value);
                }
            }
        }
        headers
    }
}

#[async_trait::async_trait]
impl McpTransport for HttpTransport {
    async fn connect(&self) -> Result<Box<dyn McpConnection>, McpError> {
        let base_url = self.get_base_url()?;
        let headers = self.build_headers();

        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| McpError::invalid_request(format!("Failed to create HTTP client: {}", e), None))?;

        let pending = Arc::new(Mutex::new(HashMap::new()));
        let sse_task = Some(sse::spawn_sse_listener(&client, &base_url, Arc::clone(&pending)));

        // Create a mock service, as rmcp's transport isn't used directly.
        let dummy_service = ()
            .serve(tokio::io::empty())
            .await
            .map_err(|e| McpError::internal(format!("Failed to create dummy MCP service: {}", e), None))?;

        Ok(Box::new(HttpClient {
            service: dummy_service,
            client,
            base_url,
            sse_task,
            pending,
            id_counter: AtomicU64::new(1),
        }))
    }

    async fn health_check(&self) -> Result<HealthCheckResult, McpError> {
        let start = std::time::Instant::now();
        let base_url = self.get_base_url()?;
        let headers = self.build_headers();

        let client = Client::builder()
            .default_headers(headers)
            .timeout(self.health_check_timeout)
            .build()
            .map_err(|e| McpError::invalid_request(format!("Failed to create HTTP client: {}", e), None))?;

        let health_url = base_url.join("/health").unwrap_or_else(|_| base_url.clone());

        match timeout(self.health_check_timeout, client.get(health_url).send()).await {
            Ok(Ok(response)) => {
                let latency = start.elapsed().as_millis() as u64;
                let healthy = response.status().is_success();
                Ok(HealthCheckResult {
                    healthy,
                    latency_ms: Some(latency),
                    error: if healthy {
                        None
                    } else {
                        Some(format!("HTTP {}", response.status()))
                    },
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
        "http"
    }

    fn server_config(&self) -> &McpServer {
        &self.server
    }
}

#[async_trait::async_trait]
impl McpConnection for HttpClient {
    fn peer(&self) -> &rmcp::service::Peer<RoleClient> {
        self.service.peer()
    }

    async fn is_alive(&self) -> bool {
        // A simple health check can determine if the connection is alive.
        // We send a GET request to the base URL.
        match self.client.get(self.base_url.clone()).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    async fn close(mut self: Box<Self>) -> Result<(), McpError> {
        if let Some(handle) = self.sse_task.take() {
            handle.abort();
        }
        Ok(())
    }
}

impl HttpClient {
    /// Sends a JSON payload to a specified path via HTTP POST and waits for a
    /// correlated response to arrive via the SSE stream.
    ///
    /// If the `body` does not contain an `id` field, one will be generated.
    pub async fn post_json(
        &self,
        path: &str,
        mut body: JsonValue,
        timeout_dur: Duration,
    ) -> Result<JsonValue, McpError> {
        let id = match body.get("id").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => {
                let next = self.id_counter.fetch_add(1, Ordering::Relaxed).to_string();
                if let JsonValue::Object(map) = &mut body {
                    map.insert("id".to_string(), JsonValue::String(next.clone()));
                }
                next
            }
        };

        let (tx, rx) = oneshot::channel();
        {
            let mut map = self.pending.lock().await;
            map.insert(id.clone(), tx);
        }

        let url = self
            .base_url
            .join(path)
            .map_err(|e| McpError::invalid_request(format!("Invalid path: {}", e), None))?;

        let resp = self
            .client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| McpError::transport(format!("HTTP POST failed: {}", e), None))?;

        if !resp.status().is_success() {
            let _ = sse::take_pending(&self.pending, &id).await;
            return Err(McpError::transport(format!("HTTP {}", resp.status()), None));
        }

        match timeout(timeout_dur, rx).await {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(_)) => Err(McpError::internal("Response channel was closed", None)),
            Err(_) => {
                let _ = sse::take_pending(&self.pending, &id).await;
                Err(McpError::timeout("Timed out waiting for SSE response", None))
            }
        }
    }

    /// A convenience method for making an MCP `callTool` request over HTTP.
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: Option<JsonMap<String, JsonValue>>,
        timeout_dur: Duration,
    ) -> Result<JsonValue, McpError> {
        let mut params = JsonMap::new();
        params.insert("name".to_string(), JsonValue::String(tool_name.to_string()));
        if let Some(args) = arguments {
            params.insert("arguments".to_string(), JsonValue::Object(args));
        }

        let body = JsonValue::Object(
            serde_json::json!({
                "method": "callTool",
                "params": params,
            })
            .as_object()
            .unwrap()
            .clone(),
        );

        self.post_json("/rpc", body, timeout_dur).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::McpServer;
    use url::Url;

    #[test]
    fn test_http_transport_creation() {
        let mut server = McpServer::default();
        server.base_url = Some(Url::parse("https://example.com").unwrap());

        let transport = HttpTransport::new(server).unwrap();
        assert_eq!(transport.transport_type(), "http");
    }

    #[test]
    fn test_http_transport_missing_url() {
        let server = McpServer::default();
        let result = HttpTransport::new(server);
        assert!(result.is_err());
    }
}
