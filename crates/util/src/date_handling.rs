//! # Date Handling Utilities
//!
//! This module provides utilities for detecting date-like fields in JSON data
//! and formatting date strings into user-friendly formats.

use chrono::{DateTime, Datelike, NaiveDate};

use crate::generated_date_fields;

/// Returns true if a JSON key looks like a date field.
///
/// Uses generated schema-derived keys with fallback heuristics.
/// Checks for common date field patterns like suffixes (_at, _on, _date)
/// and specific field names (created, updated, released).
///
/// # Arguments
/// * `key` - The JSON key to check
///
/// # Returns
/// True if the key appears to represent a date field
///
/// # Example
/// ```rust
/// use oatty_util::date_handling::is_date_like_key;
///
/// // Common date field patterns
/// assert!(is_date_like_key("created_at"));
/// assert!(is_date_like_key("updated_on"));
/// assert!(is_date_like_key("release_date"));
/// assert!(is_date_like_key("created"));
/// assert!(is_date_like_key("updated"));
/// assert!(is_date_like_key("released"));
///
/// // Non-date fields
/// assert!(!is_date_like_key("name"));
/// assert!(!is_date_like_key("description"));
/// assert!(!is_date_like_key("status"));
/// ```
pub fn is_date_like_key(key: &str) -> bool {
    let normalized_key = normalize_date_key(key);

    // Check against generated schema keys first
    if generated_date_fields::DATE_FIELD_KEYS.contains(&normalized_key.as_str()) {
        return true;
    }

    // Fallback to heuristic patterns
    is_heuristic_date_key(&normalized_key)
}

/// Normalizes a key for date field detection.
///
/// Converts to lowercase and replaces spaces and hyphens with underscores
/// to standardize the format for comparison.
///
/// # Arguments
/// * `key` - The original key string
///
/// # Returns
/// The normalized key string
///
/// # Example
/// ```rust
/// // Note: This function is private, so we can't test it directly
/// // The functionality is tested through the public `is_date_like_key` function
/// ```
fn normalize_date_key(key: &str) -> String {
    key.to_ascii_lowercase()
        .replace([' ', '-'], "_")
        .replace("createdat", "created_at")
        .replace("updatedat", "updated_at")
        .replace("releasedat", "released_at")
}

/// Applies heuristic rules to determine if a normalized key is date-like.
///
/// Checks for common date field patterns that aren't covered by the
/// generated schema.
///
/// # Arguments
/// * `normalized_key` - The normalized key string
///
/// # Returns
/// True if the key matches date field heuristics
fn is_heuristic_date_key(normalized_key: &str) -> bool {
    normalized_key.ends_with("_at")
        || normalized_key.ends_with("_on")
        || normalized_key.ends_with("_date")
        || normalized_key == "created"
        || normalized_key == "updated"
        || normalized_key == "released"
}

/// Formats common date strings into MM/DD/YYYY if parsable.
///
/// Attempts to parse the input string using common date formats
/// and returns a formatted string if successful. Supports RFC3339
/// timestamps and ISO date formats.
///
/// # Arguments
/// * `date_string` - The date string to format
///
/// # Returns
/// Some formatted date string if parsing succeeds, None otherwise
///
/// # Example
/// ```rust
/// use oatty_util::date_handling::format_date_mmddyyyy;
///
/// // RFC3339 timestamps
/// assert_eq!(format_date_mmddyyyy("2023-12-25T10:30:00Z"), Some("12/25/2023".to_string()));
/// assert_eq!(format_date_mmddyyyy("2023-12-25T15:45:30+00:00"), Some("12/25/2023".to_string()));
///
/// // ISO date formats
/// assert_eq!(format_date_mmddyyyy("2023-12-25"), Some("12/25/2023".to_string()));
/// assert_eq!(format_date_mmddyyyy("2023/12/25"), Some("12/25/2023".to_string()));
///
/// // Invalid dates
/// assert_eq!(format_date_mmddyyyy("invalid"), None);
/// assert_eq!(format_date_mmddyyyy("2023-13-45"), None);
/// ```
pub fn format_date_mmddyyyy(date_string: &str) -> Option<String> {
    // Try RFC3339 timestamp format first
    if let Some(formatted) = parse_rfc3339_date(date_string) {
        return Some(formatted);
    }

    // Try ISO date formats
    if let Some(formatted) = parse_iso_date(date_string) {
        return Some(formatted);
    }

    None
}

/// Parses an RFC3339 timestamp and formats it as MM/DD/YYYY.
///
/// This function handles RFC3339 formatted timestamps, which are commonly
/// used in APIs and include timezone information.
///
/// # Arguments
/// * `date_string` - The RFC3339 timestamp string
///
/// # Returns
/// Some formatted date string if parsing succeeds, None otherwise
///
/// # Example
/// ```rust
/// // Note: This function is private, so we can't test it directly
/// // The functionality is tested through the public `format_date_mmddyyyy` function
/// ```
fn parse_rfc3339_date(date_string: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(date_string).ok().map(|date_time| {
        let date = date_time.date_naive();
        format!("{:02}/{:02}/{}", date.month(), date.day(), date.year())
    })
}

/// Parses ISO date formats and formats them as MM/DD/YYYY.
///
/// Supports both YYYY-MM-DD and YYYY/MM/DD formats.
///
/// # Arguments
/// * `date_string` - The ISO date string
///
/// # Returns
/// Some formatted date string if parsing succeeds, None otherwise
///
/// # Example
/// ```rust
/// // Note: This function is private, so we can't test it directly
/// // The functionality is tested through the public `format_date_mmddyyyy` function
/// ```
fn parse_iso_date(date_string: &str) -> Option<String> {
    let formats = ["%Y-%m-%d", "%Y/%m/%d"];

    for format_string in formats.iter() {
        if let Ok(date) = NaiveDate::parse_from_str(date_string, format_string) {
            return Some(format!("{:02}/{:02}/{}", date.month(), date.day(), date.year()));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_key_detection() {
        // Generated schema keys should work
        // (This test may need adjustment based on actual generated content)

        // Heuristic patterns
        assert!(is_date_like_key("created_at"));
        assert!(is_date_like_key("updated_on"));
        assert!(is_date_like_key("release_date"));
        assert!(is_date_like_key("created"));
        assert!(is_date_like_key("updated"));
        assert!(is_date_like_key("released"));

        // Non-date keys
        assert!(!is_date_like_key("name"));
        assert!(!is_date_like_key("description"));
        assert!(!is_date_like_key("status"));
    }

    #[test]
    fn test_key_normalization() {
        assert_eq!(normalize_date_key("CreatedAt"), "created_at");
        assert_eq!(normalize_date_key("updated-on"), "updated_on");
        assert_eq!(normalize_date_key("Release Date"), "release_date");
        assert_eq!(normalize_date_key("CREATED_AT"), "created_at");
    }

    #[test]
    fn test_date_formatting() {
        // RFC3339 formats
        assert_eq!(format_date_mmddyyyy("2023-12-25T10:30:00Z"), Some("12/25/2023".to_string()));
        assert_eq!(format_date_mmddyyyy("2023-06-15T14:22:30+00:00"), Some("06/15/2023".to_string()));

        // ISO formats
        assert_eq!(format_date_mmddyyyy("2023-12-25"), Some("12/25/2023".to_string()));
        assert_eq!(format_date_mmddyyyy("2023/06/15"), Some("06/15/2023".to_string()));

        // Invalid dates
        assert_eq!(format_date_mmddyyyy("invalid"), None);
        assert_eq!(format_date_mmddyyyy("2023-13-45"), None);
        assert_eq!(format_date_mmddyyyy(""), None);
    }

    #[test]
    fn test_rfc3339_parsing() {
        assert_eq!(parse_rfc3339_date("2023-12-25T10:30:00Z"), Some("12/25/2023".to_string()));
        assert_eq!(parse_rfc3339_date("2023-06-15T14:22:30+00:00"), Some("06/15/2023".to_string()));
        assert_eq!(parse_rfc3339_date("invalid"), None);
    }

    #[test]
    fn test_iso_date_parsing() {
        assert_eq!(parse_iso_date("2023-12-25"), Some("12/25/2023".to_string()));
        assert_eq!(parse_iso_date("2023/06/15"), Some("06/15/2023".to_string()));
        assert_eq!(parse_iso_date("invalid"), None);
        assert_eq!(parse_iso_date(""), None);
    }
}
