//! UI components: palette, browser, help, table, pagination.

pub mod browser;
pub mod component;
pub mod help;
pub mod logs;
pub mod nav_bar;
pub mod pagination;
pub mod palette;
pub mod plugins;
pub mod table;
pub mod workflows;

pub use browser::BrowserComponent;
pub use component::*;
pub use help::HelpComponent;
pub use logs::LogsComponent;
pub use pagination::PaginationComponent;
pub use plugins::PluginsComponent;
pub use table::TableComponent;
pub use workflows::WorkflowsComponent;
