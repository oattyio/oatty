//! State for the MCP HTTP server view.

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
    /// Focus flag for the container.
    pub container_focus: FocusFlag,
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
            container_focus: FocusFlag::new().with_name("mcp_http.container"),
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
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
