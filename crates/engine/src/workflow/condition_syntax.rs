//! Shared workflow condition normalization and validation utilities.
//!
//! The same condition syntax is used by step `if/when` and `repeat.until`.
//! This module centralizes wrapper normalization (`${{ ... }}`) and syntactic
//! validation so runtime conversion and manifest normalization do not drift.

use anyhow::{Result, bail};

/// Normalizes an optional condition string.
///
/// Trims whitespace, strips an outer `${{ ... }}` wrapper when present, and
/// returns `None` when the resulting expression is empty.
pub fn normalize_optional_condition_expression(raw_expression: Option<&str>) -> Option<String> {
    let raw_expression = raw_expression?;
    let normalized = normalize_condition_expression(raw_expression);
    if normalized.is_empty() { None } else { Some(normalized) }
}

/// Normalizes a condition string by trimming and unwrapping `${{ ... }}`.
pub fn normalize_condition_expression(raw_expression: &str) -> String {
    let trimmed = raw_expression.trim();
    if let Some(stripped) = trimmed.strip_prefix("${{") {
        let inner = stripped.trim();
        let inner = inner.strip_suffix("}}").unwrap_or(inner);
        inner.trim().to_string()
    } else {
        trimmed.to_string()
    }
}

/// Validates a workflow condition expression against supported syntax.
pub fn validate_condition_expression(expression: &str) -> Result<()> {
    let trimmed = expression.trim();
    if trimmed.is_empty() {
        bail!("expression cannot be empty");
    }

    if contains_operator(trimmed, "===") || contains_operator(trimmed, "!==") {
        bail!("strict equality operators are unsupported; use '==' or '!='");
    }
    if contains_operator(trimmed, ">=")
        || contains_operator(trimmed, "<=")
        || contains_operator(trimmed, ">")
        || contains_operator(trimmed, "<")
    {
        bail!("unsupported comparison operator; only '==', '!=', '&&', '||', '!' and '.includes(...)' are supported");
    }

    validate_condition_node(trimmed)
}

fn validate_condition_node(expression: &str) -> Result<()> {
    if let Some(parts) = split_expression(expression, "||") {
        for part in parts {
            validate_condition_node(part)?;
        }
        return Ok(());
    }
    if let Some(parts) = split_expression(expression, "&&") {
        for part in parts {
            validate_condition_node(part)?;
        }
        return Ok(());
    }

    let (_, inner) = strip_leading_negations(expression);
    let inner = inner.trim();
    if inner.is_empty() {
        bail!("expression cannot end with negation operator");
    }

    if let Some(includes_index) = find_top_level_operator(inner, ".includes(") {
        let (left_expression, right_expression_with_suffix) = inner.split_at(includes_index);
        let right_expression = right_expression_with_suffix.trim_start_matches(".includes(").trim();
        let right_expression = right_expression.strip_suffix(')').unwrap_or(right_expression).trim();
        if right_expression.is_empty() {
            bail!("includes expression is missing an argument");
        }
        validate_operand_expression(left_expression.trim())?;
        validate_operand_expression(right_expression)?;
        return Ok(());
    }

    if let Some(position) = find_top_level_operator(inner, "!=") {
        let left_expression = inner[..position].trim();
        let right_expression = inner[position + 2..].trim();
        if left_expression.is_empty() || right_expression.is_empty() {
            bail!("comparison expression must include both left and right operands");
        }
        validate_operand_expression(left_expression)?;
        validate_operand_expression(right_expression)?;
        return Ok(());
    }

    if let Some(position) = find_top_level_operator(inner, "==") {
        let left_expression = inner[..position].trim();
        let right_expression = inner[position + 2..].trim();
        if left_expression.is_empty() || right_expression.is_empty() {
            bail!("comparison expression must include both left and right operands");
        }
        validate_operand_expression(left_expression)?;
        validate_operand_expression(right_expression)?;
        return Ok(());
    }

    validate_operand_expression(inner)
}

fn validate_operand_expression(expression: &str) -> Result<()> {
    let trimmed = expression.trim();
    if trimmed.is_empty() {
        bail!("operand cannot be empty");
    }
    if is_output_root_expression(trimmed) {
        bail!("unsupported root 'output'; use 'steps.<step_id>' (optionally '.output')");
    }

    if looks_like_json_literal(trimmed) && serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
        return Ok(());
    }

    if !is_supported_path_expression(trimmed) {
        bail!(
            "unsupported expression '{}'; supported roots are env.*, inputs.*, and steps.*",
            trimmed
        );
    }

    Ok(())
}

fn is_output_root_expression(expression: &str) -> bool {
    expression == "output" || expression.starts_with("output.")
}

fn looks_like_json_literal(expression: &str) -> bool {
    let starts_like_number = expression
        .chars()
        .next()
        .map(|character| character == '-' || character.is_ascii_digit())
        .unwrap_or(false);
    expression.starts_with('[')
        || expression.starts_with('{')
        || expression.starts_with('"')
        || expression == "null"
        || expression == "true"
        || expression == "false"
        || starts_like_number
}

fn is_supported_path_expression(expression: &str) -> bool {
    if expression.contains(char::is_whitespace) {
        return false;
    }

    if let Some(environment_key) = expression.strip_prefix("env.") {
        return !environment_key.is_empty() && environment_key.chars().all(is_identifier_character);
    }

    if let Some(rest) = expression.strip_prefix("inputs.") {
        return validate_dot_path_segments(rest);
    }

    if let Some(rest) = expression.strip_prefix("steps.") {
        if rest.is_empty() {
            return false;
        }
        return validate_dot_path_segments(rest);
    }

    false
}

fn validate_dot_path_segments(path: &str) -> bool {
    path.split('.').all(validate_path_segment)
}

fn validate_path_segment(segment: &str) -> bool {
    if segment.is_empty() {
        return false;
    }

    let mut chars = segment.chars().peekable();
    let mut saw_base = false;
    while let Some(character) = chars.peek().copied() {
        if character == '[' {
            break;
        }
        if !is_identifier_character(character) {
            return false;
        }
        saw_base = true;
        chars.next();
    }

    if !saw_base {
        return false;
    }

    while let Some(character) = chars.next() {
        if character != '[' {
            return false;
        }
        let mut saw_digit = false;
        loop {
            let Some(next_character) = chars.next() else {
                return false;
            };
            if next_character == ']' {
                if !saw_digit {
                    return false;
                }
                break;
            }
            if !next_character.is_ascii_digit() {
                return false;
            }
            saw_digit = true;
        }
    }

    true
}

fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '-'
}

fn split_expression<'a>(expression: &'a str, operator: &str) -> Option<Vec<&'a str>> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let length = expression.len();

    while start < length {
        if let Some(relative_index) = find_top_level_operator(&expression[start..], operator) {
            let absolute_index = start + relative_index;
            let part = expression[start..absolute_index].trim();
            if !part.is_empty() {
                parts.push(part);
            }
            start = absolute_index + operator.len();
        } else {
            let part = expression[start..].trim();
            if !part.is_empty() {
                parts.push(part);
            }
            break;
        }
    }

    if parts.len() > 1 { Some(parts) } else { None }
}

fn strip_leading_negations(expression: &str) -> (usize, &str) {
    let mut count = 0usize;
    let mut remainder = expression.trim_start();
    while let Some(stripped) = remainder.strip_prefix('!') {
        if stripped.starts_with('=') {
            break;
        }
        count += 1;
        remainder = stripped.trim_start();
    }
    (count, remainder)
}

fn contains_operator(expression: &str, operator: &str) -> bool {
    find_top_level_operator(expression, operator).is_some()
}

fn find_top_level_operator(expression: &str, operator: &str) -> Option<usize> {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut depth = 0i32;
    let character_positions = expression.char_indices();

    for (index, character) in character_positions {
        match character {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
                continue;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
                continue;
            }
            '(' if !in_single_quote && !in_double_quote => {
                depth += 1;
                continue;
            }
            ')' if !in_single_quote && !in_double_quote => {
                if depth > 0 {
                    depth -= 1;
                }
                continue;
            }
            _ => {}
        }

        if !in_single_quote && !in_double_quote && depth == 0 && expression[index..].starts_with(operator) {
            return Some(index);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::validate_condition_expression;

    #[test]
    fn validate_condition_expression_handles_utf8_string_literals_without_panicking() {
        let result = validate_condition_expression("inputs.name == \"café\" && env.region == \"us\"");
        assert!(result.is_ok(), "expected utf8 literal to validate without panic");
    }
}
