//! HTTP execution helpers shared across TUI and Engine.
//!
//! This module centralizes remote execution of Oatty API requests based on
//! `CommandSpec`, handling headers and response parsing.
//! It also provides a convenient `fetch_json_array` helper for list endpoints.

use crate::{build_path, http, resolve_path, shell_lexing};
use anyhow::anyhow;
use indexmap::IndexSet;
use oatty_api::OattyClient;
use oatty_types::{CommandSpec, HttpCommandSpec};
use oatty_types::{EnvVar, ExecOutcome};
use reqwest::header::{self, HeaderMap};
use reqwest::{Client, Method, StatusCode};
use serde_json::{Map, Number, Value};
use std::collections::HashMap;
use std::str::FromStr;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

const RESPONSE_ARRAY_PRIORITY_KEYS: &[&str] = &[
    "items",
    "results",
    "data",
    "values",
    "entries",
    "list",
    "projects",
    "workflows",
    "commands",
    "catalogs",
    "plugins",
    "members",
];

/// Fetches a static json or text resource using GET
pub async fn fetch_static(url: &str) -> Result<(StatusCode, String), anyhow::Error> {
    let accept = header::HeaderValue::from_str("application/json,text/html").map_err(|e| anyhow!(e))?;
    let mut default_headers = HeaderMap::new();
    default_headers.insert(header::ACCEPT, accept);

    let client = Client::builder()
        .brotli(true)
        .connect_timeout(Duration::from_secs(30))
        .default_headers(default_headers)
        .build()
        .map_err(|e| anyhow!(e))?;

    let resp = client.get(url).send().await.map_err(|e| anyhow!("Network error: {}", e))?;

    let status = resp.status();
    let content = resp.text().await.unwrap_or_default();

    Ok((status, content))
}

/// Perform an asynchronous REST API call against the Oatty platform.
///
/// - Constructs the request from the `CommandSpec`.
/// - Uses the provided `base_url` override when available.
/// - Applies Range headers from the body when present.
/// - Sends the request and parses the response into [`ExecOutcome`].
/// - Returns a user-friendly `Err(String)` on HTTP/auth/network issues.
pub async fn exec_remote_from_shell_command(
    spec: &CommandSpec,
    base_url: String,
    headers: &IndexSet<EnvVar>,
    hydrated_shell_command: String,
    request_id: u64,
) -> Result<ExecOutcome, String> {
    // Parse and validate arguments
    let tokens = shell_lexing::lex_shell_like(&hydrated_shell_command);
    let (user_flags, user_args) = spec.parse_arguments(&tokens[2..]).map_err(|e| e.to_string())?;
    let body = build_request_body(spec, user_flags);
    // Prepare client and request
    let http = spec.http().ok_or_else(|| format!("Command '{}' is not HTTP-backed", spec.name))?;
    let path = resolve_path(&http.path, &user_args);

    match exec_remote_from_spec_inner(http, &base_url, headers, body, path).await {
        Ok((status, _, text)) => {
            // Handle common error status codes
            // by returning an ExecOutcome with an error message
            // and a null result JSON object
            if !status.is_success() {
                return Ok(ExecOutcome::Http {
                    status_code: status.as_u16(),
                    log_entry: format!("HTTP {}: {}", status.as_u16(), text),
                    payload: Value::Null,
                    request_id,
                });
            }
            let raw_log = format!("{}\n{}", status, text);
            let mut log = summarize_execution_outcome(&spec.canonical_id(), raw_log.as_str(), status);
            let result_json = match http::parse_response_json_strict(&text, Some(status)) {
                Ok(value) => normalize_command_payload(value, spec.http().and_then(|http_spec| http_spec.list_response_path.as_deref())),
                Err(error) => {
                    let error_message = error.to_string();
                    let sanitized_error = crate::redact_sensitive(&error_message);
                    log.push_str(&format!("\nJSON parse error: {sanitized_error}"));
                    Value::Null
                }
            };
            Ok(ExecOutcome::Http {
                status_code: status.as_u16(),
                log_entry: log,
                payload: result_json,
                request_id,
            })
        }
        Err(e) => Err(e),
    }
}

/// Normalize command payloads for list-oriented consumers.
///
/// Normalization prefers returning a collection payload:
/// 1. Use the explicit schema-derived list path when it resolves to an array.
/// 2. Otherwise, attempt deterministic wrapper-key and single-array extraction heuristics.
/// 3. If no list-like shape is found, preserve the original payload.
pub fn normalize_command_payload(payload: Value, list_response_path: Option<&str>) -> Value {
    if let Some(items) = extract_collection_items(&payload, list_response_path) {
        return Value::Array(items);
    }
    payload
}

/// Extract list-like collection items from payloads.
///
/// Extraction order:
/// 1. Use explicit schema-derived `list_response_path` if provided.
/// 2. Use top-level array payload directly.
/// 3. Apply deterministic wrapper-key heuristics.
/// 4. Fallback to a single array-valued field in wrapper objects.
pub fn extract_collection_items(payload: &Value, list_response_path: Option<&str>) -> Option<Vec<Value>> {
    if let Some(path) = list_response_path
        && let Some(items) = extract_array_at_path(payload, path)
    {
        return Some(items);
    }

    match payload {
        Value::Array(items) => Some(items.clone()),
        Value::Object(map) => {
            for key in RESPONSE_ARRAY_PRIORITY_KEYS {
                if let Some(Value::Array(items)) = map.get(*key) {
                    return Some(items.clone());
                }
            }

            let mut arrays = map.values().filter_map(|value| match value {
                Value::Array(items) => Some(items.clone()),
                _ => None,
            });
            let first = arrays.next()?;
            if arrays.next().is_none() {
                return Some(first);
            }
            None
        }
        _ => None,
    }
}

fn extract_array_at_path(payload: &Value, path: &str) -> Option<Vec<Value>> {
    if path == "." || path.is_empty() {
        return payload.as_array().cloned();
    }

    let mut current = payload;
    for segment in path.split('.') {
        if segment.is_empty() {
            continue;
        }
        current = current.get(segment)?;
    }

    current.as_array().cloned()
}

/// Executes an HTTP-backed provider command with an optional base URL override.
pub async fn exec_remote_for_provider(
    spec: &CommandSpec,
    base_url: &str,
    headers: &IndexSet<EnvVar>,
    body: Map<String, Value>,
    request_id: u64,
) -> Result<ExecOutcome, String> {
    let http = spec.http().ok_or_else(|| format!("Command '{}' is not HTTP-backed", spec.name))?;
    let path = build_path(http.path.as_str(), &body);

    match exec_remote_from_spec_inner(http, base_url, headers, body, path).await {
        Ok((status, _, text)) => {
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
            Ok(ExecOutcome::Http {
                status_code: status.as_u16(),
                log_entry: log,
                payload: result_json,
                request_id,
            })
        }
        Err(e) => Err(e),
    }
}

/// Execute a JSON-backed HTTP request and parse the response payload.
///
/// # Arguments
/// - `client`: Preconfigured HTTP client with base URL and headers.
/// - `method`: HTTP method to execute.
/// - `request_path`: Path for the request, already resolved with any path variables.
/// - `query_parameters`: Query parameters for GET/DELETE requests, or a body fallback for other methods.
/// - `body_override`: Optional body that overrides `query_parameters` for non-GET/DELETE requests.
///
/// # Returns
/// Returns the parsed JSON payload for a successful response, `Value::Null` for empty bodies,
/// or an error if the HTTP request or JSON parsing fails.
pub async fn execute_http_json_request(
    client: &OattyClient,
    method: Method,
    request_path: &str,
    query_parameters: Map<String, Value>,
    body_override: Option<Value>,
) -> anyhow::Result<Value> {
    let start = Instant::now();
    debug!(
        method = %method,
        path = %request_path,
        query_parameter_count = query_parameters.len(),
        has_body_override = body_override.is_some(),
        "http request started"
    );
    let mut request_builder = client.request(method.clone(), request_path);

    match method {
        Method::GET | Method::DELETE => {
            if !query_parameters.is_empty() {
                let query_pairs = build_query_pairs(query_parameters);
                request_builder = request_builder.query(&query_pairs);
            }
        }
        _ => {
            let request_body = build_request_body_override(body_override, query_parameters);
            let body_field_count = request_body.len();
            request_builder = request_builder.json(&Value::Object(request_body));
            debug!(
                method = %method,
                path = %request_path,
                body_field_count,
                "http request body prepared"
            );
        }
    }

    let response = request_builder.send().await.map_err(|error| anyhow::anyhow!(error))?;
    let status = response.status();
    if let Err(error) = response.error_for_status_ref() {
        warn!(
            method = %method,
            path = %request_path,
            status = %status,
            error = %error,
            duration_ms = start.elapsed().as_millis(),
            "http request failed"
        );
        return Err(anyhow::anyhow!(error));
    }
    let status = response.status();
    let body_text = response.text().await.map_err(|error| anyhow::anyhow!(error))?;

    if body_text.trim().is_empty() {
        debug!(
            method = %method,
            path = %request_path,
            status = %status,
            duration_ms = start.elapsed().as_millis(),
            "http request completed with empty response"
        );
        return Ok(Value::Null);
    }

    let parsed = http::parse_response_json_strict(&body_text, Some(status)).map_err(|error| {
        warn!(
            method = %method,
            path = %request_path,
            status = %status,
            body_len = body_text.len(),
            duration_ms = start.elapsed().as_millis(),
            error = %error,
            "http response JSON parse failed"
        );
        anyhow::anyhow!(error)
    })?;
    debug!(
        method = %method,
        path = %request_path,
        status = %status,
        duration_ms = start.elapsed().as_millis(),
        "http request completed"
    );
    Ok(parsed)
}

async fn exec_remote_from_spec_inner(
    http: &HttpCommandSpec,
    base_url: &str,
    headers: &IndexSet<EnvVar>,
    body: Map<String, Value>,
    path: String,
) -> Result<(StatusCode, HeaderMap, String), String> {
    let client = build_http_client(base_url, headers)?;

    let method = Method::from_bytes(http.method.as_bytes()).map_err(|e| e.to_string())?;
    let mut builder = client.request(method.clone(), &path);

    // Filter out special range-only fields from JSON body
    if !body.is_empty() {
        // For GET/DELETE, pass arguments as query parameters instead of a JSON body
        if method == Method::GET || method == Method::DELETE {
            let query = build_query_pairs(body);
            builder = builder.query(&query);
        } else {
            builder = builder.json(&Value::Object(body));
        }
    }

    let resp = builder
        .send()
        .await
        .map_err(|e| format!("Network error: {}. Hint: check connection/proxy and catalog configuration.", e))?;

    let status = resp.status();
    let headers = resp.headers().clone();
    let text = resp.text().await.unwrap_or_default();

    Ok((status, headers, text))
}

fn build_query_pairs(query_parameters: Map<String, Value>) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for (key, value) in query_parameters {
        match value {
            Value::Array(items) => {
                for item in items {
                    pairs.push((key.clone(), query_value_to_string(item)));
                }
            }
            other => pairs.push((key, query_value_to_string(other))),
        }
    }
    pairs
}

fn query_value_to_string(value: Value) -> String {
    match value {
        Value::String(text) => text,
        other => other.to_string(),
    }
}

fn build_request_body_override(body_override: Option<Value>, query_parameters: Map<String, Value>) -> Map<String, Value> {
    match body_override {
        Some(Value::Object(map)) => map,
        Some(other) => Map::from_iter([("value".into(), other)]),
        None => query_parameters,
    }
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

    for (flag_name, flag_value) in user_flags {
        if let Some(flag_spec) = spec.flags.iter().find(|f| f.name == flag_name) {
            if flag_spec.r#type == "boolean" {
                request_body.insert(flag_name, Value::Bool(true));
            } else if let Some(value) = flag_value
                && let Some(parsed_value) = parse_flag_value(flag_spec.r#type.as_str(), value.as_str())
            {
                request_body.insert(flag_name, parsed_value);
            }
        }
    }

    request_body
}

fn parse_flag_value(flag_type: &str, raw_value: &str) -> Option<Value> {
    match flag_type {
        "number" | "integer" => Number::from_str(raw_value).ok().map(Value::Number),
        "array" => serde_json::from_str::<Value>(raw_value).ok().filter(Value::is_array),
        "object" => serde_json::from_str::<Value>(raw_value).ok().filter(Value::is_object),
        _ => Some(Value::String(raw_value.to_string())),
    }
}

fn summarize_execution_outcome(canonical_id: &str, raw_log: &str, status_code: StatusCode) -> String {
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
/// initializes a Oatty API client, performs a GET request, and processes the response
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
/// - Returns an error if catalog authentication or header configuration is invalid.
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
/// - This function uses the `OattyClient` for API requests, and the `serde_json` crate
///   for parsing JSON responses.
/// - Ensure catalog authorization headers are configured when required.
///
/// # Notes
/// - The function unwraps the response body text (`text().await`) if reading the body fails,
///   defaulting to a placeholder value `<no body>`.
/// - Errors are formatted with helpful hints where applicable, such as checking connection
///   settings or ensuring API credentials are configured.
///
/// [`CommandSpec`]: Path to your CommandSpec type definition.
pub async fn fetch_json_array(spec: &CommandSpec, base_url: &str, headers: &IndexSet<EnvVar>) -> Result<Vec<Value>, String> {
    let http = spec.http().ok_or_else(|| format!("Command '{}' is not HTTP-backed", spec.name))?;

    let client = build_http_client(base_url, headers)?;

    let method = Method::from_bytes(http.method.as_bytes()).map_err(|e| e.to_string())?;
    if method != Method::GET {
        return Err("GET method required for list endpoints".into());
    }
    let resp = client
        .request(method, &http.path)
        .send()
        .await
        .map_err(|e| format!("Network error: {}. Hint: check connection/proxy and catalog configuration.", e))?;

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

fn build_http_client(base_url: &str, headers: &IndexSet<EnvVar>) -> Result<OattyClient, String> {
    if base_url.trim().is_empty() {
        return Err("Missing base URL for HTTP command".to_string());
    }
    OattyClient::new(base_url, headers).map_err(|error| format!("Could not build the HTTP client: {}", error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use oatty_types::CommandFlag;
    use serde_json::json;
    use std::collections::HashMap;

    fn build_test_spec(flags: Vec<CommandFlag>) -> CommandSpec {
        CommandSpec::new_http(
            "apps".to_string(),
            "list".to_string(),
            "List apps".to_string(),
            Vec::new(),
            flags,
            HttpCommandSpec::new("GET", "/apps", None, None),
            1,
        )
    }

    fn flag(name: &str, flag_type: &str) -> CommandFlag {
        CommandFlag {
            name: name.to_string(),
            short_name: None,
            required: false,
            r#type: flag_type.to_string(),
            enum_values: Vec::new(),
            default_value: None,
            description: None,
            provider: None,
        }
    }

    #[test]
    fn build_request_body_converts_supported_flag_types() {
        let spec = build_test_spec(vec![flag("async", "boolean"), flag("count", "number"), flag("label", "string")]);
        let mut user_flags = HashMap::new();
        user_flags.insert("async".to_string(), None);
        user_flags.insert("count".to_string(), Some("42".to_string()));
        user_flags.insert("label".to_string(), Some("europa".to_string()));
        user_flags.insert("ignored".to_string(), Some("value".to_string()));

        let body = build_request_body(&spec, user_flags);

        assert_eq!(body.get("async"), Some(&Value::Bool(true)));
        assert_eq!(body.get("count"), Some(&json!(42)));
        assert_eq!(body.get("label"), Some(&json!("europa")));
        assert!(body.get("ignored").is_none(), "unknown flags should be dropped");
    }

    #[test]
    fn build_request_body_skips_invalid_numbers() {
        let spec = build_test_spec(vec![flag("count", "number")]);
        let mut user_flags = HashMap::new();
        user_flags.insert("count".to_string(), Some("not-a-number".to_string()));

        let body = build_request_body(&spec, user_flags);

        assert!(body.is_empty(), "failed parses must not insert a value");
    }

    #[test]
    fn build_request_body_parses_array_and_object_flags() {
        let spec = build_test_spec(vec![flag("body", "array"), flag("project", "object"), flag("count", "integer")]);
        let mut user_flags = HashMap::new();
        user_flags.insert(
            "body".to_string(),
            Some(r#"[{"key":"DATABASE_URL","value":"postgres://demo"}]"#.to_string()),
        );
        user_flags.insert("project".to_string(), Some(r#"{"name":"starter-node"}"#.to_string()));
        user_flags.insert("count".to_string(), Some("2".to_string()));

        let body = build_request_body(&spec, user_flags);

        assert_eq!(body.get("body"), Some(&json!([{"key":"DATABASE_URL","value":"postgres://demo"}])));
        assert_eq!(body.get("project"), Some(&json!({"name":"starter-node"})));
        assert_eq!(body.get("count"), Some(&json!(2)));
    }

    #[test]
    fn build_request_body_skips_invalid_structured_json() {
        let spec = build_test_spec(vec![flag("body", "array"), flag("project", "object")]);
        let mut user_flags = HashMap::new();
        user_flags.insert("body".to_string(), Some("not-json".to_string()));
        user_flags.insert("project".to_string(), Some("[1,2,3]".to_string()));

        let body = build_request_body(&spec, user_flags);

        assert!(body.get("body").is_none());
        assert!(body.get("project").is_none());
    }

    #[test]
    fn summarize_execution_outcome_reports_status() {
        let success = summarize_execution_outcome("apps list", "HTTP 200\n{}", StatusCode::OK);
        assert_eq!(success, "apps list • success");

        let failure = summarize_execution_outcome("apps list", "HTTP 500\n{}", StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(failure, "apps list • failed");
    }

    #[test]
    fn summarize_execution_outcome_includes_error_summary() {
        let long_error = "Error: something bad happened and kept talking about the detail that should be truncated \
                          because the message is intentionally verbose to exceed the truncation threshold by a wide margin.";
        let summary = summarize_execution_outcome("apps list", long_error, StatusCode::BAD_REQUEST);
        assert!(
            summary.starts_with("apps list • failed:"),
            "error summaries should be prefixed with command id"
        );
        assert!(summary.ends_with("..."), "long messages should be truncated with ellipsis");
    }

    #[test]
    fn truncate_for_summary_trims_and_truncates() {
        let short = truncate_for_summary(" short message ", 20);
        assert_eq!(short, "short message");

        let long = truncate_for_summary("abcdefghij", 5);
        assert_eq!(long, "ab...");
    }

    #[test]
    fn build_query_pairs_repeats_array_values() {
        let query = Map::from_iter([
            ("target".to_string(), json!(["production", "preview"])),
            ("decrypt".to_string(), json!(true)),
        ]);

        let pairs = build_query_pairs(query);
        let target_values = pairs
            .iter()
            .filter(|(key, _)| key == "target")
            .map(|(_, value)| value.as_str())
            .collect::<Vec<&str>>();
        let decrypt_value = pairs
            .iter()
            .find(|(key, _)| key == "decrypt")
            .map(|(_, value)| value.as_str())
            .unwrap_or_default();

        assert_eq!(target_values, vec!["production", "preview"]);
        assert_eq!(decrypt_value, "true");
    }

    #[test]
    fn extract_collection_items_uses_schema_derived_path() {
        let payload = json!({
            "meta": { "count": 2 },
            "projects": [{ "id": "project-a" }, { "id": "project-b" }]
        });

        let items = extract_collection_items(&payload, Some("projects")).expect("projects path should extract");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["id"], json!("project-a"));
    }

    #[test]
    fn normalize_command_payload_uses_wrapper_array_when_path_missing() {
        let payload = json!({
            "meta": { "count": 1 },
            "projects": [{ "id": "project-a" }]
        });

        let normalized = normalize_command_payload(payload.clone(), Some("data.items"));
        assert_eq!(normalized, json!([{ "id": "project-a" }]));
    }

    #[test]
    fn normalize_command_payload_preserves_original_when_no_list_shape_exists() {
        let payload = json!({
            "meta": { "count": 1 },
            "project": { "id": "project-a" }
        });

        let normalized = normalize_command_payload(payload.clone(), Some("data.items"));
        assert_eq!(normalized, payload);
    }
}
