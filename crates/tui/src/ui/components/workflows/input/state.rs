use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;

/// Captures layout metadata from the most recent render pass for hit detection.
#[derive(Debug, Default, Clone, Copy)]
pub struct WorkflowInputLayout {
    /// Screen area occupied by the cancel button, if rendered.
    pub cancel_button_area: Option<Rect>,
    /// Screen area occupied by the run button, if rendered.
    pub run_button_area: Option<Rect>,
}

#[derive(Debug)]
pub struct WorkflowInputViewState {
    selected: usize,
    container_focus: FocusFlag,
    /// Focus flag tracking list navigation state.
    pub f_list: FocusFlag,
    /// Focus flag used for the cancel action button.
    pub f_cancel_button: FocusFlag,
    /// Focus flag used for the run action button.
    pub f_run_button: FocusFlag,
    layout: WorkflowInputLayout,
}

impl WorkflowInputViewState {
    pub fn new() -> Self {
        Self {
            selected: 0,
            container_focus: FocusFlag::named("workflow.inputs"),
            f_list: FocusFlag::named("workflow.inputs.list"),
            f_cancel_button: FocusFlag::named("workflow.inputs.actions.cancel"),
            f_run_button: FocusFlag::named("workflow.inputs.actions.run"),
            layout: WorkflowInputLayout::default(),
        }
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn set_selected(&mut self, index: usize) {
        self.selected = index;
    }

    pub fn clamp_selection(&mut self, total: usize) {
        if total == 0 {
            self.selected = 0;
        } else if self.selected >= total {
            self.selected = total - 1;
        }
    }

    /// Stores the latest button layout to make mouse hit-testing possible.
    pub fn set_layout(&mut self, layout: WorkflowInputLayout) {
        self.layout = layout;
    }

    /// Returns the most recently captured layout information.
    pub fn layout(&self) -> &WorkflowInputLayout {
        &self.layout
    }
}

impl HasFocus for WorkflowInputViewState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.leaf_widget(&self.f_list);
        builder.leaf_widget(&self.f_cancel_button);
        builder.leaf_widget(&self.f_run_button);
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
