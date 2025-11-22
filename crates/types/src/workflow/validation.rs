//! Validation helpers shared across workflow consumers.
//!
//! These routines ensure that default values sourced from history, environment
//! variables, or provider selections obey the declarative constraints supplied
//! in the workflow definition.

use regex::Regex;
use serde_json::Value;

use super::WorkflowInputValidation;

/// Validate a JSON candidate against the declarative workflow rules.
///
/// The checks mirror the behaviour expected by the TUI and CLI:
/// - Enumerations must include the candidate.
/// - Patterns, minimum length, and maximum length only apply to strings.
/// - Non-string values are allowed when the validation metadata does not
///   specify string-specific requirements.
pub fn validate_candidate_value(candidate: &Value, validation: &WorkflowInputValidation) -> Result<(), String> {
    if !validation.allowed_values.is_empty() {
        let matches_allowed_value = validation
            .allowed_values
            .iter()
            .any(|allowed| json_values_match(allowed, candidate));
        if !matches_allowed_value {
            return Err("value is not in the allowed set".to_string());
        }
    }

    match candidate {
        Value::String(text) => {
            if let Some(min_length) = validation.min_length
                && text.chars().count() < min_length
            {
                return Err(format!("value must be at least {} characters", min_length));
            }

            if let Some(max_length) = validation.max_length
                && text.chars().count() > max_length
            {
                return Err(format!("value must be at most {} characters", max_length));
            }

            if let Some(pattern) = &validation.pattern {
                let regex = Regex::new(pattern).map_err(|error| format!("invalid pattern '{}': {}", pattern, error))?;
                if !regex.is_match(text) {
                    return Err(format!("value must match the pattern {}", pattern));
                }
            }
            Ok(())
        }
        other => {
            if validation.pattern.is_some() || validation.min_length.is_some() || validation.max_length.is_some() {
                Err("value must be text to satisfy validation rules".to_string())
            } else if validation.allowed_values.is_empty() || validation.allowed_values.iter().any(|item| item == other) {
                Ok(())
            } else {
                Err("value is not in the allowed set".to_string())
            }
        }
    }
}

fn json_values_match(expected: &Value, candidate: &Value) -> bool {
    if expected == candidate {
        return true;
    }
    match (expected, candidate) {
        (Value::String(expected_text), Value::String(candidate_text)) => expected_text == candidate_text,
        (Value::String(expected_text), other) => expected_text == &other.to_string(),
        (other, Value::String(candidate_text)) => {
            if let Ok(parsed) = serde_json::from_str::<Value>(candidate_text) {
                other == &parsed
            } else {
                other == &Value::String(candidate_text.clone())
            }
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_validation() -> WorkflowInputValidation {
        WorkflowInputValidation {
            required: false,
            allowed_values: Vec::new(),
            pattern: None,
            min_length: None,
            max_length: None,
        }
    }

    #[test]
    fn string_candidate_matching_pattern_passes() {
        let mut validation = base_validation();
        validation.pattern = Some("^[a-z]{3,5}$".to_string());

        assert!(validate_candidate_value(&Value::String("app".to_string()), &validation).is_ok());
        assert!(validate_candidate_value(&Value::String("valid".to_string()), &validation).is_ok());
    }

    #[test]
    fn string_candidate_failing_pattern_rejects() {
        let mut validation = base_validation();
        validation.pattern = Some("^[a-z]{3,5}$".to_string());

        assert!(validate_candidate_value(&Value::String("invalid-value".to_string()), &validation).is_err());
    }

    #[test]
    fn numeric_candidate_with_allowed_values_passes() {
        let mut validation = base_validation();
        validation.allowed_values = vec![Value::Number(serde_json::Number::from(42))];

        assert!(validate_candidate_value(&Value::Number(serde_json::Number::from(42)), &validation).is_ok());
        assert!(validate_candidate_value(&Value::String("42".to_string()), &validation).is_ok());
        assert!(validate_candidate_value(&Value::Number(serde_json::Number::from(7)), &validation).is_err());
    }

    #[test]
    fn non_string_candidate_rejected_when_text_rules_present() {
        let mut validation = base_validation();
        validation.min_length = Some(2);
        assert!(validate_candidate_value(&Value::Number(serde_json::Number::from(12)), &validation).is_err());
    }
}
