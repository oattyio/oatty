use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::{text::Span, widgets::Tabs};

use crate::ui::theme::roles::ThemeRoles;

use super::roles::Theme;

/// Build a standard Block with Nord surfaces and borders.
pub fn block<'a, T: Theme + ?Sized>(theme: &'a T, title: Option<&'a str>, focused: bool) -> Block<'a> {
    let mut b = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(theme.border_style(focused));
    if let Some(t) = title {
        b = b.title(Span::styled(
            t,
            theme.text_secondary_style().add_modifier(Modifier::BOLD),
        ));
    }
    b
}

/// Style for panel-like containers (set background on widget using `.style`).
pub fn panel_style<T: Theme + ?Sized>(theme: &T) -> Style {
    let ThemeRoles { surface, text, .. } = *theme.roles();
    Style::default().bg(surface).fg(text)
}

/// Style for table headers: bold snow text with subtle surface background.
pub fn table_header_style<T: Theme + ?Sized>(theme: &T) -> Style {
    theme
        .text_secondary_style()
        .add_modifier(Modifier::BOLD)
}

/// Darken an RGB color by a multiplicative factor (0.0..=1.0).
/// If the color is not RGB, returns it unchanged.
fn darken_rgb(color: Color, factor: f32) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            let f = factor.clamp(0.0, 1.0);
            let dr = (r as f32 * f).round().clamp(0.0, 255.0) as u8;
            let dg = (g as f32 * f).round().clamp(0.0, 255.0) as u8;
            let db = (b as f32 * f).round().clamp(0.0, 255.0) as u8;
            Color::Rgb(dr, dg, db)
        }
        other => other,
    }
}

/// Returns alternating row styles for zebra striping (even/odd),
/// using slightly darker variants of the background/surface.
pub fn table_row_styles<T: Theme + ?Sized>(theme: &T) -> (Style, Style) {
    let ThemeRoles { background, surface, text, .. } = *theme.roles();
    // Use a subtle darkening so text contrast remains strong without dimming text.
    let darker_bg = darken_rgb(background, 0.55);
    let darker_surface = darken_rgb(surface, 0.55);
    let even = Style::default().bg(darker_bg).fg(text);
    let odd = Style::default().bg(darker_surface).fg(text);
    (even, odd)
}

/// Row style for a given row index, alternating between darker background/surface.
/// This avoids using dim/other modifiers to ensure text brightness is unaffected.
pub fn table_row_style<T: Theme + ?Sized>(theme: &T, row_index: usize) -> Style {
    let (even, odd) = table_row_styles(theme);
    if row_index % 2 == 0 { even } else { odd }
}

/// Style for a selected row.
pub fn table_selected_style<T: Theme + ?Sized>(theme: &T) -> Style {
    theme.selection_style().add_modifier(Modifier::BOLD)
}

/// Build tabs with active/inactive styles.
pub fn tabs<'a, T: Theme + ?Sized>(theme: &T, titles: Vec<Span<'a>>, index: usize) -> Tabs<'a> {
    Tabs::new(titles)
        .select(index)
        .highlight_style(
            theme
                .text_primary_style()
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
        )
        .style(theme.text_secondary_style())
}

/// Style for input fields; caller sets the block border based on focus.
pub fn input_style<T: Theme + ?Sized>(theme: &T, valid: bool, focused: bool) -> Style {
    let ThemeRoles { surface, text, error, .. } = *theme.roles();
    let mut style = Style::default().bg(surface).fg(text);
    if !valid {
        style = style.fg(error);
    }
    if focused {
        style = style.add_modifier(Modifier::BOLD);
    }
    style
}

/// Primary button style (filled accent background).
pub fn button_primary_style<T: Theme + ?Sized>(theme: &T, enabled: bool) -> Style {
    if enabled {
        let ThemeRoles { accent_primary, text, .. } = *theme.roles();
        Style::default()
            .bg(accent_primary)
            .fg(text)
            .add_modifier(Modifier::BOLD)
    } else {
        let ThemeRoles { surface_muted, text_muted, .. } = *theme.roles();
        Style::default().bg(surface_muted).fg(text_muted)
    }
}

/// Secondary button style (outline-like, rely on border color in Block).
pub fn button_secondary_style<T: Theme + ?Sized>(theme: &T, enabled: bool) -> Style {
    if enabled {
        let ThemeRoles { accent_secondary, .. } = theme.roles().clone();
        Style::default().fg(accent_secondary)
    } else {
        theme.text_muted_style()
    }
}

/// Badge/tag style (filled accent, readable text).
pub fn badge_style<T: Theme + ?Sized>(theme: &T) -> Style {
    let ThemeRoles { accent_secondary, .. } = theme.roles().clone();
    Style::default().bg(accent_secondary).fg(Color::Black)
}

/// Build a standard paragraph styled with primary text.
pub fn paragraph<'a, T: Theme + ?Sized>(theme: &T, text: impl Into<Span<'a>>) -> Paragraph<'a> {
    Paragraph::new(text.into()).style(theme.text_primary_style())
}
