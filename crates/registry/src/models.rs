use anyhow::{Result, anyhow};
use indexmap::{IndexMap, IndexSet, set::MutableValues};
use oatty_types::{
    CommandSpec, EnvSource, EnvVar, ProviderContract,
    manifest::{RegistryCatalog, RegistryManifest},
    workflow::WorkflowDefinition,
};
use oatty_util::{interpolate_string, sort_and_dedup_commands};
use std::{collections::HashSet, convert::Infallible, path::Path, sync::Arc};
use tokio::sync::broadcast;

use crate::RegistryConfig;
use crate::workflows::load_runtime_workflows;

const REGISTRY_EVENT_CHANNEL_CAPACITY: usize = 64;

/// Result alias for catalog mutation operations that return typed mutation failures.
pub type CatalogMutationResult<T> = std::result::Result<T, CatalogMutationError>;

/// Result alias for canonical command resolution operations.
pub type CommandResolutionResult<T> = std::result::Result<T, CommandResolutionError>;

/// Parsed canonical identifier in `<group> <command>` form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalCommandId {
    /// Top-level command group.
    pub group: String,
    /// Command name within the group.
    pub command: String,
}

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
    /// Broadcast sender for Command events (lazy)
    #[serde(skip)]
    event_tx: Option<broadcast::Sender<CommandRegistryEvent>>,
}

/// Header mutation mode used for catalog header editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogHeaderEditMode {
    /// Insert/replace matching keys and preserve non-matching existing keys.
    Upsert,
    /// Remove matching keys from existing headers.
    Remove,
    /// Replace all existing headers with the provided rows.
    ReplaceAll,
}

/// Header row used to mutate catalog headers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogHeaderEditRow {
    /// Header name.
    pub key: String,
    /// Header value. Required for `Upsert` and `ReplaceAll`.
    pub value: Option<String>,
    /// Source hint for the header.
    pub source: EnvSource,
    /// Whether the header value is effective.
    pub effective: bool,
}

/// Typed failures for runtime catalog mutation operations.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum CatalogMutationError {
    /// No runtime catalogs are currently configured.
    #[error("no catalogs configured")]
    NoCatalogsConfigured,
    /// The requested catalog title was not found.
    #[error("catalog '{title}' not found")]
    CatalogNotFound {
        /// Catalog title used for lookup.
        title: String,
    },
    /// A required base URL value was empty.
    #[error("base URL cannot be empty")]
    EmptyBaseUrl,
    /// A required header key value was empty.
    #[error("header key cannot be empty")]
    EmptyHeaderKey,
    /// A header value was required but missing for the given key.
    #[error("header '{key}' is missing value")]
    MissingHeaderValue {
        /// Header key missing an associated value.
        key: String,
    },
}

/// Resolved command metadata returned by canonical-id lookups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCommand {
    /// Fully resolved command specification.
    pub command: CommandSpec,
    /// Catalog identifier that owns the resolved command when it can be determined.
    pub catalog_identifier: Option<usize>,
    /// Vendor name associated with the resolved command when present.
    pub vendor: Option<String>,
}

/// Outcome for exact canonical-id search attempts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactSearchHit {
    /// Exactly one command matches the canonical query and optional vendor scope.
    Unique(Box<ResolvedCommand>),
    /// More than one command matches, so discovery should continue with ranked results.
    Ambiguous,
}

/// Typed failures produced while resolving a canonical command identifier.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommandResolutionError {
    /// The canonical identifier does not follow the `<group> <command>` shape.
    #[error("canonical_id must be in 'group command' format")]
    InvalidCanonicalId {
        /// User-provided canonical identifier.
        canonical_id: String,
    },
    /// No command matched the requested canonical identifier and optional vendor.
    #[error("{group} {command} command not found")]
    NotFound {
        /// User-provided canonical identifier.
        canonical_id: String,
        /// Parsed group name.
        group: String,
        /// Parsed command name.
        command: String,
        /// Optional vendor filter used during lookup.
        vendor: Option<String>,
        /// Vendors that contain the same canonical identifier under a different scope.
        matching_vendors: Vec<String>,
    },
    /// More than one enabled catalog matched the canonical identifier.
    #[error("canonical_id '{canonical_id}' is ambiguous across multiple catalogs")]
    Ambiguous {
        /// User-provided canonical identifier.
        canonical_id: String,
        /// Optional vendor filter used during lookup.
        vendor: Option<String>,
        /// Vendors that currently expose the canonical identifier.
        matching_vendors: Vec<String>,
    },
}

impl CommandRegistry {
    pub fn with_commands(mut self, commands: Vec<CommandSpec>) -> Self {
        self.commands = commands;
        self
    }

    /// Subscribe to command registry events
    pub fn subscribe(&mut self) -> broadcast::Receiver<CommandRegistryEvent> {
        let tx = self.event_tx.get_or_insert_with(|| {
            let (tx, _) = broadcast::channel(REGISTRY_EVENT_CHANNEL_CAPACITY);
            tx
        });

        tx.subscribe()
    }
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
        let mut commands = Vec::new();
        let mut provider_contracts = IndexMap::new();

        if let Some(catalogs) = config.catalogs.as_mut() {
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
        }

        let workflows = load_runtime_workflows().unwrap_or_else(|error| {
            tracing::warn!(error = %error, "failed to load runtime workflows from filesystem");
            Vec::new()
        });

        Ok(CommandRegistry {
            config,
            commands,
            workflows,
            provider_contracts,
            event_tx: None,
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
    ///
    /// This legacy helper does not detect duplicate `(group, command)` pairs across vendors.
    /// Prefer `find_by_group_and_cmd_cloned_for_vendor` or `resolve_command_by_canonical_id`
    /// when catalogs from multiple providers can overlap.
    pub fn find_by_group_and_cmd_cloned(&self, group: &str, cmd: &str) -> Result<CommandSpec> {
        self.commands
            .iter()
            .find(|c| c.group == group && c.name == cmd)
            .cloned()
            .ok_or(anyhow!("{} {} command not found", group, cmd))
    }

    /// Finds a specific command by group, command name, and optional vendor filter.
    pub fn find_by_group_and_cmd_cloned_for_vendor(
        &self,
        group: &str,
        cmd: &str,
        vendor: Option<&str>,
    ) -> CommandResolutionResult<ResolvedCommand> {
        self.resolve_command_by_canonical_id(&format!("{group} {cmd}"), vendor)
    }

    ///  Finds a command specification within the collection of commands, based on the provided group
    ///  and command name.
    ///
    ///  # Parameters
    ///  - `group`: A string slice that specifies the group name of the command.
    ///  - `cmd`: A string slice that specifies the name of the command.
    ///
    ///  # Returns
    ///  - `Ok(&CommandSpec)`: A reference to the `CommandSpec` if a matching command is found.
    ///  - `Err(anyhow::Error)`: An error containing a descriptive message if no matching command is found.
    ///
    ///  # Errors
    ///  Returns an error if no command in the collection matches the provided `group` and `cmd`.
    ///
    ///  This legacy helper does not detect duplicate `(group, command)` pairs across vendors.
    ///  Prefer `find_by_group_and_cmd_ref_for_vendor` or `resolve_command_by_canonical_id`
    ///  when catalogs from multiple providers can overlap.
    ///
    ///  # Example
    ///  ```ignore
    ///   let group = "admin";
    ///   let cmd = "delete_user";
    ///   match commands.find_by_group_and_cmd_ref(group, cmd) {
    ///       Ok(command) => println!("Command found: {:?}", command),
    ///       Err(e) => println!("Error: {}", e),
    ///   }
    ///  ```
    pub fn find_by_group_and_cmd_ref(&self, group: &str, cmd: &str) -> Result<&CommandSpec> {
        self.commands
            .iter()
            .find(|c| c.group == group && c.name == cmd)
            .ok_or(anyhow!("{} {} command not found", group, cmd))
    }

    /// Finds a specific command reference by group, command name, and optional vendor filter.
    pub fn find_by_group_and_cmd_ref_for_vendor(
        &self,
        group: &str,
        cmd: &str,
        vendor: Option<&str>,
    ) -> CommandResolutionResult<&CommandSpec> {
        let resolved_command = self.find_by_group_and_cmd_cloned_for_vendor(group, cmd, vendor)?;
        self.commands
            .iter()
            .find(|command| {
                command.group == resolved_command.command.group
                    && command.name == resolved_command.command.name
                    && command.summary == resolved_command.command.summary
                    && command.catalog_identifier == resolved_command.command.catalog_identifier
            })
            .ok_or_else(|| CommandResolutionError::NotFound {
                canonical_id: resolved_command.command.canonical_id(),
                group: group.to_string(),
                command: cmd.to_string(),
                vendor: vendor.map(ToOwned::to_owned),
                matching_vendors: Vec::new(),
            })
    }

    /// Parses a canonical command identifier in `<group> <command>` form.
    pub fn parse_canonical_command_id(canonical_id: &str) -> Option<CanonicalCommandId> {
        split_canonical_id_components(canonical_id).map(|(group, command)| CanonicalCommandId {
            group: group.to_string(),
            command: command.to_string(),
        })
    }

    /// Resolves the catalog identifier for a command, accounting for stale catalog indexes.
    pub fn resolve_catalog_identifier_for_command(&self, command: &CommandSpec) -> Option<usize> {
        let catalogs = self.config.catalogs.as_ref()?;

        if let Some(catalog) = catalogs.get(command.catalog_identifier)
            && catalog_contains_command(catalog, command)
        {
            return Some(command.catalog_identifier);
        }

        let mut matching_catalog_identifiers = catalogs
            .iter()
            .enumerate()
            .filter(|(_, catalog)| catalog_contains_command(catalog, command))
            .map(|(catalog_identifier, _)| catalog_identifier);

        let catalog_identifier = matching_catalog_identifiers.next()?;
        if matching_catalog_identifiers.next().is_some() {
            return None;
        }

        Some(catalog_identifier)
    }

    /// Returns the configured vendor for a command when it can be resolved uniquely.
    pub fn command_vendor(&self, command: &CommandSpec) -> Option<String> {
        let catalog_identifier = self.resolve_catalog_identifier_for_command(command)?;
        let catalog = self.config.catalogs.as_ref()?.get(catalog_identifier)?;
        Self::catalog_vendor_value(catalog).map(ToOwned::to_owned)
    }

    /// Returns true when a command belongs to the requested vendor.
    pub fn command_matches_vendor(&self, command: &CommandSpec, vendor_name: &str) -> bool {
        let Some(catalog_identifier) = self.resolve_catalog_identifier_for_command(command) else {
            return false;
        };
        let Some(catalog) = self.config.catalogs.as_ref().and_then(|catalogs| catalogs.get(catalog_identifier)) else {
            return false;
        };

        catalog.is_enabled && Self::catalog_vendor_matches(catalog, vendor_name)
    }

    /// Returns true when every enabled catalog containing the canonical identifier matches the requested vendor.
    pub fn canonical_id_matches_vendor(&self, canonical_id: &str, vendor_name: &str) -> bool {
        let Some(catalogs) = self.config.catalogs.as_ref() else {
            return false;
        };

        let mut catalogs_with_command = catalogs.iter().filter(|catalog| {
            catalog.is_enabled
                && catalog.manifest.as_ref().is_some_and(|manifest| {
                    manifest
                        .commands
                        .iter()
                        .any(|catalog_command| catalog_command.canonical_id() == canonical_id)
                })
        });

        let Some(first_catalog) = catalogs_with_command.next() else {
            return false;
        };

        Self::catalog_vendor_matches(first_catalog, vendor_name)
            && catalogs_with_command.all(|catalog| Self::catalog_vendor_matches(catalog, vendor_name))
    }

    /// Returns whether an enabled catalog exists for the requested vendor.
    pub fn has_enabled_catalog_for_vendor(&self, vendor_name: &str) -> bool {
        self.config.catalogs.as_ref().is_some_and(|catalogs| {
            catalogs
                .iter()
                .filter(|catalog| catalog.is_enabled)
                .any(|catalog| Self::catalog_vendor_matches(catalog, vendor_name))
        })
    }

    /// Returns a unique exact canonical-id hit when discovery can short-circuit safely.
    pub fn resolve_exact_search_hit(&self, query: &str, vendor: Option<&str>) -> Option<ExactSearchHit> {
        let parsed_canonical_id = Self::parse_canonical_command_id(query)?;
        let matching_commands = self
            .commands
            .iter()
            .filter(|command| {
                command.group == parsed_canonical_id.group
                    && command.name == parsed_canonical_id.command
                    && vendor.is_none_or(|vendor_name| self.command_matches_vendor(command, vendor_name))
            })
            .cloned()
            .collect::<Vec<CommandSpec>>();

        match matching_commands.as_slice() {
            [command] => Some(ExactSearchHit::Unique(Box::new(ResolvedCommand {
                catalog_identifier: self.resolve_catalog_identifier_for_command(command),
                vendor: self.command_vendor(command),
                command: command.clone(),
            }))),
            [] => None,
            _ => Some(ExactSearchHit::Ambiguous),
        }
    }

    /// Resolves a canonical command identifier to a unique command and vendor.
    pub fn resolve_command_by_canonical_id(&self, canonical_id: &str, vendor: Option<&str>) -> CommandResolutionResult<ResolvedCommand> {
        let Some(parsed_canonical_id) = Self::parse_canonical_command_id(canonical_id) else {
            return Err(CommandResolutionError::InvalidCanonicalId {
                canonical_id: canonical_id.to_string(),
            });
        };

        let matching_commands = self
            .commands
            .iter()
            .filter(|command| command.group == parsed_canonical_id.group && command.name == parsed_canonical_id.command)
            .cloned()
            .collect::<Vec<CommandSpec>>();
        let vendor_scoped_commands = matching_commands
            .iter()
            .filter(|command| vendor.is_none_or(|vendor_name| self.command_matches_vendor(command, vendor_name)))
            .cloned()
            .collect::<Vec<CommandSpec>>();

        if let [command] = vendor_scoped_commands.as_slice() {
            return Ok(ResolvedCommand {
                catalog_identifier: self.resolve_catalog_identifier_for_command(command),
                vendor: self.command_vendor(command),
                command: command.clone(),
            });
        }

        let matching_vendors = matching_commands
            .iter()
            .filter_map(|command| self.command_vendor(command))
            .collect::<std::collections::BTreeSet<String>>()
            .into_iter()
            .collect::<Vec<String>>();

        if vendor_scoped_commands.len() > 1 || (vendor.is_none() && matching_commands.len() > 1) {
            return Err(CommandResolutionError::Ambiguous {
                canonical_id: canonical_id.to_string(),
                vendor: vendor.map(ToOwned::to_owned),
                matching_vendors,
            });
        }

        Err(CommandResolutionError::NotFound {
            canonical_id: canonical_id.to_string(),
            group: parsed_canonical_id.group,
            command: parsed_canonical_id.command,
            vendor: vendor.map(ToOwned::to_owned),
            matching_vendors,
        })
    }

    fn get_catalog(&self, id: usize) -> Option<&RegistryCatalog> {
        let catalogs = self.config.catalogs.as_ref()?;

        catalogs.get(id)
    }

    /// Returns the vendor configured for a catalog from either top-level or manifest metadata.
    pub fn catalog_vendor_value(catalog: &RegistryCatalog) -> Option<&str> {
        catalog
            .vendor
            .as_deref()
            .or_else(|| catalog.manifest.as_ref().map(|manifest| manifest.vendor.as_str()))
            .filter(|vendor| !vendor.trim().is_empty())
    }

    /// Returns whether the catalog belongs to the requested vendor.
    pub fn catalog_vendor_matches(catalog: &RegistryCatalog, vendor_name: &str) -> bool {
        let vendor_name = vendor_name.trim();
        !vendor_name.is_empty()
            && Self::catalog_vendor_value(catalog).is_some_and(|catalog_vendor| catalog_vendor.eq_ignore_ascii_case(vendor_name))
    }

    /// Inserts the synthetic commands from an MCP client's
    /// tool definitions and deduplicates them.
    pub fn insert_commands(&mut self, commands: Arc<[CommandSpec]>) {
        self.commands.extend_from_slice(commands.as_ref());
        sort_and_dedup_commands(&mut self.commands);
        if let Some(tx) = self.event_tx.as_ref() {
            let _ = tx.send(CommandRegistryEvent::CommandsAdded(commands));
        }
    }

    /// Removes the synthetic commands from the vec
    pub fn remove_commands(&mut self, command_ids: Vec<String>) {
        let set: HashSet<String> = command_ids.into_iter().collect();
        let commands: Vec<_> = self.commands.extract_if(.., |c| set.contains(&c.canonical_id())).collect();
        if let Some(tx) = self.event_tx.as_ref() {
            let _ = tx.send(CommandRegistryEvent::CommandsRemoved(Arc::from(commands)));
        }
    }

    pub fn remove_workflows(&mut self, workflow_ids: Vec<String>) {
        let set: HashSet<String> = workflow_ids.into_iter().collect();
        let workflows: Vec<_> = self.workflows.extract_if(.., |w| set.contains(&w.workflow)).collect();
        if let Some(tx) = self.event_tx.as_ref() {
            let _ = tx.send(CommandRegistryEvent::WorkflowsRemoved(Arc::from(workflows)));
        }
    }

    pub fn insert_workflows(&mut self, workflows: Arc<[WorkflowDefinition]>) {
        self.workflows.extend_from_slice(workflows.as_ref());
        if let Some(tx) = self.event_tx.as_ref() {
            let _ = tx.send(CommandRegistryEvent::WorkflowsAdded(workflows));
        }
    }

    /// Inserts a catalog into the registry
    pub fn insert_catalog(&mut self, mut catalog: RegistryCatalog) -> Result<()> {
        let catalogs = self.config.catalogs.get_or_insert(Vec::with_capacity(1));

        if catalogs.iter().any(|c| c.title == catalog.title) {
            return Err(anyhow!("Catalog already exists"));
        }
        let catalog_identifier = catalogs.len();
        if catalog.is_enabled
            && let Some(manifest) = catalog.manifest.as_ref()
        {
            let mut commands = manifest.commands.clone();
            for command in &mut commands {
                command.catalog_identifier = catalog_identifier;
            }
            self.insert_commands(Arc::from(commands));
            self.provider_contracts.extend(manifest.provider_contracts.clone());
            sort_and_dedup_commands(&mut self.commands);
        }

        if let Some(manifest) = catalog.manifest.as_mut() {
            for command in &mut manifest.commands {
                command.catalog_identifier = catalog_identifier;
            }
        }

        self.config
            .catalogs
            .as_mut()
            .ok_or_else(|| anyhow!("expected a catalog to extend but found none"))?
            .push(catalog);
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
        let command_ids = catalogs[index]
            .manifest
            .as_ref()
            .map(|m| {
                let command_ids: Vec<String> = m.commands.iter().map(|c| c.canonical_id()).collect();
                command_ids
            })
            .unwrap_or_default();

        self.remove_commands(command_ids);
        Ok(())
    }

    pub fn enable_catalog(&mut self, catalog_identifier: &str) -> Result<()> {
        let (commands_to_insert, provider_contracts_to_insert) = {
            let catalogs = self.config.catalogs.as_mut().ok_or_else(|| anyhow!("No catalogs configured"))?;
            let Some(index) = catalogs.iter().position(|catalog| catalog.title == catalog_identifier) else {
                return Err(anyhow!("Catalog not found"));
            };
            catalogs[index].is_enabled = true;

            let (commands, provider_contracts) = if let Some(manifest) = catalogs[index].manifest.as_ref() {
                let mut commands = manifest.commands.clone();
                for command in &mut commands {
                    command.catalog_identifier = index;
                }
                (commands, manifest.provider_contracts.clone())
            } else {
                (Vec::new(), IndexMap::new())
            };
            (commands, provider_contracts)
        };

        if !commands_to_insert.is_empty() {
            self.insert_commands(Arc::from(commands_to_insert));
            self.provider_contracts.extend(provider_contracts_to_insert);
            sort_and_dedup_commands(&mut self.commands);
        }
        Ok(())
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

    pub fn update_description(&mut self, description: String, title: &str) -> Result<()> {
        let catalogs = self.config.catalogs.as_mut().ok_or_else(|| anyhow!("No catalogs configured"))?;

        if let Some(index) = catalogs.iter().position(|c| c.title == title) {
            catalogs[index].description = description;
            Ok(())
        } else {
            Err(anyhow!("Catalog not found"))
        }
    }

    pub fn update_base_urls(&mut self, base_urls: Vec<String>, title: &str) -> Result<()> {
        let catalogs = self.config.catalogs.as_mut().ok_or_else(|| anyhow!("No catalogs configured"))?;

        if let Some(index) = catalogs.iter().position(|c| c.title == title) {
            catalogs[index].base_urls = base_urls;
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

    /// Sets the selected base URL for a catalog, appending it when not present.
    pub fn set_selected_base_url(&mut self, title: &str, base_url: &str) -> CatalogMutationResult<()> {
        let normalized_base_url = base_url.trim();
        if normalized_base_url.is_empty() {
            return Err(CatalogMutationError::EmptyBaseUrl);
        }
        let catalogs = self.config.catalogs.as_mut().ok_or(CatalogMutationError::NoCatalogsConfigured)?;
        let Some(index) = catalogs.iter().position(|catalog| catalog.title == title) else {
            return Err(CatalogMutationError::CatalogNotFound { title: title.to_string() });
        };
        let catalog = &mut catalogs[index];
        let selected_index = catalog
            .base_urls
            .iter()
            .position(|candidate| candidate == normalized_base_url)
            .unwrap_or_else(|| {
                catalog.base_urls.push(normalized_base_url.to_string());
                catalog.base_urls.len().saturating_sub(1)
            });
        catalog.base_url_index = selected_index;
        Ok(())
    }

    /// Edits catalog headers using the provided mutation mode and rows.
    pub fn edit_catalog_headers(
        &mut self,
        title: &str,
        mode: CatalogHeaderEditMode,
        rows: &[CatalogHeaderEditRow],
    ) -> CatalogMutationResult<()> {
        let catalogs = self.config.catalogs.as_mut().ok_or(CatalogMutationError::NoCatalogsConfigured)?;
        let Some(index) = catalogs.iter().position(|catalog| catalog.title == title) else {
            return Err(CatalogMutationError::CatalogNotFound { title: title.to_string() });
        };
        let catalog = &mut catalogs[index];
        catalog.headers = match mode {
            CatalogHeaderEditMode::Upsert => apply_header_upserts(&catalog.headers, rows)?,
            CatalogHeaderEditMode::Remove => apply_header_removals(&catalog.headers, rows),
            CatalogHeaderEditMode::ReplaceAll => build_headers_from_rows(rows)?,
        };
        Ok(())
    }
}

fn split_canonical_id_components(canonical_id: &str) -> Option<(&str, &str)> {
    let trimmed = canonical_id.trim();
    let (group, command_name) = trimmed.split_once(' ')?;
    let normalized_group = group.trim();
    let normalized_command_name = command_name.trim();
    if normalized_group.is_empty() || normalized_command_name.is_empty() || normalized_command_name.contains(' ') {
        return None;
    }

    Some((normalized_group, normalized_command_name))
}

fn catalog_contains_command(catalog: &RegistryCatalog, command: &CommandSpec) -> bool {
    catalog
        .manifest
        .as_ref()
        .is_some_and(|manifest| manifest.commands.iter().any(|catalog_command| catalog_command == command))
}

fn apply_header_upserts(existing_headers: &IndexSet<EnvVar>, rows: &[CatalogHeaderEditRow]) -> CatalogMutationResult<IndexSet<EnvVar>> {
    let mut retained_headers = existing_headers.iter().cloned().collect::<Vec<EnvVar>>();
    for row in rows {
        let normalized_row_key = normalize_header_key(&row.key);
        if normalized_row_key.is_empty() {
            return Err(CatalogMutationError::EmptyHeaderKey);
        }
        retained_headers.retain(|header| normalize_header_key(&header.key) != normalized_row_key);
        retained_headers.push(env_var_from_row(row)?);
    }
    Ok(retained_headers.into_iter().collect())
}

fn apply_header_removals(existing_headers: &IndexSet<EnvVar>, rows: &[CatalogHeaderEditRow]) -> IndexSet<EnvVar> {
    let keys_to_remove = rows
        .iter()
        .map(|row| normalize_header_key(&row.key))
        .filter(|key| !key.is_empty())
        .collect::<HashSet<String>>();
    existing_headers
        .iter()
        .filter(|header| !keys_to_remove.contains(&normalize_header_key(&header.key)))
        .cloned()
        .collect()
}

fn build_headers_from_rows(rows: &[CatalogHeaderEditRow]) -> CatalogMutationResult<IndexSet<EnvVar>> {
    let mut headers = Vec::<EnvVar>::new();
    for row in rows {
        let normalized_row_key = normalize_header_key(&row.key);
        if normalized_row_key.is_empty() {
            return Err(CatalogMutationError::EmptyHeaderKey);
        }
        headers.retain(|header| normalize_header_key(&header.key) != normalized_row_key);
        headers.push(env_var_from_row(row)?);
    }
    Ok(headers.into_iter().collect())
}

fn env_var_from_row(row: &CatalogHeaderEditRow) -> CatalogMutationResult<EnvVar> {
    let normalized_key = row.key.trim();
    if normalized_key.is_empty() {
        return Err(CatalogMutationError::EmptyHeaderKey);
    }
    let value = row.value.clone().ok_or_else(|| CatalogMutationError::MissingHeaderValue {
        key: normalized_key.to_string(),
    })?;
    Ok(EnvVar {
        key: normalized_key.to_string(),
        value,
        source: row.source.clone(),
        effective: row.effective,
    })
}

fn normalize_header_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[derive(Debug, Clone)]
pub enum CommandRegistryEvent {
    CommandsAdded(Arc<[CommandSpec]>),
    CommandsRemoved(Arc<[CommandSpec]>),
    WorkflowsAdded(Arc<[WorkflowDefinition]>),
    WorkflowsRemoved(Arc<[WorkflowDefinition]>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexSet;
    use oatty_types::manifest::RegistryCatalog;

    fn catalog_with_title(title: &str) -> RegistryCatalog {
        RegistryCatalog {
            title: title.to_string(),
            description: String::new(),
            vendor: Some("test".to_string()),
            manifest_path: "/tmp/test.bin".to_string(),
            import_source: None,
            import_source_type: None,
            headers: IndexSet::new(),
            base_urls: vec!["https://example.test".to_string()],
            base_url_index: 0,
            manifest: Some(RegistryManifest::default()),
            is_enabled: true,
        }
    }

    #[test]
    fn set_selected_base_url_returns_typed_error_for_missing_catalog() {
        let mut registry = CommandRegistry::default();
        registry.config.catalogs = Some(vec![catalog_with_title("alpha")]);

        let result = registry.set_selected_base_url("missing", "https://api.example.com");

        assert_eq!(
            result.expect_err("missing catalog should fail"),
            CatalogMutationError::CatalogNotFound {
                title: "missing".to_string()
            }
        );
    }

    #[test]
    fn edit_catalog_headers_returns_typed_error_for_missing_header_value() {
        let mut registry = CommandRegistry::default();
        registry.config.catalogs = Some(vec![catalog_with_title("alpha")]);

        let rows = vec![CatalogHeaderEditRow {
            key: "Authorization".to_string(),
            value: None,
            source: EnvSource::Raw,
            effective: true,
        }];
        let result = registry.edit_catalog_headers("alpha", CatalogHeaderEditMode::Upsert, &rows);

        assert_eq!(
            result.expect_err("missing value should fail"),
            CatalogMutationError::MissingHeaderValue {
                key: "Authorization".to_string()
            }
        );
    }

    #[test]
    fn replace_all_normalizes_duplicate_header_keys_case_insensitively() {
        let mut registry = CommandRegistry::default();
        registry.config.catalogs = Some(vec![catalog_with_title("alpha")]);

        let rows = vec![
            CatalogHeaderEditRow {
                key: "Authorization".to_string(),
                value: Some("Bearer first".to_string()),
                source: EnvSource::Raw,
                effective: true,
            },
            CatalogHeaderEditRow {
                key: "authorization".to_string(),
                value: Some("Bearer second".to_string()),
                source: EnvSource::Raw,
                effective: true,
            },
        ];

        registry
            .edit_catalog_headers("alpha", CatalogHeaderEditMode::ReplaceAll, &rows)
            .expect("replace all should succeed");

        let catalogs = registry.config.catalogs.expect("catalogs should exist");
        let headers = &catalogs[0].headers;
        assert_eq!(headers.len(), 1);
        let header = headers.iter().next().expect("single header should exist");
        assert_eq!(header.key, "authorization");
        assert_eq!(header.value, "Bearer second");
    }

    #[test]
    fn resolve_command_by_canonical_id_requires_vendor_for_duplicate_commands() {
        let vercel_projects = CommandSpec::new_http(
            "projects".to_string(),
            "list".to_string(),
            "List Vercel projects".to_string(),
            Vec::new(),
            Vec::new(),
            oatty_types::command::HttpCommandSpec::new("GET", "/v1/projects", None, None),
            0,
        );
        let render_projects = CommandSpec::new_http(
            "projects".to_string(),
            "list".to_string(),
            "List Render projects".to_string(),
            Vec::new(),
            Vec::new(),
            oatty_types::command::HttpCommandSpec::new("GET", "/v1/services", None, None),
            1,
        );

        let mut registry = CommandRegistry::default().with_commands(vec![vercel_projects.clone(), render_projects.clone()]);
        registry.config.catalogs = Some(vec![
            catalog_with_commands("Vercel", "vercel", vec![vercel_projects]),
            catalog_with_commands("Render", "render", vec![render_projects.clone()]),
        ]);

        let error = registry
            .resolve_command_by_canonical_id("projects list", None)
            .expect_err("duplicate commands should require vendor");

        assert_eq!(
            error,
            CommandResolutionError::Ambiguous {
                canonical_id: "projects list".to_string(),
                vendor: None,
                matching_vendors: vec!["render".to_string(), "vercel".to_string()],
            }
        );
    }

    #[test]
    fn resolve_command_by_canonical_id_scopes_to_vendor() {
        let vercel_projects = CommandSpec::new_http(
            "projects".to_string(),
            "list".to_string(),
            "List Vercel projects".to_string(),
            Vec::new(),
            Vec::new(),
            oatty_types::command::HttpCommandSpec::new("GET", "/v1/projects", None, None),
            0,
        );
        let render_projects = CommandSpec::new_http(
            "projects".to_string(),
            "list".to_string(),
            "List Render projects".to_string(),
            Vec::new(),
            Vec::new(),
            oatty_types::command::HttpCommandSpec::new("GET", "/v1/services", None, None),
            1,
        );

        let mut registry = CommandRegistry::default().with_commands(vec![vercel_projects.clone(), render_projects.clone()]);
        registry.config.catalogs = Some(vec![
            catalog_with_commands("Vercel", "vercel", vec![vercel_projects]),
            catalog_with_commands("Render", "render", vec![render_projects.clone()]),
        ]);

        let resolved_command = registry
            .resolve_command_by_canonical_id("projects list", Some("render"))
            .expect("vendor-scoped resolution should succeed");

        assert_eq!(resolved_command.command.summary, "List Render projects");
        assert_eq!(resolved_command.vendor.as_deref(), Some("render"));
        assert_eq!(resolved_command.catalog_identifier, Some(1));
    }

    #[test]
    fn resolve_exact_search_hit_reports_ambiguity_for_duplicate_canonical_ids() {
        let vercel_projects = CommandSpec::new_http(
            "projects".to_string(),
            "list".to_string(),
            "List Vercel projects".to_string(),
            Vec::new(),
            Vec::new(),
            oatty_types::command::HttpCommandSpec::new("GET", "/v1/projects", None, None),
            0,
        );
        let render_projects = CommandSpec::new_http(
            "projects".to_string(),
            "list".to_string(),
            "List Render projects".to_string(),
            Vec::new(),
            Vec::new(),
            oatty_types::command::HttpCommandSpec::new("GET", "/v1/services", None, None),
            1,
        );

        let mut registry = CommandRegistry::default().with_commands(vec![vercel_projects.clone(), render_projects.clone()]);
        registry.config.catalogs = Some(vec![
            catalog_with_commands("Vercel", "vercel", vec![vercel_projects]),
            catalog_with_commands("Render", "render", vec![render_projects]),
        ]);

        assert_eq!(
            registry.resolve_exact_search_hit("projects list", None),
            Some(ExactSearchHit::Ambiguous)
        );
    }

    #[test]
    fn resolve_catalog_identifier_recovers_stale_index_for_duplicate_canonical_ids() {
        let vercel_projects = CommandSpec::new_http(
            "projects".to_string(),
            "list".to_string(),
            "List Vercel projects".to_string(),
            Vec::new(),
            Vec::new(),
            oatty_types::command::HttpCommandSpec::new("GET", "/v1/projects", None, None),
            0,
        );
        let render_projects = CommandSpec::new_http(
            "projects".to_string(),
            "list".to_string(),
            "List Render projects".to_string(),
            Vec::new(),
            Vec::new(),
            oatty_types::command::HttpCommandSpec::new("GET", "/v1/services", None, None),
            1,
        );

        let mut registry = CommandRegistry::default().with_commands(vec![vercel_projects.clone(), render_projects.clone()]);
        registry.config.catalogs = Some(vec![
            catalog_with_commands("Vercel", "vercel", vec![vercel_projects]),
            catalog_with_commands("Render", "render", vec![render_projects.clone()]),
        ]);

        registry.config.catalogs.as_mut().expect("catalogs exist").remove(0);
        registry.commands = vec![render_projects.clone()];

        let resolved_command = registry
            .resolve_command_by_canonical_id("projects list", Some("render"))
            .expect("stale catalog index should still resolve to the owning vendor");

        assert_eq!(resolved_command.command.summary, "List Render projects");
        assert_eq!(resolved_command.vendor.as_deref(), Some("render"));
        assert_eq!(resolved_command.catalog_identifier, Some(0));
    }

    fn catalog_with_commands(title: &str, vendor: &str, commands: Vec<CommandSpec>) -> RegistryCatalog {
        RegistryCatalog {
            title: title.to_string(),
            description: format!("{title} APIs"),
            vendor: Some(vendor.to_string()),
            manifest_path: String::new(),
            import_source: None,
            import_source_type: None,
            headers: IndexSet::new(),
            base_urls: vec![format!("https://api.{vendor}.com")],
            base_url_index: 0,
            manifest: Some(RegistryManifest {
                commands,
                provider_contracts: Default::default(),
                vendor: vendor.to_string(),
            }),
            is_enabled: true,
        }
    }
}
