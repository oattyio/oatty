use ratatui::style::Color;

use super::roles::{Theme, ThemeRoles};

// Dracula palette (https://draculatheme.com/contribute)
// Core
pub const BG: Color = Color::Rgb(0x28, 0x2A, 0x36); // #282a36 - Background
pub const CURRENT_LINE: Color = Color::Rgb(0x44, 0x47, 0x5A); // #44475a - Current line / selection
pub const FOREGROUND: Color = Color::Rgb(0xF8, 0xF8, 0xF2); // #f8f8f2 - Foreground text
pub const COMMENT: Color = Color::Rgb(0x62, 0x72, 0xA4); // #6272a4 - Muted / comments

// Accents
pub const CYAN: Color = Color::Rgb(0x8B, 0xE9, 0xFD); // #8be9fd
pub const GREEN: Color = Color::Rgb(0x50, 0xFA, 0x7B); // #50fa7b
pub const ORANGE: Color = Color::Rgb(0xFF, 0xB8, 0x6C); // #ffb86c
pub const PINK: Color = Color::Rgb(0xFF, 0x79, 0xC6); // #ff79c6
pub const PURPLE: Color = Color::Rgb(0xBD, 0x93, 0xF9); // #bd93f9
pub const RED: Color = Color::Rgb(0xFF, 0x55, 0x55); // #ff5555
// THEME.md authoritative aliases (Dracula mapping)
pub const BG_MAIN: Color = BG; // App/root background
pub const BG_PANEL: Color = BG; // Panels share background in Dracula
pub const BG_MODAL_OVERLAY: Color = Color::Rgb(0x1D, 0x1F, 0x27); // Darkened overlay for modals
pub const UI_BORDER: Color = CURRENT_LINE; // Borders/dividers/scrollbars
pub const TEXT_MUTED: Color = COMMENT; // Ghost text/hints/placeholders

pub const TEXT_PRIMARY: Color = FOREGROUND; // Default text
pub const TEXT_SECONDARY: Color = COMMENT; // Titles/headers/labels (bold)
pub const TEXT_SELECTED: Color = FOREGROUND; // Highlighted text

// Per THEME.md: Pink = interactive primary; Cyan = focus/progress secondary
pub const ACCENT_PRIMARY: Color = PINK; // Interactive elements / prompts
pub const ACCENT_SECONDARY: Color = CYAN; // Focus, progress, keywords
pub const ACCENT_SUBTLE: Color = COMMENT; // Subtle accent

pub const STATUS_INFO: Color = CYAN;
pub const STATUS_OK: Color = GREEN;
pub const STATUS_WARN: Color = ORANGE; // warnings/modified
pub const STATUS_ERROR: Color = RED;

/// Default Dracula theme tuned for dark terminals.
#[derive(Debug, Clone)]
pub struct DraculaTheme {
    roles: ThemeRoles,
}

impl DraculaTheme {
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

                accent_primary: ACCENT_PRIMARY,
                accent_secondary: ACCENT_SECONDARY,
                accent_subtle: ACCENT_SUBTLE,

                info: STATUS_INFO,
                success: STATUS_OK,
                warning: STATUS_WARN,
                error: STATUS_ERROR,

                selection_bg: CURRENT_LINE,
                selection_fg: TEXT_SELECTED,
                focus: ACCENT_SECONDARY, // Cyan for active/focused borders
                modal_bg: BG_MODAL_OVERLAY,

                scrollbar_track: UI_BORDER,
                scrollbar_thumb: COMMENT,
            },
        }
    }
}

impl Theme for DraculaTheme {
    fn roles(&self) -> &ThemeRoles {
        &self.roles
    }
}

/// High-contrast Dracula: stronger borders and selection.
#[derive(Debug, Clone)]
pub struct DraculaThemeHighContrast {
    roles: ThemeRoles,
}

impl DraculaThemeHighContrast {
    pub fn new() -> Self {
        Self {
            roles: ThemeRoles {
                background: BG_MAIN,
                surface: BG_PANEL,
                surface_muted: UI_BORDER,
                border: PURPLE, // stronger borders for clarity
                divider: UI_BORDER,

                text: TEXT_SELECTED,
                text_secondary: TEXT_SECONDARY,
                text_muted: TEXT_MUTED,

                accent_primary: ACCENT_PRIMARY,
                accent_secondary: ACCENT_SECONDARY,
                accent_subtle: ACCENT_SUBTLE,

                info: STATUS_INFO,
                success: STATUS_OK,
                warning: STATUS_WARN,
                error: STATUS_ERROR,

                selection_bg: CURRENT_LINE,
                selection_fg: TEXT_SELECTED,
                focus: ACCENT_SECONDARY,
                modal_bg: BG_MODAL_OVERLAY,

                scrollbar_track: UI_BORDER,
                scrollbar_thumb: PURPLE,
            },
        }
    }
}

impl Theme for DraculaThemeHighContrast {
    fn roles(&self) -> &ThemeRoles {
        &self.roles
    }
}
