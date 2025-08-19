use ratatui::style::{Color, Modifier, Style};

// Conservative dark theme, preferring Heroku Cloud Blue accents.
// Cloud Blue approximation: #00A3E0 with a dim variant.
pub const ACCENT: Color = Color::Rgb(0, 163, 224);
pub const FG: Color = Color::Rgb(224, 224, 230);
pub const FG_MUTED: Color = Color::Rgb(168, 168, 175);
pub const BORDER: Color = Color::Rgb(72, 72, 80);
pub const BORDER_FOCUS: Color = ACCENT;
pub const BG_PANEL: Color = Color::Rgb(18, 18, 24);
pub const BG_HIGHLIGHT: Color = Color::Rgb(20, 32, 44); // subtle blue-tinted dark for general highlight
pub const BG_SELECT: Color = Color::Rgb(18, 28, 38); // even subtler for list selection
pub const WARN: Color = Color::Rgb(220, 96, 110);

pub fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(BORDER_FOCUS)
    } else {
        Style::default().fg(BORDER)
    }
}

pub fn title_style() -> Style {
    Style::default().fg(FG_MUTED).add_modifier(Modifier::BOLD)
}

pub fn text_style() -> Style {
    Style::default().fg(FG)
}
pub fn text_muted() -> Style {
    Style::default().fg(FG_MUTED)
}
pub fn highlight_style() -> Style {
    Style::default().fg(FG).bg(BG_HIGHLIGHT)
}
pub fn list_highlight_style() -> Style {
    Style::default().fg(FG_MUTED).bg(BG_SELECT)
}
