use std::collections::HashMap;

use anyhow::{Context, Result};
use heck::ToKebabCase;
use heroku_types::{CommandFlag, CommandSpec, PositionalArgument};
use percent_encoding::percent_decode_str;
use serde_json::Value;

/// Generates command specifications from a JSON schema string.
///
/// This function parses the provided JSON schema, derives command
/// specifications from it, and adds synthetic workflow commands.
///
/// # Arguments
///
/// * `schema_json` - The JSON schema as a string.
///
/// # Errors
///
/// Returns an error if the JSON parsing fails or if command derivation
/// encounters issues.
///
/// # Returns
///
/// A vector of `CommandSpec` on success.
pub fn generate_commands(schema_json: &str) -> Result<Vec<CommandSpec>> {
    let v: Value = serde_json::from_str(schema_json).context("parse schema json")?;
    let commands = derive_commands_from_schema(&v)?;
    Ok(commands)
}

/// Derives command specifications from a JSON schema Value.
///
/// This function traverses the schema to find "links" sections, extracts
/// command details such as href, method, title, description, positional
/// arguments, flags, and constructs `CommandSpec` instances.
///
/// Commands are sorted and deduplicated by name, method, and path.
///
/// # Arguments
///
/// * `v` - The root JSON schema Value.
///
/// # Errors
///
/// Returns an error if schema traversal or extraction fails.
///
/// # Returns
///
/// A sorted and deduplicated vector of `CommandSpec`.
pub fn derive_commands_from_schema(v: &Value) -> Result<Vec<CommandSpec>> {
    let mut cmds: Vec<CommandSpec> = Vec::new();
    let mut cmd_names: HashMap<String, bool> = HashMap::new();

    fn walk<'a>(val: &'a Value, out: &mut Vec<&'a Value>) {
        match val {
            Value::Object(map) => {
                if map.contains_key("links") {
                    out.push(val);
                }
                for v in map.values() {
                    walk(v, out);
                }
            }
            Value::Array(arr) => {
                for v in arr {
                    walk(v, out);
                }
            }
            _ => {}
        }
    }

    let mut nodes = Vec::new();
    walk(v, &mut nodes);

    for node in nodes {
        let Some(links) = node.get("links").and_then(|x| x.as_array()) else {
            continue;
        };
        for link in links {
            let Some(href) = link.get("href").and_then(|x| x.as_str()) else {
                continue;
            };
            let Some(method) = link.get("method").and_then(|x| x.as_str()) else {
                continue;
            };
            let title = link.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();

            let desc = link
                .get("description")
                .and_then(|x| x.as_str())
                .or_else(|| Some(&title))
                .unwrap_or("")
                .to_string();

            if let Some((_, action)) = classify_command(href, method) {
                let (path_tmpl, positional_args) = path_and_vars_with_help(href, v);
                let (flags, _required_names) = extract_flags_resolved(link, v);
                let ranges = extract_ranges(link);

                if path_tmpl.is_empty() {
                    continue;
                }
                let (mut group, mut name) = derive_command_group_and_name(href, &title.to_kebab_case());
                if cmd_names.insert(name.clone(), true).is_some() {
                    (group, name) = derive_command_group_and_name(href, &action);
                }
                let spec = CommandSpec {
                    group,
                    name,
                    summary: desc.clone(),
                    positional_args,
                    flags,
                    method: method.to_string(),
                    path: path_tmpl,
                    ranges,
                };
                cmds.push(spec);
            }
        }
    }

    cmds.sort_by(|a, b| a.name.cmp(&b.name));
    cmds.dedup_by(|a, b| a.name == b.name && a.method == b.method && a.path == b.path);
    Ok(cmds)
}

/// Classifies a command based on its href and HTTP method.
///
/// Determines the group and action (e.g., "info", "list", "create") based on
/// the path structure and method.
///
/// # Arguments
///
/// * `href` - The API endpoint path.
/// * `method` - The HTTP method.
///
/// # Returns
///
/// An option containing the group and action if classifiable, otherwise None.
fn classify_command(href: &str, method: &str) -> Option<(String, String)> {
    let path = href.trim_start_matches('/');
    let segs: Vec<&str> = path.split('/').collect();
    if segs.is_empty() {
        return None;
    }

    let mut group = None;
    for seg in segs.iter().rev() {
        if seg.starts_with('{') {
            continue;
        }
        group = Some(seg.to_string());
        break;
    }
    let mut group = group.unwrap_or_else(|| segs[0].to_string());
    if group == "config-vars" {
        group = "config".into();
    }

    let is_resource = segs.last().map(|s| s.starts_with('{')) == Some(true);
    let ends_with_collection = segs.last().map(|s| !s.starts_with('{')) == Some(true);
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

/// Extracts concrete segments from an href.
///
/// Filters out empty or placeholder segments.
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
        .map(|s| s.to_string())
        .collect()
}

/// Normalizes a group name.
///
/// Special handling for certain groups like "config-vars".
///
/// # Arguments
///
/// * `s` - The group name.
///
/// # Returns
///
/// The normalized group name.
fn normalize_group(s: &str) -> String {
    if s == "config-vars" {
        "config".into()
    } else {
        s.to_string()
    }
}

/// Derives the command group and name from href and action.
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
    let segs = concrete_segments(href);
    if segs.is_empty() {
        return ("misc".to_string(), action.to_string());
    }
    let group = normalize_group(&segs[0]);
    let rest = if segs.len() > 1 {
        segs[1..].join(":")
    } else {
        String::new()
    };
    let sub = if rest.is_empty() {
        action.to_string()
    } else {
        format!("{}:{}", rest, action)
    };
    (group, sub)
}

/// Extracts the path template, positional arguments, and help descriptions.
///
/// Processes the href to identify placeholders, singularizes names, and
/// resolves descriptions recursively from the schema.
///
/// # Arguments
///
/// * `href` - The API endpoint path.
/// * `root` - The root JSON schema Value.
///
/// # Returns
///
/// A tuple of path template, vector of positional args, and hashmap of help.
fn path_and_vars_with_help(href: &str, root: &Value) -> (String, Vec<PositionalArgument>) {
    let mut args: Vec<PositionalArgument> = Vec::new();
    let mut out_segs: Vec<String> = Vec::new();
    let mut prev: Option<&str> = None;
    for seg in href.trim_start_matches('/').split('/') {
        if seg.starts_with('{') {
            let name = prev.map(singularize).unwrap_or_else(|| "id".to_string());
            let mut help_text: Option<String> = None;

            if let Some(ptr_enc) = extract_placeholder_ptr(seg) {
                let decoded = percent_decode_str(&ptr_enc).decode_utf8_lossy().to_string();
                let ptr = decoded.strip_prefix('#').unwrap_or(&decoded);
                if let Some(val) = root.pointer(ptr)
                    && let Some(desc) = get_description(val, root)
                {
                    help_text = Some(desc);
                }
            }
            args.push(PositionalArgument {
                name: name.clone(),
                help: help_text,
            });
            out_segs.push(format!("{{{}}}", name));
        } else {
            out_segs.push(seg.to_string());
        }
        prev = Some(seg);
    }
    (format!("/{}", out_segs.join("/")), args)
}

/// Singularizes a segment name.
///
/// Removes plural 's' if present and replaces '-' with '_'.
///
/// # Arguments
///
/// * `s` - The segment string.
///
/// # Returns
///
/// The singularized name.
fn singularize(s: &str) -> String {
    let s = s.trim_matches(|c: char| c == '{' || c == '}' || c == ' ');
    let s = s.replace('-', "_");
    if s.ends_with('s') && s.len() > 1 {
        s[..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Extracts the pointer from a placeholder segment.
///
/// # Arguments
///
/// * `seg` - The segment string.
///
/// # Returns
///
/// An option containing the extracted pointer.
fn extract_placeholder_ptr(seg: &str) -> Option<String> {
    let inner = seg.trim_start_matches('{').trim_end_matches('}');
    let inner = inner.trim();
    let ptr = if inner.starts_with('(') && inner.ends_with(')') {
        inner.trim_start_matches('(').trim_end_matches(')').to_string()
    } else {
        inner.to_string()
    };
    if ptr.is_empty() { None } else { Some(ptr) }
}

/// Extracts range fields from a link schema.
///
/// # Arguments
///
/// * `link` - The link JSON Value.
///
/// # Returns
///
/// A vector of range field names.
fn extract_ranges(link: &Value) -> Vec<String> {
    link.get("ranges")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default()
}

/// Extracts flags and required names from a link schema.
///
/// Resolves properties recursively, handling $ref, anyOf, etc., for type,
/// description, enum values, and defaults. Generates short flags as the first
/// letter of the long name. Also adds range-related flags if ranges are
/// supported.
///
/// # Arguments
///
/// * `link` - The link JSON Value.
/// * `root` - The root JSON schema Value.
///
/// # Returns
///
/// A tuple of vector of `CommandFlag` and vector of required names.
fn extract_flags_resolved(link: &Value, root: &Value) -> (Vec<CommandFlag>, Vec<String>) {
    let mut flags = Vec::new();
    let mut required_names: Vec<String> = Vec::new();
    if let Some(schema) = link.get("schema") {
        if let Some(req) = schema.get("required").and_then(|x| x.as_array()) {
            for r in req {
                if let Some(s) = r.as_str() {
                    required_names.push(s.to_string());
                }
            }
        }
        if let Some(props) = schema.get("properties").and_then(|x| x.as_object()) {
            for (name, val) in props.iter() {
                let mut merged = val.clone();
                if let Some(r) = val.get("$ref").and_then(|x| x.as_str()) {
                    let ptr = r.strip_prefix('#').unwrap_or(r);
                    if let Some(target) = root.pointer(ptr) {
                        if merged.get("type").is_none()
                            && let Some(t) = target.get("type")
                        {
                            merged.as_object_mut().unwrap().insert("type".into(), t.clone());
                        }
                        if merged.get("description").is_none()
                            && let Some(d) = target.get("description")
                        {
                            merged.as_object_mut().unwrap().insert("description".into(), d.clone());
                        }
                        if merged.get("enum").is_none()
                            && let Some(e) = target.get("enum")
                        {
                            merged.as_object_mut().unwrap().insert("enum".into(), e.clone());
                        }
                        if merged.get("default").is_none()
                            && let Some(df) = target.get("default")
                        {
                            merged.as_object_mut().unwrap().insert("default".into(), df.clone());
                        }
                    }
                }

                let ty = get_type(&merged, root);
                let required = required_names.iter().any(|n| n == name);
                let enum_values = get_enum_values(&merged, root);
                let mut default_value = get_default(&merged, root);
                let description = get_description(&merged, root);

                if default_value.is_none() && !enum_values.is_empty() {
                    default_value = Some(enum_values[0].clone());
                }
                flags.push(CommandFlag {
                    name: name.clone(),
                    short_name: name
                        .chars()
                        .next()
                        .map(|c| c.to_string())
                        .filter(|s| s.chars().all(|c| c.is_alphabetic())), // Only alphabetic chars
                    required,
                    r#type: ty,
                    enum_values,
                    default_value,
                    description,
                });
            }
        }
    }

    // Add range-related flags if ranges are supported
    let ranges = extract_ranges(link);
    if !ranges.is_empty() {
        flags.push(CommandFlag {
            name: "range-field".to_string(),
            short_name: Some("r".to_string()),
            required: false,
            r#type: "string".to_string(),
            enum_values: ranges.clone(),
            default_value: Some(ranges[0].clone()),
            description: Some("Field to use for range-based pagination".to_string()),
        });

        flags.push(CommandFlag {
            name: "range-start".to_string(),
            short_name: Some("s".to_string()),
            required: false,
            r#type: "string".to_string(),
            enum_values: vec![],
            default_value: None,
            description: Some("Start value for range (inclusive)".to_string()),
        });

        flags.push(CommandFlag {
            name: "range-end".to_string(),
            short_name: Some("e".to_string()),
            required: false,
            r#type: "string".to_string(),
            enum_values: vec![],
            default_value: None,
            description: Some("End value for range (inclusive)".to_string()),
        });
        
        flags.push(CommandFlag {
            name: "max".to_string(),
            short_name: Some("m".to_string()),
            required: false,
            r#type: "number".to_string(),
            enum_values: vec![],
            default_value: Some("25".into()),
            description: Some("Max number of items to retrieve".to_string()),
        });

        flags.push(CommandFlag {
            name: "order".to_string(),
            short_name: Some("e".to_string()),
            required: false,
            r#type: "enum".to_string(),
            enum_values: vec!["asc".into(), "desc".into()],
            default_value: Some("desc".into()),
            description: Some("Sort order of the results".to_string()),
        });
    }

    (flags, required_names)
}

/// Recursively resolves the description from a schema.
///
/// Follows $ref, uses direct description, or concatenates from anyOf/oneOf with
/// " or ", from allOf with " and ".
///
/// # Arguments
///
/// * `schema` - The schema JSON Value.
/// * `root` - The root JSON schema Value.
///
/// # Returns
///
/// An option containing the resolved description.
fn get_description(schema: &Value, root: &Value) -> Option<String> {
    if let Some(r) = schema.get("$ref").and_then(|x| x.as_str()) {
        let p = r.strip_prefix('#').unwrap_or(r);
        if let Some(t) = root.pointer(p) {
            return get_description(t, root);
        }
        return None;
    }

    if let Some(d) = schema.get("description").and_then(|x| x.as_str()) {
        return Some(d.to_string());
    }

    if let Some(any) = schema.get("anyOf").and_then(|x| x.as_array()) {
        let descs: Vec<String> = any.iter().filter_map(|item| get_description(item, root)).collect();
        if !descs.is_empty() {
            return Some(descs.join(" or "));
        }
    }

    if let Some(one) = schema.get("oneOf").and_then(|x| x.as_array()) {
        let descs: Vec<String> = one.iter().filter_map(|item| get_description(item, root)).collect();
        if !descs.is_empty() {
            return Some(descs.join(" or "));
        }
    }

    if let Some(all) = schema.get("allOf").and_then(|x| x.as_array()) {
        let descs: Vec<String> = all.iter().filter_map(|item| get_description(item, root)).collect();
        if !descs.is_empty() {
            return Some(descs.join(" and "));
        }
    }

    None
}

/// Recursively resolves the type from a schema.
///
/// Follows $ref, uses direct type, or determines from anyOf/oneOf.
///
/// # Arguments
///
/// * `schema` - The schema JSON Value.
/// * `root` - The root JSON schema Value.
///
/// # Returns
///
/// The resolved type string, defaulting to "string".
fn get_type(schema: &Value, root: &Value) -> String {
    if let Some(r) = schema.get("$ref").and_then(|x| x.as_str()) {
        let p = r.strip_prefix('#').unwrap_or(r);
        if let Some(t) = root.pointer(p) {
            return get_type(t, root);
        }
    }

    // Handle direct string type
    if let Some(ty) = schema.get("type").and_then(|x| x.as_str()) {
        return ty.to_string();
    }
    // Handle JSON Schema where type can be an array of strings (union), e.g.
    // ["boolean"], ["string","null"]
    if let Some(arr) = schema.get("type").and_then(|x| x.as_array()) {
        let mut types: std::collections::HashSet<String> =
            arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
        // Prefer non-null concrete type when present
        types.retain(|t| t != "null");
        if types.len() == 1 {
            return types.into_iter().next().unwrap();
        }
        // If multiple or empty after removing null, fall back to string
    }

    if let Some(any) = schema.get("anyOf").and_then(|x| x.as_array()) {
        let types: Vec<String> = any.iter().map(|item| get_type(item, root)).collect();
        let unique: std::collections::HashSet<String> = types.into_iter().collect();
        if unique.len() == 1 {
            return unique.into_iter().next().unwrap();
        }
    }

    if let Some(one) = schema.get("oneOf").and_then(|x| x.as_array()) {
        let types: Vec<String> = one.iter().map(|item| get_type(item, root)).collect();
        let unique: std::collections::HashSet<String> = types.into_iter().collect();
        if unique.len() == 1 {
            return unique.into_iter().next().unwrap();
        }
    }

    "string".to_string()
}

/// Recursively collects enum values from a schema.
///
/// Follows $ref, uses direct enum, or unions from anyOf/oneOf.
///
/// # Arguments
///
/// * `schema` - The schema JSON Value.
/// * `root` - The root JSON schema Value.
///
/// # Returns
///
/// A vector of enum value strings.
fn get_enum_values(schema: &Value, root: &Value) -> Vec<String> {
    if let Some(r) = schema.get("$ref").and_then(|x| x.as_str()) {
        let p = r.strip_prefix('#').unwrap_or(r);
        if let Some(t) = root.pointer(p) {
            return get_enum_values(t, root);
        }
        return vec![];
    }

    if let Some(en) = schema.get("enum").and_then(|x| x.as_array()) {
        return en.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
    }

    if let Some(any) = schema.get("anyOf").and_then(|x| x.as_array()) {
        let mut all = vec![];
        for item in any {
            all.extend(get_enum_values(item, root));
        }
        return all;
    }

    if let Some(one) = schema.get("oneOf").and_then(|x| x.as_array()) {
        let mut all = vec![];
        for item in one {
            all.extend(get_enum_values(item, root));
        }
        return all;
    }

    vec![]
}

/// Recursively resolves the default value from a schema.
///
/// Follows $ref, uses direct default, or first from anyOf/oneOf.
///
/// # Arguments
///
/// * `schema` - The schema JSON Value.
/// * `root` - The root JSON schema Value.
///
/// # Returns
///
/// An option containing the default value as string.
fn get_default(schema: &Value, root: &Value) -> Option<String> {
    if let Some(r) = schema.get("$ref").and_then(|x| x.as_str()) {
        let p = r.strip_prefix('#').unwrap_or(r);
        if let Some(t) = root.pointer(p) {
            return get_default(t, root);
        }
    }

    if let Some(def) = schema.get("default") {
        return def
            .as_str()
            .map(|s| s.to_string())
            .or_else(|| def.as_bool().map(|b| b.to_string()))
            .or_else(|| def.as_i64().map(|n| n.to_string()))
            .or_else(|| def.as_u64().map(|n| n.to_string()))
            .or_else(|| def.as_f64().map(|n| n.to_string()));
    }

    if let Some(any) = schema.get("anyOf").and_then(|x| x.as_array()) {
        for item in any {
            if let Some(d) = get_default(item, root) {
                return Some(d);
            }
        }
    }

    if let Some(one) = schema.get("oneOf").and_then(|x| x.as_array()) {
        for item in one {
            if let Some(d) = get_default(item, root) {
                return Some(d);
            }
        }
    }

    None
}
