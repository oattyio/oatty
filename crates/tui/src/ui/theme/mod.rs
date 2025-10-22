//! Theme styling module for the TUI UI layer.
//!
//! This module defines multiple color palettes (Dracula, Nord, Cyberpunk),
//! an ANSI 256-color fallback, semantic theme roles, and helper builders for
//! Ratatui widgets and styles. Prefer these helpers over hard-coding colors
//! to keep the UI consistent and elegant.

use std::env;

use tracing::debug;

pub mod ansi256;
pub mod cyberpunk;
pub mod dracula;
pub mod nord;
pub mod roles;
pub mod theme_helpers;

pub use ansi256::{Ansi256Theme, Ansi256ThemeHighContrast};
pub use cyberpunk::{CyberpunkTheme, CyberpunkThemeHighContrast};
pub use dracula::{DraculaTheme, DraculaThemeHighContrast};
pub use nord::{NordTheme, NordThemeHighContrast};
pub use roles::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorCapability {
    Truecolor,
    Ansi256,
}

/// Selects a theme based on `TUI_THEME` environment variable.
///
/// Supported values:
/// - `dracula` (default)
/// - `dracula_hc`, `dracula-high-contrast`, `dracula-hc`
/// - `nord`, `nord_hc`, `nord-high-contrast`, `nord-hc`
/// - `cyberpunk`, `cyberpunk_hc`, `cyberpunk-high-contrast`, `cyberpunk-hc`
/// - `ansi256`, `ansi256_hc`, `ansi256-high-contrast`, `ansi256-hc`
pub fn load_from_env() -> Box<dyn Theme> {
    if let Ok(theme_name) = env::var("TUI_THEME")
        && let Some(theme) = parse_theme_override(theme_name.trim())
    {
        return theme;
    }

    match detect_color_capability() {
        ColorCapability::Truecolor => Box::new(DraculaTheme::new()),
        ColorCapability::Ansi256 => {
            debug!("Detected ANSI 256-color terminal; loading fallback palette");
            Box::new(Ansi256Theme::new())
        }
    }
}

fn parse_theme_override(theme_name: &str) -> Option<Box<dyn Theme>> {
    match theme_name.to_ascii_lowercase().as_str() {
        "cyberpunk_hc" | "cyberpunk-high-contrast" | "cyberpunk-hc" | "cyberpunkhc" => Some(Box::new(CyberpunkThemeHighContrast::new())),
        "cyberpunk" => Some(Box::new(CyberpunkTheme::new())),
        "dracula_hc" | "dracula-high-contrast" | "dracula-hc" | "draculahc" => Some(Box::new(DraculaThemeHighContrast::new())),
        "dracula" => Some(Box::new(DraculaTheme::new())),
        "nord_hc" | "nord-high-contrast" | "nord-hc" | "nordhc" => Some(Box::new(NordThemeHighContrast::new())),
        "nord" => Some(Box::new(NordTheme::new())),
        "ansi256_hc" | "ansi256-high-contrast" | "ansi256-hc" | "ansi256hc" => Some(Box::new(Ansi256ThemeHighContrast::new())),
        "ansi256" => Some(Box::new(Ansi256Theme::new())),
        _ => None,
    }
}

fn detect_color_capability() -> ColorCapability {
    if let Some(mode) = env::var("TUI_COLOR_MODE").ok().and_then(|value| parse_color_mode(value.trim())) {
        return mode;
    }

    if env::var("TUI_FORCE_TRUECOLOR")
        .ok()
        .map(|value| is_truthy(value.trim()))
        .unwrap_or(false)
    {
        return ColorCapability::Truecolor;
    }

    let color_term = env::var("COLORTERM").unwrap_or_default().to_ascii_lowercase();
    if color_term.contains("truecolor") || color_term.contains("24bit") {
        return ColorCapability::Truecolor;
    }

    let term = env::var("TERM").unwrap_or_default().to_ascii_lowercase();
    if term.contains("truecolor") {
        return ColorCapability::Truecolor;
    }

    ColorCapability::Ansi256
}

fn parse_color_mode(value: &str) -> Option<ColorCapability> {
    match value.to_ascii_lowercase().as_str() {
        "truecolor" | "24bit" => Some(ColorCapability::Truecolor),
        "ansi256" | "256" | "8bit" => Some(ColorCapability::Ansi256),
        _ => None,
    }
}

fn is_truthy(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on" | "enable" | "enabled"
    )
}
