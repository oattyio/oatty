//! Nord-themed styling module for the TUI UI layer.
//!
//! This module defines a Nord color palette, semantic theme roles, and
//! helper builders for Ratatui widgets and styles. Prefer these helpers over
//! hard-coding colors to keep the UI consistent and elegant.

pub mod helpers;
pub mod nord;
pub mod roles;

pub use nord::{NordTheme, NordThemeHighContrast};
pub use roles::Theme;

/// Selects a theme based on `TUI_THEME` environment variable.
///
/// Supported values: `nord` (default), `nord_hc`, `nord-high-contrast`, `nord-hc`.
pub fn load_from_env() -> Box<dyn Theme> {
    match std::env::var("TUI_THEME").ok().as_deref() {
        Some("nord_hc") | Some("nord-high-contrast") | Some("nord-hc") | Some("nordhc") => {
            Box::new(NordThemeHighContrast::new())
        }
        _ => Box::new(NordTheme::new()),
    }
}
