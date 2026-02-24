//! Shared scrollbar rendering helpers.
//!
//! This module centralizes themed scrollbar rendering so components can reuse
//! the same visuals while preserving their local scroll-state math.

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::ui::theme::roles::Theme;

/// Renders a themed vertical scrollbar on the right side of the given area.
///
/// The caller provides the already-computed scroll state values because
/// different views use slightly different content-length semantics.
pub fn render_vertical_scrollbar(
    frame: &mut Frame,
    area: Rect,
    theme: &dyn Theme,
    state_length: usize,
    position: usize,
    viewport_content_length: usize,
) {
    if viewport_content_length == 0 || state_length == 0 {
        return;
    }
    let mut scrollbar_state = ScrollbarState::new(state_length)
        .position(position)
        .viewport_content_length(viewport_content_length);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .thumb_style(Style::default().fg(theme.roles().scrollbar_thumb))
        .track_style(Style::default().fg(theme.roles().scrollbar_track));
    frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
}
