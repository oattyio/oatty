//! UI utilities and helper functions for the TUI application.
//!
//! This module provides utility functions and helper traits that are used
//! across the UI components. It includes layout utilities, string helpers,
//! and other common functionality needed for UI rendering.

use std::collections::{BTreeSet, HashMap};

use ratatui::prelude::*;
use serde_json::{Map, Value};

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
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    let area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1]);
    area[1]
}

// Helper methods moved from tables.rs
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
        // Ensure at least 4 columns by adding additional keys by frequency of appearance
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
/// Infer columns from a JSON value (array or object containing array).
pub fn infer_columns_from_json(json: &Value) -> Vec<String> {
    let arr = match json {
        Value::Array(a) => Some(a.as_slice()),
        Value::Object(m) => m.values().find_map(|v| match v {
            Value::Array(a) => Some(a.as_slice()),
            _ => None,
        }),
        _ => None,
    };
    match arr {
        Some(a) => infer_columns(a),
        None => vec![],
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

/// Extension trait for String to provide fallback values when empty.
///
/// This trait adds a method to String that returns an alternative value
/// when the string is empty, useful for displaying placeholder text
/// in UI components.
///
/// # Examples
///
/// ```rust,ignore
/// use heroku_tui::ui::utils::IfEmptyStr;
///
/// let empty = String::new();
/// let result = empty.if_empty_then("default".to_string());
/// assert_eq!(result, "default");
///
/// let non_empty = "hello".to_string();
/// let result = non_empty.if_empty_then("default".to_string());
/// assert_eq!(result, "hello");
/// ```
pub trait IfEmptyStr {
    /// Returns the string if non-empty, otherwise returns the alternative value.
    ///
    /// # Arguments
    ///
    /// * `alt` - The alternative string to return if self is empty
    ///
    /// # Returns
    ///
    /// The original string if non-empty, otherwise the alternative string.
    fn if_empty_then(self, alt: String) -> String;
}

impl IfEmptyStr for String {
    fn if_empty_then(self, alt: String) -> String {
        if self.is_empty() { alt } else { self }
    }
}
