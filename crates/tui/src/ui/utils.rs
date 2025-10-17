//! UI utilities and helper functions for the TUI application.
//!
//! This module provides utility functions and helper traits that are used
//! across the UI components. It includes layout utilities, string helpers,
//! and other common functionality needed for UI rendering.

use std::collections::{BTreeSet, HashMap};

use heck::ToTitleCase;
use heroku_util::{format_date_mmddyyyy, is_date_like_key, redact_json, redact_sensitive};
use ratatui::prelude::*;
use serde_json::{Map, Value};

use crate::{
    app,
    ui::{components::logs::state::LogEntry, theme::roles::Theme as UiTheme},
};

/// Creates a centered rectangular area within a given rectangle.
///
/// This utility function calculates a centered rectangle based on percentage
/// dimensions relative to the parent rectangle. It's commonly used for
/// creating modal dialogs and popup windows.
///
/// # Arguments
///
/// * `percent_x` - The width of the centered rectangle as a percentage (0-100)
/// * `percent_y` - The height of the centered rectangle as a percentage (0-100)
/// * `r` - The parent rectangle to center within
///
/// # Returns
///
/// A new rectangle centered within the parent rectangle with the specified
/// percentage dimensions.
///
/// # Examples
///
/// ```rust,ignore
/// use ratatui::prelude::*;
/// use heroku_tui::ui::utils::centered_rect;
///
/// let parent = Rect::new(0, 0, 100, 50);
/// let centered = centered_rect(80, 70, parent);
/// // Creates a rectangle that's 80% wide and 70% tall, centered in parent
/// ```
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);
    let area = Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1]);
    area[1]
}
/// Infer a set of column names from a list of JSON-like values.
///
/// This function analyzes the provided array of JSON-like objects (`arr`) to infer which
/// keys (columns) are most relevant based on scoring logic and frequency of occurrence.
/// It ensures that at least four column names are returned, prioritizing higher-scoring
/// keys while falling back on frequently appearing keys if necessary.
///
/// # Scoring Logic:
/// - Each key in an object is assigned a score based on its characteristics:
///   - A base score is determined per key by the `base_key_score` function.
///   - A boost is added for the key's property frequency using the `property_frequency_boost` function.
///   - A penalty is applied for nested arrays, objects, or long strings:
///     - Nested arrays: Score is reduced by a value derived from the array length.
///     - Nested objects: Score is reduced by a fixed amount.
///     - Long strings (greater than 80 characters): Score is reduced by a fixed amount.
/// - Scores are aggregated across a sample of at most 50 items.
///
/// # Column Selection:
/// - The keys are sorted by descending score, and the top-scoring keys are selected.
/// - At most, 6 keys are selected based on the scoring logic.
/// - If fewer than 4 keys are selected, additional keys are chosen based on their frequency
///   of occurrence in the data. Frequencies are determined from a sample of up to 100 items.
///
/// # Parameters:
/// - `arr: &[Value]`:
///   A slice of JSON-like objects, where each object is represented as a `Value` (typically from
///   the `serde_json` crate). The function expects elements to be of type `Value::Object`.
///
/// # Returns:
/// - `Vec<String>`:
///   A vector containing the inferred column names, sorted by their relevance according to
///   the scoring logic and frequency of occurrence. At least 4 column names will be included,
///   with a maximum of 6.
///
/// # Examples:
/// ```
/// use serde_json::Value;
/// use your_crate::infer_columns;
///
/// let data = vec![
///     Value::Object(serde_json::json!({"name": "Alice", "age": 30}).as_object().unwrap().clone()),
///     Value::Object(serde_json::json!({"name": "Bob", "age": 25, "location": "USA"}).as_object().unwrap().clone()),
///     Value::Object(serde_json::json!({"name": "Charlie", "status": "active"}).as_object().unwrap().clone())
/// ];
///
/// let columns = infer_columns(&data);
/// assert!(columns.len() >= 4);
/// println!("{:?}", columns);
/// ```
///
/// # Notes:
/// - The function assumes that the input slice contains values of type `Value::Object`.
///   Other types (e.g., `Value::Array` or `Value::String`) are ignored during processing.
/// - Scoring logic and penalties may require adjustment based on specific use cases or
///   domain-specific importance of certain keys.
pub fn infer_columns(arr: &[Value]) -> Vec<String> {
    let mut score: HashMap<String, i32> = HashMap::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let sample = arr.iter().take(50); // sample up to 50 rows
    for item in sample {
        if let Value::Object(map) = item {
            for (header, v) in map.iter() {
                seen.insert(header.clone());
                let mut s = base_key_score(header) + property_frequency_boost(header);
                // Penalize nested arrays/objects (not scalar-ish)
                match v {
                    Value::Array(a) => s -= (a.len() as i32).min(3) + 3,
                    Value::Object(_) => s -= 5,
                    Value::String(sv) if sv.len() > 80 => s -= 3,
                    _ => {}
                }
                *score.entry(header.clone()).or_insert(0) += s;
            }
        }
    }
    let mut keys: Vec<(String, i32)> = seen
        .into_iter()
        .map(|header| (header.clone(), *score.get(&header).unwrap_or(&0)))
        .collect();
    keys.sort_by(|a, b| b.1.cmp(&a.1));
    let mut cols: Vec<String> = keys.into_iter().take(6).map(|(header, _)| header).collect();
    if cols.len() < 4 {
        // Ensure at least 4 columns by adding additional keys by frequency of
        // appearance
        let mut freq: HashMap<String, usize> = HashMap::new();
        for item in arr.iter().take(100) {
            if let Value::Object(map) = item {
                for header in map.keys() {
                    *freq.entry(header.clone()).or_insert(0) += 1;
                }
            }
        }
        let mut extras: Vec<(String, usize)> = freq.into_iter().filter(|(header, _)| !cols.contains(header)).collect();
        extras.sort_by(|a, b| b.1.cmp(&a.1));
        for (header, _) in extras.into_iter() {
            cols.push(header);
            if cols.len() >= 4 {
                break;
            }
        }
    }
    cols
}

pub fn is_status_like(key: &str) -> bool {
    matches!(key.to_ascii_lowercase().as_str(), "status" | "state")
}

pub fn status_color_for_value(value: &str, theme: &dyn UiTheme) -> Option<ratatui::style::Color> {
    let v = value.to_ascii_lowercase();
    if matches!(v.as_str(), "ok" | "succeeded" | "success" | "passed") {
        Some(theme.roles().success)
    } else if matches!(v.as_str(), "error" | "failed" | "fail") {
        Some(theme.roles().error)
    } else if matches!(v.as_str(), "warning" | "warn" | "unstable" | "modified" | "change") {
        Some(theme.roles().warning)
    } else {
        None
    }
}

pub fn base_key_score(key: &str) -> i32 {
    match key {
        "name" | "description" | "app" | "dyno" | "addon" | "config_var" => 100,
        "status" | "state" | "type" | "region" | "stack" => 80,
        "owner" | "user" | "email" => 60,
        "created_at" | "updated_at" | "released_at" => 40,
        "id" => -100,
        _ => 20,
    }
}
/// Generates a sorted vector of keys from a given map, arranged in descending order of their computed scores.
///
/// This function calculates the combined score for each key using two components:
/// 1. `base_key_score`: A base score derived from the key itself.
/// 2. `property_frequency_boost`: A frequency adjustment or weighting added to the base score.
///
/// The sorting process orders the keys based on their scores in descending order (i.e., higher scores appear first).
///
/// # Parameters
/// - `map`: A reference to a `Map` containing string keys and associated values of type `Value`.
///
/// # Returns
/// A `Vec<String>` containing all the keys in the input map, sorted by their scores in descending order.
///
/// # Example
/// ```
/// use your_crate::get_scored_keys;
/// use serde_json::Map;
/// use serde_json::Value;
///
/// let mut map = Map::new();
/// map.insert("apple".to_string(), Value::Null);
/// map.insert("banana".to_string(), Value::Null);
///
/// let sorted_keys = get_scored_keys(&map);
/// println!("{:?}", sorted_keys); // Example output: ["banana", "apple"], depending on scoring logic
/// ```
///
/// # Notes
/// - This function relies on the implementations of `base_key_score` and `property_frequency_boost`
///   to compute individual scores.
/// - The behavior and order of the returned keys depend on the scoring logic defined in those functions.
///
/// # Panics
/// This function does not explicitly handle panics unless the underlying operations (e.g., `key()` or scoring functions) panic.
pub fn get_scored_keys(map: &Map<String, Value>) -> Vec<String> {
    let mut keys: Vec<String> = map.keys().cloned().collect();
    keys.sort_by(|a, b| {
        let sa = base_key_score(a) + property_frequency_boost(a);
        let sb = base_key_score(b) + property_frequency_boost(b);
        sb.cmp(&sa)
    });
    keys
}
/// Applies frequency-based scoring boost for common API properties.
///
/// This function provides additional scoring based on the frequency
/// of property names in typical API responses.
///
/// # Arguments
///
/// * `header` - The column key to score
///
/// # Returns
///
/// A boost score for common properties.
fn property_frequency_boost(header: &str) -> i32 {
    let l = header.to_lowercase();
    match l.as_str() {
        // Very common, highly informative
        "name" => 11,
        // Timestamps
        "created_at" | "updated_at" => 8,
        // Common resource scoping/identity
        "app" | "owner" | "email" => 6,
        // Lifecycle/status
        "type" | "state" | "status" => 6,
        // Misc descriptive
        "description" => 3,
        // Resource context
        "region" | "team" | "stack" | "user" | "plan" | "pipeline" => 5,
        // URLs
        "url" | "web_url" | "git_url" => 4,
        // roles and others
        "role" => 3,
        _ => 0,
    }
}

/// Formatter kinds that influence header/value display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnFormatter {
    /// Apply Title Case to header labels (from snake_case keys).
    TitleCaseHeader,
    /// Format string values that look like dates into MM/DD/YYYY.
    DateValue,
}

/// Column metadata with measured maximum string length for rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnWithSize {
    /// Display name (header label), typically Title Case of the key.
    pub name: String,
    /// JSON key to extract values from each row object.
    pub key: String,
    /// Maximum string length among header and sampled, formatted cell values.
    pub max_len: usize,
}

/// Normalize a header for display: replace '_' with ' ' and title case.
pub fn normalize_header(key: &str) -> String {
    key.replace('_', " ").to_string().to_title_case()
}

/// Infer columns and compute their maximum formatted string lengths from JSON.
/// - Uses `infer_columns_from_json` to determine keys.
/// - Applies light formatting (date for date-like keys) before measuring.
/// - Includes the header label length in the max.
/// - Samples up to `sample` rows for performance.
pub fn infer_columns_with_sizes_from_json(array: &[Value], sample: usize) -> Vec<ColumnWithSize> {
    let keys = infer_columns(array);
    if keys.is_empty() {
        return Vec::new();
    }
    let sample_rows = array.iter().take(sample);

    let mut out: Vec<ColumnWithSize> = Vec::with_capacity(keys.len());
    for key in keys.iter() {
        let mut formatters = vec![ColumnFormatter::TitleCaseHeader];
        if is_date_like_key(key) {
            formatters.push(ColumnFormatter::DateValue);
        }
        let header = normalize_header(key);
        let mut max_len = header.len();
        for row in sample_rows.clone() {
            let formatted = match row.get(key) {
                Some(Value::String(s)) => {
                    if formatters.contains(&ColumnFormatter::DateValue) {
                        format_date_mmddyyyy(s).unwrap_or_else(|| s.clone())
                    } else {
                        s.clone()
                    }
                }
                Some(Value::Number(n)) => n.to_string(),
                Some(Value::Bool(b)) => b.to_string(),
                Some(Value::Null) => "null".to_string(),
                Some(Value::Object(map)) => {
                    // Fall back to highest-scoring key as a string
                    if let Some(best) = get_scored_keys(map).first() {
                        map.get(best)
                            .map(|v| v.as_str().unwrap_or(&v.to_string()).to_string())
                            .unwrap_or_else(|| "".to_string())
                    } else {
                        "".to_string()
                    }
                }
                Some(other) => other.to_string(),
                None => String::new(),
            };
            let l = formatted.len();
            if l > max_len {
                max_len = l;
            }
        }
        out.push(ColumnWithSize {
            name: header,
            key: key.clone(),
            max_len,
        });
    }
    out
}
/// Renders a given `Value` into a `String` representation, applying specific formatting
/// or obfuscation rules based on the provided key and value type.
///
/// # Parameters
/// - `key`: A reference to a `&str` representing the key associated with the `value`.
///   This is used to determine if specific rules, such as masking sensitive
///   data or formatting date-like strings, should be applied.
/// - `value`: A reference to a `Value` instance representing the data to be rendered
///   into a string. The `Value` type can represent various data types, such
///   as strings, numbers, booleans, nulls, objects, etc., often used
///   in JSON data representations.
///
/// # Behavior
/// - If `value` is a `Value::String`:
///     - Checks if the key is sensitive using `is_sensitive_key(key)`. If true, applies
///       a middle obfuscation mask with `ellipsize_middle_if_sha_like()` (truncates
///       sensitive data, typically SHA-like strings, to a shorter form).
///     - Otherwise, checks if the key suggests a date using `is_date_like_key(key)`. If true,
///       attempts to format the string as a date in `MM/DD/YYYY` format using
///       `format_date_mmddyyyy()`. If formatting fails, returns the original string.
///     - If neither condition is met, returns the string unchanged.
/// - If `value` is a `Value::Number`, converts it to a `String` using the number's
///   `to_string()` method.
/// - If `value` is a `Value::Bool`, converts it to a string representation (`"true"` or `"false"`).
/// - If `value` is a `Value::Null`, returns the string `"null"`.
/// - If `value` is a `Value::Object`, attempts to render the object as follows:
///     - Scores the keys of the object using `get_scored_keys()`.
///     - If the highest scoring key exists, retrieves its associated value.
///         - If this nested value is a string, it is returned as-is.
///         - Otherwise, converts the nested value to a string.
///     - If no keys are present, falls back to converting the entire `Value` object
///       to a string representation.
/// - For any other value types (e.g., arrays), falls back to converting the value
///   directly to a string representation using its `to_string()` method.
///
/// # Returns
/// - A `String` containing the rendered representation of the `value` based on the rules described above.
///
/// # Example
/// ```
/// use serde_json::Value;
///
/// let key = "password";
/// let value = Value::String("123456789abcdef123456789abcdef".to_string());
/// let rendered = render_value(key, &value);
/// assert_eq!(rendered, "1234...cdef"); // Sensitive key obfuscation applied.
///
/// let key = "date_of_birth";
/// let value = Value::String("2023-09-15".to_string());
/// let rendered = render_value(key, &value);
/// assert_eq!(rendered, "09/15/2023"); // Date formatting applied.
///
/// let key = "active";
/// let value = Value::Bool(true);
/// let rendered = render_value(key, &value);
/// assert_eq!(rendered, "true"); // Boolean converted to string.
///
/// let key = "some_key";
/// let value = Value::Null;
/// let rendered = render_value(key, &value);
/// assert_eq!(rendered, "null"); // Null value represented as "null".
///
/// let key = "nested_obj";
/// let value = serde_json::json!({
///     "score1": "value1",
///     "score2": "value2"
/// });
/// let rendered = render_value(key, &value);
/// assert_eq!(rendered, "value1"); // First/highest score key's value returned as string.
/// ```
pub fn render_value(key: &str, value: &Value) -> String {
    match value {
        Value::String(s) => {
            if is_sensitive_key(key) {
                ellipsize_middle_if_sha_like(s, 12)
            } else if is_date_like_key(key) {
                format_date_mmddyyyy(s).unwrap_or_else(|| s.clone())
            } else {
                s.clone()
            }
        }
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        // Take the highest scoring key from the object as a string
        Value::Object(map) => {
            if let Some(key) = get_scored_keys(map).first() {
                let value = map.get(key).unwrap();
                if let Some(s) = value.as_str() {
                    s.to_string()
                } else {
                    value.to_string()
                }
            } else {
                value.to_string()
            }
        }
        _ => value.to_string(),
    }
}

pub fn is_sensitive_key(key: &str) -> bool {
    matches!(key, "token" | "key" | "secret" | "password" | "api_key" | "auth_token")
}
/// Truncates a potentially SHA-like hexadecimal string in the middle if it meets certain conditions.
///
/// This function takes a string `s` and a `keep_total` parameter (indicating the total number of
/// characters to retain). If the string appears to be hex-like (a heuristic based on length and
/// content) and is longer than the `keep_total`, it truncates the string by keeping the first `keep_total / 2`
/// characters and the last `keep_total - (keep_total / 2)` characters, replacing the middle portion with an
/// ellipsis (`…`). If the string does not meet the hex-like heuristic or is short enough, it returns the
/// input string unmodified.
///
/// # Arguments
///
/// * `s` - The input string to be potentially truncated.
/// * `keep_total` - The desired total length for the truncated string, including the ellipsis (`…`).
///
/// # Returns
///
/// A new `String`:
/// - If the input string is recognized as SHA-like and its length exceeds `keep_total`, the string is truncated
///   in the middle with its center replaced by `…`.
/// - Otherwise, the function simply returns a copy of the input string.
///
/// # Heuristic for SHA-like determination
///
/// - The string must have a length of at least 16 characters.
/// - The string must consist entirely of ASCII hexadecimal digits (`[0-9a-fA-F]`).
///
/// # Examples
///
/// ```
/// let s = "1234567890abcdef1234567890abcdef";
/// assert_eq!(ellipsize_middle_if_sha_like(s, 10), "12345…bcdef");
///
/// let s = "not-a-sha-string";
/// assert_eq!(ellipsize_middle_if_sha_like(s, 10), "not-a-sha-string");
///
/// let s = "12345";
/// assert_eq!(ellipsize_middle_if_sha_like(s, 10), "12345");
/// ```
fn ellipsize_middle_if_sha_like(s: &str, keep_total: usize) -> String {
    // Heuristic: hex-looking and long → compress
    let is_hexish = s.len() >= 16 && s.chars().all(|c| c.is_ascii_hexdigit());
    if !is_hexish || s.len() <= keep_total {
        return s.to_string();
    }
    let head = keep_total / 2;
    let tail = keep_total - head;
    format!("{}…{}", &s[..head], &s[s.len() - tail..])
}
// ============================================================================
// Copy and Text Processing Methods
// ============================================================================

/// Builds the text content to be copied to clipboard based on the current
/// selection.
///
/// This method handles different copy scenarios:
///
/// - **Single API entry with JSON**: Returns formatted JSON if pretty mode
///   enabled
/// - **Single API entry without JSON**: Returns raw log content
/// - **Multi-selection**: Returns concatenated log entries
///
/// All output is automatically redacted for security.
///
/// # Arguments
///
/// * `app` - The application state containing logs and selection
///
/// # Returns
///
/// A redacted string containing the selected log content
pub fn build_copy_text(app: &app::App) -> String {
    if app.logs.entries.is_empty() {
        return String::new();
    }
    let (start, end) = app.logs.selection.range();
    if start >= app.logs.entries.len() {
        return String::new();
    }

    // Handle a single selection with special JSON formatting
    if start == end
        && let Some(LogEntry::Api { json, raw, .. }) = app.logs.rich_entries.get(start)
    {
        if let Some(j) = json
            && app.logs.pretty_json
        {
            let red = redact_json(j);
            return serde_json::to_string_pretty(&red).unwrap_or_else(|_| redact_sensitive(raw));
        }
        return redact_sensitive(raw);
    }

    // Multi-select or text fallback: concatenate visible strings
    let mut buf = String::new();
    for i in start..=end.min(app.logs.entries.len() - 1) {
        let line = app.logs.entries.get(i).cloned().unwrap_or_default();
        if !buf.is_empty() {
            buf.push('\n');
        }
        buf.push_str(&line);
    }
    redact_sensitive(&buf)
}

/// Normalize execution payloads to ensure single-key collections render in the results table.
///
/// Some APIs return objects shaped as `{ "items": [ ... ] }`. The table expects an array at
/// the root level, so this helper unwraps objects that meet this pattern. All other payloads
/// are returned unchanged.
pub fn normalize_result_payload(value: Value) -> Value {
    if let Value::Object(map) = &value
        && map.len() == 1
        && let Some(inner_value) = map.values().next()
        && inner_value.is_array()
    {
        return inner_value.clone();
    }
    value
}
