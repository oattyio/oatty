//! # HTTP Utilities
//!
//! This module provides utility functions for working with HTTP requests and responses,
//! including header parsing, request body manipulation, and response handling.

use reqwest::StatusCode;
use serde_json::Value;
use thiserror::Error;

/// Return a user-friendly error message for common HTTP status codes.
///
/// This function provides helpful hints for common HTTP error responses,
/// guiding users toward solutions for authentication and authorization issues.
///
/// # Arguments
/// * `status_code` - The HTTP status code
///
/// # Returns
/// `Some(error_message)` for known status codes, `None` for others
///
/// # Example
/// ```rust
/// use oatty_util::http::status_error_message;
///
/// let error_401 = status_error_message(401).unwrap();
/// assert!(error_401.contains("OATTY_API_TOKEN"));
/// assert!(error_401.contains("Unauthorized"));
///
/// let error_403 = status_error_message(403).unwrap();
/// assert!(error_403.contains("Forbidden"));
/// assert!(error_403.contains("team/app access"));
///
/// assert!(status_error_message(404).is_none());
/// ```
pub fn status_error_message(status_code: u16) -> Option<String> {
    match status_code {
        401 => Some("Unauthorized (401). Hint: set OATTY_API_TOKEN=...".into()),
        403 => Some("Forbidden (403). Hint: check team/app access, permissions, and role membership".into()),
        _ => None,
    }
}

/// Parse response text as JSON, returning the parsed value and a flag indicating table suitability.
///
/// This function attempts to parse HTTP response text as JSON and provides
/// a boolean flag indicating whether the response is suitable for tabular display.
///
/// # Arguments
/// * `text` - The response text to parse
///
/// # Returns
/// A tuple of `(Option<Value>, bool)` where the first element is the parsed JSON
/// (or `None` if parsing failed) and the second element indicates table suitability
///
/// # Example
/// ```rust
/// use oatty_util::http::parse_response_json;
///
/// let json = parse_response_json(r#"{"name": "myapp"}"#);
/// assert!(json.is_some());
///
/// let json = parse_response_json("invalid json");
/// assert!(json.is_none());
/// ```
pub fn parse_response_json(text: &str) -> Option<Value> {
    serde_json::from_str::<Value>(text).ok()
}

/// Parse HTTP response text into JSON, providing detailed errors on failure.
///
/// This helper performs strict JSON deserialization and decorates any parsing
/// error with context about the originating HTTP status code plus a truncated
/// preview of the response body. Use this when a JSON response is required and
/// the caller should surface failures instead of silently degrading to `null`.
///
/// # Arguments
/// * `text` - The raw HTTP response body text
/// * `status` - Optional HTTP status code for error context
///
/// # Errors
/// Returns a [`JsonParseError`] describing the parse failure. The message
/// includes the original serde error and up to 200 characters of the response
/// body (with whitespace collapsed) to aid debugging truncated or malformed
/// payloads.
pub fn parse_response_json_strict(text: &str, status: Option<StatusCode>) -> Result<Value, JsonParseError> {
    serde_json::from_str::<Value>(text).map_err(|error| {
        let status_note = status
            .map(|code| format!("status {code}"))
            .unwrap_or_else(|| "unknown status".to_string());
        let preview = truncate_response_preview(text, 200);

        JsonParseError::new(status_note, error, preview)
    })
}

fn truncate_response_preview(text: &str, limit: usize) -> String {
    if text.trim().is_empty() {
        return "<empty>".to_string();
    }

    let mut preview = String::new();
    for ch in text.chars() {
        if preview.len() >= limit {
            preview.push_str("...");
            break;
        }
        match ch {
            '\n' | '\r' | '\t' => {
                if !preview.ends_with(' ') {
                    preview.push(' ');
                }
            }
            _ => preview.push(ch),
        }
    }

    preview.trim().to_string()
}

/// Error returned when strict JSON parsing of an HTTP response fails.
#[derive(Debug, Error)]
#[error("failed to parse JSON response ({status_note}): {source}. body preview: {body_preview}")]
pub struct JsonParseError {
    status_note: String,
    #[source]
    source: serde_json::Error,
    body_preview: String,
}

impl JsonParseError {
    /// Create a new [`JsonParseError`] with contextual information.
    pub fn new(status_note: String, source: serde_json::Error, body_preview: String) -> Self {
        Self {
            status_note,
            source,
            body_preview,
        }
    }

    /// Access the truncated response preview captured during parsing.
    pub fn body_preview(&self) -> &str {
        &self.body_preview
    }

    /// Access the underlying serde parse error for logging or inspection.
    pub fn source_error(&self) -> &serde_json::Error {
        &self.source
    }
}
