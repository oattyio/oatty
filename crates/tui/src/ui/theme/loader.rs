//! Theme selection and terminal capability detection.

use crate::ui::theme::{Theme, ThemeDefinition, catalog};
use std::env;
use tracing::debug;

/// Loaded theme plus metadata about which definition produced it.
pub struct LoadedTheme {
    pub definition: &'static ThemeDefinition,
    pub theme: Box<dyn Theme>,
}

impl LoadedTheme {
    fn from_definition(definition: &'static ThemeDefinition) -> Self {
        Self {
            definition,
            theme: definition.build(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorCapability {
    Truecolor,
    Ansi256,
}

/// Selects a theme from explicit overrides, user preference, and terminal capability.
pub fn load(preferred_theme: Option<&str>) -> LoadedTheme {
    let capability = detect_color_capability();
    if matches!(capability, ColorCapability::Ansi256) {
        debug!("ANSI-only terminal detected; ignoring theme overrides and forcing fallback palette.");
        return LoadedTheme::from_definition(catalog::default_ansi());
    }

    if let Ok(theme_name) = env::var("TUI_THEME")
        && let Some(definition) = catalog::resolve(theme_name.trim())
    {
        return LoadedTheme::from_definition(definition);
    }

    if let Some(name) = preferred_theme
        && let Some(definition) = catalog::resolve(name.trim())
    {
        return LoadedTheme::from_definition(definition);
    }

    LoadedTheme::from_definition(catalog::default_truecolor())
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

/// Returns whether theme picker is relevant for this terminal capability.
pub fn supports_theme_picker() -> bool {
    matches!(detect_color_capability(), ColorCapability::Truecolor)
}
