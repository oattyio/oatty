use rat_focus::{FocusFlag, HasFocus};
use ratatui::{layout::Rect, widgets::ListState};

use crate::ui::components::{
    common::key_value_editor::{EnvRow, KeyValueEditorState},
    library::{CatalogProjection, DetailsEditorState},
};

/// Tracks focus and selection state for the registry library view.
#[derive(Debug, Default)]
pub struct LibraryState {
    list_state: ListState,
    mouse_over_index: Option<usize>,
    details_editor_state: DetailsEditorState,
    kv_state: KeyValueEditorState,
    error_message: Option<String>,
    projections: Vec<CatalogProjection>,
    /// Container focus
    container: FocusFlag,
    pub f_import_button: FocusFlag,
    pub f_remove_button: FocusFlag,
    pub f_list_view: FocusFlag,
    pub f_selection_checkbox: FocusFlag,
    pub f_details_area: FocusFlag,
    pub f_modal_confirmation_button: FocusFlag,
}

impl LibraryState {
    /// Returns mutable access to the list widget state backing the staged manifests view.
    pub fn list_state_mut(&mut self) -> &mut ListState {
        &mut self.list_state
    }

    /// Returns the currently selected manifest index.
    pub fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub fn set_selected_index(&mut self, index: Option<usize>) {
        self.list_state.select(index);
        self.kv_state.rows = index
            .and_then(|idx| self.projections.get(idx))
            .and_then(|p| Some(p.headers.iter().map(EnvRow::from).collect()))
            .unwrap_or_default()
    }

    pub fn offset(&self) -> usize {
        self.list_state.offset()
    }

    pub fn mouse_over_index(&self) -> Option<usize> {
        self.mouse_over_index
    }

    pub fn set_mouse_over_index(&mut self, index: Option<usize>) {
        self.mouse_over_index = index;
    }

    pub fn details_editor_state_mut(&mut self) -> &mut DetailsEditorState {
        &mut self.details_editor_state
    }

    pub fn details_editor_state(&self) -> &DetailsEditorState {
        &self.details_editor_state
    }

    pub fn kv_state_mut(&mut self) -> &mut KeyValueEditorState {
        &mut self.kv_state
    }

    pub fn kv_state(&self) -> &KeyValueEditorState {
        &self.kv_state
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub fn set_error_message(&mut self, message: Option<String>) {
        self.error_message = message;
    }

    pub fn projections_mut(&mut self) -> &mut Vec<CatalogProjection> {
        &mut self.projections
    }

    pub fn projections(&self) -> &Vec<CatalogProjection> {
        &self.projections
    }

    pub fn set_projections(&mut self, projections: Vec<CatalogProjection>) {
        self.projections = projections;
    }

    pub fn selected_projection(&self) -> Option<&CatalogProjection> {
        let Some(index) = self.list_state.selected() else {
            return None;
        };
        self.projections.get(index)
    }
}

impl HasFocus for LibraryState {
    fn build(&self, builder: &mut rat_focus::FocusBuilder) {
        let start = builder.start(self);

        builder.leaf_widget(&self.f_import_button);
        builder.leaf_widget(&self.f_remove_button);

        builder.leaf_widget(&self.f_list_view);
        if self.list_state.selected().is_some() {
            builder.leaf_widget(&self.f_selection_checkbox);
        }

        builder.leaf_widget(&self.f_details_area);
        builder.end(start);
    }

    fn focus(&self) -> FocusFlag {
        self.container.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
