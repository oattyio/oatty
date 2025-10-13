use crate::ui::components::plugins::{PluginDetailsModalState, logs::PluginLogsState, plugin_editor::state::PluginEditViewState};
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::prelude::Rect;

use super::table::PluginsTableState;

/// UI state for the Plugins view.
#[derive(Debug, Clone)]
pub struct PluginsState {
    pub container_focus: FocusFlag,
    /// Table-specific state including filter, selection, and grid focus.
    pub table: PluginsTableState,
    /// Logs drawer state, if open.
    pub logs: Option<PluginLogsState>,
    /// Add plugin view state
    pub add: Option<PluginEditViewState>,
    /// Whether the plugin logs overlay is currently open
    pub logs_open: bool,
    /// Plugin details modal state, if open
    pub details: Option<PluginDetailsModalState>,
}

impl PluginsState {
    pub fn new() -> Self {
        Self {
            container_focus: FocusFlag::named("plugins"),
            table: PluginsTableState::new(),
            logs: None,
            add: None,
            logs_open: false,
            details: None,
        }
    }

    /// Checks if the add plugin can be opened (no other overlays are open).
    pub fn can_open_add_plugin(&self) -> bool {
        self.logs.is_none()
    }

    pub fn open_logs(&mut self, name: String) {
        self.logs = Some(PluginLogsState::new(name));
    }

    pub fn ensure_details_state(&mut self) -> &mut PluginDetailsModalState {
        if self.details.is_none() {
            self.details = Some(PluginDetailsModalState::new());
        }
        self.details.as_mut().expect("details state should be present")
    }

    pub fn clear_details_state(&mut self) {
        if let Some(details) = &mut self.details {
            details.reset();
        }
        self.details = None;
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
        // Include add plugin view if visible
        if let Some(add) = &self.add {
            builder.widget(add);
        }
        // Header search input and main grid
        builder.widget(&self.table);

        // Include overlays if open
        if let Some(logs) = &self.logs {
            builder.widget(logs);
        }
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
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
        f.focus(&s.table.f_search);
        f.focus(&s.table.f_grid);
    }
}
