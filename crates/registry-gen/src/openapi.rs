//! OpenAPI command generation utilities.
//!
//! This module parses OpenAPI v3 documents directly into `CommandSpec` entries,
//! extracting path parameters, query/body flags, response schemas, and server
//! base URLs without converting to Hyper-Schema.

use anyhow::{Result, anyhow};
use heck::ToKebabCase;
use oatty_types::{CommandFlag, CommandSpec, HttpCommandSpec, PositionalArgument};
use oatty_util::{
    OpenApiValidationViolation, get_description, get_type, resolve_output_schema, sort_and_dedup_commands, validate_openapi_preflight,
};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use url::Url;

use crate::provider_resolver::resolve_and_infer_providers;

/// Generate command specifications from an OpenAPI document.
///
/// # Arguments
///
/// * `document` - Parsed OpenAPI document value.
///
/// # Returns
///
/// A list of `CommandSpec` entries derived from the OpenAPI paths and operations.
///
/// The command group for each entry is derived from the registrable domain of the
/// first server URL in the document.
///
/// # Errors
///
/// Returns an error if the document is not OpenAPI v3 or lacks required sections.
pub fn derive_commands_from_openapi(document: &Value, vendor: &str) -> Result<Vec<CommandSpec>> {
    if let Err(violations) = validate_openapi_preflight(document) {
        return Err(anyhow!(
            "unsupported OpenAPI document: {}",
            format_openapi_preflight_violations(&violations)
        ));
    }

    let mut commands = derive_commands_from_oas3(document, vendor)?;
    sort_and_dedup_commands(&mut commands);
    resolve_and_infer_providers(&mut commands);
    Ok(commands)
}

fn derive_commands_from_oas3(document: &Value, vendor: &str) -> Result<Vec<CommandSpec>> {
    let paths = document
        .get("paths")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("OpenAPI document missing paths"))?;

    let mut commands = Vec::new();
    let mut command_names: HashSet<String> = HashSet::new();

    for (path, path_item) in paths {
        let Some(path_item_object) = path_item.as_object() else {
            return Err(anyhow!("OpenAPI path item is not an object: {}", path));
        };

        for (method, operation) in path_item_object {
            if !is_supported_http_method(method) {
                continue;
            }
            let Some(operation_object) = operation.as_object() else {
                continue;
            };

            if let Some(command_spec) =
                build_command_from_operation(document, vendor, path, path_item, operation_object, method, &mut command_names)?
            {
                commands.push(command_spec);
            }
        }
    }

    Ok(commands)
}

fn build_command_from_operation(
    document: &Value,
    vendor: &str,
    path: &str,
    path_item: &Value,
    operation: &Map<String, Value>,
    method: &str,
    command_names: &mut HashSet<String>,
) -> Result<Option<CommandSpec>> {
    let method_upper = method.to_ascii_uppercase();
    let Some((_, action)) = classify_command(path, method_upper.as_str()) else {
        return Ok(None);
    };

    let (title, description) = extract_operation_summary(operation);

    let parameters = collect_parameters(document, path_item, operation);
    let (path_template, positional_args) = build_path_template_and_positionals(path, &parameters, document);
    if path_template.is_empty() {
        return Ok(None);
    }

    let mut flags = collect_flags_from_operation(document, &parameters, operation);
    normalize_flags(&mut flags);

    let http_spec = build_http_command_spec(document, operation, method_upper, path_template)?;

    let (group, name) = derive_unique_command_name(vendor, path, &action, &title, method, command_names);
    Ok(Some(CommandSpec::new_http(
        group,
        name,
        description,
        positional_args,
        flags,
        http_spec,
        0,
    )))
}

fn extract_operation_summary(operation: &Map<String, Value>) -> (String, String) {
    let title = operation
        .get("summary")
        .and_then(Value::as_str)
        .or_else(|| operation.get("operationId").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();
    let description = operation.get("description").and_then(Value::as_str).unwrap_or(&title).to_string();
    (title, description)
}

fn normalize_flags(flags: &mut [CommandFlag]) {
    assign_unique_short_names(flags);
    flags.sort_by(|left, right| {
        if left.required && right.required {
            return left.name.cmp(&right.name);
        }
        if left.required {
            return std::cmp::Ordering::Less;
        }
        if right.required {
            return std::cmp::Ordering::Greater;
        }
        std::cmp::Ordering::Equal
    });
}

fn build_http_command_spec(document: &Value, operation: &Map<String, Value>, method: String, path: String) -> Result<HttpCommandSpec> {
    let target_schema = build_target_schema_from_oas3(document, operation);
    let output_schema = resolve_output_schema(target_schema.as_ref(), document);

    Ok(HttpCommandSpec {
        method,
        path,
        output_schema,
    })
}

fn derive_unique_command_name(
    vendor: &str,
    path: &str,
    action: &str,
    title: &str,
    method: &str,
    command_names: &mut HashSet<String>,
) -> (String, String) {
    let (group, name) = derive_command_group_and_name(vendor, path, action);
    if command_names.insert(format!("{}{}", group, name)) {
        return (group, name);
    }

    if !title.is_empty() {
        let (title_group, title_name) = derive_command_group_and_name(vendor, path, &title.to_kebab_case());
        if command_names.insert(format!("{}{}", title_group, title_name)) {
            return (title_group, title_name);
        }
    }

    let (method_group, method_name) = derive_command_group_and_name(vendor, path, &method.to_lowercase());
    command_names.insert(format!("{}{}", method_group, method_name));
    (method_group, method_name)
}

fn collect_flags_from_operation(document: &Value, parameters: &[Value], operation: &Map<String, Value>) -> Vec<CommandFlag> {
    let mut flags_by_name: HashMap<String, CommandFlag> = HashMap::new();

    for parameter in parameters {
        if parameter.get("in").and_then(Value::as_str) != Some("query") {
            continue;
        }
        if let Some(flag) = build_flag_from_parameter(document, parameter) {
            flags_by_name
                .entry(flag.name.clone())
                .and_modify(|existing| existing.required |= flag.required)
                .or_insert(flag);
        }
    }

    for flag in build_flags_from_request_body(document, operation) {
        flags_by_name
            .entry(flag.name.clone())
            .and_modify(|existing| existing.required |= flag.required)
            .or_insert(flag);
    }

    flags_by_name.into_values().collect()
}

fn build_flag_from_parameter(document: &Value, parameter: &Value) -> Option<CommandFlag> {
    let name = parameter.get("name").and_then(Value::as_str)?.to_string();
    let required = parameter.get("required").and_then(Value::as_bool).unwrap_or(false);

    let schema = parameter.get("schema").cloned().unwrap_or_else(|| Value::Object(Map::new()));
    let merged_schema = resolve_schema_properties(&schema, document);
    let schema_type = get_type(&merged_schema, document);
    let enum_values = get_enum_values(&merged_schema, document);
    let default_value = get_default(&merged_schema, document)
        .or_else(|| get_default(parameter, document))
        .or_else(|| enum_values.first().cloned());
    let description = parameter
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| get_description(&merged_schema, document));

    Some(CommandFlag {
        name,
        short_name: None,
        required,
        r#type: schema_type,
        enum_values,
        default_value,
        description,
        provider: None,
    })
}

fn build_flags_from_request_body(document: &Value, operation: &Map<String, Value>) -> Vec<CommandFlag> {
    let Some(request_body) = operation.get("requestBody") else {
        return Vec::new();
    };
    let Some(schema) = request_body
        .get("content")
        .and_then(|content| content.get("application/json"))
        .and_then(|content| content.get("schema"))
    else {
        return Vec::new();
    };

    let resolved_schema = if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        resolve_local_ref(document, reference).unwrap_or_else(|| schema.clone())
    } else {
        schema.clone()
    };

    let merged_schema = resolve_schema_properties(&resolved_schema, document);
    let required_names = collect_required(&merged_schema);
    let mut flags = Vec::new();

    if let Some(properties) = collect_properties(&merged_schema) {
        for (name, value) in properties {
            let merged_property = resolve_schema_properties(&value, document);
            let schema_type = get_type(&merged_property, document);
            let enum_values = get_enum_values(&merged_property, document);
            let default_value = get_default(&merged_property, document).or_else(|| enum_values.first().cloned());
            let description = get_description(&merged_property, document);
            let is_required = required_names.contains(&name);

            flags.push(CommandFlag {
                name,
                short_name: None,
                required: is_required,
                r#type: schema_type,
                enum_values,
                default_value,
                description,
                provider: None,
            });
        }
    } else {
        let schema_type = get_type(&merged_schema, document);
        let enum_values = get_enum_values(&merged_schema, document);
        let default_value = get_default(&merged_schema, document).or_else(|| enum_values.first().cloned());
        let description = get_description(&merged_schema, document);
        let is_required = request_body.get("required").and_then(Value::as_bool).unwrap_or(false);

        flags.push(CommandFlag {
            name: "body".to_string(),
            short_name: None,
            required: is_required,
            r#type: schema_type,
            enum_values,
            default_value,
            description,
            provider: None,
        });
    }

    flags
}

fn build_path_template_and_positionals(path: &str, parameters: &[Value], document: &Value) -> (String, Vec<PositionalArgument>) {
    let mut parameter_descriptions: HashMap<String, Option<String>> = HashMap::new();
    for parameter in parameters {
        if parameter.get("in").and_then(Value::as_str) != Some("path") {
            continue;
        }
        let Some(name) = parameter.get("name").and_then(Value::as_str) else {
            continue;
        };
        let description = parameter
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| parameter.get("schema").and_then(|schema| get_description(schema, document)));
        parameter_descriptions.insert(sanitize_placeholder_name(name), description);
    }

    let mut args = Vec::new();
    let mut segments_out = Vec::new();
    let mut previous_concrete: Option<&str> = None;

    for segment in path.trim_start_matches('/').split('/') {
        if segment.starts_with('{') && segment.ends_with('}') {
            let raw_name = &segment[1..segment.len() - 1];
            let sanitized = sanitize_placeholder_name(raw_name);
            let arg_name = if sanitized == "id" {
                previous_concrete
                    .filter(|value| !is_version_segment(value))
                    .map(singularize)
                    .unwrap_or_else(|| "id".to_string())
            } else {
                sanitized.clone()
            };
            let help = parameter_descriptions.get(&sanitized).cloned().unwrap_or(None);

            args.push(PositionalArgument {
                name: arg_name.clone(),
                help,
                provider: None,
            });
            segments_out.push(format!("{{{}}}", arg_name));
        } else {
            segments_out.push(segment.to_string());
            if !segment.is_empty() && !is_version_segment(segment) {
                previous_concrete = Some(segment);
            }
        }
    }

    (format!("/{}", segments_out.join("/")), args)
}

/// Collects normalized base URLs from the OpenAPI document servers list.
pub fn collect_base_urls_from_document(document: &Value) -> Vec<String> {
    let Some(servers) = document.get("servers").and_then(Value::as_array) else {
        return Vec::new();
    };
    collect_base_urls_from_servers(servers)
}

pub fn derive_vendor_from_document(document: &Value) -> String {
    let Some(servers) = document.get("servers").and_then(Value::as_array) else {
        return "misc".to_string();
    };
    let Some(resolved_url) = resolve_servers_base_url(servers) else {
        return "misc".to_string();
    };

    derive_vendor_from_base_url(&resolved_url).unwrap_or_else(|| "misc".to_string())
}

pub fn derive_vendor_from_base_url(base_url: &str) -> Option<String> {
    let url = Url::parse(base_url).ok()?;
    let host_str = url.host_str()?;
    let labels: Vec<&str> = host_str.split('.').filter(|l| !l.is_empty()).collect();

    if labels.is_empty() {
        return None;
    }
    if labels.len() == 1 {
        return Some(labels[0].to_lowercase());
    }

    let mut candidate = labels[labels.len() - 2];
    let common_slds = [
        "co",
        "com",
        "net",
        "org",
        "gov",
        "edu",
        "ac",
        "io",
        "ne",
        "or",
        "go",
        "ed",
        "lg",
        "gr",
        "blogspot",
        "github",
        "herokuapp",
        "vercel",
        "pages",
        "appspot",
        "cloudfront",
        "wordpress",
        "blog",
        "dev",
        "xyz",
        "local",
    ];
    if labels.len() >= 3 && common_slds.contains(&candidate) {
        candidate = labels[labels.len() - 3];
    }

    if candidate.is_empty() {
        None
    } else {
        Some(candidate.to_lowercase())
    }
}

fn resolve_servers_base_url(servers: &[Value]) -> Option<String> {
    for server in servers {
        if let Some(url) = server.get("url").and_then(Value::as_str)
            && let Some(resolved) = resolve_server_url(url, server.get("variables"))
        {
            {
                return Some(resolved);
            }
        }
    }
    None
}

fn collect_base_urls_from_servers(servers: &[Value]) -> Vec<String> {
    let mut urls = Vec::new();
    let mut seen = HashSet::new();

    for server in servers {
        let Some(url) = server.get("url").and_then(Value::as_str) else {
            continue;
        };
        let Some(resolved) = resolve_server_url(url, server.get("variables")) else {
            continue;
        };
        let Some(normalized) = normalize_base_url(&resolved) else {
            continue;
        };
        if seen.insert(normalized.clone()) {
            urls.push(normalized);
        }
    }

    urls
}

fn resolve_server_url(url: &str, variables: Option<&Value>) -> Option<String> {
    let mut resolved = url.to_string();
    if let Some(variable_map) = variables.and_then(Value::as_object) {
        for (name, value) in variable_map {
            if let Some(default_value) = value.get("default").and_then(Value::as_str) {
                let placeholder = format!("{{{}}}", name);
                resolved = resolved.replace(&placeholder, default_value);
            }
        }
    }

    if resolved.contains('{') || resolved.contains('}') {
        return None;
    }
    Some(resolved)
}

fn normalize_base_url(raw_url: &str) -> Option<String> {
    let trimmed = raw_url.trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Some(trimmed.to_string());
    }
    None
}

fn build_target_schema_from_oas3(root: &Value, operation: &Map<String, Value>) -> Option<Value> {
    let responses = operation.get("responses")?.as_object()?;
    let preferred = ["200", "201", "202", "204"];
    let mut response_schema: Option<&Value> = None;

    for key in preferred {
        if let Some(response) = responses.get(key) {
            response_schema = Some(response);
            break;
        }
    }

    if response_schema.is_none() {
        for (key, response) in responses {
            if key.starts_with('2') {
                response_schema = Some(response);
                break;
            }
        }
    }

    let response = response_schema?;
    let schema = response
        .get("content")
        .and_then(|content| content.get("application/json"))
        .and_then(|content| content.get("schema"))?;
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        resolve_local_ref(root, reference)
    } else {
        Some(schema.clone())
    }
}

fn is_supported_http_method(method: &str) -> bool {
    matches!(method, "get" | "post" | "put" | "patch" | "delete")
}

fn format_openapi_preflight_violations(violations: &[OpenApiValidationViolation]) -> String {
    violations
        .iter()
        .map(|violation| format!("{} [{}]: {}", violation.path, violation.rule, violation.message))
        .collect::<Vec<String>>()
        .join("; ")
}

fn resolve_local_ref(root: &Value, reference: &str) -> Option<Value> {
    let pointer = reference.strip_prefix('#').unwrap_or(reference);
    root.pointer(pointer).cloned()
}

fn collect_parameters(root: &Value, path_item: &Value, operation: &Map<String, Value>) -> Vec<Value> {
    let mut collected: Vec<Value> = Vec::new();
    let mut seen: Vec<(String, String)> = Vec::new();

    let mut push_parameter = |parameter: Value| {
        let name = parameter.get("name").and_then(Value::as_str).unwrap_or("").to_string();
        let location = parameter.get("in").and_then(Value::as_str).unwrap_or("").to_string();

        if !name.is_empty() && !location.is_empty() {
            if let Some(index) = seen.iter().position(|(n, loc)| n == &name && loc == &location) {
                collected[index] = parameter;
            } else {
                collected.push(parameter);
                seen.push((name, location));
            }
        }
    };

    if let Some(params) = path_item.get("parameters").and_then(Value::as_array) {
        for parameter in params {
            let resolved = if let Some(reference) = parameter.get("$ref").and_then(Value::as_str) {
                resolve_local_ref(root, reference).unwrap_or_else(|| parameter.clone())
            } else {
                parameter.clone()
            };
            push_parameter(resolved);
        }
    }

    if let Some(params) = operation.get("parameters").and_then(Value::as_array) {
        for parameter in params {
            let resolved = if let Some(reference) = parameter.get("$ref").and_then(Value::as_str) {
                resolve_local_ref(root, reference).unwrap_or_else(|| parameter.clone())
            } else {
                parameter.clone()
            };
            push_parameter(resolved);
        }
    }

    collected
}

fn sanitize_placeholder_name(name: &str) -> String {
    name.trim_matches(|c: char| matches!(c, '{' | '}' | ' ')).replace('-', "_")
}

fn classify_command(path: &str, method: &str) -> Option<(String, String)> {
    let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    if segments.is_empty() {
        return None;
    }

    let group = segments
        .iter()
        .rev()
        .find(|segment| !segment.starts_with('{'))
        .map_or(segments[0], |segment| segment)
        .to_string();

    let group = if group == "config-vars" { "config".into() } else { group };
    let is_resource = segments.last().map(|segment| segment.starts_with('{')) == Some(true);
    let ends_with_collection = segments.last().map(|segment| !segment.starts_with('{')) == Some(true);

    let action = match method {
        "GET" if is_resource => "info",
        "GET" if ends_with_collection => "list",
        "POST" => "create",
        "PATCH" | "PUT" => "update",
        "DELETE" => "delete",
        _ => return None,
    };

    Some((group, action.to_string()))
}

fn concrete_segments(path: &str) -> Vec<String> {
    path.trim_start_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty() && !segment.starts_with('{') && !is_version_segment(segment))
        .map(str::to_string)
        .collect()
}

fn is_version_segment(segment: &str) -> bool {
    let value = segment.trim();
    value.len() > 1 && value.starts_with('v') && value[1..].chars().all(|c| c.is_ascii_digit())
}

fn normalize_group(group: &str) -> String {
    if group == "config-vars" {
        "config".to_string()
    } else {
        group.to_string()
    }
}

fn derive_command_group_and_name(vendor: &str, path: &str, action: &str) -> (String, String) {
    let segments: Vec<String> = concrete_segments(path)
        .into_iter()
        .map(|segment| normalize_group(&segment))
        .collect();
    let group = if vendor.is_empty() {
        "misc".to_string()
    } else {
        vendor.to_string()
    };
    let name = if segments.is_empty() {
        action.to_string()
    } else {
        format!("{}:{}", segments.join(":"), action)
    };

    (group, name)
}

fn singularize(segment: &str) -> String {
    let value = segment.trim_matches(|c: char| matches!(c, '{' | '}' | ' '));
    let value = value.replace('-', "_");
    if value.ends_with('s') && value.len() > 1 {
        value[..value.len() - 1].to_string()
    } else {
        value
    }
}

fn assign_unique_short_names(flags: &mut [CommandFlag]) {
    let mut used = HashSet::new();

    for flag in flags.iter_mut() {
        if let Some(existing) = flag.short_name.take() {
            let normalized = existing.to_ascii_lowercase();
            if is_valid_short_name(&normalized) && used.insert(normalized.clone()) {
                flag.short_name = Some(normalized);
            }
        }
    }

    for flag in flags.iter_mut() {
        if flag.short_name.is_some() {
            continue;
        }

        for candidate in generate_short_name_candidates(&flag.name) {
            if used.insert(candidate.clone()) {
                flag.short_name = Some(candidate);
                break;
            }
        }
    }
}

fn generate_short_name_candidates(name: &str) -> Vec<String> {
    let sanitized = sanitize_flag_name(name);
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    if !sanitized.is_empty() {
        for length in 1..=sanitized.len().min(3) {
            let candidate = sanitized[..length].to_string();
            push_candidate(&mut candidates, &mut seen, candidate);
        }
    }

    let segments: Vec<&str> = name.split(|c: char| ['-', '_', '.'].contains(&c)).collect();
    if segments.len() > 1 {
        let mut initials = String::new();
        for segment in segments {
            if let Some(ch) = segment.chars().find(|c| c.is_ascii_alphabetic()) {
                initials.push(ch.to_ascii_lowercase());
                if initials.len() == 3 {
                    break;
                }
            }
        }
        if !initials.is_empty() {
            for length in 1..=initials.len() {
                let candidate = initials[..length].to_string();
                push_candidate(&mut candidates, &mut seen, candidate);
            }
        }
    }

    if let Some(first) = sanitized.chars().next() {
        for suffix in 1..=9 {
            let candidate = format!("{}{}", first, suffix);
            push_candidate(&mut candidates, &mut seen, candidate);
        }
    }

    candidates
}

fn sanitize_flag_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn push_candidate(candidates: &mut Vec<String>, seen: &mut HashSet<String>, candidate: String) {
    if candidate.is_empty() {
        return;
    }
    if candidate.chars().count() > 3 {
        return;
    }
    if !candidate.chars().all(|c| c.is_ascii_alphanumeric()) {
        return;
    }
    if seen.insert(candidate.clone()) {
        candidates.push(candidate);
    }
}

fn is_valid_short_name(candidate: &str) -> bool {
    let length = candidate.chars().count();
    (1..=3).contains(&length) && candidate.chars().all(|c| c.is_ascii_alphanumeric())
}

fn resolve_schema_properties(schema: &Value, root: &Value) -> Value {
    let mut merged = schema.clone();
    if let Some(pointer) = schema.get("$ref").and_then(Value::as_str) {
        let pointer = pointer.strip_prefix('#').unwrap_or(pointer);
        if let Some(target) = root.pointer(pointer)
            && let Some(merged_object) = merged.as_object_mut()
        {
            let target_object = target.as_object().cloned().unwrap_or_default();
            for (key, value) in target_object {
                merged_object.entry(key).or_insert(value);
            }
        }
    }
    merged
}

fn collect_properties(schema: &Value) -> Option<Map<String, Value>> {
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        return Some(properties.clone());
    }
    for key in ["allOf", "anyOf", "oneOf"] {
        if let Some(items) = schema.get(key).and_then(Value::as_array) {
            let mut merged = Map::new();
            for item in items {
                if let Some(properties) = item.get("properties").and_then(Value::as_object) {
                    for (property_key, property_value) in properties {
                        merged.entry(property_key.clone()).or_insert_with(|| property_value.clone());
                    }
                }
            }
            if !merged.is_empty() {
                return Some(merged);
            }
        }
    }
    None
}

fn collect_required(schema: &Value) -> Vec<String> {
    let mut required: Vec<String> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(|value| value.as_str().map(str::to_string)).collect())
        .unwrap_or_default();

    for key in ["allOf", "anyOf", "oneOf"] {
        if let Some(items) = schema.get(key).and_then(Value::as_array) {
            for item in items {
                required.extend(
                    item.get("required")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flat_map(|values| values.iter().filter_map(|value| value.as_str().map(str::to_string))),
                );
            }
        }
    }

    required.sort();
    required.dedup();
    required
}

fn get_enum_values(schema: &Value, root: &Value) -> Vec<String> {
    if let Some(pointer) = schema.get("$ref").and_then(Value::as_str) {
        let pointer = pointer.strip_prefix('#').unwrap_or(pointer);
        return root.pointer(pointer).map_or(Vec::new(), |target| get_enum_values(target, root));
    }

    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        return values.iter().filter_map(|value| value.as_str().map(str::to_string)).collect();
    }

    ["anyOf", "oneOf"]
        .iter()
        .filter_map(|key| schema.get(*key).and_then(Value::as_array))
        .flat_map(|values| values.iter().flat_map(|item| get_enum_values(item, root)))
        .collect()
}

fn get_default(schema: &Value, root: &Value) -> Option<String> {
    if let Some(pointer) = schema.get("$ref").and_then(Value::as_str) {
        let pointer = pointer.strip_prefix('#').unwrap_or(pointer);
        return root.pointer(pointer).and_then(|target| get_default(target, root));
    }

    if let Some(default) = schema.get("default") {
        return default
            .as_str()
            .map(str::to_string)
            .or_else(|| default.as_bool().map(|value| value.to_string()))
            .or_else(|| default.as_i64().map(|value| value.to_string()))
            .or_else(|| default.as_u64().map(|value| value.to_string()))
            .or_else(|| default.as_f64().map(|value| value.to_string()));
    }

    for key in ["anyOf", "oneOf"] {
        if let Some(values) = schema.get(key).and_then(Value::as_array) {
            for item in values {
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
    use super::collect_base_urls_from_document;
    use serde_json::json;

    #[test]
    fn collects_base_urls_from_document_servers() {
        let document = json!({
            "openapi": "3.0.0",
            "servers": [
                { "url": "https://api.example.com/" },
                { "url": "https://{region}.example.com", "variables": { "region": { "default": "eu" } } },
                { "url": "https://api.example.com" }
            ]
        });

        let base_urls = collect_base_urls_from_document(&document);
        assert_eq!(base_urls, vec!["https://api.example.com", "https://eu.example.com"]);
    }
}
