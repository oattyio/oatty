//! HTTP execution helpers shared across TUI and Engine.
//!
//! This module centralizes remote execution of Heroku API requests based on
//! `CommandSpec`, handling headers, pagination, and response parsing.
//! It also provides a convenient `fetch_json_array` helper for list endpoints.

use crate::{build_path, http, resolve_path, shell_lexing};
use heroku_api::HerokuClient;
use heroku_types::ExecOutcome;
use heroku_types::{CommandSpec, HttpCommandSpec};
use reqwest::header::{CONTENT_RANGE, HeaderMap};
use reqwest::{Method, StatusCode};
use serde_json::{Map as JsonMap, Map, Number, Value};
use std::collections::HashMap;
use std::str::FromStr;

/// Perform an asynchronous REST API call against the Heroku platform.
///
/// - Constructs the request from the `CommandSpec`.
/// - Applies Range headers from the body when present.
/// - Sends the request and parses the response into [`ExecOutcome`].
/// - Returns a user-friendly `Err(String)` on HTTP/auth/network issues.
pub async fn exec_remote_from_shell_command(
    spec: &CommandSpec,
    hydrated_shell_command: String,
    range_header_override: Option<String>,
    request_id: u64,
) -> Result<ExecOutcome, String> {
    // Parse and validate arguments
    let tokens = shell_lexing::lex_shell_like(&hydrated_shell_command);
    let (user_flags, user_args) = spec.parse_arguments(&tokens[2..]).map_err(|e| e.to_string())?;
    let mut body = build_request_body(spec, user_flags);
    if let Some(range_header_override) = range_header_override {
        body.insert("next-range".to_string(), Value::String(range_header_override));
    }
    // Prepare client and request
    let http = spec.http().ok_or_else(|| format!("Command '{}' is not HTTP-backed", spec.name))?;
    let path = resolve_path(&http.path, &user_args);

    match exec_remote_from_spec_inner(http, body, path).await {
        Ok((status, headers, text, maybe_range_header)) => {
            let mut pagination = headers
                .get(CONTENT_RANGE)
                .and_then(|v| v.to_str().ok())
                .and_then(http::parse_content_range_value);

            // Handle Next-Range header for 206 responses
            if status.as_u16() == 206
                && let Some(pagination_mut) = pagination.as_mut()
            {
                pagination_mut.hydrated_shell_command = Some(hydrated_shell_command);
                if let Some(next_range_header) = headers.get("next-range") {
                    pagination_mut.next_range = Some(next_range_header.to_str().unwrap().to_string());
                }
                if let Some(range_header) = maybe_range_header {
                    pagination_mut.this_range = Some(range_header);
                }
            }

            // Handle common error status codes
            // by returning an ExecOutcome with an error message
            // and a null result JSON object
            if !status.is_success() {
                return Ok(ExecOutcome::Http(
                    status.as_u16(),
                    format!("HTTP {}: {}", status.as_u16(), text),
                    Value::Null,
                    pagination,
                    request_id,
                ));
            }
            let raw_log = format!("{}\n{}", status, text);
            let mut log = summarize_execution_outcome(&spec.canonical_id(), raw_log.as_str(), status);
            let result_json = match http::parse_response_json_strict(&text, Some(status)) {
                Ok(value) => value,
                Err(error) => {
                    let error_message = error.to_string();
                    let sanitized_error = crate::redact_sensitive(&error_message);
                    log.push_str(&format!("\nJSON parse error: {sanitized_error}"));
                    Value::Null
                }
            };
            Ok(ExecOutcome::Http(status.as_u16(), log, result_json, pagination, request_id))
        }
        Err(e) => Err(e),
    }
}

pub async fn exec_remote_for_provider(spec: &CommandSpec, body: Map<String, Value>, request_id: u64) -> Result<ExecOutcome, String> {
    let http = spec.http().ok_or_else(|| format!("Command '{}' is not HTTP-backed", spec.name))?;
    let path = build_path(http.path.as_str(), &body);

    match exec_remote_from_spec_inner(http, body, path).await {
        Ok((status, _, text, _)) => {
            let raw_log = format!("{}\n{}", status, text);
            let mut log = summarize_execution_outcome(&spec.canonical_id(), raw_log.as_str(), status);
            let result_json = match http::parse_response_json_strict(&text, Some(status)) {
                Ok(value) => value,
                Err(error) => {
                    let error_message = error.to_string();
                    let sanitized_error = crate::redact_sensitive(&error_message);
                    log.push_str(&format!("\nJSON parse error: {sanitized_error}"));
                    Value::Null
                }
            };
            Ok(ExecOutcome::Http(status.as_u16(), log, result_json, None, request_id))
        }
        Err(e) => Err(e),
    }
}

async fn exec_remote_from_spec_inner(
    http: &HttpCommandSpec,
    body: Map<String, Value>,
    path: String,
) -> Result<(StatusCode, HeaderMap, String, Option<String>), String> {
    let client = HerokuClient::new_from_service_id(http.service_id)
        .map_err(|e| format!("Auth setup failed: {}. Hint: set HEROKU_API_KEY or configure ~/.netrc", e))?;

    let method = Method::from_bytes(http.method.as_bytes()).map_err(|e| e.to_string())?;
    let mut builder = client.request(method.clone(), &path);
    // Build and apply Range header
    let maybe_range_header = get_range_header_value(&body);
    if let Some(range_header) = maybe_range_header.as_ref() {
        builder = builder.header("Range", range_header);
    }

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
    let text = resp.text().await.unwrap_or_default();

    Ok((status, headers, text, maybe_range_header))
}

/// Builds a JSON request body from user-provided flags.
///
/// This function converts the parsed flags into a JSON object that can be sent
/// as the request body for the HTTP command execution.
///
/// # Arguments
///
/// * `user_flags` - The flags provided by the user
/// * `command_spec` - The command specification for type information
///
/// # Returns
///
/// Returns a JSON Map containing the flag values with appropriate types.
///
/// # Type Conversion
///
/// - Boolean flags are converted to `true` if present
/// - String flags are converted to their string values
/// - Flags not in the specification are ignored
pub fn build_request_body(spec: &CommandSpec, user_flags: HashMap<String, Option<String>>) -> Map<String, Value> {
    let mut request_body = Map::new();

    for (flag_name, flag_value) in user_flags.into_iter() {
        if let Some(flag_spec) = spec.flags.iter().find(|f| f.name == flag_name) {
            if flag_spec.r#type == "boolean" {
                request_body.insert(flag_name, Value::Bool(true));
            } else if let Some(value) = flag_value {
                match flag_spec.r#type.as_str() {
                    "number" => {
                        if let Ok(number) = Number::from_str(value.as_str()) {
                            request_body.insert(flag_name, Value::Number(number));
                        }
                    }
                    _ => {
                        request_body.insert(flag_name, Value::String(value));
                    }
                };
            }
        }
    }

    request_body
}

fn summarize_execution_outcome(canonical_id: &String, raw_log: &str, status_code: StatusCode) -> String {
    let trimmed_log = raw_log.trim();

    if let Some(error_message) = trimmed_log.strip_prefix("Error:") {
        let redacted = crate::redact_sensitive(error_message.trim());
        let truncated = truncate_for_summary(&redacted, 160);
        return format!("{} • failed: {}", canonical_id, truncated);
    }

    let success = if status_code.is_success() { "success" } else { "failed" };
    format!("{} • {}", canonical_id, success)
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

    match http::parse_response_json_strict(&text, Some(status)) {
        Ok(Value::Array(array)) => Ok(array),
        Ok(_) => Err("Response is not a JSON array".into()),
        Err(error) => {
            let error_message = error.to_string();
            let sanitized_error = crate::redact_sensitive(&error_message);
            Err(format!("Invalid JSON: {}", sanitized_error))
        }
    }
}

fn get_range_header_value(body: &JsonMap<String, Value>) -> Option<String> {
    // Raw Next-Range override takes precedence
    if let Some(next_raw) = body.get("next-range").and_then(|v| v.as_str()).map(String::from) {
        return Some(next_raw);
    }

    // Compose range header from individual components
    if let Some(range_header) = http::build_range_header_from_body(body) {
        return Some(range_header);
    }
    None
}
