//! Provides the Dracula theme implementations that align with the canonical
//! JetBrains Dracula palette while adapting it to the application's theme roles.

use ratatui::style::Color;

use super::{
    roles::{Theme, ThemeRoles},
    theme_helpers::{darken_rgb, lighten_rgb},
};

// Core backgrounds from the JetBrains Dracula theme.
pub const PRIMARY_BACKGROUND: Color = Color::Rgb(0x41, 0x44, 0x50); // #414450
pub const SECONDARY_BACKGROUND: Color = Color::Rgb(0x3A, 0x3D, 0x4C); // #3A3D4C
pub const HOVER_BACKGROUND: Color = Color::Rgb(0x28, 0x2A, 0x36); // #282A36
pub const SELECTION_BACKGROUND: Color = Color::Rgb(0x62, 0x72, 0xA4); // #6272A4
pub const SELECTION_INACTIVE_BACKGROUND: Color = Color::Rgb(0x4E, 0x5A, 0x82); // #4E5A82
pub const BORDER_COLOR: Color = Color::Rgb(0x28, 0x2A, 0x36); // #282A36
pub const SEPARATOR_COLOR: Color = Color::Rgb(0x5D, 0x5E, 0x66); // #5D5E66

// Foregrounds.
pub const PRIMARY_FOREGROUND: Color = Color::Rgb(0xF8, 0xF8, 0xF2); // #F8F8F2
pub const MUTED_FOREGROUND: Color = Color::Rgb(0x85, 0x89, 0x94); // #858994

// Canonical accents and status colors.
pub const ACCENT_PINK: Color = Color::Rgb(0xFF, 0x79, 0xC6); // #FF79C6
pub const ACCENT_PURPLE: Color = Color::Rgb(0xBD, 0x93, 0xF9); // #BD93F9
pub const ACCENT_BLUE: Color = Color::Rgb(0x5D, 0xA3, 0xF4); // #5DA3F4
pub const ACCENT_CYAN: Color = Color::Rgb(0x8B, 0xE9, 0xFD); // #8BE9FD
pub const ACCENT_GREEN: Color = Color::Rgb(0x2F, 0xC8, 0x64); // #2FC864
pub const ACCENT_ORANGE: Color = Color::Rgb(0xFF, 0xB8, 0x6C); // #FFB86C
pub const ACCENT_RED: Color = Color::Rgb(0xFF, 0x55, 0x54); // #FF5554
pub const ACCENT_YELLOW: Color = Color::Rgb(0xF1, 0xFA, 0x8C); // #F1FA8C

// Theme role aliases.
pub const BG_MAIN: Color = PRIMARY_BACKGROUND; // App/root background
pub const BG_PANEL: Color = SECONDARY_BACKGROUND; // Panels/cards
pub const BG_PANEL_MUTED: Color = HOVER_BACKGROUND; // Muted or inactive surfaces
pub const BG_MODAL_OVERLAY: Color = Color::Rgb(0x1D, 0x1F, 0x27); // Darkened overlay for modals
pub const UI_BORDER: Color = BORDER_COLOR; // Borders
pub const UI_DIVIDER: Color = SEPARATOR_COLOR; // Dividers/scrollbars

pub const TEXT_PRIMARY: Color = PRIMARY_FOREGROUND; // Default text
pub const TEXT_SECONDARY: Color = ACCENT_PURPLE; // Titles/headers/labels
pub const TEXT_MUTED: Color = MUTED_FOREGROUND; // Ghost text/hints/placeholders
pub const TEXT_SELECTED: Color = PRIMARY_FOREGROUND; // Highlighted text

pub const ACCENT_PRIMARY: Color = ACCENT_PINK; // Interactive elements / prompts
pub const ACCENT_SECONDARY: Color = ACCENT_PURPLE; // Focus, progress
pub const ACCENT_SUBTLE: Color = MUTED_FOREGROUND; // Subtle accent

pub const STATUS_INFO: Color = ACCENT_BLUE;
pub const STATUS_OK: Color = ACCENT_GREEN;
pub const STATUS_WARN: Color = ACCENT_YELLOW; // warnings/modified
pub const STATUS_ERROR: Color = ACCENT_RED;
pub const SEARCH_HIGHLIGHT: Color = ACCENT_YELLOW;

fn build_dracula_roles() -> ThemeRoles {
    ThemeRoles {
        background: BG_MAIN,
        surface: BG_PANEL,
        surface_muted: BG_PANEL_MUTED,
        border: UI_BORDER,
        divider: UI_DIVIDER,
        text: TEXT_PRIMARY,
        text_secondary: TEXT_SECONDARY,
        text_muted: TEXT_MUTED,
        accent_primary: ACCENT_PRIMARY,
        accent_secondary: ACCENT_SECONDARY,
        accent_subtle: ACCENT_SUBTLE,
        info: STATUS_INFO,
        success: STATUS_OK,
        warning: STATUS_WARN,
        error: STATUS_ERROR,
        selection_bg: SELECTION_BACKGROUND,
        selection_fg: TEXT_SELECTED,
        focus: ACCENT_SECONDARY, // Secondary accent for focus
        search_highlight: SEARCH_HIGHLIGHT,
        syntax_keyword: ACCENT_PINK,
        syntax_function: ACCENT_PURPLE,
        syntax_string: ACCENT_GREEN,
        syntax_number: ACCENT_ORANGE,
        syntax_type: ACCENT_CYAN,
        modal_bg: BG_MODAL_OVERLAY,
        scrollbar_track: UI_DIVIDER,
        scrollbar_thumb: ACCENT_SECONDARY,
        table_row_even: darken_rgb(BG_PANEL, 0.55),
        table_row_odd: darken_rgb(UI_BORDER, 0.55),
    }
}

fn build_dracula_high_contrast_roles() -> ThemeRoles {
    let mut roles = build_dracula_roles();
    roles.surface_muted = lighten_rgb(roles.surface_muted, 0.10);
    roles.border = lighten_rgb(roles.border, 0.35);
    roles.divider = lighten_rgb(roles.divider, 0.20);
    roles.text = TEXT_SELECTED;
    roles.text_secondary = TEXT_SELECTED;
    roles.text_muted = TEXT_SECONDARY;
    roles.selection_bg = lighten_rgb(SELECTION_INACTIVE_BACKGROUND, 0.10);
    roles.focus = ACCENT_PRIMARY;
    roles.scrollbar_thumb = lighten_rgb(roles.scrollbar_thumb, 0.25);
    roles.table_row_even = darken_rgb(BG_PANEL, 0.45);
    roles.table_row_odd = darken_rgb(UI_BORDER, 0.45);
    roles
}

/// Default Dracula theme tuned for dark terminals.
#[derive(Debug, Clone)]
pub struct DraculaTheme {
    roles: ThemeRoles,
}

impl DraculaTheme {
    /// Construct a Dracula theme instance using the canonical palette.
    pub fn new() -> Self {
        Self {
            roles: build_dracula_roles(),
        }
    }
}

impl Theme for DraculaTheme {
    fn roles(&self) -> &ThemeRoles {
        &self.roles
    }
}

/// High-contrast Dracula variant derived from the canonical palette.
#[derive(Debug, Clone)]
pub struct DraculaThemeHighContrast {
    roles: ThemeRoles,
}

impl DraculaThemeHighContrast {
    /// Construct the Dracula high-contrast variant by brightening text and borders.
    pub fn new() -> Self {
        Self {
            roles: build_dracula_high_contrast_roles(),
        }
    }
}

impl Theme for DraculaThemeHighContrast {
    fn roles(&self) -> &ThemeRoles {
        &self.roles
    }
}
