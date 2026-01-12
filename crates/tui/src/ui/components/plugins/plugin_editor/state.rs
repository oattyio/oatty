use std::borrow::Cow;

use crate::ui::components::common::key_value_editor::KeyValueEditorState;
use oatty_mcp::PluginDetail;
use oatty_types::value_objects::EnvRow;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;

/// Add Plugin view state
#[derive(Debug, Default)]
pub struct PluginEditViewState {
    pub visible: bool,
    /// Selected transport for the plugin: Local (stdio) or Remote (http/sse)
    pub transport: PluginTransport,
    pub name: String,
    /// Remembers the original plugin name when editing an existing entry.
    pub original_name: Option<String>,
    pub name_cursor: usize,
    pub command: String,
    pub command_cursor: usize,
    pub args: String,
    pub args_cursor: usize,
    pub base_url: String,
    pub base_url_cursor: usize,
    /// Editor state for environment variables on local transports.
    pub kv_editor: KeyValueEditorState,
    // Focus flags for focusable controls
    pub container_focus: FocusFlag,
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
        let instance = Self {
            visible: true,
            container_focus: FocusFlag::new().with_name("plugins.add"),
            f_transport: FocusFlag::new().with_name("plugins.add.transport"),
            f_name: FocusFlag::new().with_name("plugins.add.name"),
            f_command: FocusFlag::new().with_name("plugins.add.command"),
            f_args: FocusFlag::new().with_name("plugins.add.args"),
            f_base_url: FocusFlag::new().with_name("plugins.add.base_url"),
            f_btn_validate: FocusFlag::new().with_name("plugins.add.btn.validate"),
            f_btn_save: FocusFlag::new().with_name("plugins.add.btn.save"),
            f_btn_cancel: FocusFlag::new().with_name("plugins.add.btn.cancel"),
            ..Default::default()
        };
        // Set initial focus to transport selector instead of name field
        instance.f_transport.set(true);
        instance
    }

    pub fn from_detail(client: PluginDetail) -> Self {
        let mut instance = Self::new();
        instance.transport = PluginTransport::from(client.transport_type.as_str());
        instance.original_name = Some(client.name.clone());
        instance.name = client.name.clone();
        instance.name_cursor = instance.name.len();

        instance.args = client.args.unwrap_or_default();
        instance.args_cursor = instance.args.len();
        let rows: Vec<EnvRow> = client
            .env
            .iter()
            .map(|e| EnvRow {
                key: e.key.clone(),
                value: e.value.clone(),
                is_secret: e.is_secret(),
            })
            .collect();
        instance.kv_editor.set_rows(rows);
        if instance.transport == PluginTransport::Local {
            instance.command = client.command_or_url;
            instance.command_cursor = instance.command.len();
        } else {
            instance.base_url = client.command_or_url;
            instance.base_url_cursor = instance.base_url.len();
        }
        instance
    }

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

    /// Provides a transport-specific label for the key/value table.
    pub fn update_key_value_table_label(&mut self) {
        let label = match self.transport {
            PluginTransport::Local => Cow::from("Env Vars"),
            PluginTransport::Remote => Cow::from("Headers"),
        };

        self.kv_editor.set_block_label(label);
    }
}

/// Transport selection for Add Plugin view
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum PluginTransport {
    #[default]
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
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
