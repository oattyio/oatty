use serde_json::Value;
use std::collections::HashMap;

/// Resolves a path template by replacing placeholders with actual values.
/// The path template follows the same placeholder format used by OpenAPI
/// (`/resource/{id}`).
///
/// This function takes a path template containing placeholders in the format
/// `{key}` and replaces them with corresponding values from the provided
/// HashMap.
///
/// # Arguments
/// - `template`: A string containing path placeholders in the format `{key}`
/// - `pos`: A HashMap mapping placeholder keys to their replacement values
///
/// # Returns
/// Returns a `String` with all placeholders replaced by their corresponding
/// values. If a placeholder key is not found in the HashMap, it remains
/// unchanged in the output.
///
/// # Examples
/// ```ignore
/// use std::collections::HashMap;
///
/// let template = "/apps/{app}/dynos/{dyno}";
/// let mut pos = HashMap::new();
/// pos.insert("app".to_string(), "my-app".to_string());
/// pos.insert("dyno".to_string(), "web.1".to_string());
///
/// let result = resolve_path(template, &pos);
/// assert_eq!(result, "/apps/my-app/dynos/web.1");
///
/// // Missing placeholder remains unchanged
/// let template = "/apps/{app}/config/{missing}";
/// let mut pos = HashMap::new();
/// pos.insert("app".to_string(), "my-app".to_string());
///
/// let result = resolve_path(template, &pos);
/// assert_eq!(result, "/apps/my-app/config/{missing}");
/// ```
pub fn resolve_path(template: &str, pos: &HashMap<String, String>) -> String {
    let mut out = template.to_string();
    for (k, v) in pos {
        let needle = format!("{{{}}}", k);
        out = out.replace(&needle, v);
    }
    out
}

pub fn build_path(template: &str, variables: &serde_json::Map<String, Value>) -> String {
    let mut path = template.to_string();
    for (k, v) in variables.iter() {
        let val = match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let enc = encode_path_placeholder_value(val.as_str());
        path = path.replace(&format!("{{{}}}", k), &enc);
    }
    path
}

/// Percent-encodes a path placeholder value while preserving RFC3986 unreserved bytes.
///
/// Unreserved bytes (`A-Z`, `a-z`, `0-9`, `-`, `.`, `_`, `~`) are emitted as-is.
/// All other bytes are percent-encoded using uppercase hex.
fn encode_path_placeholder_value(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if is_unreserved_path_byte(byte) {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push(to_upper_hex((byte >> 4) & 0x0f));
            encoded.push(to_upper_hex(byte & 0x0f));
        }
    }
    encoded
}

fn is_unreserved_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

fn to_upper_hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'A' + (nibble - 10)) as char,
        _ => unreachable!("nibble must be in [0, 15]"),
    }
}

#[cfg(test)]
mod tests {
    use super::build_path;
    use serde_json::{Map, Value};

    #[test]
    fn build_path_preserves_unreserved_identifier_bytes() {
        let mut variables = Map::new();
        variables.insert("service_id".to_string(), Value::String("srv-d5f6a7b8".to_string()));

        let path = build_path("/v1/services/{service_id}", &variables);
        assert_eq!(path, "/v1/services/srv-d5f6a7b8");
    }

    #[test]
    fn build_path_encodes_reserved_bytes_for_placeholder_values() {
        let mut variables = Map::new();
        variables.insert("project".to_string(), Value::String("team/app name".to_string()));

        let path = build_path("/v1/projects/{project}", &variables);
        assert_eq!(path, "/v1/projects/team%2Fapp%20name");
    }
}
