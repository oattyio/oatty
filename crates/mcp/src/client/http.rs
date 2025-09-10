//! HTTP/SSE transport implementation for MCP clients.

use crate::McpError;
use crate::client::{HealthCheckResult, McpConnection, McpTransport};
use crate::config::McpServer;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::Map as JsonMap;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;
use tokio::time::timeout;

/// HTTP transport for MCP clients.
pub struct HttpTransport {
    /// Server configuration.
    server: McpServer,

    /// Health check timeout.
    health_check_timeout: Duration,
}

/// HTTP connection wrapper.
pub struct HttpClient {
    /// HTTP client.
    client: Client,

    /// Base URL.
    base_url: url::Url,

    /// Background SSE listener task.
    sse_task: Option<JoinHandle<()>>,

    /// Pending request correlator map: id -> response sender
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<JsonValue>>>>,

    /// Monotonic id counter for requests
    id_counter: AtomicU64,
}

impl HttpTransport {
    /// Create a new HTTP transport.
    pub fn new(server: McpServer) -> Result<Self, McpError> {
        Self::init(server, Duration::from_secs(20))
    }

    /// Create a new HTTP transport with custom timeout.
    pub fn with_timeout(server: McpServer, timeout: Duration) -> Result<Self, McpError> {
        Self::init(server, timeout)
    }

    /// Shared constructor that validates configuration and client settings.
    fn init(server: McpServer, timeout: Duration) -> Result<Self, McpError> {
        // Validate base URL presence
        if server.base_url.is_none() {
            return Err(McpError::invalid_request(
                "No baseUrl specified for HTTP transport",
                None,
            ));
        }

        // Build a client to validate timeout and global settings; not stored
        Self::try_build_client(timeout)?;

        Ok(Self {
            server,
            health_check_timeout: timeout,
        })
    }

    /// Get the base URL from the server configuration.
    fn get_base_url(&self) -> Result<url::Url, McpError> {
        self.server
            .base_url
            .as_ref()
            .ok_or_else(|| McpError::invalid_request("No baseUrl specified for HTTP transport", None))
            .cloned()
    }

    /// Build a `reqwest::Client` with the given timeout to validate settings.
    fn try_build_client(timeout: Duration) -> Result<(), McpError> {
        Client::builder()
            .timeout(timeout)
            .build()
            .map(|_| ())
            .map_err(|e| McpError::invalid_request(format!("Failed to create HTTP client: {}", e), None))
    }

    /// Build headers for requests.
    fn build_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();

        if let Some(config_headers) = &self.server.headers {
            for (key, value) in config_headers {
                if let (Ok(key), Ok(value)) = (
                    reqwest::header::HeaderName::try_from(key),
                    reqwest::header::HeaderValue::try_from(value),
                ) {
                    headers.insert(key, value);
                }
            }
        }

        // Inject Basic Authorization header from auth config if present
        if let Some(auth) = &self.server.auth {
            if auth.scheme == "basic" {
                if headers.contains_key(reqwest::header::AUTHORIZATION) {
                    // Respect explicit headers if already provided in config
                } else if let Some(value) = build_basic_auth_header(auth) {
                    headers.insert(reqwest::header::AUTHORIZATION, value);
                }
            }
        }

        headers
    }

    /// Build a candidate health URL with sensible fallbacks.
    fn build_health_url(base_url: &url::Url) -> Result<url::Url, McpError> {
        base_url
            .join("/health")
            .or_else(|_| base_url.join("/ping"))
            .or_else(|_| Ok(base_url.clone()))
            .map_err(|e: url::ParseError| McpError::invalid_request(format!("Invalid health URL: {}", e), None))
    }
}

/// Build a Basic Authorization header value from auth config (already interpolated).
fn build_basic_auth_header(auth: &crate::config::McpAuthConfig) -> Option<reqwest::header::HeaderValue> {
    use base64::Engine;
    let engine = base64::engine::general_purpose::STANDARD;
    // Prefer explicit username/password when both present
    let pair = match (&auth.username, &auth.password, &auth.token) {
        (Some(u), Some(p), _) => format!("{}:{}", u, p),
        (Some(u), None, Some(t)) => format!("{}:{}", u, t),
        (None, None, Some(t)) => format!("{}:", t),
        _ => return None,
    };
    let encoded = engine.encode(pair);
    let value = format!("Basic {}", encoded);
    reqwest::header::HeaderValue::from_str(&value).ok()
}

#[async_trait::async_trait]
impl McpTransport for HttpTransport {
    async fn connect(&self) -> Result<Box<dyn McpConnection>, McpError> {
        let base_url = self.get_base_url()?;
        let headers = self.build_headers();

        // Create a client with the configured headers
        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| McpError::invalid_request(format!("Failed to create HTTP client: {}", e), None))?;

        // Start a background SSE listener to begin wiring HTTP/SSE behavior.
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let sse_task = Some(spawn_sse_listener(&client, &base_url, Arc::clone(&pending)));

        Ok(Box::new(HttpClient {
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

        // Perform a simple HTTP health check
        let client = Client::builder()
            .default_headers(headers)
            .timeout(self.health_check_timeout)
            .build()
            .map_err(|e| McpError::invalid_request(format!("Failed to create HTTP client: {}", e), None))?;

        let health_url = Self::build_health_url(&base_url)?;

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
    async fn is_alive(&mut self) -> bool {
        // Perform a simple HTTP request to check if the service is alive
        match self.client.get(self.base_url.clone()).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    async fn close(self: Box<Self>) -> Result<(), McpError> {
        // Stop SSE task if running
        if let Some(handle) = self.sse_task {
            handle.abort();
        }
        Ok(())
    }
}

// Mock HTTP service implementation.
// In a real implementation, this would implement the MCP protocol over HTTP/SSE.
// HTTP transport implementation would go here

/// Spawn a background task that listens to an SSE endpoint and
/// dispatches correlated responses back to awaiting requests.
fn spawn_sse_listener(
    client: &Client,
    base_url: &url::Url,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<JsonValue>>>>,
) -> JoinHandle<()> {
    let client = client.clone();
    let base = base_url.clone();
    tokio::spawn(async move {
        let mut last_event_id: Option<String> = None;
        let mut backoff_ms: u64 = 500; // start 0.5s, max 10s
        loop {
            let events_url = sse_events_url(&base);
            let req = sse_request(&client, &events_url, last_event_id.as_deref());

            match req.send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let mut stream = resp.bytes_stream();
                        let mut buf = Vec::<u8>::new();
                        // Reset backoff after a successful connect
                        backoff_ms = 500;
                        'stream_loop: while let Some(chunk) = stream.next().await {
                            match chunk {
                                Ok(bytes) => {
                                    buf.extend_from_slice(&bytes);
                                    last_event_id = process_sse_buffer(&mut buf, last_event_id, &pending).await;
                                }
                                Err(e) => {
                                    tracing::warn!(target: "mcp_http_sse", "SSE stream error: {}", e);
                                    break 'stream_loop;
                                }
                            }
                        }
                    } else {
                        tracing::warn!(target: "mcp_http_sse", "SSE request failed: {}", resp.status());
                    }
                }
                Err(e) => {
                    tracing::warn!(target: "mcp_http_sse", "SSE connect error: {}", e);
                }
            }

            // Reconnect with backoff
            let delay = std::time::Duration::from_millis(backoff_ms);
            tokio::time::sleep(delay).await;
            backoff_ms = (backoff_ms * 2).min(10_000);
        }
    })
}

/// Build the SSE events URL using common fallbacks.
fn sse_events_url(base: &url::Url) -> url::Url {
    base.join("/events")
        .ok()
        .or_else(|| base.join("/sse").ok())
        .unwrap_or_else(|| base.clone())
}

/// Create a pre-configured SSE request with optional last-event id.
fn sse_request(client: &Client, url: &url::Url, last_event_id: Option<&str>) -> reqwest::RequestBuilder {
    let builder = client
        .get(url.clone())
        .header(reqwest::header::ACCEPT, "text/event-stream");
    match last_event_id {
        Some(id) => builder.header("Last-Event-ID", id),
        None => builder,
    }
}

struct SseFrame {
    data: Option<String>,
    id: Option<String>,
    _event: Option<String>,
    _retry: Option<u64>,
}

/// Parse a single SSE frame from a text block.
fn parse_sse_frame(frame: &str) -> Option<SseFrame> {
    let mut data_lines = Vec::new();
    let mut id: Option<String> = None;
    let mut event: Option<String> = None;
    let mut retry: Option<u64> = None;
    for line in frame.lines() {
        if let Some(rest) = line.strip_prefix(':') {
            // comment
            let _ = rest;
            continue;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.strip_prefix(' ').unwrap_or(rest));
            continue;
        }
        if let Some(rest) = line.strip_prefix("id:") {
            id = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("event:") {
            event = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("retry:") {
            if let Ok(ms) = rest.trim().parse::<u64>() {
                retry = Some(ms);
            }
            continue;
        }
    }
    if data_lines.is_empty() && id.is_none() && event.is_none() && retry.is_none() {
        None
    } else {
        Some(SseFrame {
            data: if data_lines.is_empty() {
                None
            } else {
                Some(data_lines.join("\n"))
            },
            id,
            _event: event,
            _retry: retry,
        })
    }
}

/// Extract a string id from a JSON value, accepting string or number forms.
fn extract_id(v: &JsonValue) -> Option<String> {
    match v.get("id") {
        Some(JsonValue::String(s)) => Some(s.clone()),
        Some(JsonValue::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

async fn take_pending(
    pending: &Arc<Mutex<HashMap<String, oneshot::Sender<JsonValue>>>>,
    id: &str,
) -> Option<oneshot::Sender<JsonValue>> {
    let mut map = pending.lock().await;
    map.remove(id)
}

/// Process accumulated SSE bytes, delivering any complete frames to awaiting callers.
/// Returns the latest seen event id (if any), to be used for reconnection.
async fn process_sse_buffer(
    buf: &mut Vec<u8>,
    mut last_event_id: Option<String>,
    pending: &Arc<Mutex<HashMap<String, oneshot::Sender<JsonValue>>>>,
) -> Option<String> {
    // Parse SSE frames separated by blank lines using substring search
    let mut start = 0;
    while let Some((end, sep_len)) = find_frame_end(&buf[start..]) {
        let absolute_end = start + end;
        let frame_bytes = &buf[start..absolute_end];
        if let Ok(text) = std::str::from_utf8(frame_bytes) {
            if let Some(frame) = parse_sse_frame(text) {
                if let Some(id) = &frame.id {
                    last_event_id = Some(id.clone());
                }
                if let Some(data) = frame.data {
                    if let Ok(json) = serde_json::from_str::<JsonValue>(&data) {
                        let key = extract_id(&json).or_else(|| last_event_id.clone());
                        if let Some(id) = key {
                            if let Some(tx) = take_pending(pending, &id).await {
                                let _ = tx.send(json);
                            } else {
                                tracing::debug!(target: "mcp_http_sse", "unmatched response id={}", id);
                            }
                        } else {
                            tracing::debug!(target: "mcp_http_sse", "event(data): {}", data);
                        }
                    } else {
                        tracing::debug!(target: "mcp_http_sse", "non-json SSE data: {}", data);
                    }
                }
            }
        }
        // Advance past separator
        start = absolute_end + sep_len;
    }
    // Retain any partial frame
    if start > 0 {
        buf.drain(0..start);
    }

    // Cap buffer size to avoid unbounded growth (e.g., 2MB)
    const MAX_SSE_BUFFER_BYTES: usize = 2 * 1024 * 1024;
    if buf.len() > MAX_SSE_BUFFER_BYTES {
        tracing::warn!(target: "mcp_http_sse", "SSE buffer exceeded {} bytes; dropping partial data", MAX_SSE_BUFFER_BYTES);
        buf.clear();
    }
    last_event_id
}

/// Find the end of the next SSE frame within `buf`, returning the index of the
/// last byte of the frame and the separator length to skip (2 for \n\n, 4 for \r\n\r\n).
fn find_frame_end(buf: &[u8]) -> Option<(usize, usize)> {
    // Prefer CRLFCRLF when both present at the same position; otherwise choose earliest
    let pos_lf = buf.windows(2).position(|w| w == b"\n\n");
    let pos_crlf = buf.windows(4).position(|w| w == b"\r\n\r\n");
    match (pos_lf, pos_crlf) {
        (Some(a), Some(b)) => {
            if a < b {
                Some((a, 2))
            } else {
                Some((b, 4))
            }
        }
        (Some(a), None) => Some((a, 2)),
        (None, Some(b)) => Some((b, 4)),
        (None, None) => None,
    }
}

impl HttpClient {
    /// Post a JSON body to a relative path and await a correlated SSE response.
    /// If the body has no `id`, one is generated and inserted.
    pub async fn post_json(
        &self,
        path: &str,
        mut body: JsonValue,
        timeout_dur: Duration,
    ) -> Result<JsonValue, McpError> {
        // Determine or assign id
        let id = match body.get("id") {
            Some(JsonValue::String(s)) => s.clone(),
            Some(JsonValue::Number(n)) => n.to_string(),
            _ => {
                let next = self.id_counter.fetch_add(1, Ordering::Relaxed).to_string();
                match &mut body {
                    JsonValue::Object(map) => {
                        map.insert("id".to_string(), JsonValue::String(next.clone()));
                        next
                    }
                    _ => return Err(McpError::invalid_request("post_json body must be a JSON object", None)),
                }
            }
        };

        // Register waiter
        let (tx, rx) = oneshot::channel();
        {
            let mut map = self.pending.lock().await;
            map.insert(id.clone(), tx);
        }

        // Build URL and send
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
            .map_err(|e| McpError::invalid_request(format!("HTTP post failed: {}", e), None))?;

        if !resp.status().is_success() {
            // Remove pending and error
            let _ = take_pending(&self.pending, &id).await;
            return Err(McpError::invalid_request(format!("HTTP {}", resp.status()), None));
        }

        // Await correlated SSE response with timeout
        match tokio::time::timeout(timeout_dur, rx).await {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(_)) => Err(McpError::invalid_request("response channel closed", None)),
            Err(_) => {
                let _ = take_pending(&self.pending, &id).await;
                Err(McpError::invalid_request("response timeout", None))
            }
        }
    }

    /// Convenience method to call a tool over HTTP RPC.
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

        let mut body = JsonMap::new();
        body.insert("method".to_string(), JsonValue::String("callTool".to_string()));
        body.insert("params".to_string(), JsonValue::Object(params));

        self.post_json("/rpc", JsonValue::Object(body), timeout_dur).await
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
