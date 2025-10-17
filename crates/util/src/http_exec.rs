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
use reqwest::{Method, StatusCode};
use serde_json::{Map as JsonMap, Value};

/// Perform an asynchronous REST API call against the Heroku platform.
///
/// - Constructs the request from the `CommandSpec` and `path`.
/// - Applies Range headers from the body when present.
/// - Sends the request and parses the response into [`ExecOutcome`].
/// - Returns a user-friendly `Err(String)` on HTTP/auth/network issues.
pub async fn exec_remote(spec: &CommandSpec, body: JsonMap<String, Value>, request_id: u64) -> Result<ExecOutcome, String> {
    let http = spec.http().ok_or_else(|| format!("Command '{}' is not HTTP-backed", spec.name))?;

    let client = HerokuClient::new_from_service_id(http.service_id)
        .map_err(|e| format!("Auth setup failed: {}. Hint: set HEROKU_API_KEY or configure ~/.netrc", e))?;

    let method = Method::from_bytes(http.method.as_bytes()).map_err(|e| e.to_string())?;
    let mut builder = client.request(method.clone(), &http.path);

    // Build and apply Range header
    builder = apply_range_headers(builder, &body);

    // Filter out special range-only fields from JSON body
    let filtered_body = http::strip_range_body_fields(body);
    if !filtered_body.is_empty() {
        // For GET/DELETE, pass arguments as query parameters instead of a JSON body
        if method == Method::GET || method == Method::DELETE {
            let query: Vec<(String, String)> = filtered_body
                .into_iter()
                .map(|(k, v)| {
                    let s = match v {
                        Value::String(s) => s,
                        other => other.to_string(),
                    };
                    (k, s)
                })
                .collect();
            builder = builder.query(&query);
        } else {
            builder = builder.json(&Value::Object(filtered_body));
        }
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
    if !status.is_success() {
        return Err(format!("HTTP {}: {}", status.as_u16(), text));
    }
    let raw_log = format!("{}\n{}", status, text);
    let log = summarize_execution_outcome(spec, raw_log.as_str(), status);
    let result_json = http::parse_response_json(&text);
    Ok(ExecOutcome::Http(
        status.as_u16(),
        log,
        result_json.unwrap_or(Value::Null),
        pagination,
        request_id,
    ))
}

fn summarize_execution_outcome(command_spec: &CommandSpec, raw_log: &str, status_code: StatusCode) -> String {
    let trimmed_log = raw_log.trim();
    let canonical_name = command_spec.canonical_id();

    if let Some(error_message) = trimmed_log.strip_prefix("Error:") {
        let redacted = crate::redact_sensitive(error_message.trim());
        let truncated = truncate_for_summary(&redacted, 160);
        return format!("{} • failed: {}", canonical_name, truncated);
    }

    let success = if status_code.is_success() { "success" } else { "failed" };
    format!("{} • {}", canonical_name, success)
}

fn truncate_for_summary(text: &str, max_len: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_len {
        return trimmed.to_string();
    }

    // Reserve space for the trailing ellipsis ("...").
    let target_len = max_len.saturating_sub(3);
    let mut truncated = String::new();
    for (idx, ch) in trimmed.chars().enumerate() {
        if idx >= target_len {
            break;
        }
        truncated.push(ch);
    }
    let trimmed_truncated = truncated.trim_end();
    format!("{}...", trimmed_truncated)
}

/// Fetches a JSON array from a remote HTTP endpoint.
///
/// # Description
/// This asynchronous function retrieves a JSON array from a remote endpoint defined
/// in the [`CommandSpec`] parameter. It verifies the HTTP service configuration,
/// initializes a Heroku API client, performs a GET request, and processes the response
/// to validate and extract the desired JSON array.
///
/// # Parameters
/// - `spec`: A reference to a [`CommandSpec`] object that contains the HTTP
///   service configuration, including the `name` and `path` details.
///
/// # Returns
/// - `Ok(Vec<Value>)`: If the HTTP request is successful, the response is a valid JSON array.
/// - `Err(String)`: If an error occurs at any stage (e.g., missing HTTP configuration,
///   network issues, invalid JSON), a descriptive error message is returned.
///
/// # Errors
/// - Returns an error if the [`CommandSpec`] is not associated with an HTTP-backed service.
/// - Returns an error if authentication setup for the Heroku API client fails (e.g., missing
///   `HEROKU_API_KEY` or improperly configured `~/.netrc` file).
/// - Returns an error if the HTTP request fails (e.g., network error, invalid proxy settings).
/// - Returns an error if the response status code indicates failure (non-2xx status code).
/// - Returns an error if the response body is not a valid JSON array or cannot be deserialized.
///
/// # Example
/// ```rust ignore
/// use serde_json::Value;
///
/// #[tokio::main]
/// async fn main() {
///     let spec = CommandSpec::new(); // Example initialization
///     match fetch_json_array(&spec).await {
///         Ok(json_array) => println!("Received data: {:?}", json_array),
///         Err(err) => eprintln!("Failed to fetch JSON array: {}", err),
///     }
/// }
/// ```
///
/// # Dependencies
/// - This function uses the `HerokuClient` for API requests, and the `serde_json` crate
///   for parsing JSON responses.
/// - Ensure the environment variable `HEROKU_API_KEY` or the `~/.netrc` configuration is
///   set up properly for authentication.
///
/// # Notes
/// - The function unwraps the response body text (`text().await`) if reading the body fails,
///   defaulting to a placeholder value `<no body>`.
/// - Errors are formatted with helpful hints where applicable, such as checking connection
///   settings or ensuring API credentials are configured.
///
/// [`CommandSpec`]: Path to your CommandSpec type definition.
pub async fn fetch_json_array(spec: &CommandSpec) -> Result<Vec<Value>, String> {
    let http = spec.http().ok_or_else(|| format!("Command '{}' is not HTTP-backed", spec.name))?;

    let client = HerokuClient::new_from_service_id(http.service_id)
        .map_err(|e| format!("Auth setup failed: {}. Hint: set HEROKU_API_KEY or configure ~/.netrc", e))?;

    let method = Method::from_bytes(http.method.as_bytes()).map_err(|e| e.to_string())?;
    if method != Method::GET {
        return Err("GET method required for list endpoints".into());
    }
    let resp = client.request(method, &http.path).send().await.map_err(|e| {
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
