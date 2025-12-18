use oatty_types::CommandSpec;

/// State container for the help modal.
///
/// The help modal renders potentially long paragraphs of documentation. This
/// struct tracks which command is being displayed along with the scroll
/// mechanics required to page through the rendered content.
#[derive(Debug, Clone, Default)]
pub struct HelpState {
    spec: Option<CommandSpec>,
    scroll_offset: u16,
    content_height: u16,
    viewport_height: u16,
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
        self.reset_scroll_metrics();
    }

    /// Current vertical scroll offset in terminal rows.
    pub fn scroll_offset(&self) -> u16 {
        self.scroll_offset
    }

    /// Total number of rendered lines for the current help text.
    pub fn content_height(&self) -> u16 {
        self.content_height
    }

    /// Updates the viewport height available to the modal content.
    pub fn update_viewport_height(&mut self, height: u16) {
        self.viewport_height = height;
        self.clamp_scroll();
    }

    pub fn viewport_height(&self) -> u16 {
        self.viewport_height
    }

    /// Updates the measured content height (in rows) for the current help text.
    pub fn update_content_height(&mut self, height: u16) {
        self.content_height = height;
        self.clamp_scroll();
    }

    /// Scrolls by a relative line delta (positive is down, negative is up).
    pub fn scroll_lines(&mut self, delta: i16) {
        if delta == 0 {
            return;
        }
        self.apply_scroll_delta(delta as i32);
    }

    /// Scrolls by an entire viewport height (page up/down semantics).
    pub fn scroll_pages(&mut self, delta_pages: i16) {
        if delta_pages == 0 || self.viewport_height == 0 {
            return;
        }
        let lines_per_page = self.viewport_height as i32;
        let delta = lines_per_page.saturating_mul(delta_pages as i32);
        self.apply_scroll_delta(delta);
    }

    /// Jumps to the beginning of the help content.
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    /// Jumps to the end of the help content.
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.max_scroll_offset();
    }

    /// Returns true when the rendered content exceeds the viewport height.
    pub fn is_scrollable(&self) -> bool {
        self.content_height > self.viewport_height && self.viewport_height > 0
    }

    fn apply_scroll_delta(&mut self, delta: i32) {
        if delta == 0 || !self.is_scrollable() {
            return;
        }

        let current = i32::from(self.scroll_offset);
        let max = i32::from(self.max_scroll_offset());
        let next = (current + delta).clamp(0, max);
        self.scroll_offset = next as u16;
    }

    fn reset_scroll_metrics(&mut self) {
        self.scroll_offset = 0;
        self.content_height = 0;
        self.viewport_height = 0;
    }

    fn clamp_scroll(&mut self) {
        self.scroll_offset = self.scroll_offset.min(self.max_scroll_offset());
    }

    fn max_scroll_offset(&self) -> u16 {
        self.content_height.saturating_sub(self.viewport_height)
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
