//! Plugin lifecycle management.

use crate::types::{HealthStatus, PluginStatus};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout};
use tracing::{debug, warn};

/// Lifecycle manager for handling plugin startup, shutdown, and recovery.
#[derive(Debug)]
pub struct LifecycleManager {
    /// Plugin lifecycle states.
    states: Arc<Mutex<HashMap<String, LifecycleState>>>,

    /// Startup timeout.
    startup_timeout: Duration,

    /// Shutdown timeout.
    shutdown_timeout: Duration,

    /// Restart delay.
    restart_delay: Duration,

    /// Maximum restart attempts.
    max_restart_attempts: u32,
}

/// Lifecycle state for a plugin.
#[derive(Debug, Clone)]
pub struct LifecycleState {
    /// Current status.
    pub status: PluginStatus,

    /// Health status.
    pub health: HealthStatus,

    /// Number of restart attempts.
    pub restart_attempts: u32,

    /// Last restart time.
    pub last_restart: Option<SystemTime>,

    /// Startup time.
    pub startup_time: Option<SystemTime>,

    /// Shutdown time.
    pub shutdown_time: Option<SystemTime>,
}

impl LifecycleManager {
    /// Create a new lifecycle manager.
    pub fn new() -> Self {
        Self {
            states: Arc::new(Mutex::new(HashMap::new())),
            startup_timeout: Duration::from_secs(30),
            shutdown_timeout: Duration::from_secs(10),
            restart_delay: Duration::from_secs(5),
            max_restart_attempts: 3,
        }
    }

    /// Create a new lifecycle manager with custom settings.
    pub fn with_settings(
        startup_timeout: Duration,
        shutdown_timeout: Duration,
        restart_delay: Duration,
        max_restart_attempts: u32,
    ) -> Self {
        Self {
            states: Arc::new(Mutex::new(HashMap::new())),
            startup_timeout,
            shutdown_timeout,
            restart_delay,
            max_restart_attempts,
        }
    }

    /// Register a plugin for lifecycle management.
    pub async fn register_plugin(&self, name: String) {
        let name_clone = name.clone();
        let mut states = self.states.lock().await;
        states.insert(
            name,
            LifecycleState {
                status: PluginStatus::Stopped,
                health: HealthStatus::new(),
                restart_attempts: 0,
                last_restart: None,
                startup_time: None,
                shutdown_time: None,
            },
        );
        debug!("Registered plugin for lifecycle management: {}", name_clone);
    }

    /// Unregister a plugin from lifecycle management.
    pub async fn unregister_plugin(&self, name: &str) {
        let mut states = self.states.lock().await;
        states.remove(name);
        debug!("Unregistered plugin from lifecycle management: {}", name);
    }

    /// Start a plugin with lifecycle management.
    pub async fn start_plugin<F>(&self, name: &str, start_fn: F) -> Result<(), LifecycleError>
    where
        F: FnOnce() -> std::pin::Pin<Box<dyn Future<Output = Result<(), String>> + Send>>,
    {
        let start_time = SystemTime::now();

        // Update state
        {
            let mut states = self.states.lock().await;
            if let Some(state) = states.get_mut(name) {
                state.status = PluginStatus::Starting;
                state.startup_time = Some(start_time);
            }
        }

        // Attempt to start the plugin with timeout
        let result = timeout(self.startup_timeout, start_fn()).await;

        match result {
            Ok(Ok(())) => {
                // Success
                let mut states = self.states.lock().await;
                if let Some(state) = states.get_mut(name) {
                    state.status = PluginStatus::Running;
                    state.health.mark_healthy();
                    state.restart_attempts = 0;
                }
                debug!("Plugin started successfully: {}", name);
                Ok(())
            }
            Ok(Err(error)) => {
                // Start function failed
                let mut states = self.states.lock().await;
                if let Some(state) = states.get_mut(name) {
                    state.status = PluginStatus::Error;
                    state.health.mark_unhealthy(error.clone());
                }
                Err(LifecycleError::StartupFailed {
                    name: name.to_string(),
                    reason: error,
                })
            }
            Err(_) => {
                // Timeout
                let mut states = self.states.lock().await;
                if let Some(state) = states.get_mut(name) {
                    state.status = PluginStatus::Error;
                    state.health.mark_unhealthy("Startup timeout".to_string());
                }
                Err(LifecycleError::StartupTimeout {
                    name: name.to_string(),
                    timeout: self.startup_timeout,
                })
            }
        }
    }

    /// Stop a plugin with lifecycle management.
    pub async fn stop_plugin<F>(&self, name: &str, stop_fn: F) -> Result<(), LifecycleError>
    where
        F: FnOnce() -> std::pin::Pin<Box<dyn Future<Output = Result<(), String>> + Send>>,
    {
        let shutdown_time = SystemTime::now();

        // Update state
        {
            let mut states = self.states.lock().await;
            if let Some(state) = states.get_mut(name) {
                state.status = PluginStatus::Stopping;
                state.shutdown_time = Some(shutdown_time);
            }
        }

        // Attempt to stop the plugin with timeout
        let result = timeout(self.shutdown_timeout, stop_fn()).await;

        match result {
            Ok(Ok(())) => {
                // Success
                let mut states = self.states.lock().await;
                if let Some(state) = states.get_mut(name) {
                    state.status = PluginStatus::Stopped;
                    state.health.mark_unhealthy("Stopped".to_string());
                }
                debug!("Plugin stopped successfully: {}", name);
                Ok(())
            }
            Ok(Err(error)) => {
                // Stop function failed
                let mut states = self.states.lock().await;
                if let Some(state) = states.get_mut(name) {
                    state.status = PluginStatus::Error;
                    state.health.mark_unhealthy(error.clone());
                }
                Err(LifecycleError::ShutdownFailed {
                    name: name.to_string(),
                    reason: error,
                })
            }
            Err(_) => {
                // Timeout
                let mut states = self.states.lock().await;
                if let Some(state) = states.get_mut(name) {
                    state.status = PluginStatus::Error;
                    state.health.mark_unhealthy("Shutdown timeout".to_string());
                }
                Err(LifecycleError::ShutdownTimeout {
                    name: name.to_string(),
                    timeout: self.shutdown_timeout,
                })
            }
        }
    }

    /// Restart a plugin with lifecycle management.
    pub async fn restart_plugin<F, G>(&self, name: &str, stop_fn: F, start_fn: G) -> Result<(), LifecycleError>
    where
        F: FnOnce() -> std::pin::Pin<Box<dyn Future<Output = Result<(), String>> + Send>>,
        G: FnOnce() -> std::pin::Pin<Box<dyn Future<Output = Result<(), String>> + Send>>,
    {
        let restart_time = SystemTime::now();

        // Update state
        {
            let mut states = self.states.lock().await;
            if let Some(state) = states.get_mut(name) {
                state.last_restart = Some(restart_time);
                state.restart_attempts += 1;
            }
        }

        // Stop the plugin
        if let Err(e) = self.stop_plugin(name, stop_fn).await {
            warn!("Failed to stop plugin {} during restart: {}", name, e);
        }

        // Wait before restarting
        sleep(self.restart_delay).await;

        // Start the plugin
        self.start_plugin(name, start_fn).await?;

        debug!("Plugin restarted successfully: {}", name);
        Ok(())
    }

    /// Get lifecycle state for a plugin.
    pub async fn get_state(&self, name: &str) -> Option<LifecycleState> {
        let states = self.states.lock().await;
        states.get(name).cloned()
    }

    /// Get all lifecycle states.
    pub async fn get_all_states(&self) -> HashMap<String, LifecycleState> {
        let states = self.states.lock().await;
        states.clone()
    }

    /// Check if a plugin can be restarted.
    pub async fn can_restart(&self, name: &str) -> bool {
        let states = self.states.lock().await;
        if let Some(state) = states.get(name) {
            state.restart_attempts < self.max_restart_attempts
        } else {
            false
        }
    }

    /// Get the number of restart attempts for a plugin.
    pub async fn get_restart_attempts(&self, name: &str) -> u32 {
        let states = self.states.lock().await;
        states.get(name).map(|state| state.restart_attempts).unwrap_or(0)
    }

    /// Reset restart attempts for a plugin.
    pub async fn reset_restart_attempts(&self, name: &str) {
        let mut states = self.states.lock().await;
        if let Some(state) = states.get_mut(name) {
            state.restart_attempts = 0;
        }
    }

    /// Get startup timeout.
    pub fn startup_timeout(&self) -> Duration {
        self.startup_timeout
    }

    /// Get shutdown timeout.
    pub fn shutdown_timeout(&self) -> Duration {
        self.shutdown_timeout
    }

    /// Get restart delay.
    pub fn restart_delay(&self) -> Duration {
        self.restart_delay
    }

    /// Get maximum restart attempts.
    pub fn max_restart_attempts(&self) -> u32 {
        self.max_restart_attempts
    }
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during lifecycle management.
#[derive(Debug, thiserror::Error)]
pub enum LifecycleError {
    #[error("Plugin not found: {name}")]
    PluginNotFound { name: String },

    #[error("Startup failed for plugin {name}: {reason}")]
    StartupFailed { name: String, reason: String },

    #[error("Startup timeout for plugin {name}: {timeout:?}")]
    StartupTimeout { name: String, timeout: Duration },

    #[error("Shutdown failed for plugin {name}: {reason}")]
    ShutdownFailed { name: String, reason: String },

    #[error("Shutdown timeout for plugin {name}: {timeout:?}")]
    ShutdownTimeout { name: String, timeout: Duration },

    #[error("Restart failed for plugin {name}: {reason}")]
    RestartFailed { name: String, reason: String },

    #[error("Maximum restart attempts exceeded for plugin {name}")]
    MaxRestartAttemptsExceeded { name: String },

    #[error("Lifecycle operation failed: {reason}")]
    OperationFailed { reason: String },
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;

    use super::*;

    #[tokio::test]
    async fn test_lifecycle_manager() {
        let manager = LifecycleManager::new();

        // Register a plugin
        manager.register_plugin("test-plugin".to_string()).await;

        // Check initial state
        let state = manager.get_state("test-plugin").await.unwrap();
        assert_eq!(state.status, PluginStatus::Stopped);
        assert_eq!(state.restart_attempts, 0);

        // Test startup
        let start_fn = || Box::pin(async { Ok(()) }) as Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
        manager.start_plugin("test-plugin", start_fn).await.unwrap();

        let state = manager.get_state("test-plugin").await.unwrap();
        assert_eq!(state.status, PluginStatus::Running);
        assert!(state.startup_time.is_some());

        // Test shutdown
        let stop_fn = || Box::pin(async { Ok(()) }) as Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
        manager.stop_plugin("test-plugin", stop_fn).await.unwrap();

        let state = manager.get_state("test-plugin").await.unwrap();
        assert_eq!(state.status, PluginStatus::Stopped);
        assert!(state.shutdown_time.is_some());

        // Unregister plugin
        manager.unregister_plugin("test-plugin").await;
        assert!(manager.get_state("test-plugin").await.is_none());
    }

    #[tokio::test]
    async fn test_restart_attempts() {
        let manager = LifecycleManager::new();

        manager.register_plugin("test-plugin".to_string()).await;

        // Test restart attempts
        assert_eq!(manager.get_restart_attempts("test-plugin").await, 0);
        assert!(manager.can_restart("test-plugin").await);

        // Simulate restart attempts
        let mut states = manager.states.lock().await;
        if let Some(state) = states.get_mut("test-plugin") {
            state.restart_attempts = 3;
        }
        drop(states);

        assert_eq!(manager.get_restart_attempts("test-plugin").await, 3);
        assert!(!manager.can_restart("test-plugin").await);

        // Reset restart attempts
        manager.reset_restart_attempts("test-plugin").await;
        assert_eq!(manager.get_restart_attempts("test-plugin").await, 0);
        assert!(manager.can_restart("test-plugin").await);
    }
}
