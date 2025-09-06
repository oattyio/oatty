use heroku_types::CommandSpec;

// Re-export public items from modules
pub mod io;
pub mod openapi;
pub mod schema;
pub mod provider_resolver;

pub use io::{write_manifest, write_manifest_json};
pub use schema::generate_commands;
use serde::{Deserialize, Serialize};

/// A registry containing a list of command specifications.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Registry {
    pub commands: Vec<CommandSpec>,
}
