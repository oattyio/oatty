use serde_json::Value as JsonValue;

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
