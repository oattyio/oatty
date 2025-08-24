//! Command preview and request generation for the Heroku TUI.
//!
//! This module provides functionality for generating previews of commands
//! and HTTP requests that would be executed. It helps users understand
//! what will happen before running commands.

use serde_json::Map;

/// Resolves path template placeholders with actual values.
///
/// This function replaces placeholder tokens in a path template (e.g., "{app}")
/// with the corresponding values from the provided map.
///
/// # Arguments
///
/// * `template` - The path template containing placeholders
/// * `pos` - Map of placeholder names to their values
///
/// # Returns
///
/// The resolved path with placeholders replaced by actual values.
///
/// # Examples
///
/// ```rust
/// use std::collections::HashMap;
/// use crate::preview::resolve_path;
///
/// let mut params = HashMap::new();
/// params.insert("app".to_string(), "my-app".to_string());
///
/// let template = "/apps/{app}/dynos";
/// let resolved = resolve_path(template, &params);
/// assert_eq!(resolved, "/apps/my-app/dynos");
/// ```
pub fn resolve_path(template: &str, pos: &std::collections::HashMap<String, String>) -> String {
    let mut out = template.to_string();
    for (k, v) in pos {
        let needle = format!("{{{}}}", k);
        out = out.replace(&needle, v);
    }
    out
}

/// Generates a CLI command preview from a command specification and field values.
///
/// This function creates a human-readable representation of the command that
/// would be executed, including all arguments and flags with their values.
///
/// # Arguments
///
/// * `spec` - The command specification containing metadata
/// * `fields` - The current field values for the command
///
/// # Returns
///
/// A formatted string representing the CLI command.
///
/// # Examples
///
/// ```rust
/// use heroku_registry::CommandSpec;
/// use crate::app::Field;
/// use crate::preview::cli_preview;
///
/// let spec = CommandSpec {
///     name: "apps:info".to_string(),
///     // ... other fields
/// };
///
/// let fields = vec![
///     Field {
///         name: "app".to_string(),
///         value: "my-app".to_string(),
///         // ... other fields
///     }
/// ];
///
/// let preview = cli_preview(&spec, &fields);
/// assert_eq!(preview, "heroku apps info --app=my-app");
/// ```
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

/// Generates a JSON preview of the HTTP request that would be made.
///
/// This function creates a detailed JSON representation of the HTTP request
/// including method, URL, headers, and body that would be sent to the
/// Heroku API.
///
/// # Arguments
///
/// * `spec` - The command specification containing HTTP metadata
/// * `path` - The resolved API path
/// * `body` - The request body as a JSON map
///
/// # Returns
///
/// A formatted JSON string representing the HTTP request.
///
/// # Examples
///
/// ```rust
/// use heroku_registry::CommandSpec;
/// use serde_json::Map;
/// use crate::preview::request_preview;
///
/// let spec = CommandSpec {
///     method: "GET".to_string(),
///     // ... other fields
/// };
///
/// let path = "/apps/my-app";
/// let mut body = Map::new();
///
/// let preview = request_preview(&spec, path, &body);
/// // Returns JSON with method, URL, headers, and body
/// ```
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
