use std::borrow::Cow;

use oatty_types::{TransientMessage, value_objects::EnvRow};
use rat_focus::{FocusFlag, HasFocus};
use ratatui::{layout::Rect, widgets::ListState};

use crate::ui::components::{
    common::{TextInputState, key_value_editor::KeyValueEditorState},
    library::CatalogProjection,
};

pub enum LibraryEditorField {
    Description,
    BaseUrl,
}

/// Tracks focus and selection state for the registry library view.
#[derive(Debug, Default)]
pub struct LibraryState {
    api_list_state: ListState,
    url_list_state: ListState,
    api_mouse_over_index: Option<usize>,
    kv_state: KeyValueEditorState,
    message: Option<TransientMessage>,
    projections: Vec<CatalogProjection>,
    description_input: TextInputState,
    base_url_input: TextInputState,
    base_url_err: Option<Cow<'static, str>>,
    is_dirty: bool,
    /// Container focus
    container: FocusFlag,
    pub f_import_button: FocusFlag,
    pub f_remove_button: FocusFlag,
    pub f_description_input: FocusFlag,
    pub f_api_list: FocusFlag,

    pub f_url_list_container: FocusFlag,
    pub f_base_url_input: FocusFlag,
    pub f_add_url_button: FocusFlag,
    pub f_remove_url_button: FocusFlag,
    pub f_url_list: FocusFlag,
    // Focus flag used in the confirmation modal
    // presented whent the user removes a catalog
    pub f_modal_confirmation_button: FocusFlag,
}

impl LibraryState {
    pub fn new() -> Self {
        Self {
            kv_state: KeyValueEditorState::new(Cow::from("Headers"), Cow::from("Header"), Cow::from("Value (optional")),
            ..Default::default()
        }
    }

    pub fn set_base_url_err(&mut self, err: Option<Cow<'static, str>>) {
        self.base_url_err = err;
    }

    pub fn base_url_err(&self) -> Option<Cow<'static, str>> {
        self.base_url_err.clone()
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn api_list_state_mut(&mut self) -> &mut ListState {
        &mut self.api_list_state
    }

    pub fn api_selected_index(&self) -> Option<usize> {
        self.api_list_state.selected()
    }

    pub fn set_api_selected_index(&mut self, index: Option<usize>) {
        self.api_list_state.select(index);
        self.load_input_for_selected_api();
        self.update_kv_state();
    }

    pub fn api_select_previous(&mut self) {
        self.api_list_state.select_previous();
        self.load_input_for_selected_api();
        self.update_kv_state();
    }

    pub fn api_select_next(&mut self) {
        self.api_list_state.select_next();
        self.load_input_for_selected_api();
        self.update_kv_state();
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
        self.load_input_for_selected_url();
    }

    pub fn url_select_previous(&mut self) {
        self.url_list_state.select_previous();
        self.load_input_for_selected_url();
    }

    pub fn url_select_next(&mut self) {
        self.url_list_state.select_next();
        self.load_input_for_selected_url();
    }

    pub fn url_list_offset(&self) -> usize {
        self.url_list_state.offset()
    }

    pub fn url_text_input(&self) -> &TextInputState {
        &self.base_url_input
    }

    pub fn url_add_row(&mut self) -> bool {
        let Some(projection) = self.selected_projection_mut() else {
            return false;
        };
        projection.base_urls.push(String::new());
        let len = projection.base_urls.len();
        self.url_list_state.select(Some(len - 1));
        self.base_url_input.clear();
        true
    }

    pub fn remove_url_row(&mut self) -> bool {
        let Some(selected) = self.url_list_state.selected() else {
            return false;
        };
        let Some(projection) = self.selected_projection_mut() else {
            return false;
        };
        projection.base_urls.remove(selected);
        self.url_list_state.select_previous();
        self.base_url_input.clear();
        self.is_dirty = true;
        true
    }

    pub fn description_text_input(&self) -> &TextInputState {
        &self.description_input
    }

    pub fn kv_state_mut(&mut self) -> &mut KeyValueEditorState {
        &mut self.kv_state
    }

    pub fn kv_state(&self) -> &KeyValueEditorState {
        &self.kv_state
    }

    pub fn message_ref(&self) -> Option<&TransientMessage> {
        self.message.as_ref()
    }

    pub fn set_message(&mut self, message: Option<TransientMessage>) {
        self.message = message;
    }

    pub fn projections(&self) -> &Vec<CatalogProjection> {
        &self.projections
    }

    pub fn set_projections(&mut self, projections: Vec<CatalogProjection>) {
        if self.api_selected_index().is_none() {
            let idx = self.api_list_state.selected().unwrap_or(0).min(projections.len());
            self.set_api_selected_index(Some(idx));
        }
        self.projections = projections;
    }

    pub fn clear_projections(&mut self) {
        self.projections.clear();
        self.update_kv_state();
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

    pub fn active_input_field(&self) -> Option<LibraryEditorField> {
        if self.f_description_input.get() {
            Some(LibraryEditorField::Description)
        } else if self.f_base_url_input.get() {
            Some(LibraryEditorField::BaseUrl)
        } else {
            None
        }
    }

    /// Inserts a character into the focused input
    ///
    /// # Arguments
    ///
    /// * `character` - The character to insert at the current cursor position
    pub fn insert_character_for_active_field(&mut self, character: char) {
        match self.active_input_field() {
            None => {}
            Some(LibraryEditorField::Description) => {
                self.description_input.insert_char(character);
            }
            Some(LibraryEditorField::BaseUrl) => {
                self.base_url_input.insert_char(character);
            }
        };
        if self.update_projection_with_input().is_some() {
            self.is_dirty = true;
        }
    }

    /// Removes the character before the cursor in the focused input.
    pub fn delete_previous_character(&mut self) {
        match self.active_input_field() {
            None => {}
            Some(LibraryEditorField::Description) => {
                self.description_input.backspace();
            }
            Some(LibraryEditorField::BaseUrl) => {
                self.base_url_input.backspace();
            }
        }
        if self.update_projection_with_input().is_some() {
            self.is_dirty = true;
        }
    }

    /// Removes the character after the cursor in the focused input.
    pub fn delete_next_character(&mut self) {
        match self.active_input_field() {
            None => {}
            Some(LibraryEditorField::Description) => {
                self.description_input.delete();
            }
            Some(LibraryEditorField::BaseUrl) => {
                self.base_url_input.delete();
            }
        }
        if self.update_projection_with_input().is_some() {
            self.is_dirty = true;
        }
    }

    /// Moves the cursor left in the focused input.
    pub fn move_cursor_left(&mut self) {
        match self.active_input_field() {
            None => {}
            Some(LibraryEditorField::Description) => self.description_input.move_left(),
            Some(LibraryEditorField::BaseUrl) => self.base_url_input.move_left(),
        }
    }

    /// Moves the cursor right in the focused input.
    pub fn move_cursor_right(&mut self) {
        match self.active_input_field() {
            None => {}
            Some(LibraryEditorField::Description) => self.description_input.move_right(),
            Some(LibraryEditorField::BaseUrl) => self.base_url_input.move_right(),
        }
    }

    fn update_projection_with_input(&mut self) -> Option<()> {
        match self.active_input_field() {
            None => return None,
            Some(LibraryEditorField::Description) => {
                let value = self.description_input.input().to_string();
                self.selected_projection_mut()?.description = Cow::from(value);
            }
            Some(LibraryEditorField::BaseUrl) => {
                let value = self.base_url_input.input().to_string();
                let (idx, p) = (self.url_selected_index()?, self.selected_projection_mut()?);
                let s = p.base_urls.get_mut(idx)?;
                *s = value;
            }
        };
        Some(())
    }

    fn selected_projection_mut(&mut self) -> Option<&mut CatalogProjection> {
        let index = self.api_list_state.selected()?;
        self.projections.get_mut(index)
    }

    fn update_kv_state(&mut self) {
        self.url_list_state.select(None);
        if let Some(p) = self.api_list_state.selected().and_then(|idx| self.projections.get(idx)) {
            let rows: Vec<EnvRow> = p.headers.iter().map(EnvRow::from).collect();
            self.kv_state.set_block_label(Cow::Owned(format!("{} Headers", p.title)));
            self.kv_state.set_rows(rows);
        } else {
            self.kv_state.set_rows(Vec::new());
        }
    }

    fn load_input_for_selected_api(&mut self) {
        if let Some(p) = self.api_list_state.selected().and_then(|idx| self.projections.get(idx)) {
            self.description_input.set_input(p.description.clone());
            self.description_input.set_cursor(p.description.len());
        }
    }

    fn load_input_for_selected_url(&mut self) {
        if let Some((p, index)) = self.selected_projection().zip(self.url_selected_index())
            && index < p.base_urls.len()
        {
            let val = p.base_urls[index].clone();
            let len = val.len();
            self.base_url_input.set_input(val);
            self.base_url_input.set_cursor(len);
        }
    }
}

impl HasFocus for LibraryState {
    fn build(&self, builder: &mut rat_focus::FocusBuilder) {
        let start = builder.start(self);

        builder.leaf_widget(&self.f_import_button);
        builder.leaf_widget(&self.f_remove_button);
        builder.leaf_widget(&self.f_api_list);

        // Details focus items
        if self.api_list_state.selected().is_some() {
            builder.leaf_widget(&self.f_description_input);

            let url_start = builder.start(&self.f_url_list_container);
            builder.leaf_widget(&self.f_add_url_button);
            builder.leaf_widget(&self.f_remove_url_button);
            builder.leaf_widget(&self.f_url_list);
            builder.leaf_widget(&self.f_base_url_input);
            builder.end(url_start);

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
