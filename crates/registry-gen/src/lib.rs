use anyhow::{Context, Result};
use heck::ToKebabCase;
use heroku_registry_types::{CommandFlag, CommandSpec};
use percent_encoding::percent_decode_str;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Registry {
    pub commands: Vec<CommandSpec>,
}

pub fn generate_manifest(schema_json: &str) -> Result<String> {
    let v: Value = serde_json::from_str(schema_json).context("parse schema json")?;
    let commands = derive_commands_from_schema(&v)?;
    // Do NOT add workflow commands here; they are runtime-synthesized.
    let reg = Registry { commands };
    let json = serde_json::to_string_pretty(&reg)?;
    Ok(json)
}

fn derive_commands_from_schema(v: &Value) -> Result<Vec<CommandSpec>> {
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
            let title = link
                .get("title")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();

            let desc = link
                .get("description")
                .and_then(|x| x.as_str())
                .or_else(|| Some(&title))
                .unwrap_or("")
                .to_string();

            if let Some((_, action)) = classify_command(href, method) {
                let (path_tmpl, positional_args, positional_help) =
                    path_and_vars_with_help(href, v);
                let (flags, _required_names) = extract_flags_resolved(link, v);

                if path_tmpl.is_empty() {
                    continue;
                }
                let (mut group, mut name) =
                    derive_command_group_and_name(href, &title.to_kebab_case());
                if cmd_names.insert(name.clone(), true).is_some() {
                    (group, name) = derive_command_group_and_name(href, &action);
                }
                let spec = CommandSpec {
                    group,
                    name,
                    summary: desc.clone(),
                    positional_args,
                    positional_help,
                    flags,
                    method: method.to_string(),
                    path: path_tmpl,
                };
                cmds.push(spec);
            }
        }
    }

    cmds.sort_by(|a, b| a.name.cmp(&b.name));
    cmds.dedup_by(|a, b| a.name == b.name && a.method == b.method && a.path == b.path);
    Ok(cmds)
}

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

fn concrete_segments(href: &str) -> Vec<String> {
    href.trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty() && !s.starts_with('{'))
        .map(|s| s.to_string())
        .collect()
}

fn normalize_group(s: &str) -> String {
    if s == "config-vars" {
        "config".into()
    } else {
        s.to_string()
    }
}

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

fn path_and_vars_with_help(
    href: &str,
    root: &Value,
) -> (String, Vec<String>, HashMap<String, String>) {
    let mut args: Vec<String> = Vec::new();
    let mut out_segs: Vec<String> = Vec::new();
    let mut help: HashMap<String, String> = HashMap::new();
    let mut prev: Option<&str> = None;
    for seg in href.trim_start_matches('/').split('/') {
        if seg.starts_with('{') {
            let name = prev.map(singularize).unwrap_or_else(|| "id".to_string());
            args.push(name.clone());
            out_segs.push(format!("{{{}}}", name));

            if let Some(ptr_enc) = extract_placeholder_ptr(seg) {
                let decoded = percent_decode_str(&ptr_enc).decode_utf8_lossy().to_string();
                let ptr = decoded.strip_prefix('#').unwrap_or(&decoded);
                if let Some(val) = root.pointer(ptr) {
                    if let Some(desc) = val.get("description").and_then(|x| x.as_str()) {
                        help.insert(name.clone(), desc.to_string());
                    } else if let Some(any) = val.get("anyOf").and_then(|x| x.as_array()) {
                        let mut descs: Vec<String> = Vec::new();
                        for item in any {
                            if let Some(r) = item.get("$ref").and_then(|x| x.as_str()) {
                                let p = r.strip_prefix('#').unwrap_or(r);
                                if let Some(t) = root.pointer(p) {
                                    if let Some(d) = t.get("description").and_then(|x| x.as_str()) {
                                        descs.push(d.to_string());
                                    }
                                }
                            } else if let Some(d) = item.get("description").and_then(|x| x.as_str())
                            {
                                descs.push(d.to_string());
                            }
                        }
                        if !descs.is_empty() {
                            let combined = descs.join(" or ");
                            help.insert(name.clone(), combined);
                        }
                    }
                }
            }
        } else {
            out_segs.push(seg.to_string());
        }
        prev = Some(seg);
    }
    (format!("/{}", out_segs.join("/")), args, help)
}

fn singularize(s: &str) -> String {
    let s = s.trim_matches(|c: char| c == '{' || c == '}' || c == ' ');
    let s = s.replace('-', "_");
    if s.ends_with('s') && s.len() > 1 {
        s[..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn extract_placeholder_ptr(seg: &str) -> Option<String> {
    let inner = seg.trim_start_matches('{').trim_end_matches('}');
    let inner = inner.trim();
    let ptr = if inner.starts_with('(') && inner.ends_with(')') {
        inner
            .trim_start_matches('(')
            .trim_end_matches(')')
            .to_string()
    } else {
        inner.to_string()
    };
    if ptr.is_empty() {
        None
    } else {
        Some(ptr)
    }
}

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
                        if merged.get("type").is_none() {
                            if let Some(t) = target.get("type") {
                                merged
                                    .as_object_mut()
                                    .unwrap()
                                    .insert("type".into(), t.clone());
                            }
                        }
                        if merged.get("description").is_none() {
                            if let Some(d) = target.get("description") {
                                merged
                                    .as_object_mut()
                                    .unwrap()
                                    .insert("description".into(), d.clone());
                            }
                        }
                        if merged.get("enum").is_none() {
                            if let Some(e) = target.get("enum") {
                                merged
                                    .as_object_mut()
                                    .unwrap()
                                    .insert("enum".into(), e.clone());
                            }
                        }
                        if merged.get("default").is_none() {
                            if let Some(df) = target.get("default") {
                                merged
                                    .as_object_mut()
                                    .unwrap()
                                    .insert("default".into(), df.clone());
                            }
                        }
                    }
                }

                let ty = merged
                    .get("type")
                    .and_then(|x| x.as_str())
                    .unwrap_or("string");
                let required = required_names.iter().any(|n| n == name);
                let enum_values = merged
                    .get("enum")
                    .and_then(|x| x.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let mut default_value = merged.get("default").and_then(|x| {
                    x.as_str()
                        .map(|s| s.to_string())
                        .or_else(|| x.as_bool().map(|b| b.to_string()))
                        .or_else(|| x.as_i64().map(|n| n.to_string()))
                        .or_else(|| x.as_u64().map(|n| n.to_string()))
                        .or_else(|| x.as_f64().map(|n| n.to_string()))
                });
                let description = merged
                    .get("description")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string());
                if default_value.is_none() && !enum_values.is_empty() {
                    default_value = Some(enum_values[0].clone());
                }
                flags.push(CommandFlag {
                    name: name.clone(),
                    required,
                    r#type: ty.to_string(),
                    enum_values,
                    default_value,
                    description,
                });
            }
        }
    }
    (flags, required_names)
}
