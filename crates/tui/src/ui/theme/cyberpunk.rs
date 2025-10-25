//! Cyberpunk-inspired theme definitions for the TUI layer.
//!
//! This module provides a neon-forward, high-contrast palette tuned for dark
//! terminals. Colors are mapped into the semantic [`ThemeRoles`] structure so
//! components can render consistently without hard-coding styling details.

use ratatui::style::Color;

use super::roles::{Theme, ThemeRoles};

/// Primary surfaces for the cyberpunk palette.
const BACKGROUND_BASE: Color = Color::Rgb(0x0D, 0x02, 0x21); // #0d0221
const BACKGROUND_PANEL: Color = Color::Rgb(0x16, 0x06, 0x3B); // #16063b
const BACKGROUND_MODAL_OVERLAY: Color = Color::Rgb(0x09, 0x01, 0x1B); // #09011b
const SURFACE_MUTED: Color = Color::Rgb(0x24, 0x00, 0x46); // #240046
const BORDER_DEFAULT: Color = Color::Rgb(0x41, 0x33, 0x7A); // #41337a

/// Typography colors.
const TEXT_PRIMARY: Color = Color::Rgb(0xF8, 0xEE, 0xFF); // #f8eeff
const TEXT_SECONDARY: Color = Color::Rgb(0x9A, 0x86, 0xFD); // #9a86fd
const TEXT_MUTED: Color = Color::Rgb(0x6A, 0x64, 0xA4); // #6a64a4

/// Accent palette.
const ACCENT_PRIMARY: Color = Color::Rgb(0x00, 0xF6, 0xFF); // #00f6ff
const ACCENT_SECONDARY: Color = Color::Rgb(0xFF, 0x4E, 0xCD); // #ff4ecd
const ACCENT_SUBTLE: Color = Color::Rgb(0x7C, 0xFF, 0xCB); // #7cffcb

/// Status colors suitable for bright neon themes.
const STATUS_INFO: Color = ACCENT_PRIMARY;
const STATUS_SUCCESS: Color = Color::Rgb(0x72, 0xF1, 0xB8); // #72f1b8
const STATUS_WARNING: Color = Color::Rgb(0xFF, 0xD1, 0x66); // #ffd166
const STATUS_ERROR: Color = Color::Rgb(0xFF, 0x29, 0x65); // #ff2965

/// Selection and focus styling.
const SELECTION_BACKGROUND: Color = Color::Rgb(0x2A, 0x1A, 0x5E); // #2a1a5e
const SELECTION_FOREGROUND: Color = TEXT_PRIMARY;
const FOCUS_BORDER: Color = ACCENT_SECONDARY;
const SCROLLBAR_TRACK: Color = Color::Rgb(0x1C, 0x10, 0x3F); // #1c103f
const SCROLLBAR_THUMB: Color = ACCENT_SECONDARY;

/// Cyberpunk theme tuned for dark backgrounds and neon accents.
#[derive(Debug, Clone)]
pub struct CyberpunkTheme {
    roles: ThemeRoles,
}

impl CyberpunkTheme {
    /// Builds the standard cyberpunk theme.
    pub fn new() -> Self {
        Self {
            roles: ThemeRoles {
                background: BACKGROUND_BASE,
                surface: BACKGROUND_PANEL,
                surface_muted: SURFACE_MUTED,
                border: BORDER_DEFAULT,
                divider: BORDER_DEFAULT,
                text: TEXT_PRIMARY,
                text_secondary: TEXT_SECONDARY,
                text_muted: TEXT_MUTED,
                accent_primary: ACCENT_PRIMARY,
                accent_secondary: ACCENT_SECONDARY,
                accent_subtle: ACCENT_SUBTLE,
                info: STATUS_INFO,
                success: STATUS_SUCCESS,
                warning: STATUS_WARNING,
                error: STATUS_ERROR,
                selection_bg: SELECTION_BACKGROUND,
                selection_fg: SELECTION_FOREGROUND,
                focus: FOCUS_BORDER,
                search_highlight: STATUS_WARNING,
                syntax_keyword: ACCENT_SECONDARY,
                syntax_function: ACCENT_PRIMARY,
                syntax_string: STATUS_SUCCESS,
                syntax_number: STATUS_WARNING,
                syntax_type: ACCENT_SUBTLE,
                modal_bg: BACKGROUND_MODAL_OVERLAY,
                scrollbar_track: SCROLLBAR_TRACK,
                scrollbar_thumb: SCROLLBAR_THUMB,
            },
        }
    }
}

impl Theme for CyberpunkTheme {
    fn roles(&self) -> &ThemeRoles {
        &self.roles
    }
}

/// High-contrast variant with sharper borders and amplified text contrast.
#[derive(Debug, Clone)]
pub struct CyberpunkThemeHighContrast {
    roles: ThemeRoles,
}

impl CyberpunkThemeHighContrast {
    /// Builds the high-contrast cyberpunk theme.
    pub fn new() -> Self {
        Self {
            roles: ThemeRoles {
                background: BACKGROUND_BASE,
                surface: BACKGROUND_PANEL,
                surface_muted: SURFACE_MUTED,
                border: ACCENT_SECONDARY,
                divider: ACCENT_SECONDARY,
                text: TEXT_PRIMARY,
                text_secondary: TEXT_PRIMARY,
                text_muted: TEXT_SECONDARY,
                accent_primary: ACCENT_PRIMARY,
                accent_secondary: ACCENT_SECONDARY,
                accent_subtle: ACCENT_SUBTLE,
                info: STATUS_INFO,
                success: STATUS_SUCCESS,
                warning: STATUS_WARNING,
                error: STATUS_ERROR,
                selection_bg: Color::Rgb(0x3C, 0x1F, 0x7B), // brighter selection #3c1f7b
                selection_fg: SELECTION_FOREGROUND,
                focus: ACCENT_PRIMARY,
                search_highlight: STATUS_WARNING,
                syntax_keyword: ACCENT_SECONDARY,
                syntax_function: ACCENT_PRIMARY,
                syntax_string: STATUS_SUCCESS,
                syntax_number: STATUS_WARNING,
                syntax_type: ACCENT_SUBTLE,
                modal_bg: BACKGROUND_MODAL_OVERLAY,
                scrollbar_track: SCROLLBAR_TRACK,
                scrollbar_thumb: ACCENT_PRIMARY,
            },
        }
    }
}

impl Theme for CyberpunkThemeHighContrast {
    fn roles(&self) -> &ThemeRoles {
        &self.roles
    }
}
