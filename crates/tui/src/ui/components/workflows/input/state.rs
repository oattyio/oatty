use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;

#[derive(Debug)]
pub struct WorkflowInputViewState {
    selected: usize,
    container_focus: FocusFlag,
    f_list: FocusFlag,
}

impl WorkflowInputViewState {
    pub fn new() -> Self {
        Self {
            selected: 0,
            container_focus: FocusFlag::named("workflow.inputs"),
            f_list: FocusFlag::named("workflow.inputs.list"),
        }
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn select_next(&mut self, total: usize) {
        if total == 0 {
            self.selected = 0;
        } else {
            self.selected = (self.selected + 1) % total;
        }
    }

    pub fn select_prev(&mut self, total: usize) {
        if total == 0 {
            self.selected = 0;
            return;
        }
        if self.selected == 0 {
            self.selected = total - 1;
        } else {
            self.selected -= 1;
        }
    }

    pub fn clamp_selection(&mut self, total: usize) {
        if total == 0 {
            self.selected = 0;
        } else if self.selected >= total {
            self.selected = total - 1;
        }
    }
}

impl HasFocus for WorkflowInputViewState {
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
