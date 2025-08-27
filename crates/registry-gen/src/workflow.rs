use heroku_types::{CommandFlag, CommandSpec};
use std::collections::HashMap;

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
/// * `commands` - The vector of `CommandSpec` to add workflow commands to.
pub fn add_workflow_commands(commands: &mut Vec<CommandSpec>) {
    let mut add = |name: &str, summary: &str, flags: Vec<CommandFlag>| {
        let group = name.split(':').next().unwrap_or("misc").to_string();
        commands.push(CommandSpec {
            group,
            name: name.to_string(),
            summary: summary.to_string(),
            positional_args: vec![],
            positional_help: HashMap::new(),
            flags,
            method: "INTERNAL".into(),
            path: "__internal__".into(),
        });
    };

    let file_flag = |required: bool| CommandFlag {
        name: "file".into(),
        short_name: Some("f".into()),
        required,
        r#type: "string".into(),
        enum_values: vec![],
        default_value: None,
        description: Some("Path to workflow YAML/JSON".into()),
    };
    let name_flag = |required: bool| CommandFlag {
        name: "name".into(),
        short_name: Some("n".into()),
        required,
        r#type: "string".into(),
        enum_values: vec![],
        default_value: None,
        description: Some("Workflow name within the file".into()),
    };

    add("workflow:list", "List workflows in workflows/ directory", vec![]);
    add(
        "workflow:preview",
        "Preview workflow plan",
        vec![file_flag(false), name_flag(false)],
    );
    add("workflow:run", "Run workflow", vec![file_flag(false), name_flag(false)]);
}
