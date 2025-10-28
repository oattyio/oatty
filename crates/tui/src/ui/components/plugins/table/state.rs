//! Table state for the MCP plugins view, covering filtering, focus, and selection.

use crate::ui::components::plugins::PluginDetail;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use ratatui::widgets::TableState;
use std::time::{Duration, Instant};

/// State container for the MCP plugins table including filtering logic and
/// selection metadata.
///
/// The table owns the quick-search filter, the filtered selection index, and
/// the timing information that determines when the UI should poll for fresh
/// plugin status updates.
#[derive(Debug)]
pub struct PluginsTableState {
    pub table_state: TableState,
    /// Root focus scope for the plugins table cluster.
    pub container_focus: FocusFlag,
    /// Focus scopes for each of the table's sub-components.
    pub f_search: FocusFlag,
    pub f_grid: FocusFlag,
    pub f_add: FocusFlag,
    pub f_start: FocusFlag,
    pub f_stop: FocusFlag,
    pub f_delete: FocusFlag,
    pub f_edit: FocusFlag,
    /// Case-insensitive filter string entered by the user.
    pub filter: String,
    /// Flat list of plugin rows sourced from configuration or runtime updates.
    pub items: Vec<PluginDetail>,
    /// the position of the cursor in the search input
    pub cursor_position: usize,
    // the last time the table was refreshed, used to determine when to poll for updates
    last_refresh: Option<Instant>,
}

impl PluginsTableState {
    /// Create a new table state with empty data and default focus flags.
    pub fn new() -> Self {
        Self {
            table_state: TableState::default(),
            container_focus: FocusFlag::named("plugins.table"),
            f_search: FocusFlag::named("plugins.search"),
            f_grid: FocusFlag::named("plugins.grid"),
            f_add: FocusFlag::named("plugins.add"),
            f_start: FocusFlag::named("plugins.start_or_restart"),
            f_stop: FocusFlag::named("plugins.stop"),
            f_delete: FocusFlag::named("plugins.delete"),
            f_edit: FocusFlag::named("plugins.edit"),
            filter: String::new(),
            items: Vec::new(),
            last_refresh: None,
            cursor_position: 0,
        }
    }

    /// Replace the table rows and normalize the current selection accordingly.
    pub fn replace_items(&mut self, rows: Vec<PluginDetail>) {
        self.items = rows;
        self.normalize_selection();
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
            .filter(|(_, item)| self.matches_filter(item, &query))
            .map(|(index, _)| index)
            .collect()
    }

    fn matches_filter(&self, item: &PluginDetail, query: &str) -> bool {
        item.name.to_lowercase().contains(query)
            || item.command_or_url.to_lowercase().contains(query)
            || item.tags.iter().any(|tag| tag.to_lowercase().contains(query))
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
        let position = self.table_state.selected()?;
        let index = *filtered.get(position)?;
        self.items.get(index)
    }

    pub fn set_selected_index(&mut self, index: Option<usize>) {
        self.table_state.select(index);
    }

    /// Remove the trailing character from the filter and reset the selection.
    pub fn pop_filter_character(&mut self) {
        if self.cursor_position > 0 {
            self.reduce_move_cursor_left();
            self.filter.remove(self.cursor_position);
        }
        self.select_first_filtered_row();
    }

    /// Append a character to the filter and reset the selection to the top.
    pub fn push_filter_character(&mut self, value: char) {
        self.filter.insert(self.cursor_position, value);
        self.cursor_position += value.len_utf8();
        self.select_first_filtered_row();
    }

    /// Clear the filter entirely, preserving existing rows but normalizing selection.
    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.cursor_position = 0;
        self.select_first_filtered_row();
    }

    /// Expose the current filter text for read-only scenarios.
    pub fn filter_text(&self) -> &str {
        &self.filter
    }

    /// Returns true when the current selection maps to a valid filtered item.
    pub fn has_selection(&self) -> bool {
        let Some(selected_index) = self.table_state.selected() else {
            return false;
        };
        self.filtered_indices().get(selected_index).is_some()
    }

    fn select_first_filtered_row(&mut self) {
        if self.filtered_indices().is_empty() {
            self.table_state.select(None);
        } else {
            self.table_state.select(Some(0));
        }
    }

    /// Ensures the currently selected row refers to a valid filtered entry.
    ///
    /// The selection is cleared when no filtered rows remain, clamped to the last available
    /// row when it falls out of range, and defaulted to the first row when unset.
    pub fn normalize_selection(&mut self) {
        let filtered = self.filtered_indices();
        if filtered.is_empty() {
            self.table_state.select(None);
            return;
        }

        match self.table_state.selected() {
            Some(index) if index < filtered.len() => {}
            Some(_) => self.table_state.select(Some(filtered.len().saturating_sub(1))),
            None => self.table_state.select(Some(0)),
        }
    }
}
impl HasFocus for PluginsTableState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.leaf_widget(&self.f_search);
        builder.leaf_widget(&self.f_grid);
        builder.leaf_widget(&self.f_add);
        if self.has_selection() {
            builder.leaf_widget(&self.f_start);
            builder.leaf_widget(&self.f_stop);
            builder.leaf_widget(&self.f_edit);
            builder.leaf_widget(&self.f_delete);
        }
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
