//! Rendering helpers shared across workflow views.
//!
//! These utilities provide lightweight formatting and classification support
//! for workflow values so that UI components can present consistent previews
//! and syntax-aware styling across the application.

use ratatui::style::Style;
use serde_json::Value as JsonValue;

use crate::ui::theme::roles::Theme;

/// Simplified syntax categories derived from JSON values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonSyntaxRole {
    /// String literal content.
    String,
    /// Numeric literal content (integers or floats).
    Number,
    /// Boolean literals (`true` / `false`).
    Boolean,
    /// Explicit null literal.
    Null,
    /// Aggregate types such as arrays or objects.
    Collection,
}

/// Formats a JSON value for inline previewing within list/detail views.
///
/// Strings are returned verbatim, numeric and boolean values are converted to
/// their textual representation, and complex structures fall back to compact
/// JSON serialization. The result is truncated to a maximum of 40 characters so
/// that previews remain readable in constrained layouts.
pub fn format_preview(value: &JsonValue) -> String {
    let text = match value {
        JsonValue::String(s) => s.clone(),
        JsonValue::Number(n) => n.to_string(),
        JsonValue::Bool(b) => b.to_string(),
        JsonValue::Null => "<null>".into(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| "<value>".into()),
    };

    let max_chars = 40;
    if text.chars().count() <= max_chars {
        text
    } else {
        let truncated: String = text.chars().take(max_chars - 3).collect();
        format!("{truncated}...")
    }
}

/// Classifies a JSON value into a high-level syntax role.
///
/// The returned [`JsonSyntaxRole`] allows callers to select theme-aware colors
/// that match the underlying data type (strings, numbers, booleans, and so on).
pub fn classify_json_value(value: &JsonValue) -> JsonSyntaxRole {
    match value {
        JsonValue::String(_) => JsonSyntaxRole::String,
        JsonValue::Number(_) => JsonSyntaxRole::Number,
        JsonValue::Bool(_) => JsonSyntaxRole::Boolean,
        JsonValue::Null => JsonSyntaxRole::Null,
        JsonValue::Array(_) => JsonSyntaxRole::Collection,
        JsonValue::Object(_) => JsonSyntaxRole::Collection,
    }
}

/// Resolves a [`Style`] that matches the provided syntax role for the active theme.
pub fn style_for_role(role: JsonSyntaxRole, theme: &dyn Theme) -> Style {
    match role {
        JsonSyntaxRole::String => theme.syntax_string_style(),
        JsonSyntaxRole::Number => theme.syntax_number_style(),
        JsonSyntaxRole::Boolean | JsonSyntaxRole::Null => theme.syntax_keyword_style(),
        JsonSyntaxRole::Collection => theme.syntax_type_style(),
    }
}
