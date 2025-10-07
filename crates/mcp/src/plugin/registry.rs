//! Plugin registry for managing plugin metadata.

use crate::types::{PluginDetail, PluginStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Registry for managing plugin metadata.
#[derive(Clone, Debug)]
pub struct PluginRegistry {
    /// Registered plugins.
    plugins: Arc<Mutex<HashMap<String, PluginDetail>>>,

    /// Plugin status.
    status: Arc<Mutex<HashMap<String, PluginStatus>>>,
}

impl PluginRegistry {
    /// Create a new plugin registry.
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(Mutex::new(HashMap::new())),
            status: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a plugin.
    pub async fn register_plugin(&self, mut plugin: PluginDetail) -> Result<(), RegistryError> {
        let name = plugin.name.clone();
        plugin.status = PluginStatus::Stopped;

        let mut plugins = self.plugins.lock().await;
        let mut status = self.status.lock().await;

        plugins.insert(name.clone(), plugin);
        status.insert(name, PluginStatus::Stopped);

        Ok(())
    }

    /// Unregister a plugin.
    pub async fn unregister_plugin(&self, name: &str) -> Result<(), RegistryError> {
        let mut plugins = self.plugins.lock().await;
        let mut status = self.status.lock().await;

        plugins.remove(name);
        status.remove(name);

        Ok(())
    }

    /// Get a plugin by name.
    pub async fn get_plugin(&self, name: &str) -> Option<PluginDetail> {
        let plugins = self.plugins.lock().await;
        plugins.get(name).cloned()
    }

    /// Get all plugins.
    pub async fn get_all_plugins(&self) -> Vec<PluginDetail> {
        let plugins = self.plugins.lock().await;
        plugins.values().cloned().collect()
    }

    /// Get plugin status.
    pub async fn get_plugin_status(&self, name: &str) -> Option<PluginStatus> {
        let status = self.status.lock().await;
        status.get(name).cloned()
    }

    /// Set plugin status.
    pub async fn set_plugin_status(&self, name: &str, status: PluginStatus) -> Result<(), RegistryError> {
        {
            let mut plugins = self.plugins.lock().await;
            if let Some(detail) = plugins.get_mut(name) {
                detail.status = status;
            } else {
                return Err(RegistryError::PluginNotFound { name: name.to_string() });
            }
        }

        let mut status_map = self.status.lock().await;
        status_map.insert(name.to_string(), status);
        Ok(())
    }

    /// Update the tracked tool count for a plugin.
    pub async fn set_plugin_tool_count(&self, name: &str, count: usize) -> Result<(), RegistryError> {
        let mut plugins = self.plugins.lock().await;
        if let Some(detail) = plugins.get_mut(name) {
            detail.tool_count = count;
            Ok(())
        } else {
            Err(RegistryError::PluginNotFound { name: name.to_string() })
        }
    }

    /// Get all plugin names.
    pub async fn get_plugin_names(&self) -> Vec<String> {
        let plugins = self.plugins.lock().await;
        plugins.keys().cloned().collect()
    }

    /// Check if a plugin is registered.
    pub async fn is_registered(&self, name: &str) -> bool {
        let plugins = self.plugins.lock().await;
        plugins.contains_key(name)
    }

    /// Get plugins by tag.
    pub async fn get_plugins_by_tag(&self, tag: &str) -> Vec<PluginDetail> {
        let plugins = self.plugins.lock().await;
        plugins
            .values()
            .filter(|plugin| plugin.tags.contains(&tag.to_string()))
            .cloned()
            .collect()
    }

    /// Get enabled plugins.
    pub async fn get_enabled_plugins(&self) -> Vec<PluginDetail> {
        let plugins = self.plugins.lock().await;
        plugins.values().filter(|plugin| plugin.enabled).cloned().collect()
    }

    /// Get disabled plugins.
    pub async fn get_disabled_plugins(&self) -> Vec<PluginDetail> {
        let plugins = self.plugins.lock().await;
        plugins.values().filter(|plugin| !plugin.enabled).cloned().collect()
    }

    /// Update plugin information.
    pub async fn update_plugin(&self, name: &str, plugin: PluginDetail) -> Result<(), RegistryError> {
        if plugin.name != name {
            return Err(RegistryError::InvalidPluginName {
                expected: name.to_string(),
                received: plugin.name.clone(),
            });
        }

        let status_value = plugin.status;

        {
            let mut plugins = self.plugins.lock().await;
            plugins.insert(name.to_string(), plugin);
        }

        let mut status = self.status.lock().await;
        status.insert(name.to_string(), status_value);
        Ok(())
    }

    /// Clear all plugins.
    pub async fn clear(&self) -> Result<(), RegistryError> {
        let mut plugins = self.plugins.lock().await;
        let mut status = self.status.lock().await;

        plugins.clear();
        status.clear();

        Ok(())
    }

    /// Get plugin count.
    pub async fn count(&self) -> usize {
        let plugins = self.plugins.lock().await;
        plugins.len()
    }

    /// Search plugins by name or tag.
    pub async fn search_plugins(&self, query: &str) -> Vec<PluginDetail> {
        let plugins = self.plugins.lock().await;
        let query_lower = query.to_lowercase();

        plugins
            .values()
            .filter(|plugin| {
                plugin.name.to_lowercase().contains(&query_lower) || plugin.tags.iter().any(|tag| tag.to_lowercase().contains(&query_lower))
            })
            .cloned()
            .collect()
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

    #[tokio::test]
    async fn test_plugin_registry() {
        let registry = PluginRegistry::new();

        let mut plugin = PluginDetail::new("test-plugin".to_string(), "node test.js".to_string(), None);
        plugin.transport_type = "stdio".to_string();
        plugin.tags = vec!["test".to_string(), "example".to_string()];
        plugin.enabled = true;

        // Register plugin
        registry.register_plugin(plugin).await.unwrap();

        // Check if registered
        assert!(registry.is_registered("test-plugin").await);

        // Get plugin
        let retrieved = registry.get_plugin("test-plugin").await.unwrap();
        assert_eq!(retrieved.name, "test-plugin");

        // Get all plugins
        let all_plugins = registry.get_all_plugins().await;
        assert_eq!(all_plugins.len(), 1);

        // Search plugins
        let search_results = registry.search_plugins("test").await;
        assert_eq!(search_results.len(), 1);

        // Get plugins by tag
        let tagged_plugins = registry.get_plugins_by_tag("test").await;
        assert_eq!(tagged_plugins.len(), 1);

        // Unregister plugin
        registry.unregister_plugin("test-plugin").await.unwrap();
        assert!(!registry.is_registered("test-plugin").await);
    }

    #[tokio::test]
    async fn test_plugin_status() {
        let registry = PluginRegistry::new();

        let mut plugin = PluginDetail::new("test-plugin".to_string(), "node test.js".to_string(), None);
        plugin.transport_type = "stdio".to_string();

        registry.register_plugin(plugin).await.unwrap();

        // Check initial status
        let status = registry.get_plugin_status("test-plugin").await.unwrap();
        assert_eq!(status, PluginStatus::Stopped);

        // Update status
        registry.set_plugin_status("test-plugin", PluginStatus::Running).await.unwrap();

        let status = registry.get_plugin_status("test-plugin").await.unwrap();
        assert_eq!(status, PluginStatus::Running);
    }

    #[tokio::test]
    async fn update_plugin_rejects_mismatched_name() {
        let registry = PluginRegistry::new();

        let mut original_plugin = PluginDetail::new("alpha".to_string(), "node alpha.js".to_string(), None);
        original_plugin.transport_type = "stdio".to_string();
        registry.register_plugin(original_plugin).await.unwrap();

        let mut mismatched_plugin = PluginDetail::new("beta".to_string(), "node beta.js".to_string(), None);
        mismatched_plugin.transport_type = "stdio".to_string();

        let result = registry.update_plugin("alpha", mismatched_plugin).await;
        assert!(matches!(
            result,
            Err(RegistryError::InvalidPluginName { expected, received })
                if expected == "alpha" && received == "beta"
        ));
    }
}
