use super::roles::Theme;
use crate::ui::theme::roles::ThemeRoles;
use oatty_types::MessageType;
use ratatui::text::Line;
use ratatui::widgets::{HighlightSpacing, List, ListItem};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Tabs},
};

use once_cell::sync::Lazy;
use regex::Regex;
use std::borrow::Cow;

/// Applies syntax highlighting to log lines for better readability.
///
/// This method identifies and styles different parts of log entries:
///
/// - **Timestamp**: Styled with secondary accent color
/// - **UUIDs**: Styled with emphasis color for easy identification
/// - **Hex IDs**: Styled with emphasis color for long hexadecimal strings
/// - **Regular text**: Uses primary text color
///
/// # Arguments
///
/// * `theme` - The UI theme providing color schemes
/// * `line` - The log line text to style
///
/// # Returns
///
/// A styled `Line` with the appropriate color coding
pub fn styled_line(theme: &dyn Theme, line: &str) -> Line<'static> {
    // Compiled regex patterns for performance
    static TS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\[?\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2}(?:\.\d+)?Z?]?").unwrap());
    static UUID_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-5][0-9a-fA-F]{3}-[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}\b").unwrap());
    static HEXID_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b[0-9a-fA-F]{12,}\b").unwrap());

    let mut spans: Vec<Span> = Vec::new();
    let mut i = 0usize;

    // Style timestamp at the beginning of the line
    if let Some(m) = TS_RE.find(line)
        && m.start() == 0
        && m.end() > 0
    {
        spans.push(Span::styled(
            line[m.start()..m.end()].to_string(),
            Style::default().fg(theme.roles().accent_secondary),
        ));
        i = m.end();
    }

    // Style remaining text with UUID/hex ID highlighting
    let rest = &line[i..];
    let mut matches: Vec<_> = UUID_RE.find_iter(rest).chain(HEXID_RE.find_iter(rest)).collect();
    matches.sort_by_key(|m| m.start());

    let mut last = 0usize;
    for m in matches {
        if m.start() < last {
            continue; // Skip overlapping matches
        }
        // Add text before the match
        if m.start() > last {
            spans.push(Span::styled(rest[last..m.start()].to_string(), theme.text_primary_style()));
        }
        // Style the UUID/hex ID
        spans.push(Span::styled(rest[m.start()..m.end()].to_string(), theme.accent_emphasis_style()));
        last = m.end();
    }

    // Add remaining text
    if last < rest.len() {
        spans.push(Span::styled(rest[last..].to_string(), theme.text_primary_style()));
    }

    Line::from(spans)
}
/// Build a standard Block with theme surfaces and borders.
pub fn block<'a, T>(theme: &dyn Theme, title: Option<T>, focused: bool) -> Block<'a>
where
    T: Into<Cow<'a, str>>,
{
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

pub fn block_with_severity<'a>(theme: &dyn Theme, severity: MessageType, title: Option<&str>, focused: bool) -> Block<'a> {
    let style = match severity {
        MessageType::Info => theme.status_info(),
        MessageType::Warning => theme.status_warning(),
        MessageType::Error => theme.status_error(),
        MessageType::Success => theme.status_success(),
    };
    let mut title_str = format!(" {}", severity);
    if let Some(t) = title {
        title_str.push_str(&format!(": {}", t));
    }
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(theme.border_style(focused))
        .style(panel_style(theme))
        .title(Span::styled(title_str, style.add_modifier(Modifier::BOLD)))
}

/// Style for panel-like containers (set background on widget using `.style`).
pub fn panel_style(theme: &dyn Theme) -> Style {
    let ThemeRoles { surface, text, .. } = *theme.roles();
    Style::default().bg(surface).fg(text)
}

/// Style for results headers: bold snow text with a subtle surface background.
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
pub(crate) fn darken_rgb(color: Color, factor: f32) -> Color {
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

/// Lighten an RGB color by blending it toward white according to `factor` (0.0..=1.0).
/// Returns the original color unchanged when the color is not RGB.
pub(crate) fn lighten_rgb(color: Color, factor: f32) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            let f = factor.clamp(0.0, 1.0);
            let lr = (r as f32 + (255.0 - r as f32) * f).round().clamp(0.0, 255.0) as u8;
            let lg = (g as f32 + (255.0 - g as f32) * f).round().clamp(0.0, 255.0) as u8;
            let lb = (b as f32 + (255.0 - b as f32) * f).round().clamp(0.0, 255.0) as u8;
            Color::Rgb(lr, lg, lb)
        }
        other => other,
    }
}

/// Returns alternating row styles for zebra striping (even/odd),
/// using slightly darker variants of the background/surface.
pub fn table_row_styles(theme: &dyn Theme) -> (Style, Style) {
    let text = theme.roles().text;
    let even = theme.table_row_even_style().fg(text);
    let odd = theme.table_row_odd_style().fg(text);
    (even, odd)
}

/// Row style for a given row index, alternating between darker
/// background/surface. This avoids using dim/other modifiers to ensure text
/// brightness is unaffected.
pub fn table_row_style(theme: &dyn Theme, row_index: usize) -> Style {
    let (even, odd) = table_row_styles(theme);
    if row_index.is_multiple_of(2) { even } else { odd }
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

/// Primary button style (filled accent background).
pub fn button_primary_style(theme: &dyn Theme, enabled: bool, selected: bool) -> Style {
    if enabled {
        let ThemeRoles { accent_primary, text, .. } = *theme.roles();
        let style = Style::default().fg(text).add_modifier(Modifier::BOLD);
        if selected { style.bg(accent_primary) } else { style }
    } else {
        theme.text_muted_style()
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
        if selected { style.bg(selection_bg) } else { style }
    } else {
        theme.text_muted_style()
    }
}

/// Destructive button style (error accent with readable text).
pub fn button_destructive_style(theme: &dyn Theme, enabled: bool, selected: bool) -> Style {
    if enabled {
        let ThemeRoles { error, selection_fg, .. } = *theme.roles();
        let style = Style::default().fg(error).add_modifier(Modifier::BOLD);
        if selected { style.bg(error).fg(selection_fg) } else { style }
    } else {
        theme.text_muted_style()
    }
}

/// Badge/tag style (filled accent, readable text).
#[allow(dead_code)]
pub fn badge_style(theme: &dyn Theme) -> Style {
    // Delegate to Theme’s default badge style so all themes are consistent
    theme.badge_style()
}

#[derive(Default, Debug, Clone, Copy)]
pub enum ButtonType {
    #[default]
    /// Primary button style (filled accent, readable text).
    Primary,
    /// Secondary button style (outlined accent, readable text).
    Secondary,
    /// Destructive button style (filled red, white text).
    Destructive,
}
/// Immutable configuration specifying how a button should be rendered.
#[derive(Default, Debug, Clone, Copy)]
pub struct ButtonRenderOptions {
    /// Whether the button is interactable and should display active styling.
    pub enabled: bool,
    /// Indicates whether the button currently owns focus.
    pub focused: bool,
    /// Highlights the button as the selected option.
    pub selected: bool,
    /// Border configuration to apply when drawing the button.
    pub borders: Borders,
    /// The button category (primary, secondary, or destructive) that controls styling.
    pub button_type: ButtonType,
}

impl ButtonRenderOptions {
    /// Construct a new `ButtonRenderOptions` using positional arguments.
    pub const fn new(enabled: bool, focused: bool, selected: bool, borders: Borders, button_type: ButtonType) -> Self {
        Self {
            enabled,
            focused,
            selected,
            borders,
            button_type,
        }
    }
}

/// Renders a button widget within a given area on the terminal frame.
///
/// # Parameters
/// - `frame`: A mutable reference to the terminal `Frame` where the button will be rendered.
/// - `area`: A `Rect` specifying the area of the terminal where the button will be drawn.
/// - `label`: A string slice (`&str`) that represents the label or text to be displayed on the button.
/// - `theme`: A reference to an object implementing the `Theme` trait, used to retrieve styles for borders, text, and other visual elements based on button states.
/// - `options`: Aggregated button state describing enabled, focused, selected, and border configuration.
///
/// # Behavior
/// This function renders a button with the following visual traits and rules:
/// - The border style is determined by whether the button is enabled and focused.
/// - The button's main style (e.g., text color and background) is defined based on its enabled state and selection state.
/// - Padding within the button is added only if no borders are present, ensuring consistent dimensions regardless of whether borders are drawn.
/// - The label is centrally aligned and displayed inside the button's area.
///
/// # Styling
/// - Disabled buttons use a muted style provided by the `theme`.
/// - Enabled buttons rely on secondary styling, optionally accented when marked as selected.
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
///     &theme,
///     ButtonRenderOptions::new(true, true, false, Borders::ALL, ButtonType::Primary),
/// );
/// ```
pub fn render_button(frame: &mut Frame, area: Rect, label: &str, theme: &dyn Theme, options: ButtonRenderOptions) {
    let border_style = if options.enabled {
        theme.border_style(options.focused)
    } else {
        theme.text_muted_style()
    };

    let button_style = match options.button_type {
        ButtonType::Primary => button_primary_style(theme, options.enabled, options.selected),
        ButtonType::Secondary => button_secondary_style(theme, options.enabled, options.selected),
        ButtonType::Destructive => button_destructive_style(theme, options.enabled, options.selected),
    };

    let padding = if options.borders.is_empty() {
        Padding::uniform(1) // Add padding when no borders to match the bordered button size
    } else {
        Padding::uniform(0) // No padding when borders are present
    };

    frame.render_widget(
        Paragraph::new(label)
            .centered()
            .block(
                Block::bordered()
                    .borders(options.borders)
                    .border_style(border_style)
                    .padding(padding),
            )
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
pub fn create_radio_button(label: Option<&str>, is_checked: bool, is_focused: bool, theme: &dyn Theme) -> Line<'static> {
    let mut radio_spans = Vec::with_capacity(5);
    let mut lb = Span::raw("(");
    let mut rb = Span::raw(")");
    let mut icon = Span::styled(if is_checked { "●" } else { " " }, theme.text_primary_style());
    if is_focused {
        let focused_style = theme.accent_emphasis_style();
        lb = lb.patch_style(focused_style);
        rb = rb.patch_style(focused_style);
        icon = icon.patch_style(focused_style);
    }

    radio_spans.push(lb);
    radio_spans.push(icon);
    radio_spans.push(rb);

    if let Some(l) = label {
        radio_spans.push(Span::raw(" "));
        radio_spans.push(Span::styled(l.to_string(), theme.text_primary_style()));
    }

    Line::from(radio_spans)
}

/// Create a checkbox with a label.
///
/// - `label`: The text to display next to the checkbox.
/// - `is_checked`: Whether the checkbox is checked.
/// - `is_focused`: Whether the checkbox is focused.
/// - `theme`: The theme to use for styling.
///
/// Returns a `Line` containing the checkbox and label.
pub fn create_checkbox(label: Option<&str>, is_checked: bool, is_focused: bool, theme: &dyn Theme) -> Line<'static> {
    let mut checkbox_spans = Vec::with_capacity(5);
    let mut lb = Span::raw("[");
    let mut rb = Span::raw("]");
    if is_focused {
        lb = lb.patch_style(theme.accent_emphasis_style());
        rb = rb.patch_style(theme.accent_emphasis_style());
    }
    let icon = Span::styled(if is_checked { "✓" } else { " " }, theme.text_primary_style());

    checkbox_spans.push(lb);
    checkbox_spans.push(icon);
    checkbox_spans.push(rb);

    if let Some(l) = label {
        checkbox_spans.push(Span::raw(" "));
        checkbox_spans.push(Span::styled(l.to_string(), theme.text_primary_style()));
    }

    Line::from(checkbox_spans)
}

/// Render a single-line labeled input field with optional placeholder text.
pub fn create_labeled_input_field(
    theme: &dyn Theme,
    label: &str,
    value: Option<&str>,
    placeholder: &str,
    focused: bool,
) -> Paragraph<'static> {
    let line = build_syntax_highlighted_line(label, value, placeholder, focused, theme);
    let paragraph_style = if focused { theme.selection_style() } else { Style::default() };
    Paragraph::new(line).style(paragraph_style)
}

pub fn highlight_segments(needle: &str, text: &str, base: Style, highlight: Style) -> Vec<Span<'static>> {
    if text.is_empty() {
        return Vec::new();
    }
    if needle.is_empty() {
        vec![Span::styled(text.to_string(), base)]
    } else {
        create_spans_with_match(needle.to_string(), text.to_string(), base, highlight)
    }
}

pub fn create_spans_with_match(needle: String, display: String, default_style: Style, emphasis_style: Style) -> Vec<Span<'static>> {
    if needle.is_empty() {
        return vec![Span::styled(display, default_style)];
    }

    let mut spans: Vec<Span> = Vec::new();
    let hay = display.as_str();
    let mut i = 0usize;
    let needle_lower = needle.to_ascii_lowercase();
    let hay_lower = hay.to_ascii_lowercase();

    while let Some(pos) = hay_lower[i..].find(&needle_lower) {
        let start = i + pos;

        if start > i {
            spans.push(Span::styled(hay[i..start].to_string(), default_style));
        }

        let end = start + needle.len();
        spans.push(Span::styled(hay[start..end].to_string(), emphasis_style));

        i = end;
        if i >= hay.len() {
            break;
        }
    }

    if i < hay.len() {
        spans.push(Span::styled(hay[i..].to_string(), default_style));
    }

    spans
}

/// Creates a styled list with a highlight indicator.
pub fn create_list_with_highlight<'a>(
    list_items: Vec<ListItem<'a>>,
    theme: &dyn Theme,
    is_focused: bool,
    maybe_block: Option<Block<'a>>,
) -> List<'a> {
    let mut list = List::new(list_items)
        .highlight_style(theme.selection_style().add_modifier(Modifier::BOLD))
        .style(panel_style(theme))
        .highlight_symbol(if is_focused { "▸ " } else { "  " })
        .highlight_spacing(HighlightSpacing::Always);

    if let Some(block) = maybe_block {
        list = list.block(block);
    }

    list
}

pub fn build_hint_spans(theme: &dyn Theme, hints: &[(&str, &str)]) -> Vec<Span<'static>> {
    let accent = theme.accent_emphasis_style();
    let muted = theme.text_muted_style();
    let mut spans = Vec::with_capacity(hints.len() * 2);
    for (key, description) in hints {
        spans.push(Span::styled((*key).to_string(), accent));
        spans.push(Span::styled((*description).to_string(), muted));
    }
    spans
}
/// Build a [`Line`] showing a focus indicator, label, and string value using syntax colors.
pub(crate) fn build_syntax_highlighted_line(
    label: &str,
    value: Option<&str>,
    placeholder: &str,
    focused: bool,
    theme: &dyn Theme,
) -> Line<'static> {
    let spans: Vec<Span> = vec![
        build_focus_indicator_span(focused, theme),
        build_label_span(label, theme),
        build_value_span(value, placeholder, theme),
    ];
    Line::from(spans)
}

/// Build the arrow focus indicator used ahead of inline fields.
pub(crate) fn build_focus_indicator_span(focused: bool, theme: &dyn Theme) -> Span<'static> {
    let indicator = if focused { "› " } else { "  " };
    Span::styled(indicator.to_string(), theme.text_secondary_style())
}

/// Build the field label span using the syntax keyword foreground color.
pub(crate) fn build_label_span(label: &str, theme: &dyn Theme) -> Span<'static> {
    Span::styled(format!("{label}: "), theme.syntax_keyword_style())
}

/// Build the field value span, falling back to a placeholder if empty.
pub(crate) fn build_value_span(maybe_value: Option<&str>, placeholder: &str, theme: &dyn Theme) -> Span<'static> {
    if let Some(value) = maybe_value
        && !value.is_empty()
    {
        Span::styled(value.to_string(), theme.syntax_string_style())
    } else {
        let placeholder_style = theme.syntax_string_style().patch(theme.text_muted_style());
        Span::styled(placeholder.to_string(), placeholder_style)
    }
}
