//! Log formatting and redaction utilities.

use crate::types::LogEntry;
use regex::{Captures, Regex};

/// Formatter for log entries with redaction capabilities.
pub struct LogFormatter {
    /// Rules for redacting sensitive information.
    redaction_rules: RedactionRules,
}

/// Rules for redacting sensitive information from logs.
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
    pub fn format(&self, entry: &LogEntry) -> String {
        let timestamp = entry.timestamp.format("%H:%M:%S");
        let level = entry.level.to_string();
        let source = entry.source.to_string();
        let message = self.redact_message(&entry.message);

        format!("[{}] {} {}: {}", timestamp, level, source, message)
    }

    /// Format a log entry for export (without redaction).
    pub fn format_for_export(&self, entry: &LogEntry) -> String {
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
        let patterns = vec![
            // API keys and tokens (allow space, underscore, or dash between parts)
            Regex::new(r"(?i)(api[\s_-]?key|auth[\s_-]?token|token|secret|password)\s*[:=]\s*([^\s,;]+)")
                .expect("Invalid regex pattern"),
            // Bearer tokens
            Regex::new(r"Bearer\s+([A-Za-z0-9\-._~+/]+=*)").expect("Invalid regex pattern"),
            // Basic auth
            Regex::new(r"Basic\s+([A-Za-z0-9+/]+=*)").expect("Invalid regex pattern"),
            // JWT tokens (basic pattern)
            Regex::new(r"eyJ[A-Za-z0-9\-._~+/]+=*").expect("Invalid regex pattern"),
            // UUIDs that might be sensitive
            Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}").expect("Invalid regex pattern"),
            // Credit card numbers (basic pattern)
            Regex::new(r"\b\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}\b").expect("Invalid regex pattern"),
            // Email addresses (optional - might want to keep these)
            // Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b")
            //     .expect("Invalid regex pattern"),
        ];

        Self {
            patterns,
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
        let mut result = text.to_string();

        for pattern in &self.patterns {
            // Replace only the captured sensitive portion when possible,
            // preserving any descriptive prefix like "API key:" or "Bearer ".
            result = pattern
                .replace_all(&result, |caps: &Captures| {
                    // If there is at least one capturing group, treat the last capture
                    // as the sensitive value and replace only that portion.
                    if caps.len() > 1 {
                        let full = caps.get(0).unwrap().as_str();
                        let sensitive = caps.get(caps.len() - 1).unwrap();

                        // Build the redacted string by replacing the sensitive span
                        // within the full match while keeping the rest intact.
                        let mut redacted = String::with_capacity(full.len());
                        let start = sensitive.start() - caps.get(0).unwrap().start();
                        let end = sensitive.end() - caps.get(0).unwrap().start();
                        redacted.push_str(&full[..start]);
                        redacted.push_str(&self.replacement);
                        redacted.push_str(&full[end..]);
                        redacted
                    } else {
                        // No capture groups: replace the entire match.
                        self.replacement.clone()
                    }
                })
                .to_string();
        }

        result
    }

    /// Check if text contains sensitive information.
    pub fn contains_sensitive(&self, text: &str) -> bool {
        for pattern in &self.patterns {
            if pattern.is_match(text) {
                return true;
            }
        }
        false
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

// Intentionally no public formatting error type for now; callers use Strings.

#[cfg(test)]
mod tests {
    use crate::types::plugin::{LogLevel, LogSource};

    use super::*;

    #[test]
    fn test_log_formatter() {
        let formatter = LogFormatter::new();

        let entry = LogEntry::new(
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
