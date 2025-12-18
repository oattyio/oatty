use oatty_types::Severity;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;

#[derive(Default, Clone)]
pub struct ConfirmationModalState {
    title: Option<String>,
    message: Option<String>,
    buttons: Vec<(String, FocusFlag)>,
    severity: Option<Severity>,

    container_focus: FocusFlag,
}

impl ConfirmationModalState {
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn set_title(&mut self, title: Option<String>) {
        self.title = title;
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub fn set_message(&mut self, message: Option<String>) {
        self.message = message;
    }

    pub fn severity(&self) -> Option<Severity> {
        self.severity.clone()
    }

    pub fn set_severity(&mut self, severity: Option<Severity>) {
        self.severity = severity;
    }

    pub fn buttons(&self) -> &[(String, FocusFlag)] {
        &self.buttons
    }

    pub fn set_buttons(&mut self, buttons: Vec<(String, FocusFlag)>) {
        self.buttons = buttons;
    }

    pub fn is_button_focused(&self, idx: usize) -> bool {
        self.buttons.get(idx).is_some_and(|f| f.1.get())
    }
}

impl HasFocus for ConfirmationModalState {
    fn build(&self, builder: &mut FocusBuilder) {
        let start = builder.start(self);

        self.buttons.iter().for_each(|f| {
            builder.leaf_widget(&f.1);
        });

        builder.end(start);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
