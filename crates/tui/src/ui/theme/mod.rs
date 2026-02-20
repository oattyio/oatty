//! Theme styling module for the TUI UI layer.
//!
//! This module defines multiple color palettes (Dracula, Nord, Cyberpunk),
//! an ANSI 256-color fallback, semantic theme roles, and helper builders for
//! Ratatui widgets and styles. Prefer these helpers over hard-coding colors
//! to keep the UI consistent and elegant.

pub mod ansi256;
pub mod catalog;
pub mod cyberpunk;
pub mod dracula;
mod loader;
pub mod nord;
pub mod roles;
pub mod theme_helpers;

pub use ansi256::{Ansi256Theme, Ansi256ThemeHighContrast};
pub use catalog::ThemeDefinition;
pub use cyberpunk::{CyberpunkTheme, CyberpunkThemeHighContrast};
pub use dracula::{DraculaTheme, DraculaThemeHighContrast};
pub use loader::{load, supports_theme_picker};
pub use nord::{NordTheme, NordThemeHighContrast};
pub use roles::Theme;
