//! Shared template parsing and diagnostics helpers.

use crate::resolve::{RunContext, resolve_template_expression_value};
use serde_json::Value;

/// Structured unresolved template reference diagnostic.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UnresolvedTemplateRef {
    /// Source path where the template was found.
    pub source_path: String,
    /// Raw template expression without delimiters.
    pub expression: String,
}

/// Extracts template expressions from a string value.
///
/// Returned expressions do not include `${{` or `}}` delimiters.
pub fn extract_template_expressions(value: &str) -> Vec<String> {
    let mut expressions = Vec::new();
    let mut remainder = value;

    while let Some(start) = remainder.find("${{") {
        let after_start = &remainder[start + 3..];
        let Some(end) = after_start.find("}}") else {
            break;
        };
        let expression = after_start[..end].trim();
        if !expression.is_empty() {
            expressions.push(expression.to_string());
        }
        remainder = &after_start[end + 2..];
    }

    expressions
}

/// Parses a step output reference expression and returns `(step_id, normalized_field_path)`.
///
/// Supports:
/// - `steps.step_id.field`
/// - `steps.step_id.output.field`
/// - `steps.step_id.0.field`
/// - `steps.step_id[0].field`
/// - `steps.step_id.items[0].id`
pub fn parse_step_reference_expression(expression: &str) -> Option<(String, String)> {
    let trimmed = expression.trim();
    if !trimmed.starts_with("steps.")
        || trimmed.contains("==")
        || trimmed.contains("!=")
        || trimmed.contains("&&")
        || trimmed.contains("||")
    {
        return None;
    }

    let remaining = trimmed.strip_prefix("steps.")?;
    let (step_id, path_part) = split_step_identifier_and_path(remaining)?;
    let normalized_path = normalize_reference_path(path_part)?;
    Some((step_id, normalized_path))
}

fn split_step_identifier_and_path(raw: &str) -> Option<(String, &str)> {
    let mut step_identifier = String::new();
    let mut split_index = None;

    for (index, character) in raw.char_indices() {
        if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
            step_identifier.push(character);
            continue;
        }
        if character == '.' || character == '[' {
            split_index = Some(index);
            break;
        }
        return None;
    }

    if step_identifier.is_empty() {
        return None;
    }

    let index = split_index.unwrap_or(raw.len());
    let remainder = &raw[index..];
    if remainder.is_empty() {
        return None;
    }

    Some((step_identifier, remainder))
}

fn normalize_reference_path(path: &str) -> Option<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut chars = path.chars().peekable();

    while let Some(character) = chars.next() {
        match character {
            '.' => {
                if !current.is_empty() {
                    segments.push(current.clone());
                    current.clear();
                }
            }
            '[' => {
                if !current.is_empty() {
                    segments.push(current.clone());
                    current.clear();
                }
                let mut inner = String::new();
                while let Some(next_character) = chars.peek().copied() {
                    chars.next();
                    if next_character == ']' {
                        break;
                    }
                    inner.push(next_character);
                }
                let bracket_segment = if inner.trim().is_empty() {
                    "[]".to_string()
                } else {
                    inner.trim().to_string()
                };
                segments.push(bracket_segment);
            }
            _ => current.push(character),
        }
    }

    if !current.is_empty() {
        segments.push(current);
    }

    if segments.first().map(|segment| segment == "output").unwrap_or(false) {
        segments.remove(0);
    }
    if segments.is_empty() {
        return None;
    }

    Some(segments.join("."))
}

/// Collect unresolved template references from an arbitrary JSON value tree.
pub fn collect_unresolved_templates_from_value(
    value: &Value,
    source_path: &str,
    context: &RunContext,
    unresolved: &mut Vec<UnresolvedTemplateRef>,
) {
    match value {
        Value::String(raw_text) => {
            for expression in extract_template_expressions(raw_text) {
                if resolve_template_expression_value(expression.as_str(), context).is_none() {
                    unresolved.push(UnresolvedTemplateRef {
                        source_path: source_path.to_string(),
                        expression,
                    });
                }
            }
        }
        Value::Array(values) => {
            for (index, nested_value) in values.iter().enumerate() {
                collect_unresolved_templates_from_value(nested_value, format!("{source_path}[{index}]").as_str(), context, unresolved);
            }
        }
        Value::Object(map) => {
            for (key, nested_value) in map {
                collect_unresolved_templates_from_value(nested_value, format!("{source_path}.{key}").as_str(), context, unresolved);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::parse_step_reference_expression;

    #[test]
    fn parse_step_reference_expression_supports_dot_index() {
        let parsed = parse_step_reference_expression("steps.find_render_service.0.service.id").expect("parsed");
        assert_eq!(parsed.0, "find_render_service");
        assert_eq!(parsed.1, "0.service.id");
    }

    #[test]
    fn parse_step_reference_expression_supports_bracket_index() {
        let parsed = parse_step_reference_expression("steps.find_render_service[0].service.id").expect("parsed");
        assert_eq!(parsed.0, "find_render_service");
        assert_eq!(parsed.1, "0.service.id");
    }

    #[test]
    fn parse_step_reference_expression_strips_output_segment() {
        let parsed = parse_step_reference_expression("steps.deploy.output.id").expect("parsed");
        assert_eq!(parsed.0, "deploy");
        assert_eq!(parsed.1, "id");
    }
}
