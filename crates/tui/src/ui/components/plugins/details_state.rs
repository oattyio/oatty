//! State container for the plugin details modal, including loading lifecycle, tab selection,
//! and the cached data required for rendering the overview/health/environment/logs/tools tabs.

use heroku_mcp::{EnvVar, McpLogEntry, PluginDetail, PluginToolSummary};

/// Loading lifecycle for the plugin details payload.
#[derive(Debug, Clone)]
pub enum PluginDetailsLoadState {
    Idle,
    Loading,
    Loaded(Box<PluginDetailsData>),
    Error(String),
}

impl PluginDetailsLoadState {
    /// Returns true when the last fetch attempt failed.
    pub fn is_error(&self) -> bool {
        matches!(self, PluginDetailsLoadState::Error(_))
    }
}

/// Cached plugin detail payload used by the modal tabs.
#[derive(Debug, Clone)]
pub struct PluginDetailsData {
    pub detail: PluginDetail,
    pub logs: Vec<McpLogEntry>,
    pub environment: Vec<EnvVar>,
    pub tools: Vec<PluginToolSummary>,
}

impl PluginDetailsData {
    /// Construct a details payload from a `PluginDetail`, normalizing the log list to the
    /// most recent entries while preserving the original detail structure for downstream use.
    pub fn new(mut detail: PluginDetail) -> Self {
        let logs: Vec<McpLogEntry> = detail
            .logs
            .iter()
            .rev()
            .take(50)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        detail.logs = logs.clone();

        let environment = detail.env.clone();
        let tools = detail.tools.clone();

        Self {
            detail,
            logs,
            environment,
            tools,
        }
    }
}

/// UI state persisted while the "plugin details" modal is open.
#[derive(Debug, Clone)]
pub struct PluginDetailsModalState {
    selected_plugin: Option<String>,
    load_state: PluginDetailsLoadState,
    logs_scroll: usize,
}

impl PluginDetailsModalState {
    /// Create a new, idle modal state.
    pub fn new() -> Self {
        Self {
            selected_plugin: None,
            load_state: PluginDetailsLoadState::Idle,
            logs_scroll: 0,
        }
    }

    /// Currently selected plugin name, if any.
    pub fn selected_plugin(&self) -> Option<&str> {
        self.selected_plugin.as_deref()
    }

    /// Current load state for the modal payload.
    pub fn load_state(&self) -> &PluginDetailsLoadState {
        &self.load_state
    }

    /// Transition to the loading state for a new plugin selection.
    pub fn begin_load(&mut self, plugin_name: String) {
        self.selected_plugin = Some(plugin_name);
        self.logs_scroll = 0;
        self.load_state = PluginDetailsLoadState::Loading;
    }

    /// Record an error for the most recent fetch attempt.
    pub fn mark_error<S: Into<String>>(&mut self, message: S) {
        self.load_state = PluginDetailsLoadState::Error(message.into());
    }

    /// Store the successfully fetched plugin detail payload.
    pub fn apply_detail(&mut self, detail: PluginDetail) {
        self.load_state = PluginDetailsLoadState::Loaded(Box::new(PluginDetailsData::new(detail)));
    }

    /// Reset modal state back to the idle baseline.
    pub fn reset(&mut self) {
        self.selected_plugin = None;
        self.load_state = PluginDetailsLoadState::Idle;
        self.logs_scroll = 0;
    }

    pub fn logs_scroll(&self) -> usize {
        self.logs_scroll
    }

    pub fn scroll_logs_up(&mut self, amount: usize) {
        self.logs_scroll = self.logs_scroll.saturating_sub(amount);
    }

    pub fn scroll_logs_down(&mut self, amount: usize, max_scroll: usize) {
        self.logs_scroll = (self.logs_scroll + amount).min(max_scroll);
    }
}

impl Default for PluginDetailsModalState {
    fn default() -> Self {
        Self::new()
    }
}
