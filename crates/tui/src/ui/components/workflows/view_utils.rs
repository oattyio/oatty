use serde_json::Value as JsonValue;
use std::time::Duration;

pub fn human_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    if total_seconds == 0 {
        return "0s".to_string();
    }

    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    let mut parts = Vec::new();
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes}m"));
    }
    if seconds > 0 && hours == 0 {
        parts.push(format!("{seconds}s"));
    }

    if parts.is_empty() { "0s".to_string() } else { parts.join(" ") }
}

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
