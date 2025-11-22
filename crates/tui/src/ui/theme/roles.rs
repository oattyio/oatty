use std::fmt::Debug;

use ratatui::style::{Color, Modifier, Style};

/// Semantic color roles used throughout the UI.
#[derive(Debug, Clone)]
pub struct ThemeRoles {
    pub background: Color,
    pub surface: Color,
    pub surface_muted: Color,
    pub border: Color,
    pub divider: Color,

    pub text: Color,
    pub text_secondary: Color,
    pub text_muted: Color,

    pub accent_primary: Color,
    pub accent_secondary: Color,
    pub accent_subtle: Color,

    pub info: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,

    pub selection_bg: Color,
    pub selection_fg: Color,
    pub focus: Color,
    /// Foreground color used to highlight search matches or other important inline hits.
    pub search_highlight: Color,
    /// Keyword/token color following the active syntax theme rules.
    pub syntax_keyword: Color,
    /// Function/method color following the active syntax theme rules.
    pub syntax_function: Color,
    /// String literal color following the active syntax theme rules.
    pub syntax_string: Color,
    /// Numeric literal color following the active syntax theme rules.
    pub syntax_number: Color,
    /// Type/identifier color following the active syntax theme rules.
    pub syntax_type: Color,

    /// Background color used when displaying modal overlays.
    ///
    /// This color should be significantly darker than the primary background so that
    /// the active modal content appears elevated while still matching the theme
    /// palette.
    pub modal_bg: Color,

    pub scrollbar_track: Color,
    pub scrollbar_thumb: Color,
    /// Background color used for even-numbered rows in tabular views.
    pub table_row_even: Color,
    /// Background color used for odd-numbered rows in tabular views.
    pub table_row_odd: Color,
}

/// Theme trait exposes semantic roles and common style builders.
pub trait Theme: Send + Sync + Debug {
    fn roles(&self) -> &ThemeRoles;

    // Text styles
    fn text_primary_style(&self) -> Style {
        Style::default().fg(self.roles().text)
    }
    fn text_secondary_style(&self) -> Style {
        Style::default().fg(self.roles().text_secondary)
    }
    fn text_muted_style(&self) -> Style {
        // Use the muted color directly without DIM to improve readability.
        Style::default().fg(self.roles().text_muted)
    }

    // Borders and focus
    fn border_style(&self, focused: bool) -> Style {
        let color = if focused { self.roles().focus } else { self.roles().border };
        Style::default().fg(color)
    }

    // Selection
    fn selection_style(&self) -> Style {
        Style::default().bg(self.roles().selection_bg)
    }

    /// Background style applied to even-numbered rows in tables.
    fn table_row_even_style(&self) -> Style {
        Style::default().bg(self.roles().table_row_even)
    }

    /// Background style applied to odd-numbered rows in tables.
    fn table_row_odd_style(&self) -> Style {
        Style::default().bg(self.roles().table_row_odd)
    }

    /// Style used for the darkened background that appears behind modal dialogs.
    fn modal_background_style(&self) -> Style {
        Style::default().bg(self.roles().modal_bg)
    }

    /// Standard highlight style for search matches (color-only per Dracula spec).
    fn search_highlight_style(&self) -> Style {
        Style::default().fg(self.roles().search_highlight)
    }

    fn syntax_keyword_style(&self) -> Style {
        Style::default().fg(self.roles().syntax_keyword)
    }
    fn syntax_function_style(&self) -> Style {
        Style::default().fg(self.roles().syntax_function)
    }
    fn syntax_string_style(&self) -> Style {
        Style::default().fg(self.roles().syntax_string)
    }
    fn syntax_number_style(&self) -> Style {
        Style::default().fg(self.roles().syntax_number)
    }
    fn syntax_type_style(&self) -> Style {
        Style::default().fg(self.roles().syntax_type)
    }

    // Status styles
    fn status_info(&self) -> Style {
        Style::default().fg(self.roles().info)
    }
    fn status_success(&self) -> Style {
        Style::default().fg(self.roles().success)
    }
    fn status_warning(&self) -> Style {
        Style::default().fg(self.roles().warning)
    }
    fn status_error(&self) -> Style {
        Style::default().fg(self.roles().error)
    }

    // Accents
    fn accent_primary_style(&self) -> Style {
        Style::default().fg(self.roles().accent_primary)
    }
    fn accent_emphasis_style(&self) -> Style {
        Style::default().fg(self.roles().accent_primary).add_modifier(Modifier::BOLD)
    }

    // Badges (chips)
    fn badge_style(&self) -> Style {
        // Use ACCENT_SUBTLE as background and TEXT_SELECTED for foreground to ensure readability
        Style::default()
            .bg(self.roles().accent_subtle)
            .fg(self.roles().selection_fg)
            .add_modifier(Modifier::BOLD)
    }
}
