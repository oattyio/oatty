//! HTTP execution helpers shared across TUI and Engine.
//!
//! This module centralizes remote execution of Heroku API requests based on
//! `CommandSpec`, handling headers, pagination, and response parsing.
//! It also provides a convenient `fetch_json_array` helper for list endpoints.

use crate::http;
use heroku_api::HerokuClient;
use heroku_types::CommandSpec;
use heroku_types::ExecOutcome;
use reqwest::header::{CONTENT_RANGE, HeaderMap, HeaderName};
use reqwest::Method;
use serde_json::{Map as JsonMap, Value};

/// Perform an asynchronous REST API call against the Heroku platform.
///
/// - Constructs the request from the `CommandSpec` and `path`.
/// - Applies Range headers from the body when present.
/// - Sends the request and parses the response into [`ExecOutcome`].
/// - Returns a user-friendly `Err(String)` on HTTP/auth/network issues.
pub async fn exec_remote(spec: &CommandSpec, body: JsonMap<String, Value>) -> Result<ExecOutcome, String> {
    let client = HerokuClient::new_from_service_id(spec.service_id).map_err(|e| {
        format!(
            "Auth setup failed: {}. Hint: set HEROKU_API_KEY or configure ~/.netrc",
            e
        )
    })?;

    let method = Method::from_bytes(&spec.method.as_bytes()).map_err(|e| e.to_string())?;
    let mut builder = client.request(method, &spec.path);

    // Build and apply Range header
    builder = apply_range_headers(builder, &body);

    // Filter out special range-only fields from JSON body
    let filtered_body = http::strip_range_body_fields(body);
    if !filtered_body.is_empty() {
        builder = builder.json(&Value::Object(filtered_body));
    }

    let resp = builder.send().await.map_err(|e| {
        format!(
            "Network error: {}. Hint: check connection/proxy; ensure HEROKU_API_KEY or ~/.netrc is set",
            e
        )
    })?;

    let status = resp.status();
    let headers = resp.headers().clone();
    let mut pagination = headers
        .get(CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(http::parse_content_range_value);

    // Handle Next-Range header for 206 responses
    if status.as_u16() == 206 {
        handle_next_range_header(&mut pagination, &headers);
    }

    let text = resp.text().await.unwrap_or_default();

    // Handle common error status codes
    if let Some(error_msg) = http::status_error_message(status.as_u16()) {
        return Err(error_msg);
    }

    let log = format!("{}\n{}", status, text);
    let (result_json, open_table) = http::parse_response_json(&text);

    Ok(ExecOutcome {
        log,
        result_json,
        open_table,
        pagination,
    })
}

/// Fetch a JSON array from the Heroku API at the given path.
///
/// Returns Ok(Vec<Value>) when the response body parses to a JSON array.
/// On error or non-array response, returns Err with a user-friendly message.
pub async fn fetch_json_array(spec: &CommandSpec) -> Result<Vec<Value>, String> {
    let client = HerokuClient::new_from_service_id(spec.service_id).map_err(|e| {
        format!(
            "Auth setup failed: {}. Hint: set HEROKU_API_KEY or configure ~/.netrc",
            e
        )
    })?;

    let resp = client
        .request(reqwest::Method::GET, &spec.path)
        .send()
        .await
        .map_err(|e| {
            format!(
                "Network error: {}. Hint: check connection/proxy; ensure HEROKU_API_KEY or ~/.netrc is set",
                e
            )
        })?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_else(|_| String::from("<no body>"));

    if !status.is_success() {
        return Err(format!("{}\n{}", status, text));
    }

    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Array(arr)) => Ok(arr),
        Ok(_) => Err("Response is not a JSON array".into()),
        Err(e) => Err(format!("Invalid JSON: {}", e)),
    }
}

fn apply_range_headers(builder: reqwest::RequestBuilder, body: &JsonMap<String, Value>) -> reqwest::RequestBuilder {
    // Raw Next-Range override takes precedence
    if let Some(next_raw) = body.get("next-range").and_then(|v| v.as_str()) {
        return builder.header("Range", next_raw);
    }

    // Compose range header from individual components
    if let Some(range_header) = http::build_range_header_from_body(body) {
        builder.header("Range", range_header)
    } else {
        builder
    }
}

fn handle_next_range_header(pagination: &mut Option<heroku_types::Pagination>, headers: &HeaderMap) {
    let next_range_header = HeaderName::from_static("next-range");
    if let Some(p) = pagination.as_mut()
        && let Some(value) = headers.get(next_range_header)
    {
        p.next_range = value.to_str().ok().map(|s| s.to_string());
    }
}
