use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
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
    /// Use simple text viewer for plain or multi-line selections.
    Text,
}

#[derive(Debug)]
pub struct LogsState {
    /// Existing flat string entries used by the current UI list.
    pub entries: Vec<String>,
    /// Structured entries for detail view and rich behavior.
    pub rich_entries: Vec<LogEntry>,
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
    pub focus: FocusFlag,
    /// Last rendered rectangle of the detail modal for hit-testing.
    pub detail_rect: Option<Rect>,
}

impl Default for LogsState {
    fn default() -> Self {
        LogsState {
            entries: vec!["Welcome to Heroku TUI".into()],
            rich_entries: Vec::new(),
            selection: Selection::default(),
            detail: None,
            pretty_json: true,
            cached_detail_index: None,
            cached_redacted_json: None,
            focus: FocusFlag::named("root.logs"),
            detail_rect: None,
        }
    }
}

impl HasFocus for LogsState {
    fn build(&self, builder: &mut FocusBuilder) {
        builder.leaf_widget(self);
    }

    fn focus(&self) -> FocusFlag {
        self.focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
