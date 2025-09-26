//! Theme styling module for the TUI UI layer.
//!
//! This module defines the Dracula color palette (default), optional Nord
//! palette, semantic theme roles, and helper builders for Ratatui widgets
//! and styles. Prefer these helpers over hard-coding colors to keep the UI
//! consistent and elegant.

pub mod cyberpunk;
pub mod dracula;
pub mod nord;
pub mod roles;
pub mod theme_helpers;

pub use cyberpunk::{CyberpunkTheme, CyberpunkThemeHighContrast};
pub use dracula::{DraculaTheme, DraculaThemeHighContrast};
pub use nord::{NordTheme, NordThemeHighContrast};
pub use roles::Theme;

/// Selects a theme based on `TUI_THEME` environment variable.
///
/// Supported values:
/// - `dracula` (default)
/// - `dracula_hc`, `dracula-high-contrast`, `dracula-hc`
/// - `nord`, `nord_hc`, `nord-high-contrast`, `nord-hc`
/// - `cyberpunk`, `cyberpunk_hc`, `cyberpunk-high-contrast`, `cyberpunk-hc`
pub fn load_from_env() -> Box<dyn Theme> {
    match std::env::var("TUI_THEME").ok().as_deref() {
        Some("cyberpunk_hc") | Some("cyberpunk-high-contrast") | Some("cyberpunk-hc") | Some("cyberpunkhc") => {
            Box::new(CyberpunkThemeHighContrast::new())
        }
        Some("cyberpunk") => Box::new(CyberpunkTheme::new()),
        Some("dracula_hc") | Some("dracula-high-contrast") | Some("dracula-hc") | Some("draculahc") => {
            Box::new(DraculaThemeHighContrast::new())
        }
        Some("dracula") => Box::new(DraculaTheme::new()),
        Some("nord_hc") | Some("nord-high-contrast") | Some("nord-hc") | Some("nordhc") => Box::new(NordThemeHighContrast::new()),
        Some("nord") => Box::new(NordTheme::new()),
        _ => Box::new(DraculaTheme::new()),
    }
}
