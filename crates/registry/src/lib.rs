//! Registry crate for managing Oatty CLI command definitions.
//!
//! This crate provides the core data structures and functionality for loading,
//! organizing, and generating CLI commands from Oatty API schemas.

pub mod clap_builder;
pub mod config;
pub mod models;
pub mod openapi_import;
pub mod search;
pub mod workflows;

pub use clap_builder::build_clap;
pub use config::*;
pub use models::CommandRegistry;
pub use oatty_types::{
    CommandFlag, CommandSpec, ProviderArgumentContract, ProviderContract, ProviderFieldContract, ProviderReturnContract,
};
pub use openapi_import::{
    OpenApiCatalogImportError, OpenApiCatalogImportRequest, OpenApiCatalogImportResult, import_openapi_catalog_into_registry,
};

pub use search::{SearchError, SearchHandle, create_search_handle};

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, fs, time, vec};

    use indexmap::{IndexMap, IndexSet};
    use oatty_types::{
        EnvVar, ProviderContract, ProviderFieldContract, ProviderReturnContract,
        command::HttpCommandSpec,
        manifest::{RegistryCatalog, RegistryManifest},
    };

    use super::*;

    fn unique_temp_dir() -> std::path::PathBuf {
        let nanos = time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        std::env::temp_dir().join(format!("oatty-registry-test-{nanos}"))
    }

    /// Tests that the manifest loader can read catalog metadata from disk and produce a non-empty registry.
    #[test]
    fn manifest_non_empty_and_unique_names() {
        let temp_dir = unique_temp_dir();
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        let manifest_path = temp_dir.join("manifest.bin");

        let command = CommandSpec::new_http(
            "apps".into(),
            "list".into(),
            "List applications".into(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/apps", None),
            0,
        );
        let mut provider_contracts = IndexMap::new();
        provider_contracts.insert(
            "apps list".into(),
            ProviderContract {
                arguments: Vec::new(),
                returns: ProviderReturnContract {
                    fields: vec![ProviderFieldContract {
                        name: "name".into(),
                        r#type: Some("string".into()),
                        tags: vec!["display".into()],
                    }],
                },
            },
        );

        let manifest = RegistryManifest {
            commands: vec![command],
            provider_contracts,
            ..Default::default()
        };
        let manifest_bytes: Vec<u8> = manifest.clone().try_into().expect("manifest serializes");
        fs::write(&manifest_path, manifest_bytes).expect("write manifest");

        let catalog = RegistryCatalog {
            title: "Test Catalog".into(),
            description: "Generated for unit tests".into(),
            manifest_path: manifest_path.to_string_lossy().to_string(),
            headers: IndexSet::<EnvVar>::new(),
            base_urls: vec!["https://api.example.com".into()],
            base_url_index: 0,
            manifest: Some(manifest),
            is_enabled: true,
        };
        let config = RegistryConfig {
            catalogs: Some(vec![catalog]),
        };
        let registry = CommandRegistry::from_registry_config(config).expect("load registry from manifest");
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

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
