//! State container for the workflow picker list, including filtering and focus handling.
//!
//! This module isolates the list-specific view state so the top-level workflow module
//! can compose it without carrying list internals.

use crate::ui::components::common::TextInputState;
use anyhow::{Result, anyhow};
use oatty_engine::workflow::document::build_runtime_catalog;
use oatty_registry::CommandRegistry;
use oatty_types::workflow::{RuntimeWorkflow, WorkflowDefinition};
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::{layout::Rect, widgets::ListState};
use std::sync::{Arc, Mutex};

/// Maintains the workflow catalogue, filtered view, and list selection state for the picker UI.
#[derive(Debug, Default)]
pub struct WorkflowListState {
    pub selected: usize,
    pub f_list: FocusFlag,

    workflows: Vec<RuntimeWorkflow>,
    cached_workflow_definitions: Option<Vec<WorkflowDefinition>>,
    filtered_indices: Vec<usize>,
    search_input: TextInputState,
    list_state: ListState,
    container_focus: FocusFlag,
}

impl WorkflowListState {
    /// Creates a new workflow list state with default focus and selection values.
    pub fn new() -> Self {
        Self {
            workflows: Vec::new(),
            cached_workflow_definitions: None,
            filtered_indices: Vec::new(),
            search_input: TextInputState::new(),
            selected: 0,
            list_state: ListState::default(),
            container_focus: FocusFlag::new().with_name("root.workflows"),
            f_list: FocusFlag::new().with_name("root.workflows.list"),
        }
    }

    /// Loads workflow definitions from the registry when the feature flag is enabled.
    ///
    /// The list is loaded lazily and refreshed whenever the underlying
    /// workflow definitions in the shared registry change.
    pub fn ensure_loaded(&mut self, registry: &Arc<Mutex<CommandRegistry>>) -> Result<()> {
        let definitions_snapshot = {
            let registry_guard = registry.lock().map_err(|_| anyhow!("could not lock registry"))?;
            let definitions = &registry_guard.workflows;

            let definitions_changed = self
                .cached_workflow_definitions
                .as_ref()
                .map(|cached| cached != definitions)
                .unwrap_or(true);

            if !definitions_changed && !self.workflows.is_empty() {
                return Ok(());
            }

            definitions.clone()
        };

        let catalog = build_runtime_catalog(&definitions_snapshot)?;
        self.workflows = catalog.into_values().collect();
        self.cached_workflow_definitions = Some(definitions_snapshot);
        self.rebuild_filter();

        Ok(())
    }

    /// Returns the selected workflow from the filtered list, if one is available.
    pub fn selected_workflow(&self) -> Option<&RuntimeWorkflow> {
        self.filtered_indices
            .get(self.selected)
            .and_then(|index| self.workflows.get(*index))
    }

    /// Returns the currently selected index in filtered-list coordinates.
    pub fn selected_filtered_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub fn set_selected_workflow(&mut self, index: usize) {
        self.selected = index;
        self.list_state.select(Some(self.selected));
    }

    /// Advances the selection to the next visible workflow, wrapping cyclically.
    pub fn select_next(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.filtered_indices.len();
        self.list_state.select(Some(self.selected));
    }

    /// Moves the selection to the previous visible workflow, wrapping cyclically.
    pub fn select_prev(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        if self.selected == 0 {
            self.selected = self.filtered_indices.len() - 1;
        } else {
            self.selected -= 1;
        }
        self.list_state.select(Some(self.selected));
    }

    /// Exposes the internal list state for Ratatui rendering.
    pub fn list_state(&mut self) -> &mut ListState {
        &mut self.list_state
    }

    /// Returns the search query currently active for filtering.
    pub fn search_query(&self) -> &str {
        self.search_input.input()
    }

    /// Returns the search cursor position in display columns (character count).
    pub fn search_cursor_columns(&self) -> usize {
        self.search_input.cursor_columns()
    }

    /// Sets the search cursor based on a display column within the search input.
    pub fn set_search_cursor_from_column(&mut self, column: u16) {
        let cursor = self.search_input.cursor_index_for_column(column);
        self.search_input.set_cursor(cursor);
    }

    /// Move search cursor one character to the left (UTF‑8 safe).
    pub fn move_search_left(&mut self) {
        self.search_input.move_left();
    }

    /// Move search cursor one character to the right (UTF‑8 safe).
    pub fn move_search_right(&mut self) {
        self.search_input.move_right();
    }

    /// Appends a character to the search query and rebuilds the filtered view.
    pub fn append_search_char(&mut self, character: char) {
        self.search_input.insert_char(character);
        self.rebuild_filter();
    }

    /// Removes the character before the cursor and rebuilds the filter.
    pub fn pop_search_char(&mut self) {
        self.search_input.backspace();
        self.rebuild_filter();
    }

    /// Clears the search query and shows all workflows.
    pub fn clear_search(&mut self) {
        if self.search_input.is_empty() {
            return;
        }
        self.search_input.set_input("");
        self.search_input.set_cursor(0);
        self.rebuild_filter();
    }

    /// Returns the number of workflows matching the current filter.
    pub fn filtered_count(&self) -> usize {
        self.filtered_indices.len()
    }

    /// Returns the total number of workflows loaded from the registry.
    pub fn total_count(&self) -> usize {
        self.workflows.len()
    }

    /// Provides the filtered indices for callers that need to inspect visible workflows.
    pub fn filtered_indices(&self) -> &[usize] {
        &self.filtered_indices
    }

    /// Returns the workflow stored at a specific absolute index.
    pub fn workflow_by_index(&self, index: usize) -> Option<&RuntimeWorkflow> {
        self.workflows.get(index)
    }

    /// Calculates the width required for the identifier column using the filtered set.
    pub fn filtered_title_width(&self) -> usize {
        self.filtered_indices
            .iter()
            .filter_map(|index| self.workflows.get(*index))
            .map(|workflow| workflow.title.as_ref().map(|t| t.len()).unwrap_or(workflow.identifier.len()))
            .max()
            .unwrap_or(0)
    }

    fn rebuild_filter(&mut self) {
        if self.workflows.is_empty() {
            self.filtered_indices.clear();
            self.selected = 0;
            self.list_state.select(None);
            return;
        }

        let query = self.search_query().trim().to_lowercase();
        if query.is_empty() {
            self.filtered_indices = (0..self.workflows.len()).collect();
        } else {
            self.filtered_indices = self
                .workflows
                .iter()
                .enumerate()
                .filter(|(_, workflow)| Self::matches_search(workflow, &query))
                .map(|(index, _)| index)
                .collect();
        }

        if self.filtered_indices.is_empty() {
            self.selected = 0;
            self.list_state.select(None);
        } else {
            if self.selected >= self.filtered_indices.len() {
                self.selected = 0;
            }
            self.list_state.select(Some(self.selected));
        }
    }

    fn matches_search(workflow: &RuntimeWorkflow, lower_query: &str) -> bool {
        let identifier_matches = workflow.identifier.to_lowercase().contains(lower_query);
        let title_matches = workflow
            .title
            .as_deref()
            .map(|title| title.to_lowercase().contains(lower_query))
            .unwrap_or(false);
        let description_matches = workflow
            .description
            .as_deref()
            .map(|description| description.to_lowercase().contains(lower_query))
            .unwrap_or(false);

        identifier_matches || title_matches || description_matches
    }
}

impl HasFocus for WorkflowListState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.leaf_widget(&self.f_list);
        builder.end(tag);
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
    use indexmap::IndexMap;
    use oatty_types::workflow::{WorkflowDefinition, WorkflowStepDefinition};

    fn workflow_definition(identifier: &str, title: Option<&str>) -> WorkflowDefinition {
        WorkflowDefinition {
            workflow: identifier.to_string(),
            title: title.map(str::to_string),
            description: None,
            inputs: IndexMap::new(),
            steps: vec![WorkflowStepDefinition {
                id: "step".to_string(),
                run: "apps:list".to_string(),
                description: None,
                depends_on: Vec::new(),
                r#if: None,
                with: IndexMap::new(),
                body: serde_json::Value::Null,
                repeat: None,
                output_contract: None,
            }],
            final_output: None,
            requires: None,
        }
    }

    #[test]
    fn ensure_loaded_refreshes_when_registry_workflow_definition_changes() {
        let mut registry_value = CommandRegistry::default();
        registry_value.workflows = vec![workflow_definition("deploy", Some("Deploy v1"))];
        let registry = Arc::new(Mutex::new(registry_value));

        let mut state = WorkflowListState::new();
        state.ensure_loaded(&registry).expect("initial load should succeed");
        let initial_title = state.selected_workflow().and_then(|workflow| workflow.title.clone());
        assert_eq!(initial_title.as_deref(), Some("Deploy v1"));

        {
            let mut registry_guard = registry.lock().expect("registry lock");
            registry_guard.workflows = vec![workflow_definition("deploy", Some("Deploy v2"))];
        }

        state.ensure_loaded(&registry).expect("reload should succeed");
        let refreshed_title = state.selected_workflow().and_then(|workflow| workflow.title.clone());
        assert_eq!(refreshed_title.as_deref(), Some("Deploy v2"));
    }
}
