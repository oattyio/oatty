use std::sync::Arc;

use anyhow::{Context, Result};
use heroku_types::{CommandSpec, ProviderContract, manifest::RegistryManifest, workflow::WorkflowDefinition};
use heroku_util::sort_and_dedup_commands;
use indexmap::IndexMap;

static MANIFEST: &str = include_str!(concat!(env!("OUT_DIR"), "/heroku-manifest.json"));
/// The main registry containing all available Heroku CLI commands.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct CommandRegistry {
    /// Collection of all available command specifications
    pub commands: Vec<CommandSpec>,
    /// Workflow definitions bundled with the registry manifest
    pub workflows: Vec<WorkflowDefinition>,
    /// Provider argument and return contracts keyed by command identifier
    pub provider_contracts: IndexMap<String, ProviderContract>,
}

impl CommandRegistry {
    /// Creates a new Registry instance by loading command definitions from the
    /// embedded schema.
    ///
    /// This method reads the Heroku API manifest that was embedded during the
    /// build process and deserializes it into a Registry. If the workflows
    /// feature is enabled, it also adds synthetic workflow commands.
    ///
    /// # Returns
    ///
    /// - `Ok(Registry)` - Successfully loaded registry with all commands
    /// - `Err` - If the embedded manifest cannot be parsed or is invalid
    ///
    /// # Examples
    ///
    /// ```rust
    /// use heroku_registry::CommandRegistry;
    ///
    /// let registry = CommandRegistry::from_embedded_schema().expect("load registry from schema");
    /// println!("Loaded {} commands", registry.commands.len());
    /// ```
    pub fn from_embedded_schema() -> Result<Self> {
        let manifest: RegistryManifest = serde_json::from_str(MANIFEST).context("decoding manifest failed")?;

        let provider_contracts = manifest
            .provider_contracts
            .into_iter()
            .map(|entry| (entry.command_id, entry.contract))
            .collect();

        Ok(CommandRegistry {
            commands: manifest.commands,
            workflows: manifest.workflows,
            provider_contracts,
        })
    }

    /// Inserts the synthetic commands from an MCP client's
    /// tool definitions and deduplicates them.
    pub fn insert_synthetic(&mut self, synthesized: Vec<CommandSpec>) {
        self.commands.extend(synthesized);
        sort_and_dedup_commands(&mut self.commands);
    }

    /// Removes the synthetic commands from the vec
    pub fn remove_synthetic(&mut self, maybe_synthesized: Option<Arc<[CommandSpec]>>) {
        if maybe_synthesized.is_none() {
            return;
        }
        let synthesized = &*maybe_synthesized.unwrap();
        self.commands.retain(|c| !synthesized.contains(c));
    }
}
