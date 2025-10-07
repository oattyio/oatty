use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;

/// Logs drawer state for a plugin
#[derive(Debug, Clone)]
pub struct PluginLogsState {
    pub name: String,
    pub lines: Vec<String>,
    pub follow: bool,
    pub search_active: bool,
    pub search_query: String,
    /// Root focus flag for logs overlay
    pub focus: FocusFlag,
}

impl PluginLogsState {
    pub fn new(name: String) -> Self {
        Self {
            name,
            lines: Vec::new(),
            follow: true,
            search_active: false,
            search_query: String::new(),
            focus: FocusFlag::named("plugins.logs"),
        }
    }
    pub fn set_lines(&mut self, lines: Vec<String>) {
        self.lines = lines;
    }
    pub fn toggle_follow(&mut self) {
        self.follow = !self.follow;
    }
    pub fn filtered<'a>(&'a self) -> Box<dyn Iterator<Item = &'a String> + 'a> {
        if self.search_query.trim().is_empty() {
            Box::new(self.lines.iter())
        } else {
            let q = self.search_query.to_lowercase();
            Box::new(self.lines.iter().filter(move |l| l.to_lowercase().contains(&q)))
        }
    }
}

impl HasFocus for PluginLogsState {
    fn build(&self, builder: &mut FocusBuilder) {
        builder.leaf_widget(self);
    }
    fn focus(&self) -> FocusFlag {
        self.focus.clone()
    }
    fn area(&self) -> Rect {
        Rect::default()
    }
}
