use heroku_mcp::PluginDetail;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;

use crate::ui::components::plugins::EnvRow;

use super::key_value_editor::KeyValueEditorState;

/// Add Plugin view state
#[derive(Debug, Clone)]
pub struct PluginEditViewState {
    pub visible: bool,
    /// Selected transport for the plugin: Local (stdio) or Remote (http/sse)
    pub transport: PluginTransport,
    /// Index of the currently focused control (legacy; not used by add.rs)
    pub selected: usize,
    pub name: String,
    pub command: String,
    pub args: String,
    pub base_url: String,
    /// Editor state for environment variables on local transports.
    pub kv_editor: KeyValueEditorState,
    pub validation: Result<String, String>,
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

impl PluginEditViewState {
    pub fn new() -> Self {
        let kv_editor = KeyValueEditorState::new("plugins.add.env");
        let instance = Self {
            visible: true,
            transport: PluginTransport::Local,
            selected: 1,
            name: String::new(),
            command: String::new(),
            args: String::new(),
            base_url: String::new(),
            kv_editor,
            validation: Ok(String::new()),
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

    pub fn from_detail(client: PluginDetail) -> Self {
        let mut instance = Self::new();
        instance.transport = PluginTransport::from(client.transport_type.as_str());
        instance.name = client.name;

        instance.args = client.args.unwrap_or_default();
        instance.kv_editor.rows = client
            .env
            .iter()
            .map(|e| EnvRow {
                key: e.key.clone(),
                value: e.value.clone(),
                is_secret: e.is_secret(),
            })
            .collect();
        if instance.transport == PluginTransport::Local {
            instance.command = client.command_or_url;
        } else {
            instance.base_url = client.command_or_url;
        }
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
            PluginTransport::Local => {
                let command_present = !self.command.trim().is_empty();
                (command_present, name_present && command_present)
            }
            PluginTransport::Remote => {
                let base_url_present = !self.base_url.trim().is_empty();
                (base_url_present, name_present && base_url_present)
            }
        }
    }

    /// Returns the focus flag for the currently active key/value editor container.
    pub fn active_key_value_focus_flag(&self) -> FocusFlag {
        self.kv_editor.focus_flag()
    }

    /// Indicates whether the key/value editor currently holds input focus.
    pub fn is_key_value_editor_focused(&self) -> bool {
        self.active_key_value_focus_flag().get()
    }

    /// Provides a transport-specific label for the key/value table.
    pub fn key_value_table_label(&self) -> &'static str {
        match self.transport {
            PluginTransport::Local => "Env Vars",
            PluginTransport::Remote => "Headers",
        }
    }

    /// Collects key/value pairs for the active transport, excluding empty keys.
    pub fn collected_key_value_pairs(&self) -> Vec<(String, String)> {
        self.kv_editor
            .rows
            .iter()
            .filter(|row| !row.key.trim().is_empty())
            .map(|row| (row.key.trim().to_string(), row.value.clone()))
            .collect()
    }
}

/// Transport selection for Add Plugin view
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginTransport {
    Local,
    Remote,
}

impl From<&str> for PluginTransport {
    fn from(value: &str) -> Self {
        match value {
            "http" => Self::Remote,
            _ => Self::Local,
        }
    }
}

impl HasFocus for PluginEditViewState {
    fn build(&self, builder: &mut FocusBuilder) {
        let (validate_enabled, save_enabled) = self.compute_button_enablement();
        let tag = builder.start(self);
        builder.leaf_widget(&self.f_transport);
        builder.leaf_widget(&self.f_name);
        match self.transport {
            PluginTransport::Local => {
                builder.leaf_widget(&self.f_command);
                builder.leaf_widget(&self.f_args);
            }
            PluginTransport::Remote => {
                builder.leaf_widget(&self.f_base_url);
            }
        }
        builder.widget(&self.kv_editor);

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
