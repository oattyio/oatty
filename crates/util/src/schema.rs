use oatty_types::command::SchemaProperty;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

const MAX_SCHEMA_RESOLUTION_DEPTH: usize = 128;

#[derive(Default)]
struct SchemaResolutionContext {
    depth: usize,
    visited_references: HashSet<String>,
}

/// Resolves a schema definition into a `SchemaProperty` tree suitable for command output metadata.
///
/// This function safely handles recursive schemas by bounding recursion depth and short-circuiting
/// repeated `$ref` pointers in the active resolution path.
pub fn resolve_output_schema(maybe_schema: Option<&Value>, root: &Value) -> Option<SchemaProperty> {
    let mut context = SchemaResolutionContext::default();
    resolve_output_schema_internal(maybe_schema, root, &mut context)
}

fn resolve_output_schema_internal(
    maybe_schema: Option<&Value>,
    root: &Value,
    context: &mut SchemaResolutionContext,
) -> Option<SchemaProperty> {
    let schema = maybe_schema?;
    let schema_reference = extract_schema_reference(schema);

    with_resolution_frame(
        context,
        schema_reference,
        || Some(unresolved_schema_property()),
        |context| {
            let schema_type = get_type(schema, root);
            let description = get_description(schema, root).unwrap_or_default();
            let resolved_map = resolve_schema_map(schema, root);

            let properties = resolved_map
                .and_then(|map| map.get("properties"))
                .and_then(Value::as_object)
                .map(|properties| {
                    let mut collected: HashMap<String, Box<SchemaProperty>> = HashMap::new();
                    for (key, value) in properties {
                        if let Some(child) = resolve_output_schema_internal(Some(value), root, context) {
                            collected.insert(key.to_string(), Box::new(child));
                        }
                    }
                    collected
                })
                .filter(|properties: &HashMap<String, Box<SchemaProperty>>| !properties.is_empty());

            let items = if schema_type == "array" {
                resolve_array_items_internal(schema, resolved_map, root, context).map(Box::new)
            } else {
                None
            };

            let required = resolved_map
                .and_then(|map| map.get("required"))
                .and_then(Value::as_array)
                .map(|values| collect_string_values(values))
                .unwrap_or_default();

            let enum_values = resolve_enum_values(schema, resolved_map).unwrap_or_default();
            let format = resolve_format(schema, resolved_map);

            Some(SchemaProperty {
                r#type: schema_type,
                description,
                properties,
                required,
                items,
                enum_values,
                format,
                tags: Vec::new(),
            })
        },
    )
}

fn unresolved_schema_property() -> SchemaProperty {
    SchemaProperty {
        r#type: "string".to_string(),
        description: String::new(),
        properties: None,
        required: Vec::new(),
        items: None,
        enum_values: Vec::new(),
        format: None,
        tags: Vec::new(),
    }
}

fn extract_schema_reference(schema: &Value) -> Option<&str> {
    schema.as_str().or_else(|| schema.get("$ref").and_then(Value::as_str))
}

fn normalize_reference(reference: &str) -> String {
    reference.strip_prefix('#').unwrap_or(reference).to_string()
}

fn with_resolution_frame<T, FResolver, FFallback>(
    context: &mut SchemaResolutionContext,
    maybe_reference: Option<&str>,
    fallback: FFallback,
    resolver: FResolver,
) -> T
where
    FResolver: FnOnce(&mut SchemaResolutionContext) -> T,
    FFallback: FnOnce() -> T,
{
    if context.depth >= MAX_SCHEMA_RESOLUTION_DEPTH {
        return fallback();
    }

    let normalized_reference = maybe_reference.map(normalize_reference);
    if let Some(reference) = normalized_reference.as_ref()
        && !context.visited_references.insert(reference.clone())
    {
        return fallback();
    }

    context.depth += 1;
    let result = resolver(context);
    context.depth -= 1;

    if let Some(reference) = normalized_reference {
        context.visited_references.remove(&reference);
    }

    result
}

/// Recursively resolves the description from a schema, following `$ref` or combining `anyOf`/`oneOf`/`allOf`.
///
/// # Arguments
///
/// * `schema` - The schema JSON `Value`.
/// * `root` - The root JSON schema `Value`.
///
/// # Returns
///
/// An optional resolved description string.
pub fn get_description(schema: &Value, root: &Value) -> Option<String> {
    let mut context = SchemaResolutionContext::default();
    get_description_internal(schema, root, &mut context)
}

fn get_description_internal(schema: &Value, root: &Value, context: &mut SchemaResolutionContext) -> Option<String> {
    let schema_reference = schema.get("$ref").and_then(Value::as_str);

    with_resolution_frame(
        context,
        schema_reference,
        || None,
        |context| {
            if let Some(reference) = schema_reference {
                let pointer = normalize_reference(reference);
                return root
                    .pointer(&pointer)
                    .and_then(|target| get_description_internal(target, root, context));
            }

            if let Some(description) = schema.get("description").and_then(Value::as_str) {
                return Some(description.to_string());
            }

            for key in ["anyOf", "oneOf"] {
                if let Some(array) = schema.get(key).and_then(Value::as_array) {
                    let descriptions: Vec<String> = array
                        .iter()
                        .filter_map(|item| get_description_internal(item, root, context))
                        .collect();
                    if !descriptions.is_empty() {
                        return Some(descriptions.join(" or "));
                    }
                }
            }

            if let Some(array) = schema.get("allOf").and_then(Value::as_array) {
                let descriptions: Vec<String> = array
                    .iter()
                    .filter_map(|item| get_description_internal(item, root, context))
                    .collect();
                if !descriptions.is_empty() {
                    return Some(descriptions.join(" and "));
                }
            }

            None
        },
    )
}

/// Recursively resolves the type from a schema, handling `$ref`, direct types, or `anyOf`/`oneOf`.
///
/// # Arguments
///
/// * `schema` - The schema JSON `Value`.
/// * `root` - The root JSON schema `Value`.
///
/// # Returns
///
/// The resolved type string, defaulting to "string".
pub fn get_type(schema: &Value, root: &Value) -> String {
    let mut context = SchemaResolutionContext::default();
    get_type_internal(schema, root, &mut context)
}

fn get_type_internal(schema: &Value, root: &Value, context: &mut SchemaResolutionContext) -> String {
    let schema_reference = schema.get("$ref").and_then(Value::as_str);

    with_resolution_frame(
        context,
        schema_reference,
        || "string".to_string(),
        |context| {
            if let Some(reference) = schema_reference {
                let pointer = normalize_reference(reference);
                return root
                    .pointer(&pointer)
                    .map_or("string".to_string(), |target| get_type_internal(target, root, context));
            }

            if let Some(schema_type) = schema.get("type") {
                if let Some(schema_type_name) = schema_type.as_str() {
                    return schema_type_name.to_string();
                }
                if let Some(type_array) = schema_type.as_array() {
                    let types: HashSet<String> = type_array
                        .iter()
                        .filter_map(|value| value.as_str().map(str::to_string))
                        .filter(|type_name| type_name != "null")
                        .collect();
                    if types.len() == 1 {
                        return types.into_iter().next().unwrap_or_else(|| "string".to_string());
                    }
                }
            }

            for key in ["anyOf", "oneOf"] {
                if let Some(array) = schema.get(key).and_then(Value::as_array) {
                    let types: HashSet<String> = array.iter().map(|item| get_type_internal(item, root, context)).collect();
                    if types.len() == 1 {
                        return types.into_iter().next().unwrap_or_else(|| "string".to_string());
                    }
                }
            }

            "string".to_string()
        },
    )
}

fn resolve_schema_map<'a>(schema: &'a Value, root: &'a Value) -> Option<&'a serde_json::Map<String, Value>> {
    if let Some(reference) = schema.as_str() {
        let pointer = reference.strip_prefix('#').unwrap_or(reference);
        return root.pointer(pointer).and_then(Value::as_object);
    }

    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let pointer = reference.strip_prefix('#').unwrap_or(reference);
        return root.pointer(pointer).and_then(Value::as_object);
    }

    schema.as_object()
}

fn resolve_array_items_internal(
    schema: &Value,
    resolved_map: Option<&serde_json::Map<String, Value>>,
    root: &Value,
    context: &mut SchemaResolutionContext,
) -> Option<SchemaProperty> {
    let inline_items = schema.get("items");
    let resolved_items = resolved_map.and_then(|map| map.get("items"));
    let item_schema = inline_items.or(resolved_items)?;

    match item_schema {
        Value::Array(values) => values
            .first()
            .and_then(|value| resolve_output_schema_internal(Some(value), root, context)),
        other => resolve_output_schema_internal(Some(other), root, context),
    }
}

fn resolve_enum_values(schema: &Value, resolved_map: Option<&serde_json::Map<String, Value>>) -> Option<Vec<String>> {
    let from_schema = schema.get("enum");
    let from_resolved = resolved_map.and_then(|map| map.get("enum"));
    from_schema
        .or(from_resolved)
        .and_then(Value::as_array)
        .map(|values| collect_string_values(values))
}

fn resolve_format(schema: &Value, resolved_map: Option<&serde_json::Map<String, Value>>) -> Option<String> {
    schema
        .get("format")
        .or_else(|| resolved_map.and_then(|map| map.get("format")))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn collect_string_values(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .filter_map(|value| match value {
            Value::String(text) => Some(text.to_string()),
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(boolean) => Some(boolean.to_string()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolve_output_schema_handles_self_referential_refs() {
        let root = json!({
            "components": {
                "schemas": {
                    "Node": {
                        "type": "object",
                        "properties": {
                            "next": { "$ref": "#/components/schemas/Node" }
                        }
                    }
                }
            }
        });
        let schema = json!({ "$ref": "#/components/schemas/Node" });

        let resolved_schema = resolve_output_schema(Some(&schema), &root);

        assert!(resolved_schema.is_some());
        let resolved_schema = resolved_schema.expect("schema should resolve");
        assert_eq!(resolved_schema.r#type, "object");

        let next_property = resolved_schema
            .properties
            .and_then(|properties| properties.get("next").cloned())
            .expect("expected `next` property to be present");
        assert_eq!(next_property.r#type, "string");
    }

    #[test]
    fn resolve_output_schema_handles_mutual_recursive_refs() {
        let root = json!({
            "components": {
                "schemas": {
                    "A": {
                        "type": "object",
                        "properties": {
                            "b": { "$ref": "#/components/schemas/B" }
                        }
                    },
                    "B": {
                        "type": "object",
                        "properties": {
                            "a": { "$ref": "#/components/schemas/A" }
                        }
                    }
                }
            }
        });
        let schema = json!({ "$ref": "#/components/schemas/A" });

        let resolved_schema = resolve_output_schema(Some(&schema), &root);

        assert!(resolved_schema.is_some());
        let resolved_schema = resolved_schema.expect("schema should resolve");
        let b_property = resolved_schema
            .properties
            .and_then(|properties| properties.get("b").cloned())
            .expect("expected `b` property to be present");
        let a_property = b_property
            .properties
            .and_then(|properties| properties.get("a").cloned())
            .expect("expected `a` property to be present");

        assert_eq!(a_property.r#type, "string");
    }

    #[test]
    fn get_type_returns_string_when_ref_cycle_is_detected() {
        let root = json!({
            "components": {
                "schemas": {
                    "Node": { "$ref": "#/components/schemas/Node" }
                }
            }
        });
        let schema = json!({ "$ref": "#/components/schemas/Node" });

        let schema_type = get_type(&schema, &root);

        assert_eq!(schema_type, "string");
    }
}
