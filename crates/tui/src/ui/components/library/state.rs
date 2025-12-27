use rat_focus::{FocusFlag, HasFocus};
use ratatui::{layout::Rect, widgets::ListState};

#[derive(Debug, Default)]
pub struct LibraryState {
    list_state: ListState,
    /// Container focus
    container: FocusFlag,
    pub f_import_button: FocusFlag,
    pub f_remove_button: FocusFlag,
    pub f_list_view: FocusFlag,
    pub f_details_area: FocusFlag,
}

impl LibraryState {
    pub fn list_state_mut(&mut self) -> &mut ListState {
        &mut self.list_state
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }
}

impl HasFocus for LibraryState {
    fn build(&self, builder: &mut rat_focus::FocusBuilder) {
        let start = builder.start(self);

        builder.leaf_widget(&self.f_import_button);
        builder.leaf_widget(&self.f_remove_button);
        builder.leaf_widget(&self.f_list_view);
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
