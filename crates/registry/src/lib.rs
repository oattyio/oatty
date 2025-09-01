//! Registry crate for managing Heroku CLI command definitions.
//!
//! This crate provides the core data structures and functionality for loading,
//! organizing, and generating CLI commands from Heroku API schemas.

pub mod clap_builder;
pub mod feat_gate;
pub mod models;

pub use clap_builder::build_clap;
pub use heroku_types::{CommandFlag, CommandSpec};
pub use models::Registry;

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    /// Tests that the embedded manifest loads successfully and contains valid
    /// commands.
    ///
    /// This test verifies that:
    /// 1. The registry can be loaded from the embedded schema
    /// 2. The registry contains at least one command
    /// 3. All command names are unique (no duplicates)
    #[test]
    fn manifest_non_empty_and_unique_names() {
        let registry = Registry::from_embedded_schema().expect("load registry from manifest");
        assert!(!registry.commands.is_empty(), "registry commands should not be empty");
        let mut seen = HashSet::new();
        for c in &*registry.commands {
            assert!(seen.insert(&c.name), "duplicate command name detected: {}", c.name);
        }
    }
}
