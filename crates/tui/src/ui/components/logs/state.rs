use crate::ui::utils::normalize_result_payload;
use oatty_types::ExecOutcome;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use ratatui::widgets::ListState;
/// The main application state containing all UI data and business logic.
///
/// This struct serves as the central state container for the entire TUI
/// application, managing user interactions, data flow, and UI state.
use serde_json::Value;

/// Structured log entry supporting API responses and plain text.
#[derive(Debug, Clone)]
pub enum LogEntry {
    /// API response entry: keeps HTTP status, raw text, and optional parsed
    /// JSON.
    Api {
        status: u16,
        raw: String,
        json: Option<Value>,
    },
    /// Plain text log: optional level and message.
    Text {
        level: Option<String>,
        msg: String,
    },

    Mcp {
        raw: String,
        json: Option<Value>,
    },
}

/// Selection model for logs supporting single and range selection.
#[derive(Debug, Clone, Copy, Default)]
pub struct Selection {
    pub anchor: usize,
    pub cursor: usize,
}

impl Selection {
    pub fn is_single(&self) -> bool {
        self.anchor == self.cursor
    }
    pub fn range(&self) -> (usize, usize) {
        let start = self.anchor.min(self.cursor);
        let end = self.anchor.max(self.cursor);
        (start, end)
    }
}

/// Detail view mode for an opened log entry.
#[derive(Debug, Clone, Copy)]
pub enum LogDetailView {
    /// Use table view for API responses with tabular JSON; carries scroll
    /// offset.
    Table { offset: usize },
    /// Use a simple text viewer for plain or multi-line selections.
    Text,
}

#[derive(Debug)]
pub struct LogsState {
    pub list_state: ListState,
    pub is_visible: bool,
    /// Structured entries for detail view and rich behavior.
    pub rich_entries: Vec<LogEntry>,
    /// Existing flat string entries used by the current UI list.
    pub entries: Vec<String>,
    /// Current selection (single or range).
    pub selection: Selection,
    /// Optional detail view mode when open.
    pub detail: Option<LogDetailView>,
    /// Pretty-print toggle for single API JSON view/copy.
    pub pretty_json: bool,
    /// Cached redacted JSON for currently opened single API detail (by index).
    pub cached_detail_index: Option<usize>,
    pub cached_redacted_json: Option<Value>,
    /// Focus flag for rat-focus integration
    pub container_focus: FocusFlag,
}

impl LogsState {
    /// Appends a plain-text log entry and keeps the rich entry list aligned.
    ///
    /// The logs view relies on `entries` and `rich_entries` having identical
    /// lengths so that selection indices can map between the flat list and the
    /// richer detail structures. This helper should be used for every textual
    /// log append to guarantee that invariant.
    ///
    /// # Arguments
    ///
    /// * `message` - The human-readable log message to append.
    pub fn append_text_entry(&mut self, message: String) {
        self.append_text_entry_with_level(None, message);
    }

    /// Appends a plain-text log entry with an optional level descriptor.
    ///
    /// # Arguments
    ///
    /// * `level` - Optional severity level (for example, `"warn"`).
    /// * `message` - The human-readable log message to append.
    pub fn append_text_entry_with_level(&mut self, level: Option<String>, message: String) {
        self.entries.push(message.clone());
        self.rich_entries.push(LogEntry::Text { level, msg: message });
    }

    /// Appends an API log entry preserving both raw and structured payloads.
    ///
    /// # Arguments
    ///
    /// * `status` - HTTP status code associated with the log entry.
    /// * `raw` - Raw response text that should appear in the list view.
    /// * `json` - Parsed JSON payload, if available.
    pub fn append_api_entry(&mut self, status: u16, raw: String, json: Option<Value>) {
        self.entries.push(raw.clone());
        self.rich_entries.push(LogEntry::Api { status, raw, json });
    }

    /// Appends an MCP log entry with an optional structured payload.
    ///
    /// # Arguments
    ///
    /// * `raw` - Raw MCP output for list display.
    /// * `json` - Parsed MCP payload, if available.
    pub fn append_mcp_entry(&mut self, raw: String, json: Option<Value>) {
        self.entries.push(raw.clone());
        self.rich_entries.push(LogEntry::Mcp { raw, json });
    }

    /// Toggles the visibility of the logs view.
    pub fn toggle_visible(&mut self) {
        self.is_visible = !self.is_visible;
    }

    /// Processes a general execution result and appends the appropriate log entry.
    ///
    /// # Arguments
    ///
    /// * `execution_outcome` - The execution outcome to process.
    pub(crate) fn process_general_execution_result(&mut self, execution_outcome: &ExecOutcome) {
        match execution_outcome {
            ExecOutcome::Http(status, log, value, ..) => {
                self.append_api_entry(*status, log.clone(), Some(normalize_result_payload(value.clone())));
            }
            ExecOutcome::Mcp(log, value, ..) => {
                self.append_mcp_entry(log.clone(), Some(normalize_result_payload(value.clone())));
            }
            ExecOutcome::PluginDetailLoad(name, ..) => {
                let message = format!("Plugins: loading details for '{}'", name);
                self.append_text_entry(message);
            }
            ExecOutcome::Log(text)
            | ExecOutcome::PluginDetail(text, ..)
            | ExecOutcome::PluginValidationErr(text, ..)
            | ExecOutcome::PluginsRefresh(text, ..)
            | ExecOutcome::PluginValidationOk(text, ..) => {
                self.append_text_entry(text.to_string());
            }
            _ => {}
        }
    }
}

impl Default for LogsState {
    fn default() -> Self {
        let mut state = LogsState {
            list_state: ListState::default(),
            is_visible: false,
            rich_entries: Vec::new(),
            entries: Vec::new(),
            selection: Selection::default(),
            detail: None,
            pretty_json: true,
            cached_detail_index: None,
            cached_redacted_json: None,
            container_focus: FocusFlag::new().with_name("root.logs"),
        };
        state.append_text_entry("Welcome to Oatty TUI".to_string());
        state
    }
}

impl HasFocus for LogsState {
    fn build(&self, builder: &mut FocusBuilder) {
        builder.leaf_widget(self);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
