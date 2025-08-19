use anyhow::{anyhow, Context, Result};
use clap::{Arg, ArgAction, Command as ClapCommand};
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandFlag {
    pub name: String,
    pub required: bool,
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub enum_values: Vec<String>,
    #[serde(default)]
    pub default_value: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    pub name: String, // e.g., "apps:list"
    pub summary: String,
    #[serde(default)]
    pub positional_args: Vec<String>,
    #[serde(default)]
    pub positional_help: HashMap<String, String>,
    #[serde(default)]
    pub flags: Vec<CommandFlag>,
    pub method: String, // GET/POST/DELETE/...
    pub path: String,   // e.g., "/apps" or "/apps/{app}"
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Registry {
    pub commands: Vec<CommandSpec>,
}

impl Registry {
    pub fn load_from_hyper_schema_str(schema: &str) -> Result<Self> {
        let v: serde_json::Value = serde_json::from_str(schema).context("parse schema json")?;
        let cmds = derive_commands_from_schema(&v)?;
        Ok(Registry { commands: cmds })
    }

    pub fn from_embedded_schema() -> Result<Self> {
        let schema = include_str!("heroku-schema.json");
        Self::load_from_hyper_schema_str(schema)
    }

    pub fn build_clap(&self) -> ClapCommand {
        let mut root = ClapCommand::new("heroku")
            .about("Heroku CLI (experimental)")
            .arg(
                Arg::new("json")
                    .long("json")
                    .help("JSON output")
                    .global(true)
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("dry-run")
                    .long("dry-run")
                    .help("Do not execute, print requests")
                    .global(true)
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("verbose")
                    .long("verbose")
                    .help("Verbose logging")
                    .global(true)
                    .action(ArgAction::SetTrue),
            );

        // Group commands by resource prefix (before ':')
        use std::collections::BTreeMap;
        let mut groups: BTreeMap<String, Vec<&CommandSpec>> = BTreeMap::new();
        for cmd in &self.commands {
            let mut parts = cmd.name.splitn(2, ':');
            let group = parts.next().unwrap_or("misc").to_string();
            groups.entry(group).or_default().push(cmd);
        }

        // Clap requires us to leak the command names which is fine
        // since we're only doing this once throughout the life
        // of the program.
        for (group, cmds) in groups {
            let static_command_name: &'static str = Box::leak(group.into_boxed_str());
            let mut g = ClapCommand::new(static_command_name);
            for cmd in cmds {
                let subname = cmd.name.splitn(2, ':').nth(1).unwrap_or("run").to_string();
                let static_sub_name: &'static str = Box::leak(subname.into_boxed_str());
                let mut sc = ClapCommand::new(static_sub_name).about(&cmd.summary);
                // positional args
                for (i, pa) in cmd.positional_args.iter().enumerate() {
                    let arg: &'static str = Box::leak(pa.clone().into_boxed_str());
                    sc = sc.arg(Arg::new(arg).required(true).index((i + 1) as usize));
                }
                // flags
                for f in &cmd.flags {
                    let name: &'static str = Box::leak(f.name.clone().into_boxed_str());
                    let mut a = Arg::new(name).long(name).required(f.required);
                    a = if f.r#type == "boolean" {
                        a.action(ArgAction::SetTrue)
                    } else {
                        a.action(ArgAction::Set)
                    };
                    if !f.enum_values.is_empty() {
                        // Leak enum strings to satisfy 'static lifetime required by Clap builders
                        let values: Vec<&'static str> = f
                            .enum_values
                            .iter()
                            .map(|s| Box::leak(s.clone().into_boxed_str()) as &'static str)
                            .collect();
                        a = a.value_parser(clap::builder::PossibleValuesParser::new(values));
                    }
                    if f.r#type != "boolean" {
                        if let Some(def) = &f.default_value {
                            let dv: &'static str = Box::leak(def.clone().into_boxed_str());
                            a = a.default_value(dv);
                        }
                    }
                    let help_text = if let Some(desc) = &f.description {
                        desc.clone()
                    } else {
                        format!("type: {}", f.r#type)
                    };
                    sc = sc.arg(a.help(help_text));
                }
                // Store method/path in about? We'll resolve at runtime using name.
                g = g.subcommand(sc);
            }
            root = root.subcommand(g);
        }

        root
    }

    pub fn find_by_group_and_cmd(&self, group: &str, cmd: &str) -> Result<&CommandSpec> {
        let key = format!("{}:{}", group, cmd);
        self.commands
            .iter()
            .find(|c| c.name == key)
            .ok_or_else(|| anyhow!("command not found: {}", key))
    }
}

fn derive_commands_from_schema(v: &serde_json::Value) -> Result<Vec<CommandSpec>> {
    let mut cmds: Vec<CommandSpec> = Vec::new();

    // Collect every object that has a "links" array
    fn walk<'a>(val: &'a serde_json::Value, out: &mut Vec<&'a serde_json::Value>) {
        match val {
            serde_json::Value::Object(map) => {
                if map.contains_key("links") {
                    out.push(val);
                }
                for v in map.values() {
                    walk(v, out);
                }
            }
            serde_json::Value::Array(arr) => {
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
            let desc = link
                .get("description")
                .and_then(|x| x.as_str())
                .or_else(|| link.get("title").and_then(|x| x.as_str()))
                .unwrap_or("")
                .to_string();

            // Classify group/action from href + method
            if let Some((_, action)) = classify_command(href, method) {
                let (path_tmpl, positional_args, positional_help) =
                    path_and_vars_with_help(href, v);
                let (flags, _required_names) = extract_flags_resolved(link, v);

                // Skip commands without clear path
                if path_tmpl.is_empty() {
                    continue;
                }
                let name = derive_command_name(href, &action);
                let spec = CommandSpec {
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

    // Sort and remove exact duplicates (same name+method+path), but keep variations with unique names
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

    // Determine group: prefer the last concrete collection segment (non-placeholder)
    let mut group = None;
    for seg in segs.iter().rev() {
        if seg.starts_with("{") {
            continue;
        }
        // e.g., config-vars is a collection; use segment as group
        group = Some(seg.to_string());
        break;
    }
    let mut group = group.unwrap_or_else(|| segs[0].to_string());
    // Normalize group: use plural as given (apps, releases, dynos)
    if group == "config-vars" {
        group = "config".into();
    }

    // Determine action
    let is_resource = segs.last().map(|s| s.starts_with("{")) == Some(true);
    let ends_with_collection = segs.last().map(|s| !s.starts_with("{")) == Some(true);
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

fn derive_command_name(href: &str, action: &str) -> String {
    let segs = concrete_segments(href);
    if segs.is_empty() {
        return format!("misc:{}", action);
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
    format!("{}:{}", group, sub)
}
fn path_and_vars_with_help<'a>(
    href: &str,
    root: &'a serde_json::Value,
) -> (String, Vec<String>, HashMap<String, String>) {
    // Convert Heroku Hyper-Schema encoded placeholders to named args
    // Strategy: for every placeholder segment, use previous segment singularized
    let mut args: Vec<String> = Vec::new();
    let mut out_segs: Vec<String> = Vec::new();
    let mut help: HashMap<String, String> = HashMap::new();
    let mut prev: Option<&str> = None;
    for seg in href.trim_start_matches('/').split('/') {
        if seg.starts_with("{") {
            let name = prev
                .map(|s| singularize(s))
                .unwrap_or_else(|| "id".to_string());
            args.push(name.clone());
            out_segs.push(format!("{{{}}}", name));

            // Try to extract a JSON Pointer from placeholder and look up description
            if let Some(ptr_enc) = extract_placeholder_ptr(seg) {
                let decoded = percent_decode_str(&ptr_enc).decode_utf8_lossy().to_string();
                let ptr = decoded.strip_prefix('#').unwrap_or(&decoded);
                if let Some(val) = root.pointer(ptr) {
                    if let Some(desc) = val.get("description").and_then(|x| x.as_str()) {
                        help.insert(name.clone(), desc.to_string());
                    } else if let Some(any) = val.get("anyOf").and_then(|x| x.as_array()) {
                        let mut descs: Vec<String> = Vec::new();
                        for item in any {
                            // Prefer $ref descriptions, fall back to inline description
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
        s
    }
}

fn extract_placeholder_ptr(seg: &str) -> Option<String> {
    // Expects formats like {(%23%2Fdefinitions%2Fapp%2Fdefinitions%2Fidentity)} or {#/definitions/...}
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

fn extract_flags_resolved(
    link: &serde_json::Value,
    root: &serde_json::Value,
) -> (Vec<CommandFlag>, Vec<String>) {
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
                // Resolve $ref if present
                let mut merged = val.clone();
                if let Some(r) = val.get("$ref").and_then(|x| x.as_str()) {
                    let ptr = r.strip_prefix('#').unwrap_or(r);
                    if let Some(target) = root.pointer(ptr) {
                        // Merge: referenced values provide defaults for missing fields
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
                // Convert default into a string when possible
                let mut default_value = merged.get("default").and_then(|x| {
                    if let Some(s) = x.as_str() {
                        Some(s.to_string())
                    } else if let Some(b) = x.as_bool() {
                        Some(b.to_string())
                    } else if let Some(n) = x.as_i64() {
                        Some(n.to_string())
                    } else if let Some(n) = x.as_u64() {
                        Some(n.to_string())
                    } else if let Some(n) = x.as_f64() {
                        Some(n.to_string())
                    } else {
                        None
                    }
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
#[cfg(test)]
mod tests {
    use crate::Registry;

    #[test]
    fn test_registry() -> Result<(), ()> {
        let registry = Registry::from_embedded_schema().unwrap();
        let cli = registry.build_clap();
        Ok(())
    }
}
