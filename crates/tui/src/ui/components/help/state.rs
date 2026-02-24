use oatty_types::CommandSpec;

use crate::ui::components::common::ScrollMetrics;

/// State container for the help modal.
///
/// The help modal renders potentially long paragraphs of documentation. This
/// struct tracks which command is being displayed along with the scroll
/// mechanics required to page through the rendered content.
#[derive(Debug, Clone, Default)]
pub struct HelpState {
    spec: Option<CommandSpec>,
    scroll_metrics: ScrollMetrics,
}

impl HelpState {
    /// Returns the command specification currently displayed in the modal.
    pub fn spec(&self) -> Option<&CommandSpec> {
        self.spec.as_ref()
    }

    /// Sets the command specification and resets scroll positioning so the
    /// modal always opens at the top of the document.
    pub fn set_spec(&mut self, spec: Option<CommandSpec>) {
        self.spec = spec;
        self.scroll_metrics.reset();
    }

    /// Current vertical scroll offset in terminal rows.
    pub fn scroll_offset(&self) -> u16 {
        self.scroll_metrics.offset()
    }

    /// Total number of rendered lines for the current help text.
    pub fn content_height(&self) -> u16 {
        self.scroll_metrics.content_height()
    }

    /// Updates the viewport height available to the modal content.
    pub fn update_viewport_height(&mut self, height: u16) {
        self.scroll_metrics.update_viewport_height(height);
    }

    pub fn viewport_height(&self) -> u16 {
        self.scroll_metrics.viewport_height()
    }

    /// Updates the measured content height (in rows) for the current help text.
    pub fn update_content_height(&mut self, height: u16) {
        self.scroll_metrics.update_content_height(height);
    }

    /// Scrolls by a relative line delta (positive is down, negative is up).
    pub fn scroll_lines(&mut self, delta: i16) {
        self.scroll_metrics.scroll_lines(delta);
    }

    /// Scrolls by an entire viewport height (page up/down semantics).
    pub fn scroll_pages(&mut self, delta_pages: i16) {
        self.scroll_metrics.scroll_pages(delta_pages);
    }

    /// Jumps to the beginning of the help content.
    pub fn scroll_to_top(&mut self) {
        self.scroll_metrics.scroll_to_top();
    }

    /// Jumps to the end of the help content.
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_metrics.scroll_to_bottom();
    }

    /// Returns true when the rendered content exceeds the viewport height.
    pub fn is_scrollable(&self) -> bool {
        self.scroll_metrics.is_scrollable()
    }
}

#[cfg(test)]
mod tests {
    use super::HelpState;

    #[test]
    fn scroll_offsets_clamp_to_bounds() {
        let mut state = HelpState::default();
        state.update_viewport_height(5);
        state.update_content_height(20);

        state.scroll_lines(3);
        assert_eq!(state.scroll_offset(), 3);

        state.scroll_lines(-10);
        assert_eq!(state.scroll_offset(), 0);

        state.scroll_to_bottom();
        assert_eq!(state.scroll_offset(), 15);

        state.scroll_lines(10);
        assert_eq!(state.scroll_offset(), 15);
    }

    #[test]
    fn page_scroll_uses_viewport_height() {
        let mut state = HelpState::default();
        state.update_viewport_height(4);
        state.update_content_height(40);

        state.scroll_pages(1);
        assert_eq!(state.scroll_offset(), 4);

        state.scroll_pages(2);
        assert_eq!(state.scroll_offset(), 12);

        state.scroll_pages(-1);
        assert_eq!(state.scroll_offset(), 8);
    }
}
