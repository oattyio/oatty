use std::{convert::Infallible, env, path::PathBuf};

use anyhow::Error;
use dirs_next::config_dir;
use heck::{ToSnakeCase, ToSnekCase};
use indexmap::set::MutableValues;
use oatty_types::{EnvVar, manifest::RegistryCatalog};
use oatty_util::{expand_tilde, interpolate_string, tokenize_env};
use postcard::to_stdvec;
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
            std::fs::create_dir_all(&catalogs_path)?;

            for catalog in catalogs {
                tokenize_env(&mut catalog.headers, &catalog.title.to_snek_case())?;
                // The manifest is a binary format for fast loading
                // and we do not want it to be stored in the config file.
                let Some(manifest) = catalog.manifest.as_ref() else {
                    continue;
                };
                let Ok(bytes) = to_stdvec(&manifest) else {
                    continue;
                };
                let file_name = format!("{}.bin", catalog.title.to_snake_case());
                let file_path = &catalogs_path.join(file_name);
                if let Ok(exists) = std::fs::exists(file_path)
                    && !exists
                {
                    std::fs::write(file_path, bytes)?;
                }
                catalog.manifest_path = file_path.to_string_lossy().to_string();
            }
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;

        if let Some(catalogs) = self.catalogs.as_mut() {
            for catalog in catalogs {
                for j in 0..catalog.headers.len() {
                    let Some(EnvVar { value, .. }) = catalog.headers.get_index_mut2(j) else {
                        continue;
                    };
                    let Ok(val) = interpolate_string(value) else {
                        continue;
                    };
                    *value = val;
                }
            }
        }
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
