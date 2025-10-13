use super::WorkflowProviderSnapshot;
use serde_json::Value as JsonValue;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct ProviderCacheSummary {
    age: Duration,
    ttl: Option<Duration>,
    item_count: Option<usize>,
}

impl ProviderCacheSummary {
    pub fn from_snapshot(snapshot: &WorkflowProviderSnapshot) -> Self {
        Self {
            age: snapshot.last_refreshed.elapsed(),
            ttl: snapshot.ttl,
            item_count: snapshot.item_count,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.ttl.map(|ttl| self.age > ttl).unwrap_or(false)
    }

    pub fn ttl(&self) -> Option<Duration> {
        self.ttl
    }

    pub fn age(&self) -> Duration {
        self.age
    }

    pub fn item_count(&self) -> Option<usize> {
        self.item_count
    }
}

pub fn format_cache_summary(summary: &ProviderCacheSummary) -> String {
    let mut parts = Vec::new();

    if summary.age() < Duration::from_millis(500) {
        parts.push("loaded just now".to_string());
    } else {
        parts.push(format!("loaded {} ago", human_duration(summary.age())));
    }

    if let Some(ttl) = summary.ttl() {
        let ttl_text = format!("ttl {}", human_duration(ttl));
        if summary.is_expired() {
            parts.push(format!("{ttl_text} (expired)"));
        } else {
            parts.push(ttl_text);
        }
    }

    if let Some(count) = summary.item_count() {
        parts.push(format!("{count} values"));
    }

    parts.join(" • ")
}

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

pub fn summarize_values(values: &[JsonValue], limit: usize) -> String {
    if values.is_empty() {
        return "<none>".to_string();
    }

    let mut parts = Vec::new();
    for value in values.iter().take(limit) {
        parts.push(format_preview(value));
    }
    if values.len() > limit {
        parts.push("…".to_string());
    }
    parts.join(", ")
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
