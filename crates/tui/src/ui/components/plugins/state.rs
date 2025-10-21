use super::table::PluginsTableState;
use crate::ui::components::plugins::{PluginDetailsModalState, logs::PluginLogsState, plugin_editor::state::PluginEditViewState};
use heroku_types::{Effect, ExecOutcome, PluginDetail};
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::prelude::Rect;

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

    /// Handles execution completion messages and processes the results.
    ///
    /// This method processes the results of command execution, including
    /// plugin-specific responses, logs updates, and general command results.
    /// It handles special plugin responses and falls back to general result
    /// processing for regular commands.
    ///
    /// # Arguments
    ///
    /// * `execution_outcome` - The result of the command execution
    ///
    /// # Returns
    ///
    /// Returns `true` if the execution was handled as a special case (plugin response)
    /// and the caller should return early, `false` if normal processing should continue.
    pub fn handle_execution_completion(&mut self, execution_outcome: &ExecOutcome) -> Vec<Effect> {
        // Keep executing=true if other executions are still active
        match execution_outcome {
            ExecOutcome::PluginDetailLoad(name, result) => self.handle_plugin_detail_load(name, result.clone()),
            ExecOutcome::PluginDetail(_, maybe_detail) => self.handle_plugin_detail(maybe_detail.clone()),
            ExecOutcome::PluginsRefresh(_, maybe_plugins) => self.handle_plugin_refresh_response(maybe_plugins.clone()),
            _ => {}
        }

        Vec::new()
    }

    /// Handles plugin details responses from command execution.
    ///
    /// # Arguments
    ///
    /// * `log` - The raw log output for redaction
    /// * `maybe_detail` - The plugin detail to apply
    fn handle_plugin_detail(&mut self, maybe_detail: Option<PluginDetail>) {
        let Some(detail) = maybe_detail else {
            return;
        };
        if let Some(state) = self.details.as_mut()
            && state.selected_plugin().is_some_and(|selected| selected == detail.name)
        {
            state.apply_detail(detail.clone());
        }
        self.table.update_item(detail);
    }

    fn handle_plugin_detail_load(&mut self, name: &String, result: anyhow::Result<PluginDetail, String>) {
        match result {
            Ok(detail) => {
                if let Some(state) = self.details.as_mut()
                    && state.selected_plugin().is_some_and(|selected| selected == name)
                {
                    state.apply_detail(detail.clone());
                }
                self.table.update_item(detail);
            }
            Err(error) => {
                if let Some(state) = self.details.as_mut()
                    && state.selected_plugin().is_some_and(|selected| selected == name)
                {
                    state.mark_error(error);
                }
            }
        }
    }

    /// Handles plugin refresh responses from command execution.
    ///
    /// # Arguments
    ///
    /// * `log` - The raw log output for redaction
    /// * `plugin_updates` - The updates to apply
    ///
    fn handle_plugin_refresh_response(&mut self, plugin_updates: Option<Vec<PluginDetail>>) {
        let Some(updated_plugins) = plugin_updates else {
            return;
        };
        self.table.replace_items(updated_plugins);
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
