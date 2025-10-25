//! Selector modal state management for workflow inputs.
//!
//! This module encapsulates the view state, focus handling, staged selections,
//! and layout metadata used by the provider-backed workflow selector. The
//! component logic in `collector.rs` consumes these types to render the modal
//! and orchestrate user interactions.

use crate::ui::components::common::TextInputState;
use crate::ui::components::table::TableState;
use crate::ui::theme::Theme;
use heroku_types::WorkflowProviderErrorPolicy;
use indexmap::IndexMap;
use ratatui::layout::Rect;
use serde_json::Value;

/// Enriched schema metadata used to annotate selector rows and detail entries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkflowSelectorFieldMetadata {
    /// JSON type hint reported by the upstream schema.
    pub json_type: Option<String>,
    /// Semantic tags associated with the field (for example, `app_id`).
    pub tags: Vec<String>,
    /// Enumerated literals declared for the field, when available.
    pub enum_values: Vec<String>,
    /// Indicates whether the field is required for the provider payload.
    pub required: bool,
}

/// A staged workflow selector choice that is ready to be applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowSelectorStagedSelection {
    /// JSON value that will be written into the workflow input slot.
    pub value: Value,
    /// Human-readable representation of the value for status messaging.
    pub display_value: String,
    /// Source field used to extract the value from the provider row.
    pub source_field: Option<String>,
    /// Full JSON row returned by the provider.
    pub row: Value,
}

impl WorkflowSelectorStagedSelection {
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

/// Focus targets available within the workflow selector modal.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowCollectorFocus {
    /// Provider results table focus, arrow navigation enabled.
    #[default]
    Table,
    /// Inline filter input focus, text editing enabled.
    Filter,
    /// Confirmation buttons focus, Enter and arrow keys interact with buttons.
    Buttons(SelectorButtonFocus),
}

/// Identifies which selector confirmation button currently holds focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorButtonFocus {
    /// Cancel button.
    Cancel,
    /// Apply button.
    Apply,
}

/// Retained layout metadata capturing screen regions for pointer hit-testing.
#[derive(Debug, Clone, Default)]
pub struct WorkflowSelectorLayoutState {
    /// Rect covering the overall selector content area.
    pub container_area: Option<Rect>,
    /// Rect covering the filter input at the top of the modal.
    pub filter_area: Option<Rect>,
    /// Rect covering the results table.
    pub table_area: Option<Rect>,
    /// Rect covering the detail pane.
    pub detail_area: Option<Rect>,
    /// Rect covering the footer (buttons + hints).
    pub footer_area: Option<Rect>,
    /// Rect for the Cancel button inside the footer.
    pub cancel_button_area: Option<Rect>,
    /// Rect for the Apply button inside the footer.
    pub apply_button_area: Option<Rect>,
}

/// Loading status for the selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorStatus {
    Loading,
    Loaded,
    Error,
}

/// Top-level view state for the workflow selector modal.
#[derive(Debug)]
pub struct WorkflowSelectorViewState<'a> {
    /// Canonical provider identifier (e.g., "apps list").
    pub provider_id: String,
    /// Arguments resolved for the provider (from prior inputs/steps).
    pub resolved_args: serde_json::Map<String, serde_json::Value>,
    /// Backing table state used for rendering results.
    pub table: TableState<'a>,
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
    pub original_items: Option<Vec<serde_json::Value>>,
    /// Cache key currently awaiting asynchronous fetch completion.
    pub pending_cache_key: Option<String>,
    /// Lightweight inline filter buffer.
    pub filter: TextInputState,
    /// Current focus target within the selector modal.
    pub focus: WorkflowCollectorFocus,
    /// Cached metadata derived from the provider schema.
    pub field_metadata: IndexMap<String, WorkflowSelectorFieldMetadata>,
    /// Currently staged selection awaiting confirmation.
    pub staged_selection: Option<WorkflowSelectorStagedSelection>,
    /// Layout metadata from the most recent render pass.
    pub layout: WorkflowSelectorLayoutState,
}

impl<'a> WorkflowSelectorViewState<'a> {
    /// Replaces selector items and resets status to loaded.
    pub fn set_items(&mut self, items: Vec<serde_json::Value>) {
        self.original_items = Some(items);
        self.status = SelectorStatus::Loaded;
        self.error_message = None;
        self.pending_cache_key = None;
        self.clear_staged_selection();
    }

    /// Applies the current filter and refreshes the backing table state.
    pub fn refresh_table(&mut self, theme: &dyn Theme) {
        let Some(items) = self.original_items.clone() else {
            return;
        };
        let query = self.filter.input().trim().to_lowercase();
        let dataset: Vec<Value> = if query.is_empty() {
            items
        } else {
            items
                .into_iter()
                .filter(|item| match item {
                    Value::Object(map) => {
                        if let Some(display_field) = self.display_field.as_deref()
                            && let Some(value) = map.get(display_field)
                            && let Some(text) = value.as_str()
                        {
                            return text.to_lowercase().starts_with(&query);
                        }
                        map.values()
                            .any(|value| value.as_str().map(|text| text.to_lowercase().contains(&query)).unwrap_or(false))
                    }
                    Value::String(text) => text.to_lowercase().contains(&query),
                    _ => false,
                })
                .collect()
        };
        let json = Value::Array(dataset);
        self.table.apply_result_json(Some(json), theme);
        self.table.normalize();
        self.clear_staged_selection();
    }

    /// Clears any staged selection currently pending confirmation.
    pub fn clear_staged_selection(&mut self) {
        self.staged_selection = None;
    }

    /// Replaces the staged selection.
    pub fn set_staged_selection(&mut self, selection: Option<WorkflowSelectorStagedSelection>) {
        self.staged_selection = selection;
    }

    /// Returns the current staged selection, when present.
    pub fn staged_selection(&self) -> Option<&WorkflowSelectorStagedSelection> {
        self.staged_selection.as_ref()
    }

    /// Consumes and returns the staged selection, if any.
    pub fn take_staged_selection(&mut self) -> Option<WorkflowSelectorStagedSelection> {
        self.staged_selection.take()
    }

    /// Moves focus to the inline filter input, placing the cursor at the end.
    pub fn focus_filter(&mut self) {
        self.focus = WorkflowCollectorFocus::Filter;
        self.filter.set_cursor(self.filter.input().len());
    }

    /// Moves focus to the results table.
    pub fn focus_table(&mut self) {
        self.focus = WorkflowCollectorFocus::Table;
    }

    /// Moves focus to the selector buttons.
    pub fn focus_buttons(&mut self, button: SelectorButtonFocus) {
        self.focus = WorkflowCollectorFocus::Buttons(button);
    }

    /// Returns true when the inline filter currently has focus.
    pub fn is_filter_focused(&self) -> bool {
        matches!(self.focus, WorkflowCollectorFocus::Filter)
    }

    /// Indicates whether the Apply button should be enabled.
    pub fn apply_enabled(&self) -> bool {
        self.staged_selection.is_some()
    }

    /// Updates cached layout metadata for hit-testing.
    pub fn set_layout(&mut self, layout: WorkflowSelectorLayoutState) {
        self.layout = layout;
    }

    /// Returns the currently focused button, defaulting to Apply.
    pub fn button_focus(&self) -> SelectorButtonFocus {
        match self.focus {
            WorkflowCollectorFocus::Buttons(button) => button,
            _ => SelectorButtonFocus::Apply,
        }
    }

    /// Advances focus to the next element in the selector modal.
    pub fn next_focus(&mut self) {
        match self.focus {
            WorkflowCollectorFocus::Table => self.focus_filter(),
            WorkflowCollectorFocus::Filter => self.focus_buttons(SelectorButtonFocus::Cancel),
            WorkflowCollectorFocus::Buttons(SelectorButtonFocus::Cancel) => self.focus_buttons(SelectorButtonFocus::Apply),
            WorkflowCollectorFocus::Buttons(SelectorButtonFocus::Apply) => self.focus_table(),
        }
    }

    /// Moves focus to the previous element in the selector modal.
    pub fn prev_focus(&mut self) {
        match self.focus {
            WorkflowCollectorFocus::Table => self.focus_buttons(SelectorButtonFocus::Apply),
            WorkflowCollectorFocus::Filter => self.focus_table(),
            WorkflowCollectorFocus::Buttons(SelectorButtonFocus::Cancel) => self.focus_filter(),
            WorkflowCollectorFocus::Buttons(SelectorButtonFocus::Apply) => self.focus_buttons(SelectorButtonFocus::Cancel),
        }
    }

    /// Drops the staged selection when it no longer matches the visible row.
    pub fn sync_stage_with_selection(&mut self) {
        let Some(staged) = self.staged_selection.as_ref() else {
            return;
        };
        let Some(current_row) = self.table.selected_data() else {
            self.clear_staged_selection();
            return;
        };
        if staged.row != *current_row {
            self.clear_staged_selection();
        }
    }
}
