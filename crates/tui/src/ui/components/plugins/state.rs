use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;

use crate::ui::components::plugins::{PluginSecretsEditorState, logs::PluginLogsState, plugin_editor::state::PluginEditViewState};

use super::table::PluginsTableState;

/// UI state for the Plugins view.
#[derive(Debug, Clone)]
pub struct PluginsState {
    pub focus: FocusFlag,
    /// Table-specific state including filter, selection, and grid focus.
    pub table: PluginsTableState,
    /// Logs drawer state, if open.
    pub logs: Option<PluginLogsState>,
    /// Environment editor state, if open
    pub secrets: Option<PluginSecretsEditorState>,
    /// Add plugin view state
    pub add: Option<PluginEditViewState>,
    /// Whether the plugin logs overlay is currently open
    pub logs_open: bool,
}

impl PluginsState {
    pub fn new() -> Self {
        Self {
            focus: FocusFlag::named("plugins"),
            table: PluginsTableState::new(),
            logs: None,
            secrets: None,
            add: None,
            logs_open: false,
        }
    }

    /// Checks if the add plugin can be opened (no other overlays are open).
    pub fn can_open_add_plugin(&self) -> bool {
        self.secrets.is_none() && self.logs.is_none()
    }

    pub fn open_logs(&mut self, name: String) {
        self.logs = Some(crate::ui::components::plugins::logs::PluginLogsState::new(name));
    }

    pub fn close_logs(&mut self) {
        self.logs = None;
    }

    pub fn open_secrets(&mut self, name: String) {
        self.secrets = Some(crate::ui::components::plugins::secrets::PluginSecretsEditorState::new(name));
    }
    pub fn close_secrets(&mut self) {
        self.secrets = None;
    }
}

impl Default for PluginsState {
    fn default() -> Self {
        Self::new()
    }
}

impl HasFocus for PluginsState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        // Header search input and main grid
        builder.leaf_widget(&self.table.search_flag);
        // Include add plugin view if visible
        if let Some(add) = &self.add {
            builder.widget(add);
        }
        // Include overlays if open
        if let Some(logs) = &self.logs {
            builder.widget(logs);
        }
        if let Some(env) = &self.secrets {
            builder.widget(env);
        }
        builder.leaf_widget(&self.table.grid_flag);
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugins_state_focus_builds() {
        let s = PluginsState::new();
        let mut b = FocusBuilder::new(None);
        b.widget(&s);
        let f = b.build();
        // Sanity: focusing search and grid should be possible
        f.focus(&s.table.search_flag);
        f.focus(&s.table.grid_flag);
    }
}
