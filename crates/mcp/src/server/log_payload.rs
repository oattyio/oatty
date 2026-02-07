//! Helpers for building MCP HTTP log payloads.
//!
//! These utilities keep log payload shaping and optional `response.text` JSON
//! extraction in one place so tool handlers can emit consistent entries.

use serde_json::{Map, Value};

const MAX_RESPONSE_TEXT_PARSE_BYTES: usize = 256 * 1024;
const MAX_PARSED_RESPONSE_LOG_BYTES: usize = 512 * 1024;

/// Builds the standard MCP HTTP log payload.
///
/// The payload includes `request` and/or `response` when present. Returns
/// `None` when both values are absent.
pub(crate) fn build_log_payload(request: Option<Value>, response: Option<Value>) -> Option<Value> {
    let mut payload = Map::new();
    if let Some(request_value) = request {
        payload.insert("request".to_string(), request_value);
    }
    if let Some(response_value) = response {
        payload.insert("response".to_string(), response_value);
    }
    if payload.is_empty() { None } else { Some(Value::Object(payload)) }
}

/// Builds the parsed `response.text` payload used for secondary log entries.
///
/// This extracts the first nested `text` field, parses it as JSON, and returns
/// a payload with `parsed_response_text` when the parsed value is an object or
/// array and the serialized payload size is below the configured guardrail.
pub(crate) fn build_parsed_response_payload(request: Option<&Value>, response: Option<&Value>) -> Option<Value> {
    let response_value = response?;
    let parsed_response_text = extract_json_value_from_response_text(response_value)?;
    let mut payload = Map::new();
    if let Some(request_value) = request {
        payload.insert("request".to_string(), request_value.clone());
    }
    payload.insert("parsed_response_text".to_string(), parsed_response_text);
    let payload_value = Value::Object(payload);
    if is_payload_within_size_limit(&payload_value) {
        Some(payload_value)
    } else {
        None
    }
}

fn is_payload_within_size_limit(payload: &Value) -> bool {
    serde_json::to_vec(payload)
        .map(|bytes| bytes.len() <= MAX_PARSED_RESPONSE_LOG_BYTES)
        .unwrap_or(false)
}

fn extract_json_value_from_response_text(response: &Value) -> Option<Value> {
    let text = find_text_field(response)?;
    if text.len() > MAX_RESPONSE_TEXT_PARSE_BYTES {
        return None;
    }
    let parsed = serde_json::from_str::<Value>(text).ok()?;
    if parsed.is_object() || parsed.is_array() {
        Some(parsed)
    } else {
        None
    }
}

fn find_text_field(value: &Value) -> Option<&str> {
    match value {
        Value::Object(map) => {
            if let Some(text_value) = map.get("text").and_then(Value::as_str) {
                return Some(text_value);
            }
            map.values().find_map(find_text_field)
        }
        Value::Array(items) => items.iter().find_map(find_text_field),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_nested_response_text_json_object() {
        let response = serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": "{\"ok\":true,\"items\":[1,2]}"
                }
            ]
        });
        let parsed = build_parsed_response_payload(None, Some(&response)).expect("parsed payload");
        assert_eq!(parsed["parsed_response_text"]["ok"], true);
        assert_eq!(parsed["parsed_response_text"]["items"][0], 1);
    }

    #[test]
    fn ignores_non_json_text() {
        let response = serde_json::json!({
            "text": "not json"
        });
        assert!(build_parsed_response_payload(None, Some(&response)).is_none());
    }

    #[test]
    fn ignores_json_scalar_text() {
        let response = serde_json::json!({
            "text": "\"hello\""
        });
        assert!(build_parsed_response_payload(None, Some(&response)).is_none());
    }
}
