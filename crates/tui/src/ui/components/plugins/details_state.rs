//! State container for the plugin details modal, including loading lifecycle, tab selection,
//! and the cached data required for rendering the overview/health/environment/logs/tools tabs.

use crate::ui::components::common::ScrollMetrics;
use indexmap::IndexSet;
use oatty_mcp::{EnvVar, McpLogEntry, PluginDetail, PluginToolSummary};

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
    pub environment: IndexSet<EnvVar>,
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
    logs_scroll_metrics: ScrollMetrics,
    tools_scroll_metrics: ScrollMetrics,
}

impl PluginDetailsModalState {
    /// Create a new, idle modal state.
    pub fn new() -> Self {
        Self {
            selected_plugin: None,
            load_state: PluginDetailsLoadState::Idle,
            logs_scroll_metrics: ScrollMetrics::default(),
            tools_scroll_metrics: ScrollMetrics::default(),
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
        self.logs_scroll_metrics.reset();
        self.tools_scroll_metrics.reset();
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

    pub fn logs_scroll_offset(&self) -> u16 {
        self.logs_scroll_metrics.offset()
    }

    pub fn update_logs_viewport_height(&mut self, viewport_height: u16) {
        self.logs_scroll_metrics.update_viewport_height(viewport_height);
    }

    pub fn update_logs_content_height(&mut self, content_height: u16) {
        self.logs_scroll_metrics.update_content_height(content_height);
    }

    pub fn is_logs_scrollable(&self) -> bool {
        self.logs_scroll_metrics.is_scrollable()
    }

    pub fn logs_viewport_height(&self) -> u16 {
        self.logs_scroll_metrics.viewport_height()
    }

    pub fn logs_content_height(&self) -> u16 {
        self.logs_scroll_metrics.content_height()
    }

    pub fn scroll_logs_lines(&mut self, delta: i16) {
        self.logs_scroll_metrics.scroll_lines(delta);
    }

    pub fn scroll_logs_pages(&mut self, delta_pages: i16) {
        self.logs_scroll_metrics.scroll_pages(delta_pages);
    }

    pub fn scroll_logs_to_top(&mut self) {
        self.logs_scroll_metrics.scroll_to_top();
    }

    pub fn scroll_logs_to_bottom(&mut self) {
        self.logs_scroll_metrics.scroll_to_bottom();
    }

    pub fn tools_scroll_offset(&self) -> u16 {
        self.tools_scroll_metrics.offset()
    }

    pub fn update_tools_viewport_height(&mut self, viewport_height: u16) {
        self.tools_scroll_metrics.update_viewport_height(viewport_height);
    }

    pub fn update_tools_content_height(&mut self, content_height: u16) {
        self.tools_scroll_metrics.update_content_height(content_height);
    }

    pub fn is_tools_scrollable(&self) -> bool {
        self.tools_scroll_metrics.is_scrollable()
    }

    pub fn tools_viewport_height(&self) -> u16 {
        self.tools_scroll_metrics.viewport_height()
    }

    pub fn tools_content_height(&self) -> u16 {
        self.tools_scroll_metrics.content_height()
    }

    pub fn scroll_tools_lines(&mut self, delta: i16) {
        self.tools_scroll_metrics.scroll_lines(delta);
    }

    pub fn scroll_tools_pages(&mut self, delta_pages: i16) {
        self.tools_scroll_metrics.scroll_pages(delta_pages);
    }

    pub fn scroll_tools_to_top(&mut self) {
        self.tools_scroll_metrics.scroll_to_top();
    }

    pub fn scroll_tools_to_bottom(&mut self) {
        self.tools_scroll_metrics.scroll_to_bottom();
    }
}

impl Default for PluginDetailsModalState {
    fn default() -> Self {
        Self::new()
    }
}
