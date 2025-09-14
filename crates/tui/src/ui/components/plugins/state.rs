use heroku_mcp::types::plugin::AuthStatus;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

/// UI state for the Plugins view.
#[derive(Debug, Default, Clone)]
pub struct PluginsState {
    /// Focus for the quick search input in the list header.
    pub search_flag: FocusFlag,
    /// Focus for the main table/grid area.
    pub grid_flag: FocusFlag,
    /// Current quick search text ("/" behavior).
    pub filter: String,
    /// Whether the component is currently visible (overlay style).
    visible: bool,
    /// Current items loaded from config.
    pub items: Vec<PluginListItem>,
    /// Optional selection index into filtered view.
    pub selected: Option<usize>,
    /// Last refresh time for status polling.
    last_refresh: Option<Instant>,
    /// Logs drawer state, if open.
    pub logs: Option<PluginLogsState>,
    /// Environment editor state, if open
    pub env: Option<PluginSecretsEditorState>,
    /// Add plugin view state
    pub add: Option<super::add_plugin::state::PluginAddViewState>,
}

impl PluginsState {
    pub fn new() -> Self {
        Self {
            search_flag: FocusFlag::named("plugins.search"),
            grid_flag: FocusFlag::named("plugins.grid"),
            filter: String::new(),
            visible: false,
            items: Vec::new(),
            selected: None,
            last_refresh: None,
            logs: None,
            env: None,
            add: None,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, vis: bool) {
        self.visible = vis;
    }

    pub fn replace_items(&mut self, rows: Vec<PluginListItem>) {
        self.items = rows;
        // Reset selection to the first row in filtered list
        self.selected = if self.items.is_empty() { None } else { Some(0) };
    }

    /// Get indices of items matching the current filter (case-insensitive, name/url/tags).
    pub fn filtered_indices(&self) -> Vec<usize> {
        if self.filter.trim().is_empty() {
            return (0..self.items.len()).collect();
        }
        let q = self.filter.to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter(|(_, it)| {
                it.name.to_lowercase().contains(&q)
                    || it.command_or_url.to_lowercase().contains(&q)
                    || it.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Whether it's time to refresh status based on visibility and elapsed time.
    pub fn should_refresh(&mut self) -> bool {
        const INTERVAL: Duration = Duration::from_millis(1000);
        let now = Instant::now();
        match self.last_refresh {
            None => {
                self.last_refresh = Some(now);
                true
            }
            Some(t) if now.duration_since(t) >= INTERVAL => {
                self.last_refresh = Some(now);
                true
            }
            _ => false,
        }
    }

    /// Apply refresh updates (name, status, latency, last_error) to items in-place.
    pub fn apply_refresh_updates(&mut self, updates: Vec<(String, String, Option<u64>, Option<String>)>) {
        for (name, status, lat, err) in updates {
            if let Some(item) = self.items.iter_mut().find(|it| it.name == name) {
                item.status = status;
                item.latency_ms = lat;
                item.last_error = err;
            }
        }
    }

    /// Get the currently selected item (respecting the filtered view).
    pub fn selected_item(&self) -> Option<&PluginListItem> {
        let filtered = self.filtered_indices();
        let pos = self.selected?;
        let idx = *filtered.get(pos)?;
        self.items.get(idx)
    }

    pub fn open_logs(&mut self, name: String) {
        self.logs = Some(PluginLogsState::new(name));
    }

    pub fn close_logs(&mut self) {
        self.logs = None;
    }

    pub fn open_secrets(&mut self, name: String) {
        self.env = Some(PluginSecretsEditorState::new(name));
    }
    pub fn close_secrets(&mut self) {
        self.env = None;
    }
}

// Leaf wrapper for rat-focus
struct PanelLeaf(FocusFlag);
impl HasFocus for PanelLeaf {
    fn build(&self, builder: &mut FocusBuilder) {
        builder.leaf_widget(self);
    }
    fn focus(&self) -> FocusFlag {
        self.0.clone()
    }
    fn area(&self) -> Rect {
        Rect::default()
    }
}

impl HasFocus for PluginsState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        // Header search input and main grid
        builder.widget(&PanelLeaf(self.search_flag.clone()));
        builder.widget(&PanelLeaf(self.grid_flag.clone()));
        // Include add wizard if visible
        if let Some(add) = &self.add {
            builder.widget(add);
        }
        // Include overlays if open
        if let Some(logs) = &self.logs {
            builder.widget(logs);
        }
        if let Some(env) = &self.env {
            builder.widget(env);
        }
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        // Default container identity; search is a sensible default
        self.search_flag.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugins_state_focus_builds() {
        let s = PluginsState::new();
        let mut b = FocusBuilder::new(None);
        b.widget(&s);
        let f = b.build();
        // Sanity: focusing search and grid should be possible
        f.focus(&s.search_flag);
        f.focus(&s.grid_flag);
    }
}

/// A row in the Plugins table.
#[derive(Debug, Clone, Default)]
pub struct PluginListItem {
    pub name: String,
    pub status: String,
    pub command_or_url: String,
    pub tags: Vec<String>,
    pub latency_ms: Option<u64>,
    pub last_error: Option<String>,
    pub auth_status: AuthStatus,
}

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

/// Environment editor state for a plugin
#[derive(Debug, Default, Clone)]
pub struct PluginSecretsEditorState {
    pub name: String,
    pub rows: Vec<EnvRow>,
    pub selected: usize,
    pub editing: bool,
    pub input: String,
    /// Root focus flag for env overlay
    pub focus: FocusFlag,
}

#[derive(Debug, Clone)]
pub struct EnvRow {
    pub key: String,
    pub value: String,
    pub is_secret: bool,
}

impl PluginSecretsEditorState {
    pub fn new(name: String) -> Self {
        Self {
            name,
            rows: Vec::new(),
            selected: 0,
            editing: false,
            input: String::new(),
            focus: FocusFlag::named("plugins.env"),
        }
    }
    pub fn set_rows(&mut self, rows: Vec<EnvRow>) {
        self.rows = rows;
        self.selected = 0;
        self.editing = false;
        self.input.clear();
    }
}

impl HasFocus for PluginSecretsEditorState {
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
