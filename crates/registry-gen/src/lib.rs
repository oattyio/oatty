use heroku_types::CommandSpec;

// Re-export public items from modules
pub mod io;
pub mod schema;
pub mod workflow;

pub use io::write_manifest;
pub use schema::generate_commands;
pub use workflow::add_workflow_commands;

/// A registry containing a list of command specifications.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Registry {
    pub commands: Vec<CommandSpec>,
}
