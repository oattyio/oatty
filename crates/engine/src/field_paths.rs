//! Shared helpers for resolving field paths and leaf candidates.
//!
//! These utilities are used by both runtime selection flows (TUI collector)
//! and preflight/schema validation (MCP workflow validation) so behavior stays
//! consistent across surfaces.

use oatty_types::SchemaProperty;
use serde_json::Value;

/// Shared missing-path diagnostic details for `select.value_field`.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SelectValueFieldMissingDetails {
    /// Configured `select.value_field` path.
    pub configured_path: String,
    /// Final path segment used for leaf candidate search.
    pub leaf: String,
    /// Nested scalar candidate paths matching the leaf.
    pub nested_candidates: Vec<String>,
    /// Available top-level fields in the inspected payload/schema.
    pub available_fields: Vec<String>,
}

impl SelectValueFieldMissingDetails {
    /// Builds a runtime-facing collector error message.
    pub fn runtime_message(&self) -> String {
        if self.nested_candidates.len() > 1 {
            return format!(
                "value_field '{}' not found directly; found multiple nested '{}' candidates ({}). Set select.value_field to an explicit path or press F2 for manual entry.",
                self.configured_path,
                self.leaf,
                self.nested_candidates.join(", ")
            );
        }
        let available_fields = if self.available_fields.is_empty() {
            "none".to_string()
        } else {
            self.available_fields.join(", ")
        };
        format!(
            "value_field '{}' not found in provider row (fields: {}). Update select.value_field or press F2 for manual entry.",
            self.configured_path, available_fields
        )
    }

    /// Builds a validation-facing message for workflow preflight.
    pub fn validation_message(&self, input_name: &str) -> String {
        if self.nested_candidates.len() == 1 {
            return format!(
                "input '{}' select.value_field '{}' was not found; did you mean '{}'?",
                input_name,
                self.configured_path,
                self.nested_candidates.first().expect("single candidate")
            );
        }
        if self.nested_candidates.len() > 1 {
            return format!(
                "input '{}' select.value_field '{}' is ambiguous; nested '{}' candidates found: {}",
                input_name,
                self.configured_path,
                self.leaf,
                self.nested_candidates.join(", ")
            );
        }
        format!(
            "input '{}' select.value_field '{}' was not found in provider item schema",
            input_name, self.configured_path
        )
    }

    /// Builds a consistent suggested next step for missing-path diagnostics.
    pub fn suggested_next_step(&self) -> String {
        if self.nested_candidates.len() == 1 {
            return format!(
                "Set select.value_field to '{}' and rerun workflow.validate.",
                self.nested_candidates.first().expect("single candidate")
            );
        }
        if self.nested_candidates.len() > 1 {
            return format!(
                "Set select.value_field to an explicit scalar path (candidates: {}) and rerun workflow.validate.",
                self.nested_candidates.join(", ")
            );
        }
        if self.available_fields.is_empty() {
            return "Set select.value_field to a scalar field path and rerun workflow.validate.".to_string();
        }
        format!(
            "Set select.value_field to one of the available fields ({}) and rerun workflow.validate.",
            self.available_fields.join(", ")
        )
    }
}

/// Collect scalar leaf candidates from a JSON payload by leaf key name.
///
/// Returns tuples of `(path, value)` for every scalar value whose final segment
/// matches `leaf`.
pub fn nested_scalar_leaf_candidates_from_json(value: &Value, leaf: &str) -> Vec<(String, Value)> {
    let mut matches = Vec::new();
    collect_nested_scalar_leaf_candidates_from_json(value, "", leaf, &mut matches);
    matches
}

fn collect_nested_scalar_leaf_candidates_from_json(value: &Value, current_path: &str, leaf: &str, matches: &mut Vec<(String, Value)>) {
    match value {
        Value::Object(map) => {
            for (key, nested_value) in map {
                let next_path = if current_path.is_empty() {
                    key.to_string()
                } else {
                    format!("{current_path}.{key}")
                };
                if key == leaf && is_scalar_json_value(nested_value) {
                    matches.push((next_path.clone(), nested_value.clone()));
                }
                collect_nested_scalar_leaf_candidates_from_json(nested_value, &next_path, leaf, matches);
            }
        }
        Value::Array(items) => {
            for (index, nested_value) in items.iter().enumerate() {
                let next_path = if current_path.is_empty() {
                    index.to_string()
                } else {
                    format!("{current_path}.{index}")
                };
                collect_nested_scalar_leaf_candidates_from_json(nested_value, &next_path, leaf, matches);
            }
        }
        _ => {}
    }
}

/// Collect scalar leaf candidate paths from a schema by leaf key name.
pub fn nested_scalar_leaf_candidates_from_schema(schema: &SchemaProperty, leaf: &str) -> Vec<String> {
    let mut matches = Vec::new();
    collect_nested_scalar_leaf_candidates_from_schema(schema, String::new(), leaf, &mut matches);
    matches.sort();
    matches.dedup();
    matches
}

/// Builds missing-path details from a runtime JSON row payload.
pub fn missing_details_from_json_row(value: &Value, configured_path: &str, max_available_fields: usize) -> SelectValueFieldMissingDetails {
    let leaf = configured_path.split('.').next_back().unwrap_or(configured_path).to_string();
    let nested_candidates = nested_scalar_leaf_candidates_from_json(value, &leaf)
        .into_iter()
        .map(|(path, _)| path)
        .collect::<Vec<_>>();
    let mut available_fields = json_top_level_fields(value);
    if available_fields.len() > max_available_fields {
        available_fields.truncate(max_available_fields);
    }
    SelectValueFieldMissingDetails {
        configured_path: configured_path.to_string(),
        leaf,
        nested_candidates,
        available_fields,
    }
}

/// Builds missing-path details from a provider output schema.
pub fn missing_details_from_schema(schema: &SchemaProperty, configured_path: &str) -> SelectValueFieldMissingDetails {
    let leaf = configured_path.split('.').next_back().unwrap_or(configured_path).to_string();
    SelectValueFieldMissingDetails {
        configured_path: configured_path.to_string(),
        leaf: leaf.clone(),
        nested_candidates: nested_scalar_leaf_candidates_from_schema(schema, &leaf),
        available_fields: schema_top_level_fields(schema),
    }
}

fn collect_nested_scalar_leaf_candidates_from_schema(schema: &SchemaProperty, path_prefix: String, leaf: &str, output: &mut Vec<String>) {
    match schema.r#type.as_str() {
        "object" => {
            let Some(properties) = schema.properties.as_ref() else {
                return;
            };
            let mut keys = properties.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                let Some(property) = properties.get(&key) else {
                    continue;
                };
                let next_prefix = if path_prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{path_prefix}.{key}")
                };
                let property = property.as_ref();
                if key == leaf && !is_non_scalar_schema_type(property) {
                    output.push(next_prefix.clone());
                }
                collect_nested_scalar_leaf_candidates_from_schema(property, next_prefix, leaf, output);
            }
        }
        "array" => {
            let Some(item_schema) = schema.items.as_deref() else {
                return;
            };
            let next_prefix = if path_prefix.is_empty() {
                "[]".to_string()
            } else {
                format!("{path_prefix}[]")
            };
            collect_nested_scalar_leaf_candidates_from_schema(item_schema, next_prefix, leaf, output);
        }
        _ => {}
    }
}

/// Resolve a dotted path against a schema definition.
pub fn resolve_schema_path<'a>(schema: &'a SchemaProperty, path: &str) -> Option<&'a SchemaProperty> {
    if path.trim().is_empty() || path == "." {
        return Some(schema);
    }

    let mut current = schema;
    for segment in path.split('.').filter(|segment| !segment.is_empty()) {
        if current.r#type == "array"
            && let Some(item_schema) = current.items.as_deref()
        {
            if segment == "[]" || segment == "*" || segment.parse::<usize>().is_ok() {
                current = item_schema;
                continue;
            }
            current = item_schema;
        }

        if current.r#type != "object" {
            return None;
        }
        let properties = current.properties.as_ref()?;
        current = properties.get(segment)?.as_ref();
    }

    Some(current)
}

/// Returns sorted top-level object field names from the schema.
pub fn schema_top_level_fields(schema: &SchemaProperty) -> Vec<String> {
    if schema.r#type != "object" {
        return Vec::new();
    }
    let Some(properties) = schema.properties.as_ref() else {
        return Vec::new();
    };
    let mut fields = properties.keys().cloned().collect::<Vec<_>>();
    fields.sort();
    fields
}

/// Returns sorted top-level object field names from a JSON value.
pub fn json_top_level_fields(value: &Value) -> Vec<String> {
    let Value::Object(map) = value else {
        return Vec::new();
    };
    let mut fields = map.keys().cloned().collect::<Vec<_>>();
    fields.sort();
    fields
}

/// Returns true when the schema type is object or array.
pub fn is_non_scalar_schema_type(schema: &SchemaProperty) -> bool {
    matches!(schema.r#type.as_str(), "object" | "array")
}

/// Returns true when the JSON value is scalar (string/number/bool/null).
pub fn is_scalar_json_value(value: &Value) -> bool {
    matches!(value, Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null)
}

/// Builds a standard runtime message when a resolved path is non-scalar.
pub fn non_scalar_runtime_message(path: &str) -> String {
    format!("value_field '{path}' resolved to a non-scalar value. Select a scalar field or press F2 for manual entry.")
}

/// Builds a standard validation message when a resolved schema path is non-scalar.
pub fn non_scalar_validation_message(input_name: &str, path: &str, resolved_type: &str) -> String {
    format!(
        "input '{}' select.value_field '{}' resolves to a non-scalar schema type '{}'",
        input_name, path, resolved_type
    )
}

/// Suggested next step for non-scalar select path diagnostics.
pub fn non_scalar_suggested_next_step() -> String {
    "Set select.value_field to a scalar path (for example owner.id) and rerun workflow.validate.".to_string()
}

#[cfg(test)]
mod tests {
    use super::{nested_scalar_leaf_candidates_from_json, nested_scalar_leaf_candidates_from_schema};
    use oatty_types::SchemaProperty;
    use serde_json::json;
    use std::collections::HashMap;

    fn schema(ty: &str) -> SchemaProperty {
        SchemaProperty {
            r#type: ty.to_string(),
            description: String::new(),
            properties: None,
            required: Vec::new(),
            items: None,
            enum_values: Vec::new(),
            format: None,
            tags: Vec::new(),
        }
    }

    fn object_schema(properties: Vec<(&str, SchemaProperty)>) -> SchemaProperty {
        let mut map = HashMap::new();
        for (name, property) in properties {
            map.insert(name.to_string(), Box::new(property));
        }
        let mut root = schema("object");
        root.properties = Some(map);
        root
    }

    #[test]
    fn collects_nested_leaf_candidates_from_json() {
        let value = json!({
            "owner": { "id": "owner-1" },
            "team": { "id": "team-1" }
        });
        let matches = nested_scalar_leaf_candidates_from_json(&value, "id");
        let paths = matches.into_iter().map(|(path, _)| path).collect::<Vec<_>>();
        assert_eq!(paths, vec!["owner.id".to_string(), "team.id".to_string()]);
    }

    #[test]
    fn collects_nested_leaf_candidates_from_schema() {
        let schema = object_schema(vec![
            ("owner", object_schema(vec![("id", schema("string"))])),
            ("team", object_schema(vec![("id", schema("string"))])),
        ]);
        let matches = nested_scalar_leaf_candidates_from_schema(&schema, "id");
        assert_eq!(matches, vec!["owner.id".to_string(), "team.id".to_string()]);
    }
}
