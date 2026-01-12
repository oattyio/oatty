//! Plugin registry for managing plugin metadata.

use crate::types::{PluginDetail, PluginStatus};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

static LOCK_ERROR_MSG: &str = "Failed to acquire lock";

/// Registry for managing plugin metadata.
#[derive(Clone, Debug)]
pub struct PluginRegistry {
    /// Registered plugins.
    plugins: Arc<RwLock<HashMap<String, PluginDetail>>>,

    /// Plugin status.
    status: Arc<RwLock<HashMap<String, PluginStatus>>>,
}

impl PluginRegistry {
    /// Create a new plugin registry.
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            status: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a plugin.
    pub fn register_plugin(&mut self, mut plugin: PluginDetail) -> Result<(), RegistryError> {
        let name = plugin.name.clone();
        plugin.status = PluginStatus::Stopped;

        let mut plugins = self.plugins.write().map_err(|_| RegistryError::OperationFailed {
            reason: LOCK_ERROR_MSG.to_string(),
        })?;
        let mut status = self.status.write().map_err(|_| RegistryError::OperationFailed {
            reason: LOCK_ERROR_MSG.to_string(),
        })?;

        plugins.insert(name.clone(), plugin);
        status.insert(name, PluginStatus::Stopped);

        Ok(())
    }

    /// Unregister a plugin.
    pub fn unregister_plugin(&mut self, name: &str) -> Result<(), RegistryError> {
        {
            let mut plugins = self.plugins.write().map_err(|_| RegistryError::OperationFailed {
                reason: LOCK_ERROR_MSG.to_string(),
            })?;
            plugins.remove(name);
        }

        let mut status = self.status.write().map_err(|_| RegistryError::OperationFailed {
            reason: LOCK_ERROR_MSG.to_string(),
        })?;

        status.remove(name);

        Ok(())
    }

    /// Get a plugin by name.
    pub fn get_plugin(&self, name: &str) -> Option<PluginDetail> {
        self.plugins.read().map(|p| p.get(name).cloned()).unwrap_or_default()
    }

    /// Get all plugins.
    pub fn get_all_plugins(&self) -> Vec<PluginDetail> {
        self.plugins.read().map(|p| p.values().cloned().collect()).unwrap_or_default()
    }

    /// Get plugin status.
    pub fn get_plugin_status(&self, name: &str) -> Option<PluginStatus> {
        self.status.read().map(|s| s.get(name).cloned()).unwrap_or_default()
    }

    /// Set plugin status.
    pub fn set_plugin_status(&mut self, name: &str, status: PluginStatus) -> Result<(), RegistryError> {
        let mut plugins = self.plugins.write().map_err(|_| RegistryError::OperationFailed {
            reason: LOCK_ERROR_MSG.to_string(),
        })?;
        let Some(detail) = plugins.get_mut(name) else {
            return Err(RegistryError::PluginNotFound { name: name.to_string() });
        };
        detail.status = status;

        let mut status_guard = self.status.write().map_err(|_| RegistryError::OperationFailed {
            reason: LOCK_ERROR_MSG.to_string(),
        })?;
        status_guard.insert(name.to_string(), status);
        Ok(())
    }

    /// Update the tracked tool count for a plugin.
    pub fn set_plugin_tool_count(&mut self, name: &str, count: usize) -> Result<(), RegistryError> {
        let mut plugins = self.plugins.write().map_err(|_| RegistryError::OperationFailed {
            reason: LOCK_ERROR_MSG.to_string(),
        })?;
        if let Some(detail) = plugins.get_mut(name) {
            detail.tool_count = count;
            Ok(())
        } else {
            Err(RegistryError::PluginNotFound { name: name.to_string() })
        }
    }

    /// Get all plugin names.
    pub fn get_plugin_names(&self) -> Vec<String> {
        let Ok(plugins) = self.plugins.read() else {
            return vec![];
        };
        plugins.keys().cloned().collect()
    }

    /// Check if a plugin is registered.
    pub fn is_registered(&self, name: &str) -> bool {
        self.plugins.read().map(|p| p.contains_key(name)).unwrap_or_default()
    }

    /// Get plugins by tag.
    pub fn get_plugins_by_tag(&self, tag: &str) -> Vec<PluginDetail> {
        self.plugins
            .read()
            .map(|p| {
                p.values()
                    .filter(|plugin| plugin.tags.contains(&tag.to_string()))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get enabled plugins.
    pub fn get_enabled_plugins(&self) -> Vec<PluginDetail> {
        self.plugins
            .read()
            .map(|p| p.values().filter(|plugin| plugin.enabled).cloned().collect())
            .unwrap_or_default()
    }

    /// Get disabled plugins.
    pub fn get_disabled_plugins(&self) -> Vec<PluginDetail> {
        self.plugins
            .read()
            .map(|p| p.values().filter(|plugin| !plugin.enabled).cloned().collect())
            .unwrap_or_default()
    }

    /// Update plugin information.
    pub fn update_plugin(&mut self, name: &str, plugin: PluginDetail) -> Result<(), RegistryError> {
        if plugin.name != name {
            return Err(RegistryError::InvalidPluginName {
                expected: name.to_string(),
                received: plugin.name.clone(),
            });
        }

        let status_value = plugin.status;

        self.plugins
            .write()
            .map(|mut p| p.insert(name.to_string(), plugin))
            .map_err(|_| RegistryError::OperationFailed {
                reason: LOCK_ERROR_MSG.to_string(),
            })?;

        self.status
            .write()
            .map(|mut s| s.insert(name.to_string(), status_value))
            .map_err(|_| RegistryError::OperationFailed {
                reason: LOCK_ERROR_MSG.to_string(),
            })?;
        Ok(())
    }

    /// Clear all plugins.
    pub fn clear(&mut self) -> Result<(), RegistryError> {
        self.plugins
            .write()
            .map(|mut p| p.clear())
            .map_err(|_| RegistryError::OperationFailed {
                reason: LOCK_ERROR_MSG.to_string(),
            })?;
        self.status
            .write()
            .map(|mut s| s.clear())
            .map_err(|_| RegistryError::OperationFailed {
                reason: LOCK_ERROR_MSG.to_string(),
            })?;
        Ok(())
    }

    /// Get plugin count.
    pub fn count(&self) -> usize {
        self.plugins.read().map(|p| p.len()).unwrap_or_default()
    }

    /// Search plugins by name or tag.
    pub fn search(&self, query: &str) -> Vec<PluginDetail> {
        let query_lower = query.to_lowercase();

        self.plugins
            .read()
            .map(|p| {
                p.values()
                    .filter(|plugin| {
                        plugin.name.to_lowercase().contains(&query_lower)
                            || plugin.tags.iter().any(|tag| tag.to_lowercase().contains(&query_lower))
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur in the plugin registry.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("Plugin not found: {name}")]
    PluginNotFound { name: String },

    #[error("Plugin already exists: {name}")]
    PluginAlreadyExists { name: String },

    #[error("Invalid plugin configuration: {reason}")]
    InvalidConfiguration { reason: String },

    #[error("Registry operation failed: {reason}")]
    OperationFailed { reason: String },

    #[error("Plugin name mismatch: expected {expected}, received {received}")]
    InvalidPluginName { expected: String, received: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_registry() {
        let mut registry = PluginRegistry::new();

        let mut plugin = PluginDetail::new("test-plugin".to_string(), "node test.js".to_string(), None);
        plugin.transport_type = "stdio".to_string();
        plugin.tags = vec!["test".to_string(), "example".to_string()];
        plugin.enabled = true;

        // Register plugin
        registry.register_plugin(plugin).unwrap();

        // Check if registered
        assert!(registry.is_registered("test-plugin"));

        // Get plugin
        let retrieved = registry.get_plugin("test-plugin").unwrap();
        assert_eq!(retrieved.name, "test-plugin");

        // Get all plugins
        let all_plugins = registry.get_all_plugins();
        assert_eq!(all_plugins.len(), 1);

        // Search plugins
        let search_results = registry.search("test");
        assert_eq!(search_results.len(), 1);

        // Get plugins by tag
        let tagged_plugins = registry.get_plugins_by_tag("test");
        assert_eq!(tagged_plugins.len(), 1);

        // Unregister plugin
        registry.unregister_plugin("test-plugin").unwrap();
        assert!(!registry.is_registered("test-plugin"));
    }

    #[test]
    fn test_plugin_status() {
        let mut registry = PluginRegistry::new();

        let mut plugin = PluginDetail::new("test-plugin".to_string(), "node test.js".to_string(), None);
        plugin.transport_type = "stdio".to_string();

        registry.register_plugin(plugin).unwrap();

        // Check initial status
        let status = registry.get_plugin_status("test-plugin").unwrap();
        assert_eq!(status, PluginStatus::Stopped);

        // Update status
        registry.set_plugin_status("test-plugin", PluginStatus::Running).unwrap();

        let status = registry.get_plugin_status("test-plugin").unwrap();
        assert_eq!(status, PluginStatus::Running);
    }

    #[test]
    fn update_plugin_rejects_mismatched_name() {
        let mut registry = PluginRegistry::new();

        let mut original_plugin = PluginDetail::new("alpha".to_string(), "node alpha.js".to_string(), None);
        original_plugin.transport_type = "stdio".to_string();
        registry.register_plugin(original_plugin).unwrap();

        let mut mismatched_plugin = PluginDetail::new("beta".to_string(), "node beta.js".to_string(), None);
        mismatched_plugin.transport_type = "stdio".to_string();

        let result = registry.update_plugin("alpha", mismatched_plugin);
        assert!(matches!(
            result,
            Err(RegistryError::InvalidPluginName { expected, received })
                if expected == "alpha" && received == "beta"
        ));
    }

    #[test]
    fn set_plugin_status_requires_registered_plugin() {
        let mut registry = PluginRegistry::new();

        let result = registry.set_plugin_status("ghost", PluginStatus::Running);
        assert!(matches!(
            result,
            Err(RegistryError::PluginNotFound { name }) if name == "ghost"
        ));
        assert!(registry.get_plugin_status("ghost").is_none());
    }
}
