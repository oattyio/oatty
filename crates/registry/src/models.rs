use std::{sync::Arc};

use anyhow::{Context, Result};
use bincode::config;
use heroku_types::CommandSpec;
use heroku_util::sort_and_dedup_commands;

static MANIFEST: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/heroku-manifest.bin"));
/// The main registry containing all available Heroku CLI commands.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Registry {
    /// Collection of all available command specifications
    pub commands: Vec<CommandSpec>,
}

impl Registry {
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
    /// use heroku_registry::Registry;
    ///
    /// let registry = Registry::from_embedded_schema().expect("load registry from schema");
    /// println!("Loaded {} commands", registry.commands.len());
    /// ```
    pub fn from_embedded_schema() -> Result<Self> {
        let config = config::standard();

        // Decode the CommandSpec struct from the bytes
        let commands: Vec<CommandSpec> = bincode::decode_from_slice(MANIFEST, config).context("decoding manifest failed")?.0;

        Ok(Registry { commands })
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
