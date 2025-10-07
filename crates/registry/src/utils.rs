use anyhow::{Result, anyhow};
use heroku_types::CommandSpec;

/// Finds a specific command by its group and command name.
///
/// This method searches for a command using the format "group:command"
/// where group is the resource type (e.g., "apps", "dynos") and command
/// is the action (e.g., "list", "create").
///
/// # Arguments
///
/// * `group` - The resource group name (e.g., "apps", "dynos", "config")
/// * `cmd` - The command action name (e.g., "list", "create", "restart")
///
/// # Returns
///
/// - `Ok(&CommandSpec)` - The matching command specification
/// - `Err` - If no command is found with the given group and command name
///
/// # Examples
///
/// ```rust
/// use heroku_registry::{Registry, utils::find_by_group_and_cmd};
///
/// let registry = Registry::from_embedded_schema().expect("load registry from schema");
/// let apps_list = find_by_group_and_cmd(&registry.commands, "apps", "list").expect("find by group and command");
/// println!("Found command: {}", apps_list.name);
/// ```
pub fn find_by_group_and_cmd(commands: &[CommandSpec], group: &str, cmd: &str) -> Result<CommandSpec> {
    commands
        .iter()
        .find(|c| c.group == group && c.name == cmd)
        .cloned()
        .ok_or(anyhow!("{}:{} command not found", group, cmd))
}
