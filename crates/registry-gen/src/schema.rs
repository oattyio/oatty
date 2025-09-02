use anyhow::{Context, Result};
use heck::ToKebabCase;
use heroku_types::{
    CommandFlag, CommandSpec, PositionalArgument, ProviderBinding, ProviderConfidence,
    ProviderParamKind,
};
use percent_encoding::percent_decode_str;
use serde_json::Value;
use std::{cmp::Ordering, collections::{HashMap, HashSet}};

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
pub fn generate_commands(schema_json: &str) -> Result<Vec<CommandSpec>> {
    let value: Value = serde_json::from_str(schema_json).context("Failed to parse schema JSON")?;
    derive_commands_from_schema(&value)
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
pub fn derive_commands_from_schema(value: &Value) -> Result<Vec<CommandSpec>> {
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
            let description = link
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or(&title)
                .to_string();

            if let Some((_, action)) = classify_command(href, method) {
                let (path_template, positional_args) = path_and_vars_with_help(href, value);
                let (flags, _required_names) = extract_flags_resolved(link, value);
                let ranges = extract_ranges(link);

                if path_template.is_empty() {
                    continue;
                }

                let (mut group, mut name) = derive_command_group_and_name(href, &title.to_kebab_case());
                if command_names.insert(name.clone(), true).is_some() {
                    (group, name) = derive_command_group_and_name(href, &action);
                }

                commands.push(CommandSpec {
                    group,
                    name,
                    summary: description,
                    positional_args,
                    flags,
                    method: method.to_string(),
                    path: path_template,
                    ranges,
                    providers: Vec::new(),
                });
            }
        }
    }

    // multi-sort: group then name
    commands.sort_by(|a, b| {
        return if a.group != b.group {
            a.group.cmp(&b.group)
        } else {
            a.name.cmp(&b.name)
        }
    });
    commands.dedup_by(|a, b| a.name == b.name && a.method == b.method && a.path == b.path);
    infer_provider_bindings(&mut commands);
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
        .filter(|s| !s.is_empty() && !s.starts_with('{'))
        .map(str::to_string)
        .collect()
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
    let mut args = Vec::new();
    let mut segments = Vec::new();
    let mut prev = None;

    for seg in href.trim_start_matches('/').split('/') {
        if seg.starts_with('{') {
            let name = prev.map_or("id".to_string(), singularize);
            let help_text = extract_placeholder_ptr(seg)
                .and_then(|ptr| {
                    let decoded = percent_decode_str(&ptr).decode_utf8_lossy().to_string();
                    let ptr = decoded.strip_prefix('#').unwrap_or(&decoded);
                    root.pointer(ptr).and_then(|val| get_description(val, root))
                });

            args.push(PositionalArgument {
                name: name.clone(),
                help: help_text,
            });
            segments.push(format!("{{{}}}", name));
        } else {
            segments.push(seg.to_string());
        }
        prev = Some(seg);
    }

    (format!("/{}", segments.join("/")), args)
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

/// Extracts the JSON pointer from a placeholder segment.
///
/// # Arguments
///
/// * `segment` - The segment string (e.g., `{id}` or `{(pointer)}`).
///
/// # Returns
///
/// An optional extracted pointer string.
fn extract_placeholder_ptr(segment: &str) -> Option<String> {
    let inner = segment.trim_start_matches('{').trim_end_matches('}').trim();
    let ptr = if inner.starts_with('(') && inner.ends_with(')') {
        inner.trim_start_matches('(').trim_end_matches(')').to_string()
    } else {
        inner.to_string()
    };
    (!ptr.is_empty()).then(|| ptr)
}

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
    let mut required_names = Vec::new();

    // Extract required names and properties from schema
    let Some(schema) = link.get("schema") else {
        return (add_range_flags(&extract_ranges(link)), Vec::new());
    };

    required_names = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|req| req.iter().filter_map(|r| r.as_str().map(str::to_string)).collect())
        .unwrap_or_default();

    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        flags = props
            .iter()
            .map(|(name, val)| {
                let merged = resolve_schema_properties(val, root);
                let ty = get_type(&merged, root);
                let required = required_names.contains(name);
                let enum_values = get_enum_values(&merged, root);
                let default_value = get_default(&merged, root)
                    .or_else(|| enum_values.first().cloned());
                let description = get_description(&merged, root);

                CommandFlag {
                    name: name.clone(),
                    short_name: name
                        .chars()
                        .next()
                        .filter(|c| c.is_alphabetic())
                        .map(|c| c.to_string()),
                    required,
                    r#type: ty,
                    enum_values,
                    default_value,
                    description,
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
            return Ordering::Greater
        }
        return Ordering::Equal;
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
        },
        CommandFlag {
            name: "range-start".to_string(),
            short_name: Some("s".to_string()),
            required: false,
            r#type: "string".to_string(),
            enum_values: vec![],
            default_value: None,
            description: Some("Start value for range (inclusive)".to_string()),
        },
        CommandFlag {
            name: "range-end".to_string(),
            short_name: Some("e".to_string()),
            required: false,
            r#type: "string".to_string(),
            enum_values: vec![],
            default_value: None,
            description: Some("End value for range (inclusive)".to_string()),
        },
        CommandFlag {
            name: "max".to_string(),
            short_name: Some("m".to_string()),
            required: false,
            r#type: "number".to_string(),
            enum_values: vec![],
            default_value: Some("25".to_string()),
            description: Some("Max number of items to retrieve".to_string()),
        },
        CommandFlag {
            name: "order".to_string(),
            short_name: Some("o".to_string()),
            required: false,
            r#type: "enum".to_string(),
            enum_values: vec!["asc".to_string(), "desc".to_string()],
            default_value: Some("desc".to_string()),
            description: Some("Sort order of the results".to_string()),
        },
    ]
}

/// Infers provider bindings for flags and positional arguments in commands.
///
/// # Arguments
///
/// * `commands` - A mutable slice of `CommandSpec` instances.
fn infer_provider_bindings(commands: &mut [CommandSpec]) {
    let list_groups: HashSet<String> = commands
        .iter()
        .filter_map(|c| {
            classify_command(&c.path, &c.method).and_then(|(grp, action)| {
                (action == "list").then(|| normalize_group(&grp))
            })
        })
        .collect();

    let synonyms: HashMap<&str, &str> = HashMap::from([
        ("app", "apps"),
        ("addon", "addons"),
        ("pipeline", "pipelines"),
        ("team", "teams"),
        ("space", "spaces"),
        ("dyno", "dynos"),
        ("release", "releases"),
        ("collaborator", "collaborators"),
        ("region", "regions"),
        ("stack", "stacks"),
    ]);

    for cmd in commands.iter_mut() {
        let mut providers = infer_positionals_from_path(&cmd.path, &list_groups);

        for flag in &cmd.flags {
            if let Some((group, confidence)) = map_flag_to_group(&flag.name, &synonyms) {
                if list_groups.contains(&group) {
                    providers.push(ProviderBinding {
                        kind: ProviderParamKind::Flag,
                        name: flag.name.clone(),
                        provider_id: format!("{}:{}", group, "list"),
                        confidence,
                    });
                }
            }
        }

        providers.sort_by(|a, b| a.name.cmp(&b.name));
        providers.dedup_by(|a, b| a.kind == b.kind && a.name == b.name);
        cmd.providers = providers;
    }
}

/// Infers provider bindings for positional arguments from a path.
///
/// # Arguments
///
/// * `path` - The API endpoint path.
/// * `list_groups` - Set of groups with list commands.
///
/// # Returns
///
/// A vector of `ProviderBinding` instances.
fn infer_positionals_from_path(path: &str, list_groups: &HashSet<String>) -> Vec<ProviderBinding> {
    let mut providers = Vec::new();
    let mut prev_concrete: Option<String> = None;

    for seg in path.trim_start_matches('/').split('/') {
        if seg.starts_with('{') && seg.ends_with('}') {
            let name = seg.trim_start_matches('{').trim_end_matches('}').trim().to_string();
            if let Some(prev) = &prev_concrete {
                let group = normalize_group(prev);
                if list_groups.contains(&group) {
                    providers.push(ProviderBinding {
                        kind: ProviderParamKind::Positional,
                        name,
                        provider_id: format!("{}:{}", group, "list"),
                        confidence: ProviderConfidence::High,
                    });
                }
            }
        } else if !seg.is_empty() {
            prev_concrete = Some(seg.to_string());
        }
    }
    providers
}

/// Maps a flag name to a group, using synonyms or conservative pluralization.
///
/// # Arguments
///
/// * `flag` - The flag name.
/// * `synonyms` - A map of singular to plural group names.
///
/// # Returns
///
/// An optional tuple of group name and confidence level.
fn map_flag_to_group(flag: &str, synonyms: &HashMap<&str, &str>) -> Option<(String, ProviderConfidence)> {
    let key = flag.to_lowercase();
    synonyms
        .get(key.as_str())
        .map(|&g| (g.to_string(), ProviderConfidence::Medium))
        .or_else(|| conservative_plural(&key).map(|p| (p, ProviderConfidence::Low)))
}

/// Conservatively pluralizes a string for group matching.
///
/// # Arguments
///
/// * `s` - The input string.
///
/// # Returns
///
/// An optional pluralized string.
fn conservative_plural(s: &str) -> Option<String> {
    if s.is_empty() {
        return None;
    }
    if s.ends_with('s') {
        return Some(s.to_string());
    }
    if s.ends_with('y') && s.len() > 1 && !matches!(s.chars().nth(s.len() - 2).unwrap(), 'a' | 'e' | 'i' | 'o' | 'u') {
        return Some(format!("{}ies", &s[..s.len() - 1]));
    }
    if s.ends_with('x') || s.ends_with("ch") || s.ends_with("sh") {
        return Some(format!("{}es", s));
    }
    Some(format!("{}s", s))
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
        let commands = derive_commands_from_schema(&value).unwrap();
        let spec = commands
            .iter()
            .find(|c| c.method == "PATCH" && c.path == "/addons/{addon}/config")
            .expect("config:update command exists");
        let binding = spec
            .providers
            .iter()
            .find(|p| p.kind == ProviderParamKind::Positional && p.name == "addon")
            .expect("positional provider for addon exists");
        assert_eq!(binding.provider_id, "addons:list");
        assert_eq!(binding.confidence, ProviderConfidence::High);
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
        let commands = derive_commands_from_schema(&value).unwrap();
        let spec = commands
            .iter()
            .find(|c| c.method == "GET" && c.path == "/config")
            .expect("GET /config command exists");
        let binding = spec
            .providers
            .iter()
            .find(|p| p.kind == ProviderParamKind::Flag && p.name == "app")
            .expect("flag provider for app exists");
        assert_eq!(binding.provider_id, "apps:list");
        assert!(matches!(binding.confidence, ProviderConfidence::Medium | ProviderConfidence::High));
    }
}
