use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;

use super::super::state::EnvRow;

/// Add Plugin view state
#[derive(Debug, Clone)]
pub struct PluginAddViewState {
    pub visible: bool,
    /// Selected transport for the plugin: Local (stdio) or Remote (http/sse)
    pub transport: AddTransport,
    /// Index of the currently focused control (legacy; not used by add.rs)
    pub selected: usize,
    pub name: String,
    pub command: String,
    pub args: String,
    pub base_url: String,
    /// Optional environment variables for Local (stdio) plugins as key/value rows.
    pub env: Vec<EnvRow>,
    /// Inline input for environment variables when adding a Local plugin.
    pub env_input: String,
    /// Inline input for HTTP headers when adding a Remote plugin.
    pub headers_input: String,
    pub editing: bool,
    pub input: String,
    pub validation: Option<String>,
    pub preview: Option<String>,
    // Focus flags for focusable controls
    pub f_transport: FocusFlag,
    pub f_name: FocusFlag,
    pub f_command: FocusFlag,
    pub f_args: FocusFlag,
    pub f_base_url: FocusFlag,
    pub f_key_value_pairs: FocusFlag,
    pub f_btn_secrets: FocusFlag,
    pub f_btn_validate: FocusFlag,
    pub f_btn_save: FocusFlag,
    pub f_btn_cancel: FocusFlag,
}

impl PluginAddViewState {
    pub fn new() -> Self {
        let instance = Self {
            visible: true,
            transport: AddTransport::Local,
            selected: 1,
            name: String::new(),
            command: String::new(),
            args: String::new(),
            base_url: String::new(),
            env: Vec::new(),
            env_input: String::new(),
            headers_input: String::new(),
            editing: false,
            input: String::new(),
            validation: None,
            preview: None,
            f_transport: FocusFlag::named("plugins.add.transport"),
            f_name: FocusFlag::named("plugins.add.name"),
            f_command: FocusFlag::named("plugins.add.command"),
            f_args: FocusFlag::named("plugins.add.args"),
            f_base_url: FocusFlag::named("plugins.add.base_url"),
            f_key_value_pairs: FocusFlag::named("plugins.add.key_value_pairs"),
            f_btn_secrets: FocusFlag::named("plugins.add.btn.secrets"),
            f_btn_validate: FocusFlag::named("plugins.add.btn.validate"),
            f_btn_save: FocusFlag::named("plugins.add.btn.save"),
            f_btn_cancel: FocusFlag::named("plugins.add.btn.cancel"),
        };
        instance.f_name.set(true);
        instance
    }
}

/// Transport selection for Add Plugin view
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddTransport {
    Local,
    Remote,
}

impl PluginAddViewState {
    /// Synchronize `selected` index from which focus flag is active.
    pub fn sync_selected_from_focus(&mut self) {
        self.selected = if self.f_transport.get() {
            0
        } else if self.f_name.get() {
            1
        } else if self.f_command.get() {
            2
        } else if self.f_args.get() {
            3
        } else if self.f_base_url.get() {
            4
        } else if self.f_key_value_pairs.get() {
            5
        } else if self.f_btn_validate.get() {
            7
        } else if self.f_btn_save.get() {
            8
        } else if self.f_btn_cancel.get() {
            9
        } else {
            1
        };
    }
}

// Minimal leaf wrapper for rat-focus usage, if needed externally
struct PanelLeaf(FocusFlag);
impl HasFocus for PanelLeaf {
    fn build(&self, builder: &mut FocusBuilder) {
        builder.leaf_widget(self);
    }
    fn focus(&self) -> FocusFlag {
        self.0.clone()
    }
    fn area(&self) -> Rect {
        Rect::default()
    }
}

impl HasFocus for PluginAddViewState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.widget(&PanelLeaf(self.f_transport.clone()));
        builder.widget(&PanelLeaf(self.f_name.clone()));
        match self.transport {
            AddTransport::Local => {
                builder.widget(&PanelLeaf(self.f_command.clone()));
                builder.widget(&PanelLeaf(self.f_args.clone()));
            }
            AddTransport::Remote => {
                builder.widget(&PanelLeaf(self.f_base_url.clone()));
            }
        }
        builder.widget(&PanelLeaf(self.f_key_value_pairs.clone()));

        // Buttons depending on enablement
        let name_present = !self.name.trim().is_empty();
        let (validate_enabled, save_enabled) = match self.transport {
            AddTransport::Local => {
                let command_present = !self.command.trim().is_empty();
                (command_present, name_present && command_present)
            }
            AddTransport::Remote => {
                let base_url_present = !self.base_url.trim().is_empty();
                (base_url_present, name_present && base_url_present)
            }
        };
        if validate_enabled {
            builder.widget(&PanelLeaf(self.f_btn_validate.clone()));
        }
        if save_enabled {
            builder.widget(&PanelLeaf(self.f_btn_save.clone()));
        }
        // Cancel always present
        builder.widget(&PanelLeaf(self.f_btn_secrets.clone()));
        builder.widget(&PanelLeaf(self.f_btn_cancel.clone()));

        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.f_name.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
