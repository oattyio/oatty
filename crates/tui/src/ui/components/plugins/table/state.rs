//! Table state for the MCP plugins view, covering filtering, focus, and selection.

use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

use crate::ui::components::plugins::PluginListItem;

/// State container for the MCP plugins table including filtering logic and
/// selection metadata.
///
/// The table owns the quick-search filter, the filtered selection index, and
/// the timing information that determines when the UI should poll for fresh
/// plugin status updates.
#[derive(Debug, Clone)]
pub struct PluginsTableState {
    /// Root focus scope for the plugins table cluster.
    pub focus: FocusFlag,
    /// Focus flag representing the quick-search input field above the table.
    pub search_flag: FocusFlag,
    /// Focus flag representing the selectable grid of plugin rows.
    pub grid_flag: FocusFlag,
    /// Case-insensitive filter string entered by the user.
    pub filter: String,
    /// Flat list of plugin rows sourced from configuration or runtime updates.
    pub items: Vec<PluginListItem>,
    /// Current selection within the filtered view (index into `filtered_indices`).
    pub selected: Option<usize>,
    last_refresh: Option<Instant>,
}

impl PluginsTableState {
    /// Create a new table state with empty data and default focus flags.
    pub fn new() -> Self {
        Self {
            focus: FocusFlag::named("plugins.table"),
            search_flag: FocusFlag::named("plugins.search"),
            grid_flag: FocusFlag::named("plugins.grid"),
            filter: String::new(),
            items: Vec::new(),
            selected: None,
            last_refresh: None,
        }
    }

    /// Replace the table rows and normalize the current selection accordingly.
    pub fn replace_items(&mut self, rows: Vec<PluginListItem>) {
        self.items = rows;
        self.selected = if self.items.is_empty() { None } else { Some(0) };
    }

    /// Compute the raw indices for rows that match the current quick-search filter.
    pub fn filtered_indices(&self) -> Vec<usize> {
        if self.filter.trim().is_empty() {
            return (0..self.items.len()).collect();
        }
        let query = self.filter.to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                item.name.to_lowercase().contains(&query)
                    || item.command_or_url.to_lowercase().contains(&query)
                    || item.tags.iter().any(|tag| tag.to_lowercase().contains(&query))
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// Determine whether enough time has elapsed to trigger a refresh of plugin status.
    pub fn should_refresh(&mut self) -> bool {
        const INTERVAL: Duration = Duration::from_millis(1000);
        let now = Instant::now();
        match self.last_refresh {
            None => {
                self.last_refresh = Some(now);
                true
            }
            Some(timestamp) if now.duration_since(timestamp) >= INTERVAL => {
                self.last_refresh = Some(now);
                true
            }
            _ => false,
        }
    }

    /// Apply status update payloads to the existing table rows in-place.
    pub fn apply_refresh_updates(&mut self, updates: Vec<(String, String, Option<u64>, Option<String>)>) {
        for (name, status, latency, last_error) in updates {
            if let Some(item) = self.items.iter_mut().find(|row| row.name == name) {
                item.status = status;
                item.latency_ms = latency;
                item.last_error = last_error;
            }
        }
    }

    /// Retrieve the currently selected item with respect to the filtered view.
    pub fn selected_item(&self) -> Option<&PluginListItem> {
        let filtered = self.filtered_indices();
        let position = self.selected?;
        let index = *filtered.get(position)?;
        self.items.get(index)
    }

    /// Remove the trailing character from the filter and reset the selection.
    pub fn pop_filter_character(&mut self) {
        self.filter.pop();
        self.selected = Some(0);
    }

    /// Append a character to the filter and reset the selection to the top.
    pub fn push_filter_character(&mut self, value: char) {
        self.filter.push(value);
        self.selected = Some(0);
    }

    /// Clear the filter entirely, preserving existing rows but normalizing selection.
    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.selected = Some(0);
    }

    /// Expose the current filter text for read-only scenarios.
    pub fn filter_text(&self) -> &str {
        &self.filter
    }
}

impl Default for PluginsTableState {
    fn default() -> Self {
        Self::new()
    }
}

impl HasFocus for PluginsTableState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.leaf_widget(&self.search_flag);
        builder.leaf_widget(&self.grid_flag);
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
