use oatty_types::manifest::RegistryManifest;

// Re-export public items from modules
pub mod io;
pub mod openapi;
pub mod provider_resolver;

pub use io::{generate_manifest, write_manifest, write_manifest_json};

/// Alias re-export for the generated registry manifest type.
pub type Registry = RegistryManifest;
