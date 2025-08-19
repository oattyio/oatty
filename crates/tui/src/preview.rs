use serde_json::Map;

pub fn resolve_path(template: &str, pos: &std::collections::HashMap<String, String>) -> String {
    let mut out = template.to_string();
    for (k, v) in pos {
        let needle = format!("{{{}}}", k);
        out = out.replace(&needle, v);
    }
    out
}

pub fn cli_preview(spec: &heroku_registry::CommandSpec, fields: &[crate::app::Field]) -> String {
    let mut parts = vec!["heroku".to_string()];
    // Clap-compatible: group + subcommand (rest may contain ':')
    let mut split = spec.name.splitn(2, ':');
    let group = split.next().unwrap_or("");
    let rest = split.next().unwrap_or("");
    if !group.is_empty() {
        parts.push(group.to_string());
    }
    if !rest.is_empty() {
        parts.push(rest.to_string());
    }
    for p in &spec.positional_args {
        if let Some(f) = fields.iter().find(|f| &f.name == p) {
            parts.push(if f.value.is_empty() {
                format!("<{}>", f.name)
            } else {
                f.value.clone()
            });
        }
    }
    for f in fields
        .iter()
        .filter(|f| !spec.positional_args.iter().any(|p| p == &f.name))
    {
        if f.is_bool {
            if !f.value.is_empty() {
                parts.push(format!("--{}", f.name));
            }
        } else if !f.value.is_empty() {
            parts.push(format!("--{}={}", f.name, f.value));
        }
    }
    parts.join(" ")
}

pub fn request_preview(
    spec: &heroku_registry::CommandSpec,
    path: &str,
    body: &Map<String, serde_json::Value>,
) -> String {
    let out = serde_json::json!({
        "method": spec.method,
        "url": format!("https://api.heroku.com{}", path),
        "headers": {
            "Accept": "application/vnd.heroku+json; version=3",
            "User-Agent": "heroku-cli-tui/0.1"
        },
        "body": if body.is_empty() { serde_json::Value::Null } else { serde_json::Value::Object(body.clone()) }
    });
    serde_json::to_string_pretty(&out).unwrap_or_default()
}
