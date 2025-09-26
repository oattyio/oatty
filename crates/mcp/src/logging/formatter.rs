//! Log formatting and redaction utilities.

use crate::types::McpLogEntry;
use heroku_util::redact_sensitive_with;
use regex::{Captures, Regex};

/// Formatter for log entries with redaction capabilities.
#[derive(Debug)]
pub struct LogFormatter {
    /// Rules for redacting sensitive information.
    redaction_rules: RedactionRules,
}

/// Rules for redacting sensitive information from logs.
#[derive(Debug)]
pub struct RedactionRules {
    /// Patterns to match sensitive values.
    patterns: Vec<Regex>,

    /// Replacement string for sensitive values.
    replacement: String,
}

impl LogFormatter {
    /// Create a new log formatter.
    pub fn new() -> Self {
        Self {
            redaction_rules: RedactionRules::default(),
        }
    }

    /// Create a new log formatter with custom redaction rules.
    pub fn with_rules(redaction_rules: RedactionRules) -> Self {
        Self { redaction_rules }
    }

    /// Format a log entry for display.
    pub fn format(&self, entry: &McpLogEntry) -> String {
        let timestamp = entry.timestamp.format("%H:%M:%S");
        let level = entry.level.to_string();
        let source = entry.source.to_string();
        let message = self.redact_message(&entry.message);

        format!("[{}] {} {}: {}", timestamp, level, source, message)
    }

    /// Format a log entry for export (without redaction).
    pub fn format_for_export(&self, entry: &McpLogEntry) -> String {
        let timestamp = entry.timestamp.format("%Y-%m-%d %H:%M:%S%.3f UTC");
        let level = entry.level.to_string();
        let source = entry.source.to_string();

        format!("[{}] {} {}: {}", timestamp, level, source, entry.message)
    }

    /// Redact sensitive information from a message.
    pub fn redact_message(&self, message: &str) -> String {
        self.redaction_rules.redact(message)
    }

    /// Get the redaction rules.
    pub fn redaction_rules(&self) -> &RedactionRules {
        &self.redaction_rules
    }
}

impl Default for LogFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl RedactionRules {
    /// Create new redaction rules with default patterns.
    pub fn new() -> Self {
        // Rely on shared util patterns by default; allow callers to supply extras.
        Self {
            patterns: Vec::new(),
            replacement: "[REDACTED]".to_string(),
        }
    }

    /// Create redaction rules with custom patterns.
    pub fn with_patterns(patterns: Vec<Regex>, replacement: String) -> Self {
        Self { patterns, replacement }
    }

    /// Add a new redaction pattern.
    pub fn add_pattern(&mut self, pattern: Regex) {
        self.patterns.push(pattern);
    }

    /// Redact sensitive information from text.
    pub fn redact(&self, text: &str) -> String {
        // First, apply shared redaction rules from util with our replacement token
        let mut result = redact_sensitive_with(text, &self.replacement);

        // Then, apply any additional local patterns (if configured)
        for pattern in &self.patterns {
            result = pattern
                .replace_all(&result, |caps: &Captures| {
                    if caps.len() > 1 {
                        let full = caps.get(0).unwrap().as_str();
                        let sensitive = caps.get(caps.len() - 1).unwrap();
                        let mut redacted = String::with_capacity(full.len());
                        let start = sensitive.start() - caps.get(0).unwrap().start();
                        let end = sensitive.end() - caps.get(0).unwrap().start();
                        redacted.push_str(&full[..start]);
                        redacted.push_str(&self.replacement);
                        redacted.push_str(&full[end..]);
                        redacted
                    } else {
                        self.replacement.clone()
                    }
                })
                .to_string();
        }

        result
    }

    /// Check if text contains sensitive information.
    pub fn contains_sensitive(&self, text: &str) -> bool {
        if redact_sensitive_with(text, &self.replacement) != text {
            return true;
        }

        self.patterns.iter().any(|pattern| pattern.is_match(text))
    }

    /// Get the number of redaction patterns.
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    /// Get the replacement string.
    pub fn replacement(&self) -> &str {
        &self.replacement
    }

    /// Set the replacement string.
    pub fn set_replacement(&mut self, replacement: String) {
        self.replacement = replacement;
    }
}

impl Default for RedactionRules {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::types::{LogLevel, LogSource};

    use super::*;

    #[test]
    fn test_log_formatter() {
        let formatter = LogFormatter::new();

        let entry = McpLogEntry::new(
            LogLevel::Info,
            "Plugin started successfully".to_string(),
            LogSource::System,
            "test-plugin".to_string(),
        );

        let formatted = formatter.format(&entry);
        assert!(formatted.contains("info"));
        assert!(formatted.contains("system"));
        assert!(formatted.contains("Plugin started successfully"));
    }

    #[test]
    fn test_redaction_rules() {
        let rules = RedactionRules::new();

        let text = "API key: abc123def456";
        let redacted = rules.redact(text);
        assert_eq!(redacted, "API key: [REDACTED]");

        let text = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";
        let redacted = rules.redact(text);
        assert_eq!(redacted, "Bearer [REDACTED]");
    }

    #[test]
    fn test_contains_sensitive() {
        let rules = RedactionRules::new();

        assert!(rules.contains_sensitive("API key: secret123"));
        assert!(rules.contains_sensitive("Bearer token123"));
        assert!(!rules.contains_sensitive("Regular log message"));
    }

    #[test]
    fn test_custom_redaction_rules() {
        let pattern = Regex::new(r"custom:\s*([^\s]+)").unwrap();
        let rules = RedactionRules::with_patterns(vec![pattern], "***".to_string());

        let text = "custom: sensitive_value";
        let redacted = rules.redact(text);
        assert_eq!(redacted, "custom: ***");
    }
}
