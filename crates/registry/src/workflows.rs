use crate::models::Registry;
use heroku_registry_types::{CommandFlag, CommandSpec};
use std::collections::HashMap;

/// Checks if the workflows feature is enabled via environment variable.
///
/// This function checks the `FEATURE_WORKFLOWS` environment variable to determine
/// whether workflow-related functionality should be enabled. The feature is enabled
/// if the variable is set to "1" or "true" (case-insensitive).
///
/// # Returns
///
/// `true` if workflows are enabled, `false` otherwise.
///
/// # Examples
///
/// ```rust
/// use registry::workflows::feature_workflows;
///
/// if feature_workflows() {
///     println!("Workflows are enabled");
/// } else {
///     println!("Workflows are disabled");
/// }
/// ```
pub fn feature_workflows() -> bool {
    std::env::var("FEATURE_WORKFLOWS")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
}

/// Adds synthetic workflow commands to the registry.
///
/// This function adds internal commands that are not HTTP API calls but provide
/// workflow functionality. These commands are only added when the workflows
/// feature is enabled via the `FEATURE_WORKFLOWS` environment variable.
///
/// The added commands include:
/// - `workflow:list` - Lists available workflows
/// - `workflow:preview` - Previews a workflow plan
/// - `workflow:run` - Executes a workflow
///
/// These commands use placeholder method and path values since they don't
/// correspond to actual HTTP endpoints.
///
/// # Arguments
///
/// * `registry` - The registry to add workflow commands to
pub fn add_workflow_commands(registry: &mut Registry) {
    // Synthetic commands for local workflows. These are not HTTP calls,
    // but exposing them via the registry makes them available to the TUI.
    let mut add = |name: &str, summary: &str, flags: Vec<CommandFlag>| {
        let group = name.split(':').next().unwrap_or("misc").to_string();
        registry.commands.push(CommandSpec {
            group,
            name: name.to_string(),
            summary: summary.to_string(),
            positional_args: vec![],
            positional_help: HashMap::new(),
            flags,
            // Method/path are unused for internal commands; keep placeholders.
            method: "INTERNAL".into(),
            path: "__internal__".into(),
        });
    };

    // Common flags
    let file_flag = |required: bool| CommandFlag {
        name: "file".into(),
        required,
        r#type: "string".into(),
        enum_values: vec![],
        default_value: None,
        description: Some("Path to workflow YAML/JSON".into()),
    };
    let name_flag = |required: bool| CommandFlag {
        name: "name".into(),
        required,
        r#type: "string".into(),
        enum_values: vec![],
        default_value: None,
        description: Some("Workflow name within the file".into()),
    };

    add(
        "workflow:list",
        "List workflows in workflows/ directory",
        vec![],
    );
    add(
        "workflow:preview",
        "Preview workflow plan",
        vec![file_flag(false), name_flag(false)],
    );
    add(
        "workflow:run",
        "Run workflow (use global --dry-run)",
        vec![file_flag(false), name_flag(false)],
    );
}
