#![allow(clippy::module_inception)]
pub mod hint_bar;
pub mod palette;
pub mod providers;
pub mod state;
pub mod suggest;

pub use hint_bar::PaletteHintBar;
pub use palette::PaletteComponent;
pub use state::PaletteState;
