use anyhow::Result;
use indexmap::IndexMap;
use oatty_types::{CommandSpec, ProviderContract, manifest::RegistryManifest, workflow::WorkflowDefinition};
use oatty_util::sort_and_dedup_commands;
use std::{convert::Infallible, sync::Arc};

use crate::RegistryConfig;

/// The main registry containing all available Oatty CLI commands.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct CommandRegistry {
    /// Collection of all available command specifications
    pub commands: Vec<CommandSpec>,
    /// Workflow definitions bundled with the registry manifest
    pub workflows: Vec<WorkflowDefinition>,
    /// Provider argument and return contracts keyed by command identifier
    pub provider_contracts: IndexMap<String, ProviderContract>,
    /// Config used to identify locations of each command catalog
    pub config: RegistryConfig,
}

impl CommandRegistry {
    /// Creates a new Registry instance by loading command definitions from the
    /// embedded schema.
    ///
    /// This method reads the Oatty API manifest that was embedded during the
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
    /// use oatty_registry::CommandRegistry;
    ///
    /// let registry = CommandRegistry::from_config().expect("load registry from schema");
    /// println!("Loaded {} commands", registry.commands.len());
    /// ```
    pub fn from_config() -> Result<Self, Infallible> {
        let config = RegistryConfig::load()?;
        let Some(catalogs) = config.catalogs.as_ref() else {
            return Ok(CommandRegistry {
                config,
                ..Default::default()
            });
        };

        let mut commands = Vec::new();
        let mut workflows = Vec::new();
        let mut provider_contracts = IndexMap::new();

        for catalog in catalogs {
            let path = &catalog.manifest_path;
            let Ok(manifest_bytes) = std::fs::read(path) else {
                continue;
            };
            let Ok(mut manifest) = RegistryManifest::try_from(manifest_bytes) else {
                continue;
            };
            commands.append(&mut manifest.commands);
            workflows.append(&mut manifest.workflows);
            provider_contracts.append(&mut manifest.provider_contracts);
        }

        Ok(CommandRegistry {
            config,
            commands,
            workflows,
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
