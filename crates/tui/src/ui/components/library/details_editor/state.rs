use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;

#[derive(Debug, Default)]
pub struct DetailsEditorState {
    // focus
    container: FocusFlag,
}

impl HasFocus for DetailsEditorState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.end(tag);
    }

    fn area(&self) -> Rect {
        Rect::default()
    }

    fn focus(&self) -> FocusFlag {
        self.container.clone()
    }
}
