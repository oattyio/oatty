//! Cyberpunk-inspired theme definitions aligned with the canonical JetBrains
//! Cyberpunk theme palette. The colors declared here are mapped into
//! [`ThemeRoles`] so UI components can rely on semantic roles instead of
//! hard-coded values.

use ratatui::style::Color;

use super::{
    roles::{Theme, ThemeRoles},
    theme_helpers::{darken_rgb, lighten_rgb},
};

// Surface colors sourced from the original theme.
const GREY4: Color = Color::Rgb(0x21, 0x21, 0x21); // #212121
const GREY5: Color = Color::Rgb(0x27, 0x27, 0x27); // #272727
const GREY6: Color = Color::Rgb(0x2E, 0x2E, 0x2E); // #2E2E2E
const GREY8: Color = Color::Rgb(0x3B, 0x3B, 0x3B); // #3B3B3B
const GREY9: Color = Color::Rgb(0x42, 0x42, 0x42); // #424242
#[allow(dead_code)]
const GREY10: Color = Color::Rgb(0x4A, 0x4A, 0x4A); // #4A4A4A

// Typography colors.
const TEXT_PRIMARY: Color = Color::Rgb(0xDB, 0xDB, 0xDB); // #DBDBDB
const TEXT_SECONDARY: Color = Color::Rgb(0xCA, 0xCA, 0xCA); // #CACACA
const TEXT_MUTED: Color = Color::Rgb(0x77, 0x77, 0x77); // #777777
const TEXT_SELECTED: Color = Color::Rgb(0xED, 0xED, 0xED); // #EDEDED

// Accent palette.
const ACCENT_PRIMARY: Color = Color::Rgb(0x00, 0xF0, 0xFF); // #00F0FF
const ACCENT_SECONDARY: Color = Color::Rgb(0x36, 0x8A, 0xEC); // #368AEC
const ACCENT_SUBTLE: Color = Color::Rgb(0xA1, 0x86, 0xE1); // #A186E1
const ACCENT_PINK: Color = Color::Rgb(0xFF, 0xAE, 0xF4); // #FFAEF4

// Status colors from the canonical palette.
const STATUS_INFO: Color = ACCENT_PRIMARY;
const STATUS_SUCCESS: Color = Color::Rgb(0x51, 0xF6, 0x6F); // #51F66F
const STATUS_WARNING: Color = Color::Rgb(0xFF, 0xC0, 0x7A); // #FFC07A
const STATUS_ERROR: Color = Color::Rgb(0xF3, 0x50, 0x5C); // #F3505C

// Selection and focus styling.
const SELECTION_BACKGROUND: Color = GREY9;
const SELECTION_INACTIVE_BACKGROUND: Color = GREY8;
const SCROLLBAR_TRACK: Color = GREY9;
const SCROLLBAR_THUMB: Color = ACCENT_SECONDARY;

fn build_cyberpunk_roles() -> ThemeRoles {
    ThemeRoles {
        background: GREY6,
        surface: GREY5,
        surface_muted: GREY8,
        border: GREY4,
        divider: GREY9,
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
        selection_fg: TEXT_SELECTED,
        focus: ACCENT_SECONDARY,
        search_highlight: STATUS_WARNING,
        syntax_keyword: ACCENT_PINK,
        syntax_function: ACCENT_PRIMARY,
        syntax_string: STATUS_SUCCESS,
        syntax_number: STATUS_WARNING,
        syntax_type: ACCENT_SUBTLE,
        modal_bg: GREY4,
        scrollbar_track: SCROLLBAR_TRACK,
        scrollbar_thumb: SCROLLBAR_THUMB,
        table_row_even: darken_rgb(GREY5, 0.55),
        table_row_odd: darken_rgb(GREY4, 0.55),
    }
}

fn build_cyberpunk_high_contrast_roles() -> ThemeRoles {
    let mut roles = build_cyberpunk_roles();
    roles.surface_muted = lighten_rgb(roles.surface_muted, 0.12);
    roles.border = lighten_rgb(roles.border, 0.25);
    roles.divider = lighten_rgb(roles.divider, 0.15);
    roles.text = TEXT_SELECTED;
    roles.text_secondary = TEXT_SELECTED;
    roles.text_muted = TEXT_SECONDARY;
    roles.selection_bg = lighten_rgb(SELECTION_INACTIVE_BACKGROUND, 0.10);
    roles.focus = ACCENT_PRIMARY;
    roles.scrollbar_thumb = lighten_rgb(roles.scrollbar_thumb, 0.20);
    roles.table_row_even = darken_rgb(GREY5, 0.45);
    roles.table_row_odd = darken_rgb(GREY4, 0.45);
    roles
}

/// Cyberpunk theme tuned for dark backgrounds and neon accents.
#[derive(Debug, Clone)]
pub struct CyberpunkTheme {
    roles: ThemeRoles,
}

impl CyberpunkTheme {
    /// Construct the standard cyberpunk theme using canonical palette values.
    pub fn new() -> Self {
        Self {
            roles: build_cyberpunk_roles(),
        }
    }
}

impl Theme for CyberpunkTheme {
    fn roles(&self) -> &ThemeRoles {
        &self.roles
    }
}

/// High-contrast variant derived from the canonical cyberpunk palette.
#[derive(Debug, Clone)]
pub struct CyberpunkThemeHighContrast {
    roles: ThemeRoles,
}

impl CyberpunkThemeHighContrast {
    /// Construct the high-contrast cyberpunk theme by brightening text and borders.
    pub fn new() -> Self {
        Self {
            roles: build_cyberpunk_high_contrast_roles(),
        }
    }
}

impl Theme for CyberpunkThemeHighContrast {
    fn roles(&self) -> &ThemeRoles {
        &self.roles
    }
}
