use std::{convert::Infallible, env, io::Error, path::PathBuf};

use dirs_next::config_dir;
use heck::ToSnakeCase;
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

    pub fn save(&mut self) -> Result<(), Error> {
        let path = default_config_path();
        if let Some(catalogs) = self.catalogs.as_mut() {
            let catalogs_path = default_catalogs_path();
            for catalog in catalogs {
                let Some(manifest) = catalog.manifest.take() else {
                    continue;
                };
                let Ok(bytes): Result<Vec<u8>, _> = manifest.try_into() else {
                    continue;
                };
                let file_name = &catalog.title.to_snake_case();
                let file_path = catalogs_path.join(file_name);
                catalog.manifest_path = file_path.to_string_lossy().to_string();
                std::fs::write(file_path, bytes)?;
            }
        }

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

/// Get the default path for the Registry catalogs file.
pub fn default_catalogs_path() -> PathBuf {
    if let Ok(path) = env::var("REGISTRY_CATALOGS_PATH")
        && !path.trim().is_empty()
    {
        return expand_tilde(&path);
    }

    config_dir().unwrap_or_else(|| PathBuf::from(".")).join("oatty").join("catalogs")
}
