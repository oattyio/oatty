use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;

use super::super::EnvRow;

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
    pub focus: FocusFlag,
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
            focus: FocusFlag::named("plugins.add"),
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
            }
            AddTransport::Remote => {
                builder.leaf_widget(&self.f_base_url);
            }
        }
        builder.leaf_widget(&self.f_key_value_pairs);

        // Buttons (order matches rendered left→right); enablement handled in UI/actions
        // Secrets is always present
        builder.leaf_widget(&self.f_btn_secrets);
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
