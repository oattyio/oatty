use anyhow::{anyhow, Context, Result};
use heroku_registry_types::CommandSpec;
use serde_json;

/// The main registry containing all available Heroku CLI commands.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Registry {
    /// Collection of all available command specifications
    pub commands: Vec<CommandSpec>,
}

impl Registry {
    /// Creates a new Registry instance by loading command definitions from the embedded schema.
    ///
    /// This method reads the Heroku API manifest that was embedded during the build process
    /// and deserializes it into a Registry. If the workflows feature is enabled, it also
    /// adds synthetic workflow commands.
    ///
    /// # Returns
    ///
    /// - `Ok(Registry)` - Successfully loaded registry with all commands
    /// - `Err` - If the embedded manifest cannot be parsed or is invalid
    ///
    /// # Examples
    ///
    /// ```rust
    /// use registry::Registry;
    ///
    /// let registry = Registry::from_embedded_schema()?;
    /// println!("Loaded {} commands", registry.commands.len());
    /// ```
    pub fn from_embedded_schema() -> Result<Self> {
        let manifest = include_str!(concat!(env!("OUT_DIR"), "/heroku-manifest.json"));
        let mut reg: Registry =
            serde_json::from_str(manifest).context("parse embedded manifest")?;
        if crate::workflows::feature_workflows() {
            reg.add_workflow_commands();
        }
        Ok(reg)
    }

    /// Finds a specific command by its group and command name.
    ///
    /// This method searches for a command using the format "group:command" where
    /// group is the resource type (e.g., "apps", "dynos") and command is the action
    /// (e.g., "list", "create").
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
    /// use registry::Registry;
    ///
    /// let registry = Registry::from_embedded_schema()?;
    /// let apps_list = registry.find_by_group_and_cmd("apps", "list")?;
    /// println!("Found command: {}", apps_list.name);
    /// ```
    pub fn find_by_group_and_cmd(&self, group: &str, cmd: &str) -> Result<&CommandSpec> {
        self.commands
            .iter()
            .find(|c| c.group == group && c.name == cmd)
            .ok_or_else(|| anyhow!("command not found: {} {}", group, cmd))
    }

    /// Adds synthetic workflow commands to the registry.
    ///
    /// This method adds internal commands that are not HTTP API calls but provide
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
    fn add_workflow_commands(&mut self) {
        crate::workflows::add_workflow_commands(self);
    }
}
