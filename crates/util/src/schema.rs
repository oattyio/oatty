use oatty_types::command::SchemaProperty;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub fn resolve_output_schema(maybe_schema: Option<&Value>, root: &Value) -> Option<SchemaProperty> {
    let schema = maybe_schema?;

    let schema_type = get_type(schema, root);
    let description = get_description(schema, root).unwrap_or_default();
    let resolved_map = resolve_schema_map(schema, root);

    let properties = resolved_map
        .and_then(|map| map.get("properties"))
        .and_then(Value::as_object)
        .map(|properties| {
            let mut collected: HashMap<String, Box<SchemaProperty>> = HashMap::new();
            for (key, value) in properties {
                if let Some(child) = resolve_output_schema(Some(value), root) {
                    collected.insert(key.to_string(), Box::new(child));
                }
            }
            collected
        })
        .filter(|props: &HashMap<String, Box<SchemaProperty>>| !props.is_empty());

    let items = if schema_type == "array" {
        resolve_array_items(schema, resolved_map, root).map(Box::new)
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
    if let Some(ptr) = schema.get("$ref").and_then(Value::as_str) {
        let ptr = ptr.strip_prefix('#').unwrap_or(ptr);
        return root.pointer(ptr).and_then(|t| get_description(t, root));
    }

    if let Some(desc) = schema.get("description").and_then(Value::as_str) {
        return Some(desc.to_string());
    }

    for key in ["anyOf", "oneOf"] {
        if let Some(arr) = schema.get(key).and_then(Value::as_array) {
            let descs: Vec<String> = arr.iter().filter_map(|item| get_description(item, root)).collect();
            if !descs.is_empty() {
                return Some(descs.join(" or "));
            }
        }
    }

    if let Some(arr) = schema.get("allOf").and_then(Value::as_array) {
        let descs: Vec<String> = arr.iter().filter_map(|item| get_description(item, root)).collect();
        if !descs.is_empty() {
            return Some(descs.join(" and "));
        }
    }

    None
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
    if let Some(ptr) = schema.get("$ref").and_then(Value::as_str) {
        let ptr = ptr.strip_prefix('#').unwrap_or(ptr);
        return root.pointer(ptr).map_or("string".to_string(), |t| get_type(t, root));
    }

    if let Some(ty) = schema.get("type") {
        if let Some(s) = ty.as_str() {
            return s.to_string();
        }
        if let Some(arr) = ty.as_array() {
            let types: HashSet<String> = arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
            let types: HashSet<_> = types.into_iter().filter(|t| t != "null").collect();
            if types.len() == 1 {
                return types.into_iter().next().unwrap();
            }
        }
    }

    for key in ["anyOf", "oneOf"] {
        if let Some(arr) = schema.get(key).and_then(Value::as_array) {
            let types: HashSet<String> = arr.iter().map(|item| get_type(item, root)).collect();
            if types.len() == 1 {
                return types.into_iter().next().unwrap();
            }
        }
    }

    "string".to_string()
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

fn resolve_array_items(schema: &Value, resolved_map: Option<&serde_json::Map<String, Value>>, root: &Value) -> Option<SchemaProperty> {
    let inline_items = schema.get("items");
    let resolved_items = resolved_map.and_then(|map| map.get("items"));
    let item_schema = inline_items.or(resolved_items)?;
    match item_schema {
        Value::Array(values) => values.first().and_then(|value| resolve_output_schema(Some(value), root)),
        other => resolve_output_schema(Some(other), root),
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
