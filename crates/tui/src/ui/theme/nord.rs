//! Provides the Nord theme implementations that map the canonical palette to the
//! application's theme roles for both default and high-contrast variants.

use ratatui::style::Color;

use super::{
    roles::{Theme, ThemeRoles},
    theme_helpers::{darken_rgb, lighten_rgb},
};

// Polar Night (base surfaces)
pub const N0: Color = Color::Rgb(0x2E, 0x34, 0x40); // #2E3440
pub const N1: Color = Color::Rgb(0x3B, 0x42, 0x52); // #3B4252
pub const N2: Color = Color::Rgb(0x43, 0x4C, 0x5E); // #434C5E
pub const N3: Color = Color::Rgb(0x4C, 0x56, 0x6A); // #4C566A

// Snow Storm (foregrounds)
pub const S0: Color = Color::Rgb(0xD8, 0xDE, 0xE9); // #D8DEE9
pub const S1: Color = Color::Rgb(0xE5, 0xE9, 0xF0); // #E5E9F0
pub const S2: Color = Color::Rgb(0xEC, 0xEF, 0xF4); // #ECEFF4

// Frost (non-semantic accents)
pub const F0: Color = Color::Rgb(0x8F, 0xBC, 0xBB); // #8FBCBB
pub const F1: Color = Color::Rgb(0x88, 0xC0, 0xD0); // #88C0D0
pub const F2: Color = Color::Rgb(0x81, 0xA1, 0xC1); // #81A1C1
pub const F3: Color = Color::Rgb(0x5E, 0x81, 0xAC); // #5E81AC
pub const F4: Color = Color::Rgb(0xB4, 0x8E, 0xAD); // #B48EAD

// Aurora (semantic status)
pub const A_RED: Color = Color::Rgb(0xBF, 0x61, 0x6A); // #BF616A
pub const A_ORANGE: Color = Color::Rgb(0xD0, 0x87, 0x70); // #D08770
pub const A_YELLOW: Color = Color::Rgb(0xEB, 0xCB, 0x8B); // #EBCB8B
pub const A_GREEN: Color = Color::Rgb(0xA3, 0xBE, 0x8C); // #A3BE8C

// THEME.md authoritative aliases
pub const BG_MAIN: Color = N0; // App/root background
pub const BG_PANEL: Color = N1; // Secondary panels/cards/inputs
pub const BG_PANEL_MUTED: Color = N2; // Muted or inactive surfaces
pub const BG_MODAL_OVERLAY: Color = Color::Rgb(0x1A, 0x1E, 0x28); // Darkened overlay for modals
pub const UI_BORDER: Color = N1; // Borders/dividers
pub const UI_DIVIDER: Color = N3; // Separators/scrollbars
pub const TEXT_MUTED: Color = Color::Rgb(0x61, 0x6E, 0x88); // #616E88 muted/disabled text

pub const TEXT_PRIMARY: Color = S0; // Default text
pub const TEXT_SECONDARY: Color = S1; // Titles/headers/labels
pub const TEXT_SELECTED: Color = S2; // Highlighted text

pub const ACCENT_CYAN: Color = F0;
pub const ACCENT_TEAL: Color = F1; // Fuzzy-match highlight, input focus underline
pub const ACCENT_BLUE: Color = F2; // Timestamps, secondary accents
pub const ACCENT_DARK: Color = F3; // Selected row background, IDs

pub const STATUS_ERROR: Color = A_RED;
pub const STATUS_WARN: Color = A_ORANGE;
pub const STATUS_PENDING: Color = A_YELLOW;
pub const STATUS_OK: Color = A_GREEN;

fn build_nord_roles() -> ThemeRoles {
    ThemeRoles {
        background: BG_MAIN,
        surface: BG_PANEL,
        surface_muted: BG_PANEL_MUTED,
        border: UI_BORDER,
        divider: UI_DIVIDER,

        text: TEXT_PRIMARY,
        text_secondary: TEXT_SECONDARY,
        text_muted: TEXT_MUTED,

        accent_primary: ACCENT_TEAL,
        accent_secondary: ACCENT_BLUE,
        accent_subtle: ACCENT_CYAN,

        info: ACCENT_BLUE,
        success: STATUS_OK,
        warning: STATUS_WARN,
        error: STATUS_ERROR,

        selection_bg: UI_DIVIDER,
        selection_fg: TEXT_SELECTED,
        focus: ACCENT_TEAL,
        search_highlight: ACCENT_TEAL,
        syntax_keyword: A_RED,
        syntax_function: A_GREEN,
        syntax_string: A_YELLOW,
        syntax_number: A_ORANGE,
        syntax_type: ACCENT_TEAL,
        modal_bg: BG_MODAL_OVERLAY,
        scrollbar_track: UI_DIVIDER,
        scrollbar_thumb: UI_BORDER,
        table_row_even: darken_rgb(BG_PANEL, 0.60),
        table_row_odd: darken_rgb(UI_BORDER, 0.60),
    }
}

fn build_nord_high_contrast_roles() -> ThemeRoles {
    let mut roles = build_nord_roles();
    roles.surface_muted = lighten_rgb(roles.surface_muted, 0.15);
    roles.border = lighten_rgb(roles.border, 0.30);
    roles.divider = lighten_rgb(roles.divider, 0.20);

    roles.text = TEXT_SELECTED;
    roles.text_secondary = TEXT_SELECTED;
    roles.text_muted = TEXT_SECONDARY;

    roles.warning = STATUS_PENDING;
    roles.selection_bg = lighten_rgb(roles.selection_bg, 0.10);
    roles.focus = ACCENT_DARK;
    roles.scrollbar_thumb = lighten_rgb(roles.scrollbar_thumb, 0.25);
    roles.table_row_even = darken_rgb(BG_PANEL, 0.50);
    roles.table_row_odd = darken_rgb(UI_DIVIDER, 0.50);
    roles
}

/// Default Nord theme tuned for dark terminals.
#[derive(Debug, Clone)]
pub struct NordTheme {
    roles: ThemeRoles,
}

impl NordTheme {
    /// Construct a Nord theme instance using the canonical palette.
    pub fn new() -> Self {
        Self { roles: build_nord_roles() }
    }
}

impl Theme for NordTheme {
    fn roles(&self) -> &ThemeRoles {
        &self.roles
    }
}

/// High-contrast variant derived from the canonical Nord palette.
#[derive(Debug, Clone)]
pub struct NordThemeHighContrast {
    roles: ThemeRoles,
}

impl NordThemeHighContrast {
    /// Construct the Nord high-contrast variant by brightening text and borders.
    pub fn new() -> Self {
        Self {
            roles: build_nord_high_contrast_roles(),
        }
    }
}

impl Theme for NordThemeHighContrast {
    fn roles(&self) -> &ThemeRoles {
        &self.roles
    }
}
