use anyhow::{Result, anyhow};
use indexmap::{IndexMap, IndexSet, set::MutableValues};
use oatty_types::{
    CommandSpec, EnvVar, ProviderContract,
    manifest::{RegistryCatalog, RegistryManifest},
    workflow::WorkflowDefinition,
};
use oatty_util::{interpolate_string, sort_and_dedup_commands};
use std::{collections::HashSet, convert::Infallible, path::Path};

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
        Self::from_registry_config(config)
    }

    /// Creates a registry instance from the provided configuration.
    pub fn from_registry_config(mut config: RegistryConfig) -> Result<Self, Infallible> {
        let Some(catalogs) = config.catalogs.as_mut() else {
            return Ok(CommandRegistry {
                config,
                ..Default::default()
            });
        };

        let mut commands = Vec::new();
        let mut workflows = Vec::new();
        let mut provider_contracts = IndexMap::new();

        for i in (0..catalogs.len()).rev() {
            let catalog = &mut catalogs[i];
            let path = &catalog.manifest_path;
            for j in 0..catalog.headers.len() {
                let Some(EnvVar { value, .. }) = catalog.headers.get_index_mut2(j) else {
                    continue;
                };
                let Ok(val) = interpolate_string(value) else {
                    continue;
                };
                *value = val;
            }

            let Ok(manifest_bytes) = std::fs::read(path) else {
                catalogs.swap_remove(i); // invalid - remove from registry
                continue;
            };
            match RegistryManifest::try_from(manifest_bytes) {
                Ok(mut manifest) => {
                    for command in &mut manifest.commands {
                        command.catalog_identifier = i;
                    }
                    if catalog.is_enabled {
                        commands.append(&mut manifest.commands.clone());
                        workflows.append(&mut manifest.workflows.clone());
                        provider_contracts.append(&mut manifest.provider_contracts.clone());
                    }
                    catalog.manifest = Some(manifest);
                }
                // We need to handle the error case here
                Err(_) => {
                    catalogs.swap_remove(i); // invalid - remove from registry
                    continue;
                }
            }
        }

        Ok(CommandRegistry {
            config,
            commands,
            workflows,
            provider_contracts,
        })
    }

    /// Resolves the selected base URL for a command from the registry catalog configuration.
    ///
    /// Returns `None` when the command is not associated with a catalog or when
    /// the catalog has no selected base URL configured.
    pub fn resolve_base_url_for_command(&self, command: &CommandSpec) -> Option<String> {
        let catalog_identifier = command.catalog_identifier;
        let catalog = self.get_catalog(catalog_identifier)?;
        catalog.selected_base_url().map(|value| value.to_string())
    }

    /// Resolves the headers for a command from the registry catalog configuration.
    ///
    /// Returns `None` when the command is not associated with a catalog or when
    /// the catalog has no headers configured.
    pub fn resolve_headers_for_command(&self, command: &CommandSpec) -> Option<&IndexSet<EnvVar>> {
        let catalog_identifier = command.catalog_identifier;
        let catalog = self.get_catalog(catalog_identifier)?;

        Some(&catalog.headers)
    }

    /// Finds a specific command by its group and command name.
    ///
    /// This method searches for a command using the format "group command"
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
    pub fn find_by_group_and_cmd(&self, group: &str, cmd: &str) -> Result<CommandSpec> {
        self.commands
            .iter()
            .find(|c| c.group == group && c.name == cmd)
            .cloned()
            .ok_or(anyhow!("{} {} command not found", group, cmd))
    }

    fn get_catalog(&self, id: usize) -> Option<&RegistryCatalog> {
        let catalogs = self.config.catalogs.as_ref()?;

        catalogs.get(id)
    }

    /// Inserts the synthetic commands from an MCP client's
    /// tool definitions and deduplicates them.
    pub fn insert_commands(&mut self, commands: &[CommandSpec]) {
        self.commands.extend_from_slice(commands);
        sort_and_dedup_commands(&mut self.commands);
    }

    /// Removes the synthetic commands from the vec
    pub fn remove_commands(&mut self, command_ids: Vec<String>) {
        let set: HashSet<String> = command_ids.into_iter().collect();
        self.commands.retain(|c| !set.contains(&c.canonical_id()));
    }

    pub fn remove_workflows(&mut self, workflow_ids: Vec<String>) {
        let set: HashSet<String> = workflow_ids.into_iter().collect();
        self.workflows.retain(|w| !set.contains(&w.workflow));
    }

    /// Inserts a catalog into the registry
    pub fn insert_catalog(&mut self, catalog: RegistryCatalog) -> Result<()> {
        let catalogs = self.config.catalogs.get_or_insert(Vec::with_capacity(1));

        if catalogs.iter().any(|c| c.title == catalog.title) {
            return Err(anyhow!("Catalog already exists"));
        }
        if catalog.is_enabled
            && let Some(manifest) = catalog.manifest.as_ref()
        {
            self.commands.extend_from_slice(&manifest.commands);
            self.workflows.extend_from_slice(&manifest.workflows);
            self.provider_contracts.extend(manifest.provider_contracts.clone());
            sort_and_dedup_commands(&mut self.commands);
        }

        catalogs.push(catalog);
        Ok(())
    }

    /// Removes a catalog from the registry
    pub fn remove_catalog(&mut self, catalog_title: &str) -> Result<()> {
        self.disable_catalog(catalog_title)?;

        let catalogs = self.config.catalogs.as_mut().ok_or_else(|| anyhow!("No catalogs configured"))?;

        if let Some(index) = catalogs.iter().position(|c| c.title == catalog_title) {
            let removed = catalogs.remove(index);
            let manifest_path = Path::new(&removed.manifest_path);
            if std::fs::exists(manifest_path).is_ok() {
                std::fs::remove_file(manifest_path)?;
            }
            Ok(())
        } else {
            Err(anyhow!("Catalog not found"))
        }
    }

    pub fn disable_catalog(&mut self, catalog_title: &str) -> Result<()> {
        let catalogs = self.config.catalogs.as_mut().ok_or_else(|| anyhow!("No catalogs configured"))?;

        let Some(index) = catalogs.iter().position(|c| c.title == catalog_title) else {
            return Err(anyhow!("Catalog not found"));
        };
        catalogs[index].is_enabled = false;
        // Note that provider contracts are not removed when disabling a catalog.
        // This is intentional because the contracts are IndexMapped and never queried
        // after a catalog is disabled.
        let (command_ids, workflow_ids) = catalogs[index]
            .manifest
            .as_ref()
            .map(|m| {
                let command_ids: Vec<String> = m.commands.iter().map(|c| c.canonical_id()).collect();
                let workflow_ids: Vec<String> = m.workflows.iter().map(|w| w.workflow.clone()).collect();
                (command_ids, workflow_ids)
            })
            .unwrap_or_default();

        self.remove_commands(command_ids);
        self.remove_workflows(workflow_ids);
        Ok(())
    }

    pub fn enable_catalog(&mut self, catalog_identifier: &str) -> Result<()> {
        let catalogs = self.config.catalogs.as_mut().ok_or_else(|| anyhow!("No catalogs configured"))?;

        if let Some(index) = catalogs.iter().position(|c| c.title == catalog_identifier) {
            catalogs[index].is_enabled = true;
            if let Some(manifest) = catalogs[index].manifest.as_ref() {
                self.commands.extend_from_slice(&manifest.commands);
                self.workflows.extend_from_slice(&manifest.workflows);
                self.provider_contracts.extend(manifest.provider_contracts.clone());
            }
            Ok(())
        } else {
            Err(anyhow!("Catalog not found"))
        }
    }

    pub fn update_base_url_index(&mut self, base_url_index: usize, title: &str) -> Result<()> {
        let catalogs = self.config.catalogs.as_mut().ok_or_else(|| anyhow!("No catalogs configured"))?;

        if let Some(index) = catalogs.iter().position(|c| c.title == title) {
            catalogs[index].base_url_index = base_url_index;
            Ok(())
        } else {
            Err(anyhow!("Catalog not found"))
        }
    }

    pub fn update_headers(&mut self, title: &str, headers: IndexSet<EnvVar>) -> Result<()> {
        let catalogs = self.config.catalogs.as_mut().ok_or_else(|| anyhow!("No catalogs configured"))?;

        if let Some(index) = catalogs.iter().position(|c| c.title == title) {
            catalogs[index].headers = headers;
            Ok(())
        } else {
            Err(anyhow!("Catalog not found"))
        }
    }
}
