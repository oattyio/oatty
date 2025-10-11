use super::roles::Theme;
use crate::ui::theme::roles::ThemeRoles;
use ratatui::text::Line;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Tabs},
};

/// Build a standard Block with theme surfaces and borders.
pub fn block<'a>(theme: &dyn Theme, title: Option<&'a str>, focused: bool) -> Block<'a> {
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(theme.border_style(focused))
        .style(panel_style(theme));
    if let Some(t) = title {
        block = block.title(Span::styled(t, theme.text_secondary_style().add_modifier(Modifier::BOLD)));
    }
    block
}

/// Style for panel-like containers (set background on widget using `.style`).
pub fn panel_style(theme: &dyn Theme) -> Style {
    let ThemeRoles { surface, text, .. } = *theme.roles();
    Style::default().bg(surface).fg(text)
}

/// Style for table headers: bold snow text with subtle surface background.
pub fn table_header_style(theme: &dyn Theme) -> Style {
    // Header text: secondary + bold
    theme.text_secondary_style().add_modifier(Modifier::BOLD)
}

/// Background style for the entire header row to avoid gaps between columns.
pub fn table_header_row_style(theme: &dyn Theme) -> Style {
    Style::default().bg(theme.roles().surface_muted).fg(theme.roles().text_secondary)
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
pub fn table_row_styles(theme: &dyn Theme) -> (Style, Style) {
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
pub fn table_row_style(theme: &dyn Theme, row_index: usize) -> Style {
    let (even, odd) = table_row_styles(theme);
    if row_index % 2 == 0 { even } else { odd }
}

/// Style for a selected row.
pub fn table_selected_style(theme: &dyn Theme) -> Style {
    theme.selection_style().add_modifier(Modifier::BOLD)
}

/// Build tabs with active/inactive styles.
#[allow(dead_code)]
pub fn tabs<'a>(theme: &dyn Theme, titles: Vec<Span<'a>>, index: usize) -> Tabs<'a> {
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
pub fn input_style(theme: &dyn Theme, valid: bool, focused: bool) -> Style {
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
#[allow(dead_code)]
pub fn button_primary_style(theme: &dyn Theme, enabled: bool) -> Style {
    if enabled {
        let ThemeRoles { accent_primary, text, .. } = *theme.roles();
        Style::default().bg(accent_primary).fg(text).add_modifier(Modifier::BOLD)
    } else {
        let ThemeRoles {
            surface_muted, text_muted, ..
        } = *theme.roles();
        Style::default().bg(surface_muted).fg(text_muted)
    }
}

/// Secondary button style (outline-like, rely on border color in Block).
pub fn button_secondary_style(theme: &dyn Theme, enabled: bool, selected: bool) -> Style {
    if enabled {
        let ThemeRoles {
            accent_secondary,
            selection_bg,
            ..
        } = theme.roles().clone();
        let style = Style::default().fg(accent_secondary);
        if selected {
            return style.bg(selection_bg);
        }
        return style;
    } else {
        theme.text_muted_style()
    }
}

/// Badge/tag style (filled accent, readable text).
#[allow(dead_code)]
pub fn badge_style(theme: &dyn Theme) -> Style {
    let ThemeRoles { accent_secondary, .. } = theme.roles().clone();
    Style::default().bg(accent_secondary).fg(Color::Black)
}

/// Build a standard paragraph styled with the primary text.
#[allow(dead_code)]
pub fn paragraph<'a>(theme: &dyn Theme, text: impl Into<Span<'a>>) -> Paragraph<'a> {
    Paragraph::new(text.into()).style(theme.text_primary_style())
}

/// Renders a button widget within a given area on the terminal frame.
///
/// # Parameters
/// - `frame`: A mutable reference to the terminal `Frame` where the button will be rendered.
/// - `area`: A `Rect` specifying the area of the terminal where the button will be drawn.
/// - `label`: A string slice (`&str`) that represents the label or text to be displayed on the button.
/// - `is_enabled`: A boolean flag indicating whether the button is enabled. Disabled buttons are styled with a muted appearance.
/// - `is_focused`: A boolean flag indicating whether the button is currently focused.
/// - `is_selected`: A boolean flag indicating whether the button is selected. May modify its visual appearance if true.
/// - `theme`: A reference to an object implementing the `Theme` trait, used to retrieve styles for borders, text, and other visual elements based on button states.
/// - `borders`: A `Borders` parameter specifying which sides of the button (if any) should have a border drawn.
///
/// # Behavior
/// This function renders a button with the following visual traits and rules:
/// - The border style is determined by whether the button is enabled (`border_style`) and focused.
/// - The button's main style (e.g., text color and background) is defined based on its enabled state and selection state.
/// - Padding within the button is added only if no borders are present, ensuring consistent dimensions regardless of whether borders are drawn.
/// - The label is centrally aligned and displayed inside the button's area.
///
/// # Styling
/// - If `is_enabled` is `false`, the button and its border use a "muted" style provided by the `theme`.
/// - If `is_enabled` is `true`, the border style depends on whether the button is focused, and the button itself may use a secondary style, optionally modified by the `is_selected` state.
///
/// # Examples
/// ```
/// use tui::widgets::Borders;
///
/// // Assuming `frame`, `theme`, and `area` are already available:
/// render_button(
///     &mut frame,
///     area,
///     "Click Me",
///     true,           // Button is enabled
///     true,           // Button is focused
///     false,          // Button is not selected
///     &theme,
///     Borders::ALL,   // Draw a border on all sides of the button
/// );
/// ```
pub fn render_button(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    is_enabled: bool,
    is_focused: bool,
    is_selected: bool,
    theme: &dyn Theme,
    borders: Borders,
) {
    let border_style = if is_enabled {
        theme.border_style(is_focused)
    } else {
        theme.text_muted_style()
    };

    let button_style = if is_enabled {
        button_secondary_style(theme, true, is_selected)
    } else {
        theme.text_muted_style()
    };

    let padding = if borders.is_empty() {
        Padding::uniform(1) // Add padding when no borders to match bordered button size
    } else {
        Padding::uniform(0) // No padding when borders are present
    };

    frame.render_widget(
        Paragraph::new(label)
            .centered()
            .block(Block::bordered().borders(borders).border_style(border_style).padding(padding))
            .style(button_style),
        area,
    );
}

/// Generates a styled radio button text representation.
///
/// This function creates a radio button-like structure in a textual format.
/// It displays a label along with a selection indicator (`[✓]` for selected,
/// `[ ]` for unselected), with styles applied based on the selection and focus states,
/// as well as the provided theme.
///
/// # Parameters
///
/// - `label`: A string slice that represents the label of the radio button.
/// - `is_selected`: A boolean that determines whether the radio button is selected.
///   - `true`: The radio button appears selected (`[✓]`).
///   - `false`: The radio button appears unselected (`[ ]`).
/// - `is_focused`: A boolean that indicates if the radio button is currently focused.
///   - If `true`, the focus-specific style from the theme is applied to the line.
///   - If `false`, the normal style from the theme is used.
/// - `theme`: A reference to an object implementing the `Theme` trait. The theme
///   provides styling options for the radio button, including text color
///   and selection styles.
///
/// # Returns
///
/// - A `Line` object, which is a styled line of text representing the radio button.
///   It includes the selection marker, spacing, and the styled label, with appropriate
///   styles applied based on the inputs.
///
/// # Example
///
/// ```rust
/// let theme = MyCustomTheme::new();
/// let radio_button = create_radio_button("Option 1", true, false, &theme);
/// println!("{:?}", radio_button);
/// ```
///
/// In this example, calling `create_radio_button` with `"Option 1"` as the label, `true`
/// for `is_selected`, and `false` for `is_focused` will generate a styled `[✓] Option 1`
/// text representation based on the styling provided by `MyCustomTheme`.
///
/// # Notes
///
/// - The `Theme` trait must provide implementations for the following methods:
///   - `status_success()`: Returns the style for a selected state.
///   - `text_primary_style()`: Returns the default text style.
///   - `selection_style()`: Returns the style for a focused state.
///
/// - Ensure the lifetime of the returned `Line` (`'static`) is compatible with
///   the downstream usage.
pub fn create_radio_button(label: &str, is_selected: bool, is_focused: bool, theme: &dyn Theme) -> Line<'static> {
    let mut radio_spans = Vec::new();
    radio_spans.push(Span::styled(
        if is_selected { "[✓]" } else { "[ ]" },
        if is_selected {
            theme.status_success()
        } else {
            theme.text_primary_style()
        },
    ));
    radio_spans.push(Span::raw(" "));
    radio_spans.push(Span::styled(label.to_string(), theme.text_primary_style()));

    if is_focused {
        Line::from(radio_spans).style(theme.selection_style())
    } else {
        Line::from(radio_spans).style(theme.text_primary_style())
    }
}
