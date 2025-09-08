//! Plugin engine for managing MCP plugins.

mod engine;
mod lifecycle;
mod registry;

pub use engine::{PluginEngine, PluginEngineError};
pub use lifecycle::{LifecycleError, LifecycleManager};
pub use registry::{PluginInfo, PluginRegistry, RegistryError};
