//! ANSI 256-color fallback theme tailored for terminals without truecolor support.
//!
//! This palette approximates the Dracula theme using indexed colors so the UI
//! remains legible inside macOS Terminal and other 8-bit color terminals.

use ratatui::style::Color;

use super::roles::{Theme, ThemeRoles};

/// ANSI 256-color approximation of the Dracula palette.
#[derive(Debug, Clone)]
pub struct Ansi256Theme {
    roles: ThemeRoles,
}

impl Ansi256Theme {
    pub fn new() -> Self {
        Self {
            roles: ThemeRoles {
                background: Color::Indexed(236),
                surface: Color::Indexed(236),
                surface_muted: Color::Indexed(239),
                border: Color::Indexed(239),
                divider: Color::Indexed(239),

                text: Color::Indexed(255),
                text_secondary: Color::Indexed(250),
                text_muted: Color::Indexed(247),

                accent_primary: Color::Indexed(212),
                accent_secondary: Color::Indexed(117),
                accent_subtle: Color::Indexed(61),

                info: Color::Indexed(117),
                success: Color::Indexed(84),
                warning: Color::Indexed(215),
                error: Color::Indexed(203),

                selection_bg: Color::Indexed(239),
                selection_fg: Color::Indexed(255),
                focus: Color::Indexed(117),
                search_highlight: Color::Indexed(229),
                syntax_keyword: Color::Indexed(212),
                syntax_function: Color::Indexed(141),
                syntax_string: Color::Indexed(120),
                syntax_number: Color::Indexed(215),
                syntax_type: Color::Indexed(123),
                modal_bg: Color::Indexed(232),

                scrollbar_track: Color::Indexed(239),
                scrollbar_thumb: Color::Indexed(61),
                table_row_even: Color::Indexed(235),
                table_row_odd: Color::Indexed(237),
            },
        }
    }
}

impl Theme for Ansi256Theme {
    fn roles(&self) -> &ThemeRoles {
        &self.roles
    }
}

/// High-contrast variant for ANSI terminals.
#[derive(Debug, Clone)]
pub struct Ansi256ThemeHighContrast {
    roles: ThemeRoles,
}

impl Ansi256ThemeHighContrast {
    pub fn new() -> Self {
        Self {
            roles: ThemeRoles {
                background: Color::Indexed(236),
                surface: Color::Indexed(236),
                surface_muted: Color::Indexed(239),
                border: Color::Indexed(141),
                divider: Color::Indexed(239),

                text: Color::Indexed(255),
                text_secondary: Color::Indexed(117),
                text_muted: Color::Indexed(61),

                accent_primary: Color::Indexed(212),
                accent_secondary: Color::Indexed(117),
                accent_subtle: Color::Indexed(61),

                info: Color::Indexed(117),
                success: Color::Indexed(84),
                warning: Color::Indexed(215),
                error: Color::Indexed(203),

                selection_bg: Color::Indexed(239),
                selection_fg: Color::Indexed(255),
                focus: Color::Indexed(117),
                search_highlight: Color::Indexed(229),
                syntax_keyword: Color::Indexed(212),
                syntax_function: Color::Indexed(141),
                syntax_string: Color::Indexed(120),
                syntax_number: Color::Indexed(215),
                syntax_type: Color::Indexed(123),
                modal_bg: Color::Indexed(235),

                scrollbar_track: Color::Indexed(239),
                scrollbar_thumb: Color::Indexed(141),
                table_row_even: Color::Indexed(235),
                table_row_odd: Color::Indexed(237),
            },
        }
    }
}

impl Theme for Ansi256ThemeHighContrast {
    fn roles(&self) -> &ThemeRoles {
        &self.roles
    }
}
