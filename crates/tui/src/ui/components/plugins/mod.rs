//! MCP Plugins UI components (list, details, add view, logs, env, search, table).
//!
//! Re-exports core components and state needed by the TUI layer. These widgets
//! follow the app's `Component` trait contract and the theme helpers for a
//! consistent look-and-feel across the interface.

mod add;
mod details;
mod env;
mod hints_bar;
mod logs;
mod plugins;
mod search;
mod state;
mod table;

pub use add::PluginsAddComponent;
pub use details::PluginsDetailsComponent;
pub use env::PluginsEnvComponent;
pub use hints_bar::PluginHintsBar;
pub use logs::PluginsLogsComponent;
pub use plugins::PluginsComponent;
pub use search::PluginsSearchComponent;
pub use state::{AddTransport, EnvRow, PluginListItem, PluginsState};
pub use table::PluginsTableComponent;
