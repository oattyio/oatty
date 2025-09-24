//! MCP Plugins UI components (list, details, add view, logs, env, search, table).
//!
//! Re-exports core components and state needed by the TUI layer. These widgets
//! follow the app's `Component` trait contract and the theme helpers for a
//! consistent look-and-feel across the interface.

mod details_component;
mod logs;
mod plugin_editor;
mod plugins_component;
mod search_component;
mod secrets;
mod state;
mod table;
mod types;

pub use details_component::PluginsDetailsComponent;
pub use heroku_mcp::PluginDetail;
pub use logs::PluginsLogsComponent;
pub use plugin_editor::PluginTransport;
pub use plugin_editor::PluginsEditComponent;
pub use plugins_component::PluginsComponent;
pub use search_component::PluginsSearchComponent;
pub use secrets::{PluginSecretsEditorState, PluginsSecretsComponent};
pub use state::PluginsState;
pub use table::{PluginsTableComponent, PluginsTableState};
pub use types::*;
