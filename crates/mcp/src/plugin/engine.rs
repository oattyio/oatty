//! Plugin engine implementation.

use crate::client::McpClientManager;
use crate::config::McpConfig;
use crate::logging::LogManager;
use crate::plugin::{LifecycleManager, PluginRegistry};
use crate::types::{PluginDetail, PluginStatus};
use std::sync::Arc;

/// Plugin engine that orchestrates all MCP plugin operations.
pub struct PluginEngine {
    /// Client manager for handling MCP connections.
    client_manager: McpClientManager,

    /// Log manager for plugin logs.
    log_manager: Arc<LogManager>,

    /// Plugin registry for metadata.
    registry: PluginRegistry,

    /// Lifecycle manager for plugin lifecycle.
    lifecycle_manager: LifecycleManager,

    /// Configuration.
    config: McpConfig,
}

impl PluginEngine {
    /// Create a new plugin engine.
    pub fn new(config: McpConfig) -> anyhow::Result<Self> {
        let client_manager = McpClientManager::new(config.clone())?;
        let log_manager = Arc::new(LogManager::new()?);
        let registry = PluginRegistry::new();
        let lifecycle_manager = LifecycleManager::new();

        Ok(Self {
            client_manager,
            log_manager,
            registry,
            lifecycle_manager,
            config,
        })
    }

    /// Start the plugin engine.
    pub async fn start(&self) -> Result<(), PluginEngineError> {
        // Start the client manager
        self.client_manager
            .start()
            .await
            .map_err(|e| PluginEngineError::ClientManagerError(e.to_string()))?;

        // Register all configured plugins
        for (name, server) in &self.config.mcp_servers {
            let plugin_info = crate::plugin::PluginInfo {
                name: name.clone(),
                command_or_url: if server.is_stdio() {
                    server.command.as_ref().unwrap().clone()
                } else {
                    server.base_url.as_ref().unwrap().to_string()
                },
                transport_type: server.transport_type().to_string(),
                tags: server.tags.clone().unwrap_or_default(),
                enabled: !server.is_disabled(),
            };

            self.registry.register_plugin(plugin_info).await?;
            self.lifecycle_manager.register_plugin(name.clone()).await;
        }

        tracing::info!("Plugin engine started");
        Ok(())
    }

    /// Stop the plugin engine.
    pub async fn stop(&self) -> Result<(), PluginEngineError> {
        // Stop the client manager
        self.client_manager
            .stop()
            .await
            .map_err(|e| PluginEngineError::ClientManagerError(e.to_string()))?;

        tracing::info!("Plugin engine stopped");
        Ok(())
    }

    /// Start a plugin.
    pub async fn start_plugin(&self, name: &str) -> Result<(), PluginEngineError> {
        // Check if plugin is registered
        if !self.registry.is_registered(name).await {
            return Err(PluginEngineError::PluginNotFound { name: name.to_string() });
        }

        // Start the plugin using lifecycle management
        let start_fn = {
            let client_manager = self.client_manager.clone();
            let name = name.to_string();
            move || {
                let client_manager = client_manager.clone();
                let name = name.clone();
                Box::pin(async move { client_manager.start_plugin(&name).await.map_err(|e| e.to_string()) })
                    as std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>
            }
        };

        self.lifecycle_manager.start_plugin(name, start_fn).await?;

        // Update registry
        self.registry.set_plugin_status(name, PluginStatus::Running).await?;

        tracing::info!("Started plugin: {}", name);
        Ok(())
    }

    /// Stop a plugin.
    pub async fn stop_plugin(&self, name: &str) -> Result<(), PluginEngineError> {
        // Check if plugin is registered
        if !self.registry.is_registered(name).await {
            return Err(PluginEngineError::PluginNotFound { name: name.to_string() });
        }

        // Stop the plugin using lifecycle management
        let stop_fn = {
            let client_manager = self.client_manager.clone();
            let name = name.to_string();
            move || {
                let client_manager = client_manager.clone();
                let name = name.clone();
                Box::pin(async move { client_manager.stop_plugin(&name).await.map_err(|e| e.to_string()) })
                    as std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>
            }
        };

        self.lifecycle_manager.stop_plugin(name, stop_fn).await?;

        // Update registry
        self.registry.set_plugin_status(name, PluginStatus::Stopped).await?;

        tracing::info!("Stopped plugin: {}", name);
        Ok(())
    }

    /// Restart a plugin.
    pub async fn restart_plugin(&self, name: &str) -> Result<(), PluginEngineError> {
        // Check if plugin is registered
        if !self.registry.is_registered(name).await {
            return Err(PluginEngineError::PluginNotFound { name: name.to_string() });
        }

        // Check if we can restart
        if !self.lifecycle_manager.can_restart(name).await {
            return Err(PluginEngineError::MaxRestartAttemptsExceeded { name: name.to_string() });
        }

        // Restart the plugin using lifecycle management
        let stop_fn = {
            let client_manager = self.client_manager.clone();
            let name = name.to_string();
            move || {
                let client_manager = client_manager.clone();
                let name = name.clone();
                Box::pin(async move { client_manager.stop_plugin(&name).await.map_err(|e| e.to_string()) })
                    as std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>
            }
        };

        let start_fn = {
            let client_manager = self.client_manager.clone();
            let name = name.to_string();
            move || {
                let client_manager = client_manager.clone();
                let name = name.clone();
                Box::pin(async move { client_manager.start_plugin(&name).await.map_err(|e| e.to_string()) })
                    as std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>
            }
        };

        self.lifecycle_manager.restart_plugin(name, stop_fn, start_fn).await?;

        // Update registry
        self.registry.set_plugin_status(name, PluginStatus::Running).await?;

        tracing::info!("Restarted plugin: {}", name);
        Ok(())
    }

    /// Get plugin details.
    pub async fn get_plugin_detail(&self, name: &str) -> Result<PluginDetail, PluginEngineError> {
        let plugin_info = self
            .registry
            .get_plugin(name)
            .await
            .ok_or_else(|| PluginEngineError::PluginNotFound { name: name.to_string() })?;

        let status = self
            .registry
            .get_plugin_status(name)
            .await
            .unwrap_or(PluginStatus::Stopped);

        let health = self.client_manager.get_plugin_health(name).await.unwrap_or_default();

        let logs = self.log_manager.get_recent_logs(name, 100).await;

        let mut detail = PluginDetail::new(name.to_string(), plugin_info.command_or_url);
        detail.status = status;
        detail.health = health;
        detail.tags = plugin_info.tags;
        detail.logs = logs;

        Ok(detail)
    }

    /// List all plugins.
    pub async fn list_plugins(&self) -> Vec<PluginDetail> {
        let mut plugins = Vec::new();

        for name in self.registry.get_plugin_names().await {
            if let Ok(detail) = self.get_plugin_detail(&name).await {
                plugins.push(detail);
            }
        }

        plugins
    }

    /// Get plugin status.
    pub async fn get_plugin_status(&self, name: &str) -> Result<PluginStatus, PluginEngineError> {
        self.registry
            .get_plugin_status(name)
            .await
            .ok_or_else(|| PluginEngineError::PluginNotFound { name: name.to_string() })
    }

    /// Check if a plugin is running.
    pub async fn is_plugin_running(&self, name: &str) -> bool {
        matches!(self.get_plugin_status(name).await, Ok(PluginStatus::Running))
    }

    /// Get the client manager.
    pub fn client_manager(&self) -> &McpClientManager {
        &self.client_manager
    }

    /// Get the log manager.
    pub fn log_manager(&self) -> &LogManager {
        &self.log_manager
    }

    /// Get the plugin registry.
    pub fn registry(&self) -> &PluginRegistry {
        &self.registry
    }

    /// Get the lifecycle manager.
    pub fn lifecycle_manager(&self) -> &LifecycleManager {
        &self.lifecycle_manager
    }

    /// Update configuration.
    pub async fn update_config(&self, config: McpConfig) -> Result<(), PluginEngineError> {
        // Stop all existing plugins
        for name in self.registry.get_plugin_names().await {
            if let Err(e) = self.stop_plugin(&name).await {
                tracing::warn!("Failed to stop plugin {} during config update: {}", name, e);
            }
        }

        // Update client manager configuration
        self.client_manager
            .update_config(config.clone())
            .await
            .map_err(|e| PluginEngineError::ClientManagerError(e.to_string()))?;

        // Clear and rebuild registry
        self.registry.clear().await?;

        for (name, server) in &config.mcp_servers {
            let plugin_info = crate::plugin::PluginInfo {
                name: name.clone(),
                command_or_url: if server.is_stdio() {
                    server.command.as_ref().unwrap().clone()
                } else {
                    server.base_url.as_ref().unwrap().to_string()
                },
                transport_type: server.transport_type().to_string(),
                tags: server.tags.clone().unwrap_or_default(),
                enabled: !server.is_disabled(),
            };

            self.registry.register_plugin(plugin_info).await?;
            self.lifecycle_manager.register_plugin(name.clone()).await;
        }

        tracing::info!("Plugin engine configuration updated");
        Ok(())
    }
}

/// Errors that can occur in the plugin engine.
#[derive(Debug, thiserror::Error)]
pub enum PluginEngineError {
    #[error("Plugin not found: {name}")]
    PluginNotFound { name: String },

    #[error("Client manager error: {0}")]
    ClientManagerError(String),

    #[error("Registry error: {0}")]
    RegistryError(#[from] crate::plugin::RegistryError),

    #[error("Lifecycle error: {0}")]
    LifecycleError(#[from] crate::plugin::LifecycleError),

    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },

    #[error("Maximum restart attempts exceeded for plugin {name}")]
    MaxRestartAttemptsExceeded { name: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::McpConfig;
    use url::Url;

    #[tokio::test]
    async fn test_plugin_engine_creation() {
        let config = McpConfig::default();
        let engine = PluginEngine::new(config).unwrap();

        let plugins = engine.list_plugins().await;
        assert!(plugins.is_empty());
    }

    #[tokio::test]
    async fn test_plugin_engine_start_stop() {
        let config = McpConfig::default();
        let engine = PluginEngine::new(config).unwrap();

        engine.start().await.unwrap();
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_engine_registers_tags_from_config() {
        let mut cfg = McpConfig::default();
        let mut server = crate::config::McpServer::default();
        server.base_url = Some(Url::parse("https://example.com").unwrap());
        server.tags = Some(vec!["alpha".into(), "beta".into()]);
        server.disabled = Some(true);
        cfg.mcp_servers.insert("svc".into(), server);

        let engine = PluginEngine::new(cfg).unwrap();
        engine.start().await.unwrap();

        let info = engine.registry().get_plugin("svc").await.unwrap();
        assert_eq!(info.tags, vec!["alpha", "beta"]);
        assert!(!info.enabled);

        engine.stop().await.unwrap();
    }
}
