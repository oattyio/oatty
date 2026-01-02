use crate::ui::components::{common::key_value_editor::KeyValueEditorState, library::CatalogProjection};

#[derive(Debug, Default)]
pub struct DetailsEditorState {
    data: Option<CatalogProjection>,
    kv_state: KeyValueEditorState,
    is_editing: bool,
}

impl DetailsEditorState {
    pub fn data(&self) -> &Option<CatalogProjection> {
        &self.data
    }

    pub fn set_data(&mut self, data: Option<CatalogProjection>) {
        self.data = data;
    }

    pub fn is_editing(&self) -> bool {
        self.is_editing
    }

    pub fn set_editing(&mut self, is_editing: bool) {
        self.is_editing = is_editing;
    }
}
