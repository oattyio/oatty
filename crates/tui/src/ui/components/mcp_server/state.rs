//! State for the MCP HTTP server view.

use std::borrow::Cow;

use crate::ui::components::common::ScrollMetrics;
use oatty_types::{MessageType, TransientMessage};
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;

/// High-level lifecycle status for the local MCP HTTP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpHttpServerStatus {
    /// Server is not running.
    Stopped,
    /// Server is starting.
    Starting,
    /// Server is running and accepting connections.
    Running,
    /// Server is stopping.
    Stopping,
    /// Server encountered an error.
    Error,
}

impl McpHttpServerStatus {
    /// Return a human-readable label for the status.
    pub fn label(&self) -> &'static str {
        match self {
            McpHttpServerStatus::Stopped => "Stopped",
            McpHttpServerStatus::Starting => "Starting",
            McpHttpServerStatus::Running => "Running",
            McpHttpServerStatus::Stopping => "Stopping",
            McpHttpServerStatus::Error => "Error",
        }
    }
}

/// UI state backing the MCP HTTP server view.
#[derive(Debug)]
pub struct McpHttpServerState {
    /// Current status of the server.
    pub status: McpHttpServerStatus,
    /// Whether the server should auto-start with the TUI.
    pub auto_start: bool,
    /// Configured bind address (for example, "127.0.0.1:0").
    pub configured_bind_address: String,
    /// Bound address when the server is running.
    pub bound_address: Option<String>,
    /// Latest observed connected client count.
    pub connected_clients: usize,
    /// Last error message captured from the server lifecycle.
    pub last_error: Option<String>,
    /// Focus flag for the start/stop button.
    pub start_stop_focus: FocusFlag,
    /// Focus flag for the auto-start checkbox.
    pub auto_start_focus: FocusFlag,
    /// Focus flag for the right-side client config list.
    pub config_list_focus: FocusFlag,
    /// Focus flag for the container.
    pub container_focus: FocusFlag,
    /// Selected index within the rendered client config snippets.
    pub selected_config_index: usize,
    /// Scroll metrics for the client config snippet list.
    config_scroll_metrics: ScrollMetrics,
    /// Transient status message shown in the MCP server view.
    pub message: Option<TransientMessage>,
}

impl McpHttpServerState {
    /// Toggle the auto-start setting.
    pub fn toggle_auto_start(&mut self) {
        self.auto_start = !self.auto_start;
    }

    /// Mark the server as starting.
    pub fn mark_starting(&mut self) {
        self.status = McpHttpServerStatus::Starting;
        self.last_error = None;
    }

    /// Mark the server as stopping.
    pub fn mark_stopping(&mut self) {
        self.status = McpHttpServerStatus::Stopping;
    }

    /// Mark the server as running and update the bound address.
    pub fn mark_running(&mut self, bound_address: String) {
        self.status = McpHttpServerStatus::Running;
        self.bound_address = Some(bound_address);
        self.last_error = None;
    }

    /// Mark the server as stopped and clear runtime details.
    pub fn mark_stopped(&mut self) {
        self.status = McpHttpServerStatus::Stopped;
        self.bound_address = None;
        self.connected_clients = 0;
    }

    /// Mark the server as errored with a descriptive message.
    pub fn mark_error(&mut self, message: String) {
        self.status = McpHttpServerStatus::Error;
        self.last_error = Some(message);
    }

    /// Update the configured bind address string.
    pub fn set_configured_bind_address(&mut self, address: String) {
        self.configured_bind_address = address;
    }

    /// Update the live connected client count.
    pub fn update_connected_clients(&mut self, count: usize) {
        self.connected_clients = count;
    }

    /// Set the currently selected client config snippet index.
    pub fn set_selected_config_index(&mut self, index: usize) {
        self.selected_config_index = index;
    }

    /// Returns the current config list scroll offset.
    pub fn config_scroll_offset(&self) -> u16 {
        self.config_scroll_metrics.offset()
    }

    /// Returns the current config list viewport height.
    pub fn config_viewport_height(&self) -> u16 {
        self.config_scroll_metrics.viewport_height()
    }

    /// Updates the config list viewport height and clamps offset.
    pub fn update_config_viewport_height(&mut self, viewport_height: u16) {
        self.config_scroll_metrics.update_viewport_height(viewport_height);
    }

    /// Updates the config list content height and clamps offset.
    pub fn update_config_content_height(&mut self, content_height: u16) {
        self.config_scroll_metrics.update_content_height(content_height);
    }

    /// Sets the config list scroll offset directly.
    pub fn set_config_scroll_offset(&mut self, offset: u16) {
        self.config_scroll_metrics.scroll_to_top();
        self.config_scroll_metrics.scroll_lines(offset as i16);
    }

    /// Scrolls the config list by relative line units.
    pub fn scroll_config_lines(&mut self, delta: i16) {
        self.config_scroll_metrics.scroll_lines(delta);
    }

    /// Scrolls the config list by viewport pages.
    pub fn scroll_config_pages(&mut self, delta_pages: i16) {
        self.config_scroll_metrics.scroll_pages(delta_pages);
    }

    /// Set a transient message for display in the server view.
    pub fn set_message(&mut self, message: Option<TransientMessage>) {
        self.message = message;
    }

    /// Convenience helper for success notifications.
    pub fn set_success_message(&mut self, message: Cow<'static, str>) {
        self.message = Some(TransientMessage::new(
            message,
            MessageType::Success,
            std::time::Duration::from_millis(2200),
        ));
    }

    /// Returns the current transient message when present.
    pub fn message_ref(&self) -> Option<&TransientMessage> {
        self.message.as_ref()
    }
}

impl Default for McpHttpServerState {
    fn default() -> Self {
        let state = Self {
            status: McpHttpServerStatus::Stopped,
            auto_start: false,
            configured_bind_address: "127.0.0.1:62889".to_string(),
            bound_address: None,
            connected_clients: 0,
            last_error: None,
            start_stop_focus: FocusFlag::new().with_name("mcp_http.start_stop"),
            auto_start_focus: FocusFlag::new().with_name("mcp_http.auto_start"),
            config_list_focus: FocusFlag::new().with_name("mcp_http.config_list"),
            container_focus: FocusFlag::new().with_name("mcp_http.container"),
            selected_config_index: 0,
            config_scroll_metrics: ScrollMetrics::default(),
            message: None,
        };
        state.start_stop_focus.set(true);
        state
    }
}

impl HasFocus for McpHttpServerState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.leaf_widget(&self.start_stop_focus);
        builder.leaf_widget(&self.auto_start_focus);
        builder.leaf_widget(&self.config_list_focus);
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
