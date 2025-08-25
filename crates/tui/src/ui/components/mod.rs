//! UI components: palette, builder, help, table.

pub mod builder;
pub mod help;
pub mod hint_bar;
pub mod logs;
pub mod palette;
pub mod table;

pub use builder::BuilderComponent;
pub use help::HelpComponent;
pub use hint_bar::HintBarComponent;
pub use logs::LogsComponent;
pub use palette::PaletteComponent;
pub use table::TableComponent;
