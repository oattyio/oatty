use anyhow::{Context, Result};
use heck::ToKebabCase;
use heroku_types::{CommandFlag, CommandSpec, HttpCommandSpec, PositionalArgument, ServiceId};
use heroku_util::sort_and_dedup_commands;
use percent_encoding::percent_decode_str;
use serde_json::Value;
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

/// Generates command specifications from a JSON schema string.
///
/// Parses the JSON schema and derives command specifications, including synthetic workflow commands.
///
/// # Arguments
///
/// * `schema_json` - The JSON schema as a string.
///
/// # Errors
///
/// Returns an error if JSON parsing or command derivation fails.
///
/// # Returns
///
/// A vector of `CommandSpec` instances.
pub fn generate_commands(schema_json: &str, service_id: ServiceId) -> Result<Vec<CommandSpec>> {
    let value: Value = serde_json::from_str(schema_json).context("Failed to parse schema JSON")?;
    derive_commands_from_schema(&value, service_id)
}

/// Derives command specifications from a JSON schema `Value`.
///
/// Traverses the schema to extract command details from "links" sections, constructs `CommandSpec`
/// instances, and sorts/deduplicates them by name, method, and path.
///
/// # Arguments
///
/// * `value` - The root JSON schema `Value`.
///
/// # Errors
///
/// Returns an error if schema traversal or extraction fails.
///
/// # Returns
///
/// A sorted, deduplicated vector of `CommandSpec` instances.
pub fn derive_commands_from_schema(value: &Value, service_id: ServiceId) -> Result<Vec<CommandSpec>> {
    let mut commands = Vec::new();
    let mut command_names = HashMap::new();

    // Recursively collect nodes containing "links"
    fn collect_links<'a>(val: &'a Value, out: &mut Vec<&'a Value>) {
        match val {
            Value::Object(map) => {
                if map.contains_key("links") {
                    out.push(val);
                }
                map.values().for_each(|v| collect_links(v, out));
            }
            Value::Array(arr) => arr.iter().for_each(|v| collect_links(v, out)),
            _ => {}
        }
    }

    let mut nodes = Vec::new();
    collect_links(value, &mut nodes);

    for node in nodes {
        let Some(links) = node.get("links").and_then(Value::as_array) else {
            continue;
        };

        for link in links {
            let Some(href) = link.get("href").and_then(Value::as_str) else {
                continue;
            };
            let Some(method) = link.get("method").and_then(Value::as_str) else {
                continue;
            };
            let title = link.get("title").and_then(Value::as_str).unwrap_or("").to_string();
            let description = link.get("description").and_then(Value::as_str).unwrap_or(&title).to_string();

            if let Some((_, action)) = classify_command(href, method) {
                let (path_template, positional_args) = path_and_vars_with_help(href, value);
                let (flags, _required_names) = extract_flags_resolved(link, value);
                let ranges = extract_ranges(link);

                if path_template.is_empty() {
                    continue;
                }
                let (mut group, mut name) = derive_command_group_and_name(href, &action);
                if command_names.insert(format!("{}{}", &group, &name), true).is_some() {
                    (group, name) = derive_command_group_and_name(href, &title.to_kebab_case());
                    if command_names.contains_key(&format!("{}{}", &group, &name)) {
                        (group, name) = derive_command_group_and_name(href, &method.to_lowercase());
                    }
                }

                let http_spec = HttpCommandSpec::new(method.to_string(), path_template, service_id, ranges);
                commands.push(CommandSpec::new_http(group, name, description, positional_args, flags, http_spec));
            }
        }
    }
    sort_and_dedup_commands(&mut commands);
    // Two-pass provider resolution: build all commands, then resolve providers.
    // This enables 100% confidence verification using the constructed index.
    super::provider_resolver::resolve_and_infer_providers(&mut commands);
    Ok(commands)
}

/// Classifies a command based on its `href` and HTTP method.
///
/// Determines the group and action (e.g., "info", "list", "create") based on path structure and method.
///
/// # Arguments
///
/// * `href` - The API endpoint path.
/// * `method` - The HTTP method.
///
/// # Returns
///
/// An optional tuple of group and action, or `None` if unclassifiable.
fn classify_command(href: &str, method: &str) -> Option<(String, String)> {
    let segments: Vec<&str> = href.trim_start_matches('/').split('/').collect();
    if segments.is_empty() {
        return None;
    }

    let group = segments
        .iter()
        .rev()
        .find(|seg| !seg.starts_with('{'))
        .map_or(segments[0], |seg| seg)
        .to_string();

    let group = if group == "config-vars" { "config".into() } else { group };
    let is_resource = segments.last().map(|s| s.starts_with('{')) == Some(true);
    let ends_with_collection = segments.last().map(|s| !s.starts_with('{')) == Some(true);

    let action = match method {
        "GET" if is_resource => "info",
        "GET" if ends_with_collection => "list",
        "POST" => "create",
        "PATCH" => "update",
        "DELETE" => "delete",
        _ => return None,
    };

    Some((group, action.to_string()))
}

/// Extracts concrete segments from an `href`.
///
/// Filters out empty or placeholder segments (e.g., `{id}`).
///
/// # Arguments
///
/// * `href` - The API endpoint path.
///
/// # Returns
///
/// A vector of concrete segment strings.
fn concrete_segments(href: &str) -> Vec<String> {
    href.trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty() && !s.starts_with('{') && !is_version_segment(s))
        .map(str::to_string)
        .collect()
}

/// Detects simple API version path segments like `v1`, `v2`, etc.
fn is_version_segment(s: &str) -> bool {
    let s = s.trim();
    s.len() > 1 && s.starts_with('v') && s[1..].chars().all(|c| c.is_ascii_digit())
}

/// Normalizes a group name, handling special cases like "config-vars".
///
/// # Arguments
///
/// * `group` - The group name.
///
/// # Returns
///
/// The normalized group name.
fn normalize_group(group: &str) -> String {
    if group == "config-vars" {
        "config".to_string()
    } else {
        group.to_string()
    }
}

/// Derives the command group and name from `href` and action.
///
/// # Arguments
///
/// * `href` - The API endpoint path.
/// * `action` - The action string.
///
/// # Returns
///
/// A tuple of group and name.
fn derive_command_group_and_name(href: &str, action: &str) -> (String, String) {
    let segments = concrete_segments(href);
    if segments.is_empty() {
        return ("misc".to_string(), action.to_string());
    }

    let group = normalize_group(&segments[0]);
    let name = if segments.len() > 1 {
        format!("{}:{}", segments[1..].join(":"), action)
    } else {
        action.to_string()
    };

    (group, name)
}

/// Extracts the path template and positional arguments with help descriptions.
///
/// Processes `href` to identify placeholders, singularizes names, and resolves descriptions
/// recursively from the schema.
///
/// # Arguments
///
/// * `href` - The API endpoint path.
/// * `root` - The root JSON schema `Value`.
///
/// # Returns
///
/// A tuple of path template and vector of positional arguments.
fn path_and_vars_with_help(href: &str, root: &Value) -> (String, Vec<PositionalArgument>) {
    fn sanitize_placeholder_name(name: &str) -> String {
        name.trim_matches(|c: char| matches!(c, '{' | '}' | ' ' | '(' | ')'))
            .replace('-', "_")
    }

    fn decode_percent(s: &str) -> String {
        percent_decode_str(s).decode_utf8_lossy().into_owned()
    }

    fn ref_name_from_pointer(ptr: &str) -> Option<String> {
        // Expect formats like "#/definitions/team" or "#/definitions/team/definitions/identity"
        let trimmed = ptr.strip_prefix('#')?;
        let parts: Vec<&str> = trimmed.trim_start_matches('/').split('/').collect();
        // Find the first occurrence of "definitions" and take the next token as the canonical name
        for i in 0..parts.len() {
            if parts[i] == "definitions" && i + 1 < parts.len() {
                return Some(singularize(parts[i + 1]));
            }
        }
        // Fallback: last token
        parts.last().map(|s| singularize(s))
    }

    let mut args = Vec::new();
    let mut segments_out = Vec::new();
    let mut prev_concrete: Option<&str> = None; // last non-placeholder, non-version

    for seg in href.trim_start_matches('/').split('/') {
        if seg.starts_with('{') && seg.ends_with('}') {
            // Extract inner placeholder content and try to decode percent-encoding
            let inner = &seg[1..seg.len() - 1];
            let decoded_inner = decode_percent(inner);
            let decoded_inner_stripped = decoded_inner.trim();
            let decoded_inner_stripped = decoded_inner_stripped
                .strip_prefix('(')
                .and_then(|s| s.strip_suffix(')'))
                .unwrap_or(decoded_inner_stripped);

            let (ref_based_name, ref_help) = if decoded_inner_stripped.starts_with('#') {
                let rn = ref_name_from_pointer(decoded_inner_stripped);
                // Try to resolve description from the referenced definition
                let help = decoded_inner_stripped
                    .strip_prefix('#')
                    .and_then(|ptr| root.pointer(ptr))
                    .and_then(|target| get_description(target, root));
                (rn, help)
            } else {
                (None, None)
            };

            let placeholder = sanitize_placeholder_name(decoded_inner_stripped);
            let arg_name = if let Some(rn) = ref_based_name {
                rn
            } else if placeholder == "id" {
                // Prefer deriving a friendly name from the previous resource segment
                // but skip version markers like v1, v2, etc.
                prev_concrete
                    .filter(|p| !is_version_segment(p))
                    .map(singularize)
                    .unwrap_or_else(|| "id".to_string())
            } else {
                placeholder
            };

            let arg_name_for_path = arg_name.clone();
            args.push(PositionalArgument {
                name: arg_name,
                help: ref_help,
                provider: None,
            });
            // Use the derived arg name for the path template to normalize encoded refs
            segments_out.push(format!("{{{}}}", arg_name_for_path));
        } else {
            segments_out.push(seg.to_string());
            if !is_version_segment(seg) && !seg.is_empty() {
                prev_concrete = Some(seg);
            }
        }
    }

    (format!("/{}", segments_out.join("/")), args)
}

/// Singularizes a segment name by removing plural 's' and replacing '-' with '_'.
///
/// # Arguments
///
/// * `segment` - The segment string.
///
/// # Returns
///
/// The singularized name.
fn singularize(segment: &str) -> String {
    let s = segment.trim_matches(|c: char| matches!(c, '{' | '}' | ' '));
    let s = s.replace('-', "_");
    if s.ends_with('s') && s.len() > 1 {
        s[..s.len() - 1].to_string()
    } else {
        s
    }
}

// No pointer extraction in strict draft-04 mode.

/// Extracts range fields from a link schema.
///
/// # Arguments
///
/// * `link` - The link JSON `Value`.
///
/// # Returns
///
/// A vector of range field names.
fn extract_ranges(link: &Value) -> Vec<String> {
    link.get("ranges")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

/// Extracts flags and required names from a link schema, resolving properties recursively.
///
/// Handles `$ref`, `anyOf`, etc., for type, description, enum values, and defaults. Generates
/// short flags and adds range-related flags if ranges are supported, ensuring no duplicate flag names.
///
/// # Arguments
///
/// * `link` - The link JSON `Value`.
/// * `root` - The root JSON schema `Value`.
///
/// # Returns
///
/// A tuple of vector of `CommandFlag` and vector of required names.
fn extract_flags_resolved(link: &Value, root: &Value) -> (Vec<CommandFlag>, Vec<String>) {
    let mut flags = Vec::new();

    // Extract required names and properties from schema
    let Some(schema) = link.get("schema") else {
        return (add_range_flags(&extract_ranges(link)), Vec::new());
    };

    // Resolve top-level $ref and flatten simple combinators for properties/required
    let schema_merged = resolve_schema_properties(schema, root);
    let required_names: Vec<String> = collect_required(&schema_merged);

    if let Some(props) = collect_properties(&schema_merged) {
        flags = props
            .iter()
            .map(|(name, val)| {
                let merged = resolve_schema_properties(val, root);
                let ty = get_type(&merged, root);
                let required = required_names.contains(name);
                let enum_values = get_enum_values(&merged, root);
                let default_value = get_default(&merged, root).or_else(|| enum_values.first().cloned());
                let description = get_description(&merged, root);

                CommandFlag {
                    name: name.clone(),
                    short_name: name.chars().next().filter(|c| c.is_alphabetic()).map(|c| c.to_string()),
                    required,
                    r#type: ty,
                    enum_values,
                    default_value,
                    description,
                    provider: None,
                }
            })
            .collect();
    }

    // Combine schema flags with range flags, avoiding duplicates
    let mut seen_names = flags.iter().map(|f| f.name.clone()).collect::<HashSet<_>>();
    let range_flags = add_range_flags(&extract_ranges(link))
        .into_iter()
        .filter(|f| seen_names.insert(f.name.clone()))
        .collect::<Vec<_>>();

    flags.extend(range_flags);
    flags.sort_by(|a, b| {
        if a.required && b.required {
            return a.name.cmp(&b.name);
        }
        if a.required {
            return Ordering::Less;
        }
        if b.required {
            return Ordering::Greater;
        }
        Ordering::Equal
    });
    (flags, required_names)
}

/// Resolves schema properties by merging `$ref` target properties into the schema.
///
/// # Arguments
///
/// * `schema` - The schema JSON `Value`.
/// * `root` - The root JSON schema `Value`.
///
/// # Returns
///
/// A merged `Value` with resolved properties.
fn resolve_schema_properties(schema: &Value, root: &Value) -> Value {
    let mut merged = schema.clone();
    if let Some(ptr) = schema.get("$ref").and_then(Value::as_str) {
        let ptr = ptr.strip_prefix('#').unwrap_or(ptr);
        if let Some(target) = root.pointer(ptr) {
            let merged_obj = merged.as_object_mut().unwrap();
            let target_obj = target.as_object().cloned().unwrap_or(serde_json::Map::new());
            for (key, value) in target_obj {
                if !merged_obj.contains_key(&key) {
                    merged_obj.insert(key.clone(), value.clone());
                }
            }
        }
    }
    merged
}

/// Collects properties from a schema, optionally flattening simple anyOf/oneOf/allOf.
fn collect_properties(schema: &Value) -> Option<serde_json::Map<String, Value>> {
    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        return Some(props.clone());
    }
    // Flatten first-level combinators by merging properties
    for key in ["allOf", "anyOf", "oneOf"] {
        if let Some(arr) = schema.get(key).and_then(Value::as_array) {
            let mut out = serde_json::Map::new();
            for item in arr {
                if let Some(props) = item.get("properties").and_then(Value::as_object) {
                    for (k, v) in props {
                        out.entry(k.clone()).or_insert_with(|| v.clone());
                    }
                }
            }
            if !out.is_empty() {
                return Some(out);
            }
        }
    }
    None
}

/// Collects required fields, including from simple allOf/anyOf/oneOf.
fn collect_required(schema: &Value) -> Vec<String> {
    let mut out: Vec<String> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|req| req.iter().filter_map(|r| r.as_str().map(str::to_string)).collect())
        .unwrap_or_default();

    for key in ["allOf", "anyOf", "oneOf"] {
        if let Some(arr) = schema.get(key).and_then(Value::as_array) {
            for item in arr {
                out.extend(
                    item.get("required")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flat_map(|req| req.iter().filter_map(|r| r.as_str().map(str::to_string))),
                );
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Adds range-related flags if ranges are supported.
///
/// # Arguments
///
/// * `ranges` - The vector of range field names.
///
/// # Returns
///
/// A vector of range-related `CommandFlag` instances.
fn add_range_flags(ranges: &[String]) -> Vec<CommandFlag> {
    if ranges.is_empty() {
        return Vec::new();
    }
    vec![
        CommandFlag {
            name: "range-field".to_string(),
            short_name: Some("r".to_string()),
            required: false,
            r#type: "string".to_string(),
            enum_values: ranges.to_vec(),
            default_value: Some(ranges[0].clone()),
            description: Some("Field to use for range-based pagination".to_string()),
            provider: None,
        },
        CommandFlag {
            name: "range-start".to_string(),
            short_name: Some("s".to_string()),
            required: false,
            r#type: "string".to_string(),
            enum_values: vec![],
            default_value: None,
            description: Some("Start value for range (inclusive)".to_string()),
            provider: None,
        },
        CommandFlag {
            name: "range-end".to_string(),
            short_name: Some("e".to_string()),
            required: false,
            r#type: "string".to_string(),
            enum_values: vec![],
            default_value: None,
            description: Some("End value for range (inclusive)".to_string()),
            provider: None,
        },
        CommandFlag {
            name: "max".to_string(),
            short_name: Some("m".to_string()),
            required: false,
            r#type: "number".to_string(),
            enum_values: vec![],
            default_value: Some("25".to_string()),
            description: Some("Max number of items to retrieve".to_string()),
            provider: None,
        },
        CommandFlag {
            name: "order".to_string(),
            short_name: Some("o".to_string()),
            required: false,
            r#type: "enum".to_string(),
            enum_values: vec!["asc".to_string(), "desc".to_string()],
            default_value: Some("desc".to_string()),
            description: Some("Sort order of the results".to_string()),
            provider: None,
        },
    ]
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
fn get_description(schema: &Value, root: &Value) -> Option<String> {
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
fn get_type(schema: &Value, root: &Value) -> String {
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

/// Recursively collects enum values from a schema, following `$ref` or combining `anyOf`/`oneOf`.
///
/// # Arguments
///
/// * `schema` - The schema JSON `Value`.
/// * `root` - The root JSON schema `Value`.
///
/// # Returns
///
/// A vector of enum value strings.
fn get_enum_values(schema: &Value, root: &Value) -> Vec<String> {
    if let Some(ptr) = schema.get("$ref").and_then(Value::as_str) {
        let ptr = ptr.strip_prefix('#').unwrap_or(ptr);
        return root.pointer(ptr).map_or(vec![], |t| get_enum_values(t, root));
    }

    if let Some(enums) = schema.get("enum").and_then(Value::as_array) {
        return enums.iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
    }

    ["anyOf", "oneOf"]
        .iter()
        .filter_map(|key| schema.get(key).and_then(Value::as_array))
        .flat_map(|arr| arr.iter().flat_map(|item| get_enum_values(item, root)))
        .collect()
}

/// Recursively resolves the default value from a schema, following `$ref` or taking first from `anyOf`/`oneOf`.
///
/// # Arguments
///
/// * `schema` - The schema JSON `Value`.
/// * `root` - The root JSON schema `Value`.
///
/// # Returns
///
/// An optional default value as a string.
fn get_default(schema: &Value, root: &Value) -> Option<String> {
    if let Some(ptr) = schema.get("$ref").and_then(Value::as_str) {
        let ptr = ptr.strip_prefix('#').unwrap_or(ptr);
        return root.pointer(ptr).and_then(|t| get_default(t, root));
    }

    if let Some(def) = schema.get("default") {
        return def
            .as_str()
            .map(str::to_string)
            .or_else(|| def.as_bool().map(|b| b.to_string()))
            .or_else(|| def.as_i64().map(|n| n.to_string()))
            .or_else(|| def.as_u64().map(|n| n.to_string()))
            .or_else(|| def.as_f64().map(|n| n.to_string()));
    }

    for key in ["anyOf", "oneOf"] {
        if let Some(arr) = schema.get(key).and_then(Value::as_array) {
            for item in arr {
                if let Some(default) = get_default(item, root) {
                    return Some(default);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn providers_for_positional_from_path() {
        let json = r#"{
            "links": [
                { "href": "/addons", "method": "GET", "title": "List addons" },
                { "href": "/addons/{addon}/config", "method": "PATCH", "title": "Update config for addon" }
            ]
        }"#;
        let value: Value = serde_json::from_str(json).unwrap();
        let commands = derive_commands_from_schema(&value, ServiceId::CoreApi).unwrap();
        let spec = commands
            .iter()
            .find(|c| {
                c.http()
                    .map(|http| http.method == "PATCH" && http.path == "/addons/{addon}/config")
                    .unwrap_or(false)
            })
            .expect("config:update command exists");
        let pos = spec.positional_args.iter().find(|a| a.name == "addon").unwrap();
        match &pos.provider {
            Some(heroku_types::ValueProvider::Command { command_id, binds: _ }) => {
                assert_eq!(command_id, "addons:list")
            }
            _ => panic!("positional provider for addon missing"),
        }
    }

    #[test]
    fn providers_for_flag_app() {
        let json = r#"{
            "links": [
                { "href": "/apps", "method": "GET", "title": "List apps" },
                { "href": "/config", "method": "GET", "title": "Get config",
                    "schema": { "properties": { "app": { "type": "string" } } } }
            ]
        }"#;
        let value: Value = serde_json::from_str(json).unwrap();
        let commands = derive_commands_from_schema(&value, ServiceId::CoreApi).unwrap();
        let spec = commands
            .iter()
            .find(|c| c.http().map(|http| http.method == "GET" && http.path == "/config").unwrap_or(false))
            .expect("GET /config command exists");
        let flag = spec.flags.iter().find(|f| f.name == "app").unwrap();
        match &flag.provider {
            Some(heroku_types::ValueProvider::Command { command_id, binds: _ }) => assert_eq!(command_id, "apps:list"),
            _ => panic!("flag provider for app missing"),
        }
    }

    #[test]
    fn placeholder_names_ignore_version_segment() {
        let json = r#"{
            "links": [
                { "href": "/data/postgres/v1/{addon}/credentials/{cred_name}/rotate", "method": "POST", "title": "Rotate credential" }
            ]
        }"#;
        let value: Value = serde_json::from_str(json).unwrap();
        let commands = derive_commands_from_schema(&value, ServiceId::CoreApi).unwrap();
        let spec = commands
            .iter()
            .find(|c| c.http().map(|http| http.method == "POST").unwrap_or(false))
            .expect("POST rotate command exists");

        let http = spec.http().expect("HTTP spec available");
        assert_eq!(http.path, "/data/postgres/v1/{addon}/credentials/{cred_name}/rotate");

        let arg_names: Vec<_> = spec.positional_args.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(arg_names, vec!["addon", "cred_name"], "positional names derived from placeholders");

        // Ensure command name does not include version segment
        assert!(!spec.name.contains(":v1:"), "command name should ignore version segments");
        assert!(
            spec.name.starts_with("postgres:credentials:rotate:"),
            "expected name to include resource path segments"
        );
    }

    #[test]
    fn resolves_ref_at_root_and_uses_property_descriptions() {
        let json = r##"{
            "definitions": {
                "Body": {
                    "type": "object",
                    "required": ["name"],
                    "properties": {
                        "name": { "type": "string", "description": "Team name" },
                        "force": { "type": "boolean", "description": "Force operation" }
                    }
                }
            },
            "links": [
                {
                    "href": "/teams/{team}/update",
                    "method": "PATCH",
                    "schema": { "$ref": "#/definitions/Body" }
                }
            ]
        }"##;
        let value: Value = serde_json::from_str(json).unwrap();
        let commands = derive_commands_from_schema(&value, ServiceId::CoreApi).unwrap();
        let spec = commands
            .iter()
            .find(|c| c.http().map(|http| http.method == "PATCH").unwrap_or(false))
            .expect("PATCH command exists");

        // Should produce flags for name and force, with descriptions and required status
        let mut fl_map: HashMap<&str, (&Option<String>, bool)> = HashMap::new();
        for f in &spec.flags {
            fl_map.insert(&f.name, (&f.description, f.required));
        }
        assert_eq!(fl_map.get("name").unwrap().0.as_deref(), Some("Team name"));
        assert!(fl_map.get("name").unwrap().1);
        assert_eq!(fl_map.get("force").unwrap().0.as_deref(), Some("Force operation"));
        assert!(!fl_map.get("force").unwrap().1);
    }

    #[test]
    fn decode_ref_placeholder_and_use_ref_name() {
        let json = r#"{
            "links": [
                { "href": "/teams/{(%23%2Fdefinitions%2Fteam%2Fdefinitions%2Fidentity)}/addons", "method": "GET", "title": "List team addons" }
            ]
        }"#;
        let value: Value = serde_json::from_str(json).unwrap();
        let commands = derive_commands_from_schema(&value, ServiceId::CoreApi).unwrap();
        let spec = commands
            .iter()
            .find(|c| c.http().map(|http| http.method == "GET").unwrap_or(false))
            .expect("GET command exists");

        // Path should be normalized to use the ref name for the placeholder
        let http = spec.http().expect("HTTP spec available");
        assert_eq!(http.path, "/teams/{team}/addons");

        // Positional should use the ref name
        let arg_names: Vec<_> = spec.positional_args.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(arg_names, vec!["team"]);
    }
}
