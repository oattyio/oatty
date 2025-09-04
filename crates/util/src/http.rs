//! # HTTP Utilities
//!
//! This module provides utility functions for working with HTTP requests and responses,
//! including header parsing, request body manipulation, and response handling.

use heroku_types::Pagination;
use serde_json::{Map, Value};

/// Parse Content-Range header value into a Pagination struct.
///
/// This function parses HTTP Content-Range headers that follow the Heroku API format.
/// The header specifies pagination information including the field name, range values,
/// maximum items per page, and sort order.
///
/// # Arguments
/// * `value` - The Content-Range header value string
///
/// # Returns
/// `Some(Pagination)` if parsing succeeds, `None` if the format is invalid
///
/// # Example
/// ```rust
/// use heroku_util::http::parse_content_range_value;
///
/// let header_value = "name app7a..app9x; max=200; order=desc;";
/// let pagination = parse_content_range_value(header_value).unwrap();
///
/// assert_eq!(pagination.field, "name");
/// assert_eq!(pagination.range_start, "app7a");
/// assert_eq!(pagination.range_end, "app9x");
/// assert_eq!(pagination.max, 200);
/// assert_eq!(pagination.order, Some("desc".to_string()));
/// ```
pub fn parse_content_range_value(value: &str) -> Option<Pagination> {
    let parts: Vec<&str> = value.split(';').map(str::trim).filter(|s| !s.is_empty()).collect();

    let range_part = parts.first()?;
    let (field, range) = range_part.split_once(' ')?;
    let field = field.to_lowercase();

    let mut iter = range.split("..");
    let range_start = iter.next().filter(|s| !s.is_empty())?.to_string();
    let range_end = iter.next().filter(|s| !s.is_empty())?.to_string();

    let mut max: Option<usize> = None;
    let mut order: Option<String> = None;

    for key_value_pair in parts.iter().skip(1) {
        if let Some(value) = key_value_pair.strip_prefix("max=") {
            if let Ok(number) = value.trim_end_matches(';').parse::<usize>() {
                max = Some(number);
            }
        } else if let Some(value) = key_value_pair.strip_prefix("order=") {
            order = Some(value.trim_end_matches(';').to_lowercase());
        }
    }

    Some(Pagination {
        range_start,
        range_end,
        field,
        max: max.unwrap_or(200),
        order,
        next_range: None,
    })
}

/// Remove request-body fields used only for Range header composition.
///
/// This function cleans up request body data by removing fields that are
/// specifically used for constructing Range headers. This prevents these
/// fields from being sent as part of the actual request payload.
///
/// # Arguments
/// * `body` - The request body as a JSON map
///
/// # Returns
/// A new JSON map with range-related fields removed
///
/// # Example
/// ```rust
/// use heroku_util::http::strip_range_body_fields;
/// use serde_json::Map;
///
/// let mut body = Map::new();
/// body.insert("name".to_string(), "myapp".into());
/// body.insert("range-field".to_string(), "name".into());
/// body.insert("range-start".to_string(), "a".into());
///
/// let cleaned = strip_range_body_fields(body);
/// assert!(!cleaned.contains_key("range-field"));
/// assert!(!cleaned.contains_key("range-start"));
/// assert!(cleaned.contains_key("name"));
/// ```
pub fn strip_range_body_fields(mut body: Map<String, Value>) -> Map<String, Value> {
    let range_fields = ["range-field", "range-start", "range-end", "order", "max", "next-range"];

    for field_name in range_fields {
        let _ = body.remove(field_name);
    }

    body
}

/// Build a Range header value from commonly used body fields.
///
/// This function constructs an HTTP Range header value from fields in the request body.
/// It's useful for converting user-friendly body parameters into the proper header format
/// expected by the Heroku API.
///
/// # Arguments
/// * `body` - The request body containing range parameters
///
/// # Returns
/// `Some(header_value)` if sufficient information is present, `None` otherwise
///
/// # Example
/// ```rust
/// use heroku_util::http::build_range_header_from_body;
/// use serde_json::{json, Map};
///
/// let mut body = Map::new();
/// body.insert("range-field".to_string(), "name".into());
/// body.insert("range-start".to_string(), "a".into());
/// body.insert("range-end".to_string(), "z".into());
/// body.insert("max".to_string(), "100".into());
/// body.insert("order".to_string(), "desc".into());
///
/// let header = build_range_header_from_body(&body).unwrap();
/// assert_eq!(header, "name a..z; max=100, order=desc;");
/// ```
pub fn build_range_header_from_body(body: &Map<String, Value>) -> Option<String> {
    let field = body
        .get("range-field")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|string| !string.is_empty())?;

    let start = body
        .get("range-start")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim();

    let end = body
        .get("range-end")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim();

    let order = body
        .get("order")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|string| !string.is_empty());

    let max = body
        .get("max")
        .and_then(|value| value.as_str())
        .and_then(|string| string.parse::<usize>().ok());

    let range_segment = format!("{}..{}", start, end);
    let mut range_header = format!("{} {}", field, range_segment);

    if let Some(maximum) = max {
        range_header.push_str(&format!("; max={}", maximum));
    }

    if let Some(sort_order) = order {
        range_header.push_str(&format!(", order={};", sort_order));
    }

    Some(range_header)
}

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
/// use heroku_util::http::status_error_message;
///
/// let error_401 = status_error_message(401).unwrap();
/// assert!(error_401.contains("HEROKU_API_KEY"));
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
        401 => Some(
            "Unauthorized (401). Hint: set HEROKU_API_KEY=... or configure ~/.netrc with machine api.heroku.com".into(),
        ),
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
/// use heroku_util::http::parse_response_json;
///
/// let (json, is_table_suitable) = parse_response_json(r#"{"name": "myapp"}"#);
/// assert!(json.is_some());
/// assert!(is_table_suitable);
///
/// let (json, is_table_suitable) = parse_response_json("invalid json");
/// assert!(json.is_none());
/// assert!(!is_table_suitable);
/// ```
pub fn parse_response_json(text: &str) -> (Option<Value>, bool) {
    match serde_json::from_str::<Value>(text) {
        Ok(json) => (Some(json), true),
        Err(_) => (None, false),
    }
}
