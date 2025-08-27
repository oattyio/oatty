use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub(crate) struct BuilderLayout;

impl BuilderLayout {
    /// Creates the vertical layout for search, main content, and footer.
    pub fn vertical_layout(inner: Rect) -> Vec<Rect> {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Search panel
                Constraint::Min(10),   // Main content
                Constraint::Length(1), // Footer
            ])
            .split(inner)
            .to_vec()
    }
}
