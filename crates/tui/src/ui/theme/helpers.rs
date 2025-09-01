use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Paragraph, Tabs},
};

use super::roles::Theme;
use crate::ui::theme::roles::ThemeRoles;

/// Build a standard Block with theme surfaces and borders.
pub fn block<'a, T: Theme + ?Sized>(theme: &'a T, title: Option<&'a str>, focused: bool) -> Block<'a> {
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(theme.border_style(focused))
        .style(panel_style(theme));
    if let Some(t) = title {
        block = block.title(Span::styled(
            t,
            theme.text_secondary_style().add_modifier(Modifier::BOLD),
        ));
    }
    block
}

/// Style for panel-like containers (set background on widget using `.style`).
pub fn panel_style<T: Theme + ?Sized>(theme: &T) -> Style {
    let ThemeRoles { surface, text, .. } = *theme.roles();
    Style::default().bg(surface).fg(text)
}

/// Style for table headers: bold snow text with subtle surface background.
pub fn table_header_style<T: Theme + ?Sized>(theme: &T) -> Style {
    // Header text: secondary + bold
    theme.text_secondary_style().add_modifier(Modifier::BOLD)
}

/// Background style for the entire header row to avoid gaps between columns.
pub fn table_header_row_style<T: Theme + ?Sized>(theme: &T) -> Style {
    Style::default()
        .bg(theme.roles().surface_muted)
        .fg(theme.roles().text_secondary)
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
    let ThemeRoles {
        surface,
        surface_muted,
        text,
        ..
    } = *theme.roles();
    // Subtly darken both surface tones for zebra striping without modifiers.
    // Keep contrast high while making alternate rows feel slightly recessed.
    let even_bg = darken_rgb(surface, 0.60);
    let odd_bg = darken_rgb(surface_muted, 0.60);
    let even = Style::default().bg(even_bg).fg(text);
    let odd = Style::default().bg(odd_bg).fg(text);
    (even, odd)
}

/// Row style for a given row index, alternating between darker
/// background/surface. This avoids using dim/other modifiers to ensure text
/// brightness is unaffected.
pub fn table_row_style<T: Theme + ?Sized>(theme: &T, row_index: usize) -> Style {
    let (even, odd) = table_row_styles(theme);
    if row_index % 2 == 0 { even } else { odd }
}

/// Style for a selected row.
pub fn table_selected_style<T: Theme + ?Sized>(theme: &T) -> Style {
    theme.selection_style().add_modifier(Modifier::BOLD)
}

/// Build tabs with active/inactive styles.
#[allow(dead_code)]
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
#[allow(dead_code)]
pub fn input_style<T: Theme + ?Sized>(theme: &T, valid: bool, focused: bool) -> Style {
    let ThemeRoles {
        surface, text, error, ..
    } = *theme.roles();
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
#[allow(dead_code)]
pub fn button_primary_style<T: Theme + ?Sized>(theme: &T, enabled: bool) -> Style {
    if enabled {
        let ThemeRoles {
            accent_primary, text, ..
        } = *theme.roles();
        Style::default()
            .bg(accent_primary)
            .fg(text)
            .add_modifier(Modifier::BOLD)
    } else {
        let ThemeRoles {
            surface_muted,
            text_muted,
            ..
        } = *theme.roles();
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
#[allow(dead_code)]
pub fn badge_style<T: Theme + ?Sized>(theme: &T) -> Style {
    let ThemeRoles { accent_secondary, .. } = theme.roles().clone();
    Style::default().bg(accent_secondary).fg(Color::Black)
}

/// Build a standard paragraph styled with primary text.
#[allow(dead_code)]
pub fn paragraph<'a, T: Theme + ?Sized>(theme: &T, text: impl Into<Span<'a>>) -> Paragraph<'a> {
    Paragraph::new(text.into()).style(theme.text_primary_style())
}
