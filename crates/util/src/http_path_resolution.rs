use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::Value;
use std::collections::HashMap;

/// Resolves a path template by replacing placeholders with actual values.
/// The path template follow the same format as JSON hyper-schema URI
/// templates. See https://json-schema.org/draft/2019-09/json-schema-hypermedia#uriTemplating
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
        let enc = utf8_percent_encode(&val, NON_ALPHANUMERIC).to_string();
        path = path.replace(&format!("{{{}}}", k), &enc);
    }
    path
}
