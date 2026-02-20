//! Selector modal state management for workflow inputs.
//!
//! This module encapsulates the view state, focus handling, staged selections,
//! and layout metadata used by the provider-backed workflow selector. The
//! component logic in `collector.rs` consumes these types to render the modal
//! and orchestrate user interactions.

use crate::ui::components::common::TextInputState;
use crate::ui::components::results::ResultsTableState;
use crate::ui::theme::Theme;
use crate::ui::utils::KeyScoreContext;
use oatty_types::WorkflowProviderErrorPolicy;
use oatty_util::fuzzy_score;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use serde_json::Value;

/// A staged workflow selector choice that is ready to be applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectorStagedSelection {
    /// JSON value that will be written into the workflow input slot.
    pub value: Value,
    /// Human-readable representation of the value for status messaging.
    pub display_value: String,
    /// Source field used to extract the value from the provider row.
    pub source_field: Option<String>,
    /// Full JSON row returned by the provider.
    pub row: Value,
}

impl CollectorStagedSelection {
    /// Constructs a new staged selection with the provided value metadata.
    pub fn new(value: Value, display_value: String, source_field: Option<String>, row: Value) -> Self {
        Self {
            value,
            display_value,
            source_field,
            row,
        }
    }
}

/// Loading status for the selector.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SelectorStatus {
    #[default]
    Loading,
    Loaded,
    Error,
}

/// Destination for applying a staged selector value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CollectorApplyTarget {
    /// Apply selection to the currently active workflow input.
    #[default]
    WorkflowInput,
    /// Apply selection back into the command palette input.
    PaletteInput {
        /// Whether the target in palette input is positional (`true`) or flag value (`false`).
        positional: bool,
    },
}

/// Source currently selected for the Apply action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CollectorSelectionSource {
    /// Apply the row/table-backed value.
    #[default]
    Table,
    /// Apply the manual override value.
    Manual,
}

/// Top-level view state for the workflow selector.
#[derive(Debug, Default)]
pub struct CollectorViewState<'a> {
    /// Canonical provider identifier (e.g., "apps list").
    pub provider_id: String,
    /// Arguments resolved for the provider (from prior inputs/steps).
    pub resolved_args: serde_json::Map<String, Value>,
    /// Backing results state used for rendering results.
    pub table: ResultsTableState<'a>,
    /// Optional value_field from `select` used to extract the workflow value.
    pub value_field: Option<String>,
    /// Optional display_field used as primary for filtering.
    pub display_field: Option<String>,
    /// Provider on_error policy to drive fallback behavior.
    pub on_error: Option<WorkflowProviderErrorPolicy>,
    /// Current status label ("loading…", "loaded", or error text).
    pub status: SelectorStatus,
    /// Optional error message to surface inline.
    pub error_message: Option<String>,
    /// Original unfiltered provider items (array of rows).
    pub original_items: Option<Vec<Value>>,
    /// Cache key currently awaiting asynchronous fetch completion.
    pub pending_cache_key: Option<String>,
    /// Destination for applying staged selector values.
    pub apply_target: CollectorApplyTarget,
    /// Lightweight inline filter buffer.
    pub filter: TextInputState,
    /// Currently staged selection awaiting confirmation.
    pub staged_selection: Option<CollectorStagedSelection>,
    /// Inline manual override text entered by the user.
    pub manual_override: TextInputState,
    /// Last interaction source used to determine what Apply will commit.
    pub selection_source: CollectorSelectionSource,
    /// Indicates the collector requested a file selection for manual override.
    pub pending_manual_file_pick: bool,
    /// Container and child widget focus flags
    pub container_focus: FocusFlag,
    pub f_table: FocusFlag,
    pub f_filter: FocusFlag,
    pub f_manual: FocusFlag,
    pub f_apply: FocusFlag,
    pub f_cancel: FocusFlag,
}

impl<'a> CollectorViewState<'a> {
    /// Replaces selector items and resets status to loaded.
    pub fn set_items(&mut self, items: Vec<Value>) {
        self.original_items = Some(items);
        self.status = SelectorStatus::Loaded;
        self.error_message = None;
        self.pending_cache_key = None;
        self.clear_staged_selection();
    }

    /// Applies the current filter and refreshes the backing results state.
    pub fn refresh_table(&mut self, theme: &dyn Theme) {
        self.clear_staged_selection();
        self.table.set_key_score_context(KeyScoreContext::ValueSelection, theme);
        let Some(items) = self.original_items.as_ref() else {
            return;
        };

        let query = self.filter.input().trim().to_lowercase();

        if query.is_empty() {
            self.table.apply_result_json(Some(Value::Array(items.clone())), theme, true);
            self.prioritize_selector_columns(theme);
            return;
        }
        let mut scores: Vec<(i64, usize)> = items
            .iter()
            .enumerate()
            .map(|(index, item)| match item {
                Value::Object(map) => (
                    map.values()
                        .map(|x| fuzzy_score(x.as_str().unwrap_or(""), &query).unwrap_or(0i64))
                        .reduce(|a, b| a.max(b))
                        .unwrap_or(0i64),
                    index,
                ),
                Value::String(text) => (fuzzy_score(text.to_lowercase().as_str(), &query).unwrap_or(0i64), index),
                _ => (0i64, index),
            })
            .filter(|(score, _)| *score > 0)
            .collect();
        scores.sort_by(|a, b| b.0.cmp(&a.0));
        let dataset = scores.into_iter().map(|(_, index)| items[index].clone()).collect();

        let json = Value::Array(dataset);
        self.table.apply_result_json(Some(json), theme, true);
        self.prioritize_selector_columns(theme);
    }

    /// Clears any staged selection currently pending confirmation.
    pub fn clear_staged_selection(&mut self) {
        self.staged_selection = None;
    }

    /// Replaces the staged selection.
    pub fn set_staged_selection(&mut self, selection: Option<CollectorStagedSelection>) {
        self.staged_selection = selection;
        if self.staged_selection.is_some() {
            self.selection_source = CollectorSelectionSource::Table;
        }
    }

    /// Returns the current staged selection, when present.
    pub fn staged_selection(&self) -> Option<&CollectorStagedSelection> {
        self.staged_selection.as_ref()
    }

    /// Consumes and returns the staged selection, if any.
    pub fn take_staged_selection(&mut self) -> Option<CollectorStagedSelection> {
        self.staged_selection.take()
    }

    /// Moves focus to the inline filter input, placing the cursor at the end.
    pub fn focus_filter(&mut self) {
        self.filter.set_cursor(self.filter.input().len());
    }

    /// Indicates whether the Apply button should be enabled.
    pub fn apply_enabled(&self) -> bool {
        match self.selection_source {
            CollectorSelectionSource::Table => self.staged_selection.is_some(),
            CollectorSelectionSource::Manual => !self.manual_override.is_empty(),
        }
    }

    /// Switches the active apply source.
    pub fn set_selection_source(&mut self, selection_source: CollectorSelectionSource) {
        self.selection_source = selection_source;
    }

    /// Drops the staged selection when it no longer matches the visible row.
    pub fn sync_stage_with_selection(&mut self, maybe_idx: Option<usize>) {
        let Some(staged) = self.staged_selection.as_ref() else {
            return;
        };
        let idx = maybe_idx.unwrap_or(0);
        if self.table.has_rows() {
            let Some(current_row) = self.table.selected_data(idx) else {
                self.clear_staged_selection();
                return;
            };
            if staged.row != *current_row {
                self.clear_staged_selection();
            }
            return;
        }

        let Some(current_entry) = self.table.selected_kv_entry(idx) else {
            self.clear_staged_selection();
            return;
        };
        let source_matches = staged.source_field.as_deref() == Some(current_entry.key.as_str());
        if staged.row != current_entry.raw_value || !source_matches {
            self.clear_staged_selection();
        }
    }

    fn prioritize_selector_columns(&mut self, theme: &dyn Theme) {
        let mut prioritized_keys = Vec::new();
        if let Some(value_field) = self.value_field.as_deref() {
            prioritized_keys.push(value_field.split('.').next_back().unwrap_or(value_field).to_string());
        }
        if let Some(display_field) = self.display_field.as_deref() {
            prioritized_keys.push(display_field.split('.').next_back().unwrap_or(display_field).to_string());
        }
        self.table.prioritize_columns(&prioritized_keys, theme);
    }
}

impl HasFocus for CollectorViewState<'_> {
    fn build(&self, builder: &mut FocusBuilder) {
        let start = builder.start(self);
        builder.leaf_widget(&self.f_filter);
        builder.leaf_widget(&self.f_table);
        builder.leaf_widget(&self.f_manual);
        builder.leaf_widget(&self.f_apply);
        builder.leaf_widget(&self.f_cancel);

        builder.end(start);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::dracula::DraculaTheme;
    use serde_json::json;

    #[test]
    fn sync_stage_with_selection_uses_key_value_selection_when_rows_are_not_tabular() {
        let theme = DraculaTheme::new();
        let mut state = CollectorViewState::default();
        state
            .table
            .apply_result_json(Some(json!({"id":"srv-1","name":"service-a"})), &theme, false);
        state.table.list_state.select(Some(0));
        let selected_entry = state.table.selected_kv_entry(0).cloned().expect("key/value entry expected");

        state.set_staged_selection(Some(CollectorStagedSelection::new(
            selected_entry.raw_value.clone(),
            selected_entry.raw_value.to_string(),
            Some(selected_entry.key.clone()),
            selected_entry.raw_value.clone(),
        )));
        state.sync_stage_with_selection(state.table.list_state.selected());
        assert!(state.staged_selection().is_some());

        state.table.list_state.select(Some(1));
        state.sync_stage_with_selection(state.table.list_state.selected());
        assert!(state.staged_selection().is_none());
    }
}
