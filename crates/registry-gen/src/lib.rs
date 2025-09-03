use heroku_types::CommandSpec;

// Re-export public items from modules
pub mod io;
pub mod schema;
pub mod openapi;

pub use io::{write_manifest, write_manifest_json};
pub use schema::generate_commands;

/// A registry containing a list of command specifications.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Registry {
    pub commands: Vec<CommandSpec>,
}
