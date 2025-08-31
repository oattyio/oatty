use ratatui::style::Color;

use super::roles::{Theme, ThemeRoles};

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

// Aurora (semantic status)
pub const A_RED: Color = Color::Rgb(0xBF, 0x61, 0x6A); // #BF616A
pub const A_ORANGE: Color = Color::Rgb(0xD0, 0x87, 0x70); // #D08770
pub const A_YELLOW: Color = Color::Rgb(0xEB, 0xCB, 0x8B); // #EBCB8B
pub const A_GREEN: Color = Color::Rgb(0xA3, 0xBE, 0x8C); // #A3BE8C
pub const A_PURPLE: Color = Color::Rgb(0xB4, 0x8E, 0xAD); // #B48EAD

// THEME.md authoritative aliases
pub const BG_MAIN: Color = N0; // App/root background
pub const BG_PANEL: Color = N1; // Secondary panels/cards/inputs
pub const UI_BORDER: Color = N2; // Borders/dividers/scrollbars
pub const TEXT_MUTED: Color = N3; // Ghost text/hints/placeholders

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
pub const STATUS_NOTE: Color = A_PURPLE; // Plugin badges/special

/// Default Nord theme tuned for dark terminals.
#[derive(Debug, Clone)]
pub struct NordTheme {
    roles: ThemeRoles,
}

impl NordTheme {
    pub fn new() -> Self {
        Self {
            roles: ThemeRoles {
                background: BG_MAIN,
                surface: BG_PANEL,
                surface_muted: UI_BORDER,
                border: UI_BORDER,
                divider: UI_BORDER,

                text: TEXT_PRIMARY,
                text_secondary: TEXT_SECONDARY,
                text_muted: TEXT_MUTED,

                // Accents: follow THEME.md (Frost for non-semantic)
                accent_primary: ACCENT_TEAL,
                accent_secondary: ACCENT_BLUE,
                accent_subtle: ACCENT_CYAN,

                // Status colors: Aurora mapping
                info: ACCENT_BLUE,
                success: STATUS_OK,
                warning: STATUS_PENDING,
                error: STATUS_ERROR,

                // Selection: ACCENT_DARK background with selected text
                selection_bg: ACCENT_DARK,
                selection_fg: TEXT_SELECTED,
                // Focus underline/border
                focus: ACCENT_TEAL,

                // Scrollbars
                scrollbar_track: UI_BORDER,
                scrollbar_thumb: UI_BORDER,
            },
        }
    }
}

impl Theme for NordTheme {
    fn roles(&self) -> &ThemeRoles {
        &self.roles
    }
}

/// High-contrast variant with stronger dividers and selection.
#[derive(Debug, Clone)]
pub struct NordThemeHighContrast {
    roles: ThemeRoles,
}

impl NordThemeHighContrast {
    pub fn new() -> Self {
        Self {
            roles: ThemeRoles {
                background: BG_MAIN,
                surface: BG_PANEL,
                surface_muted: UI_BORDER,
                border: ACCENT_DARK, // stronger borders for clarity
                divider: UI_BORDER,

                text: TEXT_SELECTED,
                text_secondary: TEXT_SELECTED, // push toward max readability
                text_muted: TEXT_SECONDARY,

                accent_primary: ACCENT_TEAL,
                accent_secondary: ACCENT_BLUE,
                accent_subtle: ACCENT_CYAN,

                info: ACCENT_BLUE,
                success: STATUS_OK,
                warning: STATUS_PENDING,
                error: STATUS_ERROR,

                selection_bg: N3, // darker neutral surface selection
                selection_fg: TEXT_SELECTED,
                focus: ACCENT_DARK,

                scrollbar_track: UI_BORDER,
                scrollbar_thumb: ACCENT_DARK,
            },
        }
    }
}

impl Theme for NordThemeHighContrast {
    fn roles(&self) -> &ThemeRoles {
        &self.roles
    }
}
