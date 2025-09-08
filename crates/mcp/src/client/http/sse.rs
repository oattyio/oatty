//! Server-Sent Events (SSE) listener and parser.
//!
//! This module contains the logic for handling an SSE connection, including
//! parsing event frames, managing a reconnect backoff strategy, and correlating
//! incoming messages with pending requests. It is designed to be used internally
//! by the HTTP transport.

use futures_util::StreamExt;
use rmcp::ErrorData as McpError;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;

/// Spawns a background task to listen for Server-Sent Events (SSE).
///
/// The task will continuously try to connect to the SSE endpoint and listen for
/// events. It handles reconnects with an exponential backoff strategy. Parsed
/// events are used to resolve pending requests.
pub(super) fn spawn_sse_listener(
    client: &reqwest::Client,
    base_url: &url::Url,
    pending: &std::sync::Arc<Mutex<HashMap<String, oneshot::Sender<JsonValue>>>>,
) -> JoinHandle<()> {
    let client = client.clone();
    let base = base_url.clone();
    let pending = std::sync::Arc::clone(pending);

    tokio::spawn(async move {
        let mut last_event_id: Option<String> = None;
        let mut backoff_ms: u64 = 500; // Start at 0.5s, max 10s

        loop {
            let events_url = base.join("/events").unwrap_or_else(|_| base.clone());

            let mut req = client
                .get(events_url)
                .header(reqwest::header::ACCEPT, "text/event-stream");
            if let Some(id) = &last_event_id {
                req = req.header("Last-Event-ID", id);
            }

            match req.send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        backoff_ms = 500; // Reset backoff on success
                        if let Err(e) = process_sse_stream(&mut resp.bytes_stream(), &pending, &mut last_event_id).await
                        {
                            tracing::warn!(target: "mcp_http_sse", "SSE stream error: {}", e);
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
            tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
            backoff_ms = (backoff_ms * 2).min(10_000);
        }
    })
}

/// Processes the byte stream of an SSE response.
async fn process_sse_stream(
    stream: &mut (impl StreamExt<Item = reqwest::Result<bytes::Bytes>> + Unpin),
    pending: &std::sync::Arc<Mutex<HashMap<String, oneshot::Sender<JsonValue>>>>,
    last_event_id: &mut Option<String>,
) -> Result<(), McpError> {
    let mut buf = Vec::<u8>::new();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| McpError::internal(e.to_string(), None))?;
        buf.extend_from_slice(&bytes);

        // Process all complete frames in the buffer
        while let Some(end) = find_frame_end(&buf) {
            let frame_bytes = &buf[..end];
            if let Ok(text) = std::str::from_utf8(frame_bytes) {
                if let Some(frame) = parse_sse_frame(text) {
                    if let Some(id) = &frame.id {
                        *last_event_id = Some(id.clone());
                    }
                    if let Some(data) = frame.data {
                        handle_sse_data(&data, last_event_id, pending).await;
                    }
                }
            }
            // Drain the processed frame from the buffer
            buf.drain(..end + find_separator_len(&buf[end..]));
        }
    }
    Ok(())
}

/// Handles the data from a single SSE frame.
async fn handle_sse_data(
    data: &str,
    last_event_id: &Option<String>,
    pending: &std::sync::Arc<Mutex<HashMap<String, oneshot::Sender<JsonValue>>>>,
) {
    if let Ok(json) = serde_json::from_str::<JsonValue>(data) {
        let key = extract_id(&json).or_else(|| last_event_id.clone());
        if let Some(id) = key {
            if let Some(tx) = take_pending(pending, &id).await {
                if tx.send(json).is_err() {
                    tracing::debug!(target: "mcp_http_sse", "SSE response receiver for id={} was dropped", id);
                }
            } else {
                tracing::debug!(target: "mcp_http_sse", "Unmatched SSE response with id={}", id);
            }
        } else {
            tracing::debug!(target: "mcp_http_sse", "Received SSE event data: {}", data);
        }
    } else {
        tracing::debug!(target: "mcp_http_sse", "Received non-JSON SSE data: {}", data);
    }
}

/// Finds the end of the first SSE frame in the buffer.
fn find_frame_end(buf: &[u8]) -> Option<usize> {
    for i in 0..buf.len() {
        if i + 1 < buf.len() && &buf[i..=i + 1] == b"\n\n" {
            return Some(i);
        }
        if i + 3 < buf.len() && &buf[i..=i + 3] == b"\r\n\r\n" {
            return Some(i);
        }
    }
    None
}

/// Finds the length of the SSE frame separator.
fn find_separator_len(slice: &[u8]) -> usize {
    if slice.starts_with(b"\n\n") {
        2
    } else if slice.starts_with(b"\r\n\r\n") {
        4
    } else {
        0
    }
}

/// Represents a single parsed SSE frame.
struct SseFrame {
    data: Option<String>,
    id: Option<String>,
    _event: Option<String>,
    _retry: Option<u64>,
}

/// Parses a string slice into an `SseFrame`.
fn parse_sse_frame(frame_text: &str) -> Option<SseFrame> {
    let mut data_lines = Vec::new();
    let mut id: Option<String> = None;
    let mut event: Option<String> = None;
    let mut retry: Option<u64> = None;

    for line in frame_text.lines() {
        if line.starts_with(':') {
            continue; // Ignore comments
        }
        if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.strip_prefix(' ').unwrap_or(rest));
        } else if let Some(rest) = line.strip_prefix("id:") {
            id = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("event:") {
            event = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("retry:") {
            if let Ok(ms) = rest.trim().parse() {
                retry = Some(ms);
            }
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

/// Extracts an `id` field from a JSON value.
fn extract_id(v: &JsonValue) -> Option<String> {
    match v.get("id") {
        Some(JsonValue::String(s)) => Some(s.clone()),
        Some(JsonValue::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

/// Atomically removes a pending request sender from the map.
pub(super) async fn take_pending(
    pending: &std::sync::Arc<Mutex<HashMap<String, oneshot::Sender<JsonValue>>>>,
    id: &str,
) -> Option<oneshot::Sender<JsonValue>> {
    let mut map = pending.lock().await;
    map.remove(id)
}
