//! MCP Plugins UI components (list, details, add view, logs, env, search, table).
//!
//! Re-exports core components and state needed by the TUI layer. These widgets
//! follow the app's `Component` trait contract and the theme helpers for a
//! consistent look-and-feel across the interface.

mod add_plugin;
mod details_component;
mod logs;
mod plugins_component;
mod search_component;
mod secrets;
mod state;
mod table;
mod types;

pub use add_plugin::AddTransport;
pub use add_plugin::PluginsAddComponent;
pub use details_component::PluginsDetailsComponent;
pub use logs::PluginsLogsComponent;
pub use plugins_component::PluginsComponent;
pub use search_component::PluginsSearchComponent;
pub use secrets::{PluginSecretsEditorState, PluginsSecretsComponent};
pub use state::{PluginListItem, PluginsState};
pub use table::{PluginsTableComponent, PluginsTableState};
pub use types::*;
