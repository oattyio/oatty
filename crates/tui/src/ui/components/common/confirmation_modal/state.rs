use oatty_types::MessageType;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;

use crate::ui::theme::theme_helpers::ButtonType;

#[derive(Default, Clone)]
pub struct ConfirmationModalOpts {
    pub buttons: Vec<ConfirmationModalButton>,
    pub title: Option<String>,
    pub message: Option<String>,
    pub r#type: Option<MessageType>,
}

#[derive(Clone)]
pub struct ConfirmationModalButton {
    pub label: String,
    pub focus: FocusFlag,
    pub button_type: ButtonType,
}

impl ConfirmationModalButton {
    pub fn new(label: impl Into<String>, focus: FocusFlag, button_type: ButtonType) -> Self {
        Self {
            label: label.into(),
            focus,
            button_type,
        }
    }
}
#[derive(Default, Clone)]
pub struct ConfirmationModalState {
    title: Option<String>,
    message: Option<String>,
    buttons: Vec<ConfirmationModalButton>,
    r#type: Option<MessageType>,

    container_focus: FocusFlag,
}

impl ConfirmationModalState {
    pub fn update_opts(&mut self, opts: ConfirmationModalOpts) {
        self.title = opts.title;
        self.message = opts.message;
        self.r#type = opts.r#type;
        self.buttons = opts.buttons;
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub fn r#type(&self) -> Option<MessageType> {
        self.r#type.clone()
    }

    pub fn buttons(&self) -> &[ConfirmationModalButton] {
        &self.buttons
    }

    pub fn is_button_focused(&self, idx: usize) -> bool {
        self.buttons.get(idx).is_some_and(|button| button.focus.get())
    }
}

impl HasFocus for ConfirmationModalState {
    fn build(&self, builder: &mut FocusBuilder) {
        let start = builder.start(self);

        self.buttons.iter().for_each(|button| {
            builder.leaf_widget(&button.focus);
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
