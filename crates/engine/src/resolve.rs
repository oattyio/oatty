//! # Template Resolution and Expression Evaluation
//!
//! This module provides functionality for resolving template expressions and evaluating
//! conditional logic within workflow specifications. It supports the `${{ ... }}` template
//! syntax and provides a flexible expression evaluation system.
//!
//! ## Key Features
//!
//! - **Template Interpolation**: Replace `${{ ... }}` expressions with resolved values
//! - **Expression Evaluation**: Support for equality comparisons and logical operations
//! - **Context Resolution**: Access to environment variables, inputs, and step outputs
//! - **Path Navigation**: Navigate nested JSON structures using dot notation
//!
//! ## Template Syntax
//!
//! Templates use the `${{ ... }}` syntax and support several expression types:
//!
//! - `${{ env.VARIABLE_NAME }}` - Environment variable lookup
//! - `${{ inputs.input_name }}` - Workflow input value
//! - `${{ steps.step_id.output.field }}` - Step output field access
//! - `${{ expression == "value" }}` - Equality comparison
//!
//! ## Usage
//!
//! ```rust
//! use oatty_engine::resolve::{RunContext, interpolate_value, eval_condition};
//! use serde_json::json;
//!
//! let mut context = RunContext::default();
//! context.environment_variables.insert("APP_NAME".into(), "myapp".into());
//! context.inputs.insert("environment".into(), json!("production"));
//!
//! let value = json!({
//!     "name": "${{ env.APP_NAME }}",
//!     "env": "${{ inputs.environment }}"
//! });
//!
//! let interpolated = interpolate_value(&value, &context);
//! let should_run = eval_condition("inputs.environment == \"production\"", &context);
//! ```

use serde_json::Value;
use std::collections::HashMap;

/// Execution context for resolving workflow templates and expressions.
///
/// The run context provides access to all the data sources that can be
/// referenced in template expressions, including environment variables,
/// workflow inputs, and outputs from completed steps.
#[derive(Debug, Default, Clone)]
pub struct RunContext {
    /// Environment variables available to the workflow
    ///
    /// These variables are typically set by the execution environment
    /// or provided by the user when starting the workflow. They can
    /// include system information, configuration values, and secrets.
    pub environment_variables: HashMap<String, String>,

    /// Workflow input values resolved to JSON
    ///
    /// Inputs represent the parameters provided when executing the
    /// workflow. They can be simple values or complex objects, and
    /// are validated against their input specifications.
    pub inputs: serde_json::Map<String, Value>,

    /// Output values from completed workflow steps
    ///
    /// Each step in the workflow can produce output that becomes
    /// available to subsequent steps. The outputs are indexed by
    /// step ID and can contain any JSON-serializable data.
    pub steps: HashMap<String, Value>,
}

/// Recursively interpolates all template expressions in a JSON value.
///
/// This function processes JSON values at all levels, replacing any
/// `${{ ... }}` template expressions with their resolved values.
/// It handles strings, arrays, and objects recursively, ensuring
/// that all template expressions are properly resolved.
///
/// # Arguments
///
/// * `value` - The JSON value to interpolate
/// * `context` - The execution context containing available values
///
/// # Returns
///
/// Returns a new JSON value with all template expressions resolved.
///
/// # Examples
///
/// ```rust
/// use oatty_engine::resolve::{RunContext, interpolate_value};
/// use serde_json::json;
///
/// let mut context = RunContext::default();
/// context.environment_variables.insert("REGION".into(), "us".into());
///
/// let value = json!({
///     "region": "${{ env.REGION }}",
///     "nested": {
///         "value": "${{ env.REGION }}"
///     }
/// });
///
/// let result = interpolate_value(&value, &context);
/// assert_eq!(result["region"], "us");
/// assert_eq!(result["nested"]["value"], "us");
/// ```
pub fn interpolate_value(value: &Value, context: &RunContext) -> Value {
    match value {
        Value::String(string_value) => Value::String(interpolate_string(string_value, context)),
        Value::Array(array_values) => Value::Array(
            array_values
                .iter()
                .map(|array_value| interpolate_value(array_value, context))
                .collect(),
        ),
        Value::Object(object_map) => {
            let mut interpolated_map = serde_json::Map::new();
            for (key, value) in object_map.iter() {
                interpolated_map.insert(key.clone(), interpolate_value(value, context));
            }
            Value::Object(interpolated_map)
        }
        _ => value.clone(),
    }
}

/// Resolves a template expression into a JSON value using the run context.
///
/// This helper accepts expressions that appear inside `${{ ... }}` templates, including:
/// `env.*`, `inputs.*`, and `steps.*` (with optional `.output`).
pub fn resolve_template_expression_value(expression: &str, context: &RunContext) -> Option<Value> {
    let trimmed = expression.trim();
    if let Some(variable_name) = trimmed.strip_prefix("env.") {
        return context.environment_variables.get(variable_name).cloned().map(Value::String);
    }

    if let Some(remaining_expression) = trimmed.strip_prefix("inputs.") {
        let mut segments = remaining_expression.split('.');
        let input_name = segments.next()?.trim();
        let input_value = context.inputs.get(input_name)?;
        let remaining_path = segments.collect::<Vec<&str>>().join(".");
        return resolve_strict_json_path(input_value, remaining_path.as_str());
    }

    if let Some(remaining_expression) = trimmed.strip_prefix("steps.") {
        let mut segments = remaining_expression.split('.');
        let step_identifier = segments.next()?.trim();
        let step_output = context.steps.get(step_identifier)?;
        let mut remaining_segments = segments.collect::<Vec<&str>>();
        if remaining_segments.first().copied() == Some("output") {
            remaining_segments.remove(0);
        }
        let remaining_path = remaining_segments.join(".");
        return resolve_strict_json_path(step_output, remaining_path.as_str());
    }

    None
}

fn resolve_strict_json_path(value: &Value, path: &str) -> Option<Value> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Some(value.clone());
    }

    let mut current = value;
    for segment in trimmed.split('.').filter(|segment| !segment.is_empty()) {
        let (key, indices) = split_indices(segment);

        if !key.is_empty() {
            current = match current {
                Value::Object(map) => map.get(key)?,
                Value::Array(array_values) => {
                    let array_index = key.parse::<usize>().ok()?;
                    array_values.get(array_index)?
                }
                _ => return None,
            };
        }

        for index in indices {
            let Value::Array(array_values) = current else {
                return None;
            };
            current = array_values.get(index)?;
        }
    }

    Some(current.clone())
}

/// Finds unresolved context references (`env.*`, `inputs.*`, `steps.*`) inside a condition expression.
pub fn find_unresolved_references_in_condition(expression: &str, context: &RunContext) -> Vec<String> {
    let mut unresolved = Vec::new();
    for token in extract_reference_tokens(expression) {
        if resolve_template_expression_value(token.as_str(), context).is_none() {
            unresolved.push(token);
        }
    }
    unresolved.sort();
    unresolved.dedup();
    unresolved
}

/// Evaluates a conditional expression against the execution context.
///
/// This function supports simple conditional logic including equality
/// comparisons and truthiness checks. The expression syntax is designed
/// to be intuitive and safe, avoiding complex logic that could lead
/// to security issues.
///
/// # Arguments
///
/// * `expression` - The conditional expression to evaluate
/// * `context` - The execution context for resolving values
///
/// # Returns
///
/// Returns `true` if the condition evaluates to true, `false` otherwise.
///
/// # Supported Operations
///
/// - **Equality**: `left == "right"` - Compares resolved values
/// - **Truthiness**: `value` - Checks if a value is truthy
/// - **Path Resolution**: `inputs.field` - Resolves nested paths
///
/// # Examples
///
/// ```rust
/// use oatty_engine::resolve::{RunContext, eval_condition};
/// use serde_json::json;
///
/// let mut context = RunContext::default();
/// context.inputs.insert("environment".into(), json!("production"));
///
/// let result = eval_condition("inputs.environment == \"production\"", &context);
/// assert!(result);
///
/// let result = eval_condition("inputs.environment", &context);
/// assert!(result); // "production" is truthy
/// ```
pub fn eval_condition(expression: &str, context: &RunContext) -> bool {
    let trimmed = expression.trim();
    if trimmed.is_empty() {
        return false;
    }

    if let Some(parts) = split_expression(trimmed, "||") {
        return parts.into_iter().any(|part| eval_condition(part, context));
    }
    if let Some(parts) = split_expression(trimmed, "&&") {
        return parts.into_iter().all(|part| eval_condition(part, context));
    }

    let (negations, inner) = strip_leading_negations(trimmed);
    let inner = inner.trim();
    if inner.is_empty() {
        return negations % 2 == 0;
    }

    let mut value = evaluate_simple_condition(inner, context);
    if negations % 2 == 1 {
        value = !value;
    }
    value
}

/// Evaluates `[...].includes(expr)` style expressions.
///
/// Returns `Some(bool)` if the expression matches the `.includes(...)` shape,
/// otherwise `None` to allow other handlers to process it.
fn evaluate_includes(expression: &str, context: &RunContext) -> Option<bool> {
    let idx = expression.find(".includes(")?;
    let (left, right_with_paren) = expression.split_at(idx);
    let right = right_with_paren.trim_start_matches(".includes(").trim();
    let right = right.strip_suffix(')').unwrap_or(right).trim();

    let list_value = resolve_value_or_literal(left.trim(), context);
    let item_value = resolve_value_or_literal(right, context);

    let list = match list_value {
        Some(Value::Array(a)) => a,
        // If left resolves to a JSON string that looks like an array, try parse
        Some(Value::String(s)) if s.trim_start().starts_with('[') => serde_json::from_str::<Value>(&s)
            .ok()
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default(),
        _ => vec![],
    };

    let needle = match item_value {
        Some(v) => format_json_value(&v),
        None => String::new(),
    };
    Some(list.iter().any(|v| format_json_value(v) == needle))
}

/// Evaluates `left == right` style equality expressions.
/// Returns `Some(bool)` if an equality operator is present, otherwise `None`.
fn evaluate_comparison(expression: &str, context: &RunContext) -> Option<bool> {
    if let Some(position) = find_top_level_operator(expression, "!=") {
        let left_expression = expression[..position].trim();
        let right_expression = expression[position + 2..].trim();
        let left_value = resolve_value_or_literal(left_expression, context).unwrap_or(Value::Null);
        let right_value = resolve_value_or_literal(right_expression, context).unwrap_or(Value::Null);
        return Some(left_value != right_value);
    }

    if let Some(position) = find_top_level_operator(expression, "==") {
        let left_expression = expression[..position].trim();
        let right_expression = expression[position + 2..].trim();
        let left_value = resolve_value_or_literal(left_expression, context).unwrap_or(Value::Null);
        let right_value = resolve_value_or_literal(right_expression, context).unwrap_or(Value::Null);
        return Some(left_value == right_value);
    }

    None
}

/// Evaluates simple truthiness of an expression.
/// Truthy if the resolved string equals "true" or "1" or is non-empty.
fn evaluate_truthiness(expression: &str, context: &RunContext) -> bool {
    match resolve_expression(expression, context) {
        Some(resolved_value) => resolved_value == "true" || resolved_value == "1" || !resolved_value.is_empty(),
        None => false,
    }
}

fn evaluate_simple_condition(expression: &str, context: &RunContext) -> bool {
    if let Some(result) = evaluate_includes(expression, context) {
        return result;
    }
    if let Some(result) = evaluate_comparison(expression, context) {
        return result;
    }
    evaluate_truthiness(expression, context)
}

fn split_expression<'a>(expression: &'a str, operator: &str) -> Option<Vec<&'a str>> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let len = expression.len();

    while start < len {
        if let Some(relative) = find_top_level_operator(&expression[start..], operator) {
            let absolute = start + relative;
            let part = expression[start..absolute].trim();
            if !part.is_empty() {
                parts.push(part);
            }
            start = absolute + operator.len();
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

fn find_top_level_operator(expression: &str, operator: &str) -> Option<usize> {
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut depth = 0i32;
    let bytes = expression.as_bytes();
    let op_len = operator.len();
    let len = expression.len();

    let mut index = 0usize;
    while index + op_len <= len {
        let ch = bytes[index] as char;
        match ch {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
                index += 1;
                continue;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
                index += 1;
                continue;
            }
            '(' if !in_single_quote && !in_double_quote => {
                depth += 1;
                index += 1;
                continue;
            }
            ')' if !in_single_quote && !in_double_quote => {
                if depth > 0 {
                    depth -= 1;
                }
                index += 1;
                continue;
            }
            _ => {}
        }

        if !in_single_quote && !in_double_quote && depth == 0 && &expression[index..index + op_len] == operator {
            return Some(index);
        }

        index += 1;
    }

    None
}

/// Resolve an expression to a JSON value, supporting literals and context paths.
/// - JSON literals: numbers, strings (".."), arrays ([..]), objects ({..})
/// - `env.*`, `inputs.*`, `steps.*` paths
fn resolve_value_or_literal(expression: &str, context: &RunContext) -> Option<Value> {
    let trimmed = expression.trim();
    // JSON literal detection
    if looks_like_json_literal(trimmed)
        && let Ok(v) = serde_json::from_str::<Value>(trimmed)
    {
        return Some(v);
    }
    // Path-based resolution
    if let Some(v) = resolve_value(trimmed, context) {
        return Some(v);
    }
    if trimmed.starts_with("env.") || trimmed.starts_with("inputs.") || trimmed.starts_with("steps.") {
        return Some(Value::Null);
    }
    // Fallback: use stringy resolution
    resolve_expression(trimmed, context).map(Value::String)
}

fn extract_reference_tokens(expression: &str) -> Vec<String> {
    let chars: Vec<char> = expression.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0usize;
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while index < chars.len() {
        let character = chars[index];
        if character == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            index += 1;
            continue;
        }
        if character == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            index += 1;
            continue;
        }
        if in_single_quote || in_double_quote {
            index += 1;
            continue;
        }

        let remainder = &expression[char_to_byte_index(&chars, index)..];
        let prefix = if remainder.starts_with("steps.") {
            Some("steps.")
        } else if remainder.starts_with("inputs.") {
            Some("inputs.")
        } else if remainder.starts_with("env.") {
            Some("env.")
        } else {
            None
        };

        let Some(prefix_value) = prefix else {
            index += 1;
            continue;
        };

        let start = index;
        index += prefix_value.chars().count();
        while index < chars.len() {
            let character = chars[index];
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | '[' | ']') {
                index += 1;
            } else {
                break;
            }
        }
        if index > start {
            let start_byte = char_to_byte_index(&chars, start);
            let end_byte = char_to_byte_index(&chars, index);
            let mut token = expression[start_byte..end_byte].to_string();
            if index < chars.len() && chars[index] == '(' {
                token = strip_condition_method_suffix(token.as_str()).to_string();
            }
            if !token.is_empty() {
                tokens.push(token);
            }
        }
    }

    tokens
}

fn strip_condition_method_suffix(token: &str) -> &str {
    token.strip_suffix(".includes").unwrap_or(token)
}

fn char_to_byte_index(characters: &[char], character_index: usize) -> usize {
    characters.iter().take(character_index).map(|character| character.len_utf8()).sum()
}

/// Resolves a template-like expression into a raw JSON value when possible.
fn resolve_value(expression: &str, context: &RunContext) -> Option<Value> {
    // env.VAR -> String
    if let Some(var) = expression.strip_prefix("env.") {
        return context.environment_variables.get(var).map(|s| Value::String(s.clone()));
    }
    // inputs.* path
    if let Some(rest) = expression.strip_prefix("inputs.") {
        let (first_segment, remainder) = split_first_path_segment(rest);
        let (input_name, leading_indices) = split_indices(first_segment);
        let input_value = context.inputs.get(input_name)?;
        let combined_path = combine_indices_and_path(&leading_indices, remainder);
        return resolve_strict_json_path(input_value, combined_path.as_str());
    }
    // steps.* path with optional .output
    if let Some(rest) = expression.strip_prefix("steps.") {
        let (first_segment, remainder) = split_first_path_segment(rest);
        let (step_id, leading_indices) = split_indices(first_segment);
        let step_value = context.steps.get(step_id)?;
        let remainder = remainder.strip_prefix("output.").unwrap_or(remainder);
        let remainder = if remainder == "output" { "" } else { remainder };
        let combined_path = combine_indices_and_path(&leading_indices, remainder);
        return resolve_strict_json_path(step_value, combined_path.as_str());
    }
    None
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

fn split_first_path_segment(path: &str) -> (&str, &str) {
    let mut parts = path.splitn(2, '.');
    let first_segment = parts.next().unwrap_or("");
    let remainder = parts.next().unwrap_or("");
    (first_segment, remainder)
}

fn combine_indices_and_path(indices: &[usize], remainder: &str) -> String {
    let mut path = String::new();
    for index in indices {
        path.push_str(format!("[{index}]").as_str());
    }
    let trimmed_remainder = remainder.trim();
    if !trimmed_remainder.is_empty() {
        if !path.is_empty() {
            path.push('.');
        }
        path.push_str(trimmed_remainder);
    }
    path
}

/// Interpolates template expressions in a string using the provided context.
///
/// This function processes a string character by character, looking for
/// `${{ ... }}` template markers. When found, it resolves the expression
/// and substitutes the result. The function handles nested expressions
/// gracefully and preserves the original string if no templates are found.
///
/// # Arguments
///
/// * `input_string` - The string containing template expressions
/// * `context` - The execution context for resolving values
///
/// # Returns
///
/// Returns a new string with all template expressions resolved.
///
/// # Template Processing
///
/// The function processes templates sequentially, allowing for complex
/// nested expressions. If a template expression is malformed (missing
/// closing `}}`), the remaining text is preserved as-is.
fn interpolate_string(input_string: &str, context: &RunContext) -> String {
    let mut output_string = String::new();
    let mut remaining_string = input_string;

    while let Some(template_start) = remaining_string.find("${{") {
        let (string_before_template, string_after_template) = remaining_string.split_at(template_start);
        output_string.push_str(string_before_template);

        if let Some(template_end_index) = string_after_template.find("}}") {
            let template_expression = &string_after_template[3..template_end_index].trim();
            let resolved_value = resolve_expression(template_expression, context).unwrap_or_default();
            output_string.push_str(&resolved_value);
            remaining_string = &string_after_template[template_end_index + 2..];
        } else {
            // No closing template marker found, preserve the rest of the string and stop processing
            output_string.push_str(string_after_template);
            return output_string;
        }
    }

    // If no templates were processed, return the original string
    if output_string.is_empty() {
        input_string.to_string()
    } else {
        // Append any remaining string content
        output_string.push_str(remaining_string);
        output_string
    }
}

/// Resolves a template expression to a string value using the execution context.
///
/// This function supports several expression types and provides a unified
/// interface for accessing different data sources. It handles path navigation
/// through nested structures and provides sensible defaults for missing values.
///
/// # Arguments
///
/// * `expression` - The template expression to resolve
/// * `context` - The execution context containing available values
///
/// # Returns
///
/// Returns `Some(String)` if the expression can be resolved, `None` otherwise.
///
/// # Supported Expression Types
///
/// - **Environment Variables**: `env.VARIABLE_NAME`
/// - **Workflow Inputs**: `inputs.input_name[.path.to.field]`
/// - **Step Outputs**: `steps.step_id[.output].path.to.field`
///
/// # Path Navigation
///
/// For complex values, dot notation can be used to navigate nested structures.
/// Array access is supported using numeric indices. The function gracefully
/// handles missing paths by returning `None`.
fn resolve_expression(expression: &str, context: &RunContext) -> Option<String> {
    resolve_value(expression, context).map(|value| format_json_value(&value))
}

/// Navigates through a JSON value using a path of field names and array indices.
///
/// This function traverses nested JSON structures following the provided path.
/// It supports both object property access and array indexing, providing a
/// flexible way to extract values from complex data structures.
///
/// # Arguments
///
/// * `root_value` - The root JSON value to navigate from
/// * `path_parts` - Array of path segments (field names or array indices)
///
/// # Returns
///
/// Returns a string representation of the value at the specified path.
///
/// # Path Format
///
/// Path parts can be:
/// - Field names for object properties
/// - Numeric strings for array indices
/// - Any combination of the above for nested structures
///
/// # Examples
///
/// - `["user", "profile", "name"]` → `root.user.profile.name`
/// - `["items", "0", "id"]` → `root.items[0].id`
#[cfg(test)]
fn navigate_json_path(root_value: &Value, path_parts: &[&str]) -> String {
    let mut current_value = root_value;

    for path_part in path_parts {
        match current_value {
            Value::Object(object_map) => match object_map.get(*path_part) {
                Some(next_value) => current_value = next_value,
                None => return String::new(),
            },
            Value::Array(array_values) => {
                if let Ok(array_index) = path_part.parse::<usize>() {
                    current_value = array_values.get(array_index).unwrap_or(&Value::Null);
                } else {
                    return String::new();
                }
            }
            _ => return format_json_value(current_value),
        }
    }

    format_json_value(current_value)
}

/// Formats a JSON value as a string representation.
///
/// This function converts JSON values to their string representations
/// in a consistent and readable format. It handles all basic JSON types
/// and provides sensible defaults for complex or null values.
///
/// # Arguments
///
/// * `value` - The JSON value to format
///
/// # Returns
///
/// Returns a string representation of the JSON value.
///
/// # Formatting Rules
///
/// - **Strings**: Returned as-is
/// - **Numbers**: Converted to string representation
/// - **Booleans**: Converted to "true" or "false"
/// - **Null**: Converted to empty string
/// - **Objects/Arrays**: Converted to JSON string representation
///
/// Select a nested JSON value by a minimal dot path with optional numeric indices.
///
/// Supports segments like `a`, `a.b`, and array indices `a[0].b[1]`. Returns `None`
/// when any segment is missing or applied to the wrong JSON type. When `path` is
/// `None`, the input `value` is cloned and returned as-is.
pub fn select_path(value: &Value, path: Option<&str>) -> Option<Value> {
    let Some(path) = path else {
        return Some(value.clone());
    };
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Some(value.clone());
    }

    let mut current = value;
    for segment in trimmed.split('.') {
        if segment.is_empty() {
            continue;
        }
        let (key, indices) = split_indices(segment);
        if !key.is_empty() {
            current = current.get(key)?;
        }
        for idx in indices {
            current = current.get(idx)?;
        }
    }
    Some(current.clone())
}

fn split_indices(segment: &str) -> (&str, Vec<usize>) {
    let mut key_end = segment.len();
    let mut indices = Vec::new();
    let bytes = segment.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'[' {
            key_end = i;
            break;
        }
    }
    let key = &segment[..key_end];
    let mut i = key_end;
    while i < bytes.len() {
        if bytes[i] != b'[' {
            break;
        }
        i += 1; // skip [
        let start = i;
        while i < bytes.len() && bytes[i] != b']' {
            i += 1;
        }
        if i <= start {
            break;
        }
        if let Ok(n) = segment[start..i].parse::<usize>() {
            indices.push(n);
        }
        i += 1; // skip ]
    }
    (key, indices)
}

fn format_json_value(value: &Value) -> String {
    match value {
        Value::String(string_value) => string_value.clone(),
        Value::Number(number_value) => number_value.to_string(),
        Value::Bool(boolean_value) => boolean_value.to_string(),
        Value::Null => String::new(),
        other_value => other_value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn test_interpolate_inputs_and_steps() {
        let mut context = RunContext::default();
        context.environment_variables.insert("REGION".into(), "us".into());
        context.inputs.insert("app".into(), json!("myapp"));
        context.steps.insert(
            "create".into(),
            json!({
                "id": "app-123",
                "name": "myapp"
            }),
        );

        let value = json!({
            "name": "${{ inputs.app }}",
            "region": "${{ env.REGION }}",
            "ref1": "${{ steps.create.output.id }}",
            "ref2": "${{ steps.create.name }}"
        });

        let result = interpolate_value(&value, &context);

        assert_eq!(result["name"], "myapp");
        assert_eq!(result["region"], "us");
        assert_eq!(result["ref1"], "app-123");
        assert_eq!(result["ref2"], "myapp");
    }

    #[test]
    fn resolve_template_expression_value_returns_none_for_missing_step_path() {
        let mut context = RunContext::default();
        context.steps.insert("fetch".into(), json!([]));
        let resolved = resolve_template_expression_value("steps.fetch.value", &context);
        assert!(resolved.is_none());
    }

    #[test]
    fn unresolved_condition_references_are_detected() {
        let mut context = RunContext::default();
        context.steps.insert("fetch".into(), json!([]));
        let unresolved = find_unresolved_references_in_condition("steps.fetch.value != null", &context);
        assert_eq!(unresolved, vec!["steps.fetch.value".to_string()]);
    }

    #[test]
    fn resolve_template_expression_value_supports_dot_index_array_paths() {
        let mut context = RunContext::default();
        context.steps.insert(
            "find".into(),
            json!([{
                "service": { "id": "srv-123" }
            }]),
        );
        let resolved = resolve_template_expression_value("steps.find.0.service.id", &context).expect("resolved");
        assert_eq!(resolved, json!("srv-123"));
    }

    #[test]
    fn unresolved_condition_references_ignore_valid_dot_index_array_paths() {
        let mut context = RunContext::default();
        context.steps.insert(
            "find".into(),
            json!([{
                "service": { "id": "srv-123" }
            }]),
        );
        let unresolved = find_unresolved_references_in_condition("steps.find.0.service.id != null", &context);
        assert!(unresolved.is_empty());
    }

    #[test]
    fn unresolved_condition_references_support_hyphenated_step_ids() {
        let mut context = RunContext::default();
        context
            .steps
            .insert("find-render-service".into(), json!([{ "service": { "id": "srv-123" } }]));
        let unresolved = find_unresolved_references_in_condition("steps.find-render-service.0.service.id != null", &context);
        assert!(unresolved.is_empty());
    }

    #[test]
    fn unresolved_condition_references_ignore_string_literals() {
        let context = RunContext::default();
        let unresolved = find_unresolved_references_in_condition("\"steps.fake.path\" == \"steps.other.path\"", &context);
        assert!(unresolved.is_empty());
    }

    #[test]
    fn unresolved_condition_references_ignore_includes_method_suffix() {
        let mut context = RunContext::default();
        context.inputs.insert("tags".into(), json!(["prod", "staging"]));
        let unresolved = find_unresolved_references_in_condition("inputs.tags.includes(\"prod\")", &context);
        assert!(unresolved.is_empty());
    }

    #[test]
    fn test_eval_condition_equality() {
        let mut context = RunContext::default();
        context.inputs.insert("environment".into(), json!("production"));

        let result = eval_condition("inputs.environment == \"production\"", &context);
        assert!(result);

        let result = eval_condition("inputs.environment == \"development\"", &context);
        assert!(!result);
    }

    #[test]
    fn test_eval_condition_truthiness() {
        let mut context = RunContext::default();
        context.inputs.insert("enabled".into(), json!("true"));
        context.inputs.insert("empty".into(), json!(""));

        let result = eval_condition("inputs.enabled", &context);
        assert!(result);

        let result = eval_condition("inputs.empty", &context);
        assert!(!result);
    }

    #[test]
    fn test_eval_condition_boolean_operators_and_negation() {
        let mut context = RunContext::default();
        context.inputs.insert("flag".into(), json!(true));
        context.inputs.insert("other".into(), json!("value"));
        context.inputs.insert("empty".into(), Value::Null);
        context.inputs.insert("count".into(), json!(0));

        assert!(eval_condition("inputs.flag && inputs.other", &context));
        assert!(!eval_condition("inputs.flag && !inputs.other", &context));
        assert!(eval_condition("inputs.flag || inputs.empty", &context));
        assert!(eval_condition("inputs.empty || inputs.other", &context));
        assert!(eval_condition("!!inputs.other", &context));
        assert!(eval_condition("!inputs.empty", &context));
        assert!(eval_condition("inputs.count != null", &context));
        assert!(eval_condition("inputs.other == \"value\"", &context));
        assert!(!eval_condition("inputs.other != \"value\"", &context));
    }

    #[test]
    fn test_navigate_json_path_object() {
        let value = json!({
            "user": {
                "profile": {
                    "name": "John Doe",
                    "email": "john@example.com"
                }
            }
        });

        let result = navigate_json_path(&value, &["user", "profile", "name"]);
        assert_eq!(result, "John Doe");

        let result = navigate_json_path(&value, &["user", "profile", "email"]);
        assert_eq!(result, "john@example.com");
    }

    #[test]
    fn test_navigate_json_path_array() {
        let value = json!({
            "items": [
                {"id": "1", "name": "Item 1"},
                {"id": "2", "name": "Item 2"}
            ]
        });

        let result = navigate_json_path(&value, &["items", "0", "name"]);
        assert_eq!(result, "Item 1");

        let result = navigate_json_path(&value, &["items", "1", "id"]);
        assert_eq!(result, "2");
    }

    #[test]
    fn test_navigate_json_path_missing() {
        let value = json!({
            "user": {
                "name": "John"
            }
        });

        let result = navigate_json_path(&value, &["user", "profile", "email"]);
        assert_eq!(result, "");

        let result = navigate_json_path(&value, &["missing", "field"]);
        assert_eq!(result, "");
    }

    #[test]
    fn test_format_json_value_types() {
        assert_eq!(format_json_value(&json!("hello")), "hello");
        assert_eq!(format_json_value(&json!(42)), "42");
        assert_eq!(format_json_value(&json!(true)), "true");
        assert_eq!(format_json_value(&json!(false)), "false");
        assert_eq!(format_json_value(&json!(null)), "");
        assert_eq!(format_json_value(&json!({"key": "value"})), r#"{"key":"value"}"#);
    }

    #[test]
    fn test_interpolate_string_complex() {
        let mut context = RunContext::default();
        context.environment_variables.insert("ENV".into(), "prod".into());
        context.inputs.insert("app".into(), json!("myapp"));

        let input = "Deploy ${{ inputs.app }} to ${{ env.ENV }} environment";
        let result = interpolate_string(input, &context);

        assert_eq!(result, "Deploy myapp to prod environment");
    }

    #[test]
    fn test_interpolate_string_malformed() {
        let context = RunContext::default();

        let input = "Value: ${{ inputs.name";
        let result = interpolate_string(input, &context);

        // Malformed template should preserve the original text
        assert_eq!(result, "Value: ${{ inputs.name");
    }

    #[test]
    fn test_interpolate_string_nested() {
        let mut context = RunContext::default();
        context.inputs.insert("greeting".into(), json!("Hello"));
        context.inputs.insert("name".into(), json!("World"));

        let input = "${{ inputs.greeting }}, ${{ inputs.name }}!";
        let result = interpolate_string(input, &context);

        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn test_eval_condition_includes_with_literal_array() {
        let mut context = RunContext::default();
        context.steps.insert("build".into(), json!({"status": "succeeded"}));
        let cond = "[\"succeeded\",\"failed\"].includes(steps.build.status)";
        assert!(eval_condition(cond, &context));
        context.steps.insert("build".into(), json!({"status": "pending"}));
        assert!(!eval_condition(cond, &context));
    }

    #[test]
    fn test_eval_condition_includes_with_input_array() {
        let mut context = RunContext::default();
        context.inputs.insert("perms".into(), json!(["view", "deploy"]));
        assert!(eval_condition("inputs.perms.includes(\"deploy\")", &context));
        assert!(!eval_condition("inputs.perms.includes(\"manage\")", &context));
    }

    #[test]
    fn test_eval_condition_supports_signed_and_decimal_literals() {
        let mut context = RunContext::default();
        context.inputs.insert("delta".into(), json!(-1));
        context.inputs.insert("ratio".into(), json!(1.5));

        assert!(eval_condition("inputs.delta == -1", &context));
        assert!(eval_condition("inputs.ratio == 1.5", &context));
        assert!(eval_condition("inputs.ratio != 2.0", &context));
    }

    #[test]
    fn test_eval_condition_supports_bracket_index_paths() {
        let mut context = RunContext::default();
        context.steps.insert(
            "fetch".into(),
            json!({
                "items": [
                    { "id": "service-123" }
                ]
            }),
        );

        assert!(eval_condition("steps.fetch.items[0].id == \"service-123\"", &context));
        assert!(eval_condition("steps.fetch.items[0].id", &context));
    }
}
