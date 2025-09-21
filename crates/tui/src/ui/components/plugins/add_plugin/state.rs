use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;

use super::super::EnvRow;
use super::key_value_editor::KeyValueEditorState;

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
    /// Editor state for environment variables on local transports.
    pub env_editor: KeyValueEditorState,
    /// Editor state for headers on remote transports.
    pub header_editor: KeyValueEditorState,
    pub validation: Option<String>,
    pub preview: Option<String>,
    // Focus flags for focusable controls
    pub focus: FocusFlag,
    pub f_transport: FocusFlag,
    pub f_name: FocusFlag,
    pub f_command: FocusFlag,
    pub f_args: FocusFlag,
    pub f_base_url: FocusFlag,
    pub f_btn_validate: FocusFlag,
    pub f_btn_save: FocusFlag,
    pub f_btn_cancel: FocusFlag,
}

impl PluginAddViewState {
    pub fn new() -> Self {
        let env_editor = KeyValueEditorState::new("plugins.add.env");
        let header_editor = KeyValueEditorState::new("plugins.add.header");
        let instance = Self {
            visible: true,
            transport: AddTransport::Local,
            selected: 1,
            name: String::new(),
            command: String::new(),
            args: String::new(),
            base_url: String::new(),
            env_editor,
            header_editor,
            validation: None,
            preview: None,
            focus: FocusFlag::named("plugins.add"),
            f_transport: FocusFlag::named("plugins.add.transport"),
            f_name: FocusFlag::named("plugins.add.name"),
            f_command: FocusFlag::named("plugins.add.command"),
            f_args: FocusFlag::named("plugins.add.args"),
            f_base_url: FocusFlag::named("plugins.add.base_url"),
            f_btn_validate: FocusFlag::named("plugins.add.btn.validate"),
            f_btn_save: FocusFlag::named("plugins.add.btn.save"),
            f_btn_cancel: FocusFlag::named("plugins.add.btn.cancel"),
        };
        // Set initial focus to transport selector instead of name field
        instance.f_transport.set(true);
        instance
    }

    ///
    /// This component now relies directly on `FocusFlag` booleans exposed on
    /// `PluginAddViewState` to route keyboard input and rendering focus. This
    /// avoids building a `FocusRing` repeatedly and eliminates the `AddControl`
    /// enum indirection that previously mapped focus flags to a variant.
    /// All event handling and rendering paths read `f_*` flags directly.
    // Removed `AddControl` enum in favor of direct focus-flag checks.

    /// Computes the enablement state of the Validate and Save buttons.
    ///
    /// This function determines whether the Validate and Save buttons should be
    /// enabled based on the current form state and transport type. The Validate
    /// button is enabled when the required fields for the current transport are
    /// filled, and the Save button is enabled when both the name and transport-specific
    /// fields are complete.
    ///
    /// # Arguments
    ///
    /// * `add_state` - Reference to the add plugin plugin state
    ///
    /// # Returns
    ///
    /// Returns a tuple `(validate_enabled, save_enabled)` indicating which buttons
    /// should be enabled.
    pub fn compute_button_enablement(&self) -> (bool, bool) {
        let name_present = !self.name.trim().is_empty();

        match self.transport {
            AddTransport::Local => {
                let command_present = !self.command.trim().is_empty();
                (command_present, name_present && command_present)
            }
            AddTransport::Remote => {
                let base_url_present = !self.base_url.trim().is_empty();
                (base_url_present, name_present && base_url_present)
            }
        }
    }

    /// Returns the editor that should be used for the active transport mode.
    pub fn active_key_value_editor(&self) -> &KeyValueEditorState {
        match self.transport {
            AddTransport::Local => &self.env_editor,
            AddTransport::Remote => &self.header_editor,
        }
    }

    /// Returns the editor that should be mutated for the active transport mode.
    pub fn active_key_value_editor_mut(&mut self) -> &mut KeyValueEditorState {
        match self.transport {
            AddTransport::Local => &mut self.env_editor,
            AddTransport::Remote => &mut self.header_editor,
        }
    }

    /// Returns the focus flag for the currently active key/value editor container.
    pub fn active_key_value_focus_flag(&self) -> FocusFlag {
        self.active_key_value_editor().focus_flag()
    }

    /// Indicates whether the key/value editor currently holds input focus.
    pub fn is_key_value_editor_focused(&self) -> bool {
        self.active_key_value_focus_flag().get()
    }

    /// Replaces all environment variable rows.
    pub fn replace_environment_rows(&mut self, rows: Vec<EnvRow>) {
        self.env_editor.rows = rows;
        self.env_editor.selected_row_index = None;
        self.env_editor.mode = Default::default();
        self.env_editor.focus_table_surface();
    }

    /// Replaces all header rows for remote transports.
    pub fn replace_header_rows(&mut self, rows: Vec<EnvRow>) {
        self.header_editor.rows = rows;
        self.header_editor.selected_row_index = None;
        self.header_editor.mode = Default::default();
        self.header_editor.focus_table_surface();
    }

    /// Provides a transport-specific label for the key/value table.
    pub fn key_value_table_label(&self) -> &'static str {
        match self.transport {
            AddTransport::Local => "Env Vars",
            AddTransport::Remote => "Headers",
        }
    }

    /// Collects key/value pairs for the active transport, excluding empty keys.
    pub fn collected_key_value_pairs(&self) -> Vec<(String, String)> {
        self.active_key_value_editor()
            .rows
            .iter()
            .filter(|row| !row.key.trim().is_empty())
            .map(|row| (row.key.trim().to_string(), row.value.clone()))
            .collect()
    }
}

/// Transport selection for Add Plugin view
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddTransport {
    Local,
    Remote,
}

impl HasFocus for PluginAddViewState {
    fn build(&self, builder: &mut FocusBuilder) {
        let (validate_enabled, save_enabled) = self.compute_button_enablement();
        let tag = builder.start(self);
        builder.leaf_widget(&self.f_transport);
        builder.leaf_widget(&self.f_name);
        match self.transport {
            AddTransport::Local => {
                builder.leaf_widget(&self.f_command);
                builder.leaf_widget(&self.f_args);
                builder.widget(&self.env_editor);
            }
            AddTransport::Remote => {
                builder.leaf_widget(&self.f_base_url);
                builder.widget(&self.header_editor);
            }
        }

        // Buttons (order matches rendered left→right); enablement handled in UI/actions
        // Secrets is always present
        // Validate / Save are not always part of the focus ring; enablement is handled in UI/actions
        if validate_enabled {
            builder.leaf_widget(&self.f_btn_validate);
        }
        if save_enabled {
            builder.leaf_widget(&self.f_btn_save);
        }
        // Cancel always present
        builder.leaf_widget(&self.f_btn_cancel);

        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
