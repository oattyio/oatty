use std::borrow::Cow;

use oatty_types::value_objects::EnvRow;
use rat_focus::{FocusFlag, HasFocus};
use ratatui::{layout::Rect, widgets::ListState};

use crate::ui::components::{common::key_value_editor::KeyValueEditorState, library::CatalogProjection};

/// Tracks focus and selection state for the registry library view.
#[derive(Debug, Default)]
pub struct LibraryState {
    api_list_state: ListState,
    url_list_state: ListState,
    api_mouse_over_index: Option<usize>,
    kv_state: KeyValueEditorState,
    error_message: Option<String>,
    projections: Vec<CatalogProjection>,
    /// Container focus
    container: FocusFlag,
    pub f_import_button: FocusFlag,
    pub f_remove_button: FocusFlag,
    pub f_api_list: FocusFlag,
    pub f_url_list: FocusFlag,
    // Focus flag used in the confirmation modal
    // presented whent the user removes a catalog
    pub f_modal_confirmation_button: FocusFlag,
}

impl LibraryState {
    pub fn new() -> Self {
        Self {
            kv_state: KeyValueEditorState::new(Cow::from("Headers"), Cow::from("HEADER"), Cow::from("VALUE")),
            ..Default::default()
        }
    }

    pub fn api_list_state_mut(&mut self) -> &mut ListState {
        &mut self.api_list_state
    }

    pub fn api_selected_index(&self) -> Option<usize> {
        self.api_list_state.selected()
    }

    pub fn set_api_selected_index(&mut self, index: Option<usize>) {
        self.api_list_state.select(index);
        self.update_details();
    }

    pub fn api_select_previous(&mut self) {
        self.api_list_state.select_previous();
        self.update_details();
    }

    pub fn api_select_next(&mut self) {
        self.api_list_state.select_next();
        self.update_details();
    }

    pub fn api_list_offset(&self) -> usize {
        self.api_list_state.offset()
    }

    pub fn api_mouse_over_index(&self) -> Option<usize> {
        self.api_mouse_over_index
    }

    pub fn api_set_mouse_over_index(&mut self, index: Option<usize>) {
        self.api_mouse_over_index = index;
    }

    // -----------------
    pub fn url_list_state_mut(&mut self) -> &mut ListState {
        &mut self.url_list_state
    }

    pub fn url_selected_index(&self) -> Option<usize> {
        self.url_list_state.selected()
    }

    pub fn set_url_selected_index(&mut self, index: Option<usize>) {
        self.url_list_state.select(index);
    }

    pub fn url_select_previous(&mut self) {
        self.url_list_state.select_previous();
    }

    pub fn url_select_next(&mut self) {
        self.url_list_state.select_next();
    }

    pub fn url_list_offset(&self) -> usize {
        self.url_list_state.offset()
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

    pub fn projections(&self) -> &Vec<CatalogProjection> {
        &self.projections
    }

    pub fn set_projections(&mut self, projections: Vec<CatalogProjection>) {
        self.projections = projections;
        self.update_details();
    }

    pub fn clear_projections(&mut self) {
        self.projections.clear();
        self.update_details();
    }

    pub fn push_projection(&mut self, projection: CatalogProjection) {
        self.projections.push(projection);
    }

    pub fn get_projection_mut(&mut self, idx: usize) -> Option<&mut CatalogProjection> {
        self.projections.get_mut(idx)
    }

    pub fn selected_projection(&self) -> Option<&CatalogProjection> {
        let index = self.api_list_state.selected()?;
        self.projections.get(index)
    }

    fn update_details(&mut self) {
        self.kv_state.set_rows(Vec::new());
        self.url_list_state.select(None);
        if let Some(p) = self.api_list_state.selected().and_then(|idx| self.projections.get(idx)) {
            let rows: Vec<EnvRow> = p.headers.iter().map(EnvRow::from).collect();
            self.kv_state.set_block_label(Cow::Owned(format!("{} Headers", p.title)));
            self.kv_state.set_rows(rows);
        }
    }
}

impl HasFocus for LibraryState {
    fn build(&self, builder: &mut rat_focus::FocusBuilder) {
        let start = builder.start(self);

        builder.leaf_widget(&self.f_import_button);
        builder.leaf_widget(&self.f_remove_button);
        builder.leaf_widget(&self.f_api_list);

        if self.api_list_state.selected().is_some() {
            builder.leaf_widget(&self.f_url_list);
            builder.widget(&self.kv_state);
        }

        builder.end(start);
    }

    fn focus(&self) -> FocusFlag {
        self.container.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
