//! Registry crate for managing Oatty CLI command definitions.
//!
//! This crate provides the core data structures and functionality for loading,
//! organizing, and generating CLI commands from Oatty API schemas.

pub mod clap_builder;
pub mod config;
pub mod models;
pub mod utils;

pub use clap_builder::build_clap;
pub use config::*;
pub use models::CommandRegistry;
pub use oatty_types::{
    CommandFlag, CommandSpec, ProviderArgumentContract, ProviderContract, ProviderFieldContract, ProviderReturnContract,
};
pub use utils::*;

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, vec};

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
        let registry = CommandRegistry::from_config().expect("load registry from manifest");
        assert!(!registry.commands.is_empty(), "registry commands should not be empty");
        let mut seen = HashSet::new();
        let mut duplicates: Vec<String> = vec![];
        for spec in &*registry.commands {
            let group_name = spec.canonical_id();
            if seen.contains(&group_name) {
                duplicates.push(format!("{} {}", group_name.clone(), spec.summary));
            }
            seen.insert(group_name);
        }
        assert!(duplicates.is_empty(), "duplicates seen: {}", duplicates.len());
        assert!(!registry.provider_contracts.is_empty(), "provider contracts should not be empty");
    }
}
