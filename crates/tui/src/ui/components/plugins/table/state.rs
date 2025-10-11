//! Table state for the MCP plugins view, covering filtering, focus, and selection.

use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

use crate::ui::components::plugins::PluginDetail;

/// State container for the MCP plugins table including filtering logic and
/// selection metadata.
///
/// The table owns the quick-search filter, the filtered selection index, and
/// the timing information that determines when the UI should poll for fresh
/// plugin status updates.
#[derive(Debug, Clone)]
pub struct PluginsTableState {
    /// Root focus scope for the plugins table cluster.
    pub container_focus: FocusFlag,
    /// Focus flag representing the quick-search input field above the table.
    pub f_search: FocusFlag,
    /// Focus flag representing the selectable grid of plugin rows.
    pub f_grid: FocusFlag,
    /// Case-insensitive filter string entered by the user.
    pub filter: String,
    /// Flat list of plugin rows sourced from configuration or runtime updates.
    pub items: Vec<PluginDetail>,
    /// Current selection within the filtered view (index into `filtered_indices`).
    pub selected: Option<usize>,
    // the position of the cursor in the search input
    pub cursor_position: usize,
    // mouse focus related fields
    pub last_area: Rect,
    pub per_item_area: Vec<Rect>,
    // the last time the table was refreshed, used to determine when to poll for updates
    last_refresh: Option<Instant>,
}

impl PluginsTableState {
    /// Create a new table state with empty data and default focus flags.
    pub fn new() -> Self {
        Self {
            container_focus: FocusFlag::named("plugins.table"),
            f_search: FocusFlag::named("plugins.search"),
            f_grid: FocusFlag::named("plugins.grid"),
            filter: String::new(),
            items: Vec::new(),
            selected: None,
            last_refresh: None,
            cursor_position: 0,
            last_area: Rect::default(),
            per_item_area: Vec::new(),
        }
    }

    /// Replace the table rows and normalize the current selection accordingly.
    pub fn replace_items(&mut self, rows: Vec<PluginDetail>) {
        self.items = rows;
        self.selected = if self.items.is_empty() { None } else { Some(0) };
    }

    pub fn update_item(&mut self, item: PluginDetail) {
        let Some(idx) = self.items.iter().position(|i| i.name == item.name) else {
            return self.items.push(item);
        };
        self.items[idx] = item;
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
        if !self.container_focus.get() {
            return false;
        }
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

    /// Move the cursor one character to the left.
    ///
    /// This method handles UTF-8 character boundaries correctly,
    /// ensuring the cursor moves by one Unicode character rather than
    /// one byte.
    ///
    /// - No-op if the cursor is already at the start of the input.
    ///
    /// Returns: nothing; updates `self.cursor` in place.
    pub fn reduce_move_cursor_left(&mut self) {
        if self.cursor_position == 0 {
            return;
        }
        let prev_len = self.filter[..self.cursor_position]
            .chars()
            .last()
            .map(|c| c.len_utf8())
            .unwrap_or(1);
        self.cursor_position = self.cursor_position.saturating_sub(prev_len);
    }

    /// Move the cursor one character to the right.
    ///
    /// This method handles UTF-8 character boundaries correctly,
    /// ensuring the cursor moves by one Unicode character rather than
    /// one byte.
    ///
    /// - No-op if the cursor is already at the end of the input.
    ///
    /// Returns: nothing; updates `self.cursor` in place.
    pub fn reduce_move_cursor_right(&mut self) {
        if self.cursor_position >= self.filter.len() {
            return;
        }
        // Advance by one Unicode scalar starting at current byte offset
        let mut iter = self.filter[self.cursor_position..].chars();
        if let Some(next) = iter.next() {
            self.cursor_position = self.cursor_position.saturating_add(next.len_utf8());
        }
    }

    /// Retrieve the currently selected item with respect to the filtered view.
    pub fn selected_item(&self) -> Option<&PluginDetail> {
        let filtered = self.filtered_indices();
        let position = self.selected?;
        let index = *filtered.get(position)?;
        self.items.get(index)
    }

    /// Remove the trailing character from the filter and reset the selection.
    pub fn pop_filter_character(&mut self) {
        if self.cursor_position > 0 {
            self.reduce_move_cursor_left();
            self.filter.remove(self.cursor_position);
        }
        self.selected = Some(0);
    }

    /// Append a character to the filter and reset the selection to the top.
    pub fn push_filter_character(&mut self, value: char) {
        self.filter.insert(self.cursor_position, value);
        self.cursor_position += value.len_utf8();
        self.selected = Some(0);
    }

    /// Clear the filter entirely, preserving existing rows but normalizing selection.
    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.cursor_position = 0;
        self.selected = Some(0);
    }

    /// Expose the current filter text for read-only scenarios.
    pub fn filter_text(&self) -> &str {
        &self.filter
    }
}
impl HasFocus for PluginsTableState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.leaf_widget(&self.f_search);
        builder.leaf_widget(&self.f_grid);
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
