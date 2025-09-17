use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;

/// Environment editor state for a plugin
#[derive(Debug, Default, Clone)]
pub struct PluginSecretsEditorState {
    pub name: String,
    pub rows: Vec<EnvRow>,
    pub selected: usize,
    pub editing: bool,
    pub input: String,
    /// Root focus flag for env overlay
    pub focus: FocusFlag,
}

#[derive(Debug, Clone)]
pub struct EnvRow {
    pub key: String,
    pub value: String,
    pub is_secret: bool,
}

impl PluginSecretsEditorState {
    pub fn new(name: String) -> Self {
        Self {
            name,
            rows: Vec::new(),
            selected: 0,
            editing: false,
            input: String::new(),
            focus: FocusFlag::named("plugins.env"),
        }
    }
    pub fn set_rows(&mut self, rows: Vec<EnvRow>) {
        self.rows = rows;
        self.selected = 0;
        self.editing = false;
        self.input.clear();
    }
}

impl HasFocus for PluginSecretsEditorState {
    fn build(&self, builder: &mut FocusBuilder) {
        builder.leaf_widget(self);
    }
    fn focus(&self) -> FocusFlag {
        self.focus.clone()
    }
    fn area(&self) -> Rect {
        Rect::default()
    }
}
