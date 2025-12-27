use std::{convert::Infallible, env, io::Error, path::PathBuf};

use dirs_next::config_dir;
use oatty_types::manifest::RegistryCatalog;
use oatty_util::expand_tilde;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryConfig {
    pub catalogs: Option<Vec<RegistryCatalog>>,
}

impl RegistryConfig {
    pub fn load() -> Result<Self, Infallible> {
        let path = default_config_path();
        if let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(config) = serde_json::from_str(&content)
        {
            return Ok(config);
        }
        Ok(RegistryConfig::default())
    }

    pub fn save(&self) -> Result<(), Error> {
        let path = default_config_path();
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}

/// Get the default path for the Registry configuration file.
pub fn default_config_path() -> PathBuf {
    if let Ok(path) = env::var("REGISTRY_CONFIG_PATH")
        && !path.trim().is_empty()
    {
        return expand_tilde(&path);
    }

    config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("oatty")
        .join("registry.json")
}
