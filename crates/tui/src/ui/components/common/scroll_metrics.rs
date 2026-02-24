//! Shared scrolling metrics for vertically scrollable text and detail panes.
//!
//! This module centralizes the common scroll behavior used across modal and
//! pane components. It tracks content height, viewport height, and current
//! scroll offset while providing bounded line/page navigation helpers.

/// Shared metrics for vertical scrolling.
///
/// The metrics use terminal row units (`u16`) so they can be applied directly
/// to ratatui paragraph scrolling and scrollbar calculations.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScrollMetrics {
    offset: u16,
    content_height: u16,
    viewport_height: u16,
}

impl ScrollMetrics {
    /// Returns current vertical scroll offset.
    pub const fn offset(&self) -> u16 {
        self.offset
    }

    /// Returns measured content height.
    pub const fn content_height(&self) -> u16 {
        self.content_height
    }

    /// Returns measured viewport height.
    pub const fn viewport_height(&self) -> u16 {
        self.viewport_height
    }

    /// Returns the maximum valid scroll offset.
    pub fn max_offset(&self) -> u16 {
        self.content_height.saturating_sub(self.viewport_height)
    }

    /// Returns whether content exceeds the current viewport.
    pub fn is_scrollable(&self) -> bool {
        self.content_height > self.viewport_height && self.viewport_height > 0
    }

    /// Resets offset and dimensions.
    pub fn reset(&mut self) {
        self.offset = 0;
        self.content_height = 0;
        self.viewport_height = 0;
    }

    /// Updates viewport height and clamps current offset.
    pub fn update_viewport_height(&mut self, viewport_height: u16) {
        self.viewport_height = viewport_height;
        self.clamp_offset();
    }

    /// Updates content height and clamps current offset.
    pub fn update_content_height(&mut self, content_height: u16) {
        self.content_height = content_height;
        self.clamp_offset();
    }

    /// Scrolls by relative line count (`+` down, `-` up).
    pub fn scroll_lines(&mut self, delta: i16) {
        if delta == 0 || !self.is_scrollable() {
            return;
        }
        let current = i32::from(self.offset);
        let max = i32::from(self.max_offset());
        let next = (current + i32::from(delta)).clamp(0, max);
        self.offset = next as u16;
    }

    /// Scrolls by viewport page increments.
    pub fn scroll_pages(&mut self, delta_pages: i16) {
        if delta_pages == 0 || self.viewport_height == 0 {
            return;
        }
        let delta = i32::from(self.viewport_height).saturating_mul(i32::from(delta_pages));
        self.scroll_lines(delta as i16);
    }

    /// Moves scroll position to the first row.
    pub fn scroll_to_top(&mut self) {
        self.offset = 0;
    }

    /// Moves scroll position to the last visible window.
    pub fn scroll_to_bottom(&mut self) {
        self.offset = self.max_offset();
    }

    fn clamp_offset(&mut self) {
        self.offset = self.offset.min(self.max_offset());
    }
}

#[cfg(test)]
mod tests {
    use super::ScrollMetrics;

    #[test]
    fn scrolling_clamps_to_bounds() {
        let mut metrics = ScrollMetrics::default();
        metrics.update_viewport_height(5);
        metrics.update_content_height(20);

        metrics.scroll_lines(3);
        assert_eq!(metrics.offset(), 3);

        metrics.scroll_lines(-10);
        assert_eq!(metrics.offset(), 0);

        metrics.scroll_to_bottom();
        assert_eq!(metrics.offset(), 15);
    }

    #[test]
    fn page_scrolling_uses_viewport_height() {
        let mut metrics = ScrollMetrics::default();
        metrics.update_viewport_height(4);
        metrics.update_content_height(40);

        metrics.scroll_pages(1);
        assert_eq!(metrics.offset(), 4);

        metrics.scroll_pages(2);
        assert_eq!(metrics.offset(), 12);

        metrics.scroll_pages(-1);
        assert_eq!(metrics.offset(), 8);
    }
}
