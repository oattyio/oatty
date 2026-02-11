use crate::ui::components::common::TextInputState;
use crate::ui::components::results::ResultsTableState;
use crate::ui::utils::normalize_result_payload_owned;
use oatty_mcp::LogLevel;
use oatty_types::ExecOutcome;
use oatty_util::fuzzy_score;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use ratatui::widgets::ListState;
/// The main application state containing all UI data and business logic.
///
/// This struct serves as the central state container for the entire TUI
/// application, managing user interactions, data flow, and UI state.
use serde_json::Value;
use std::fmt::{Display, Formatter};

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
        level: Option<LogLevel>,
        msg: String,
    },

    Mcp {
        raw: String,
        json: Option<Value>,
    },
}

impl Display for LogEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LogEntry::Text { level, msg } => {
                if let Some(level) = level {
                    write!(f, "[{}] {}", level, msg)
                } else {
                    write!(f, "{}", msg)
                }
            }
            LogEntry::Api { status, raw, .. } => {
                write!(f, "[{}] {}", status, raw)
            }
            LogEntry::Mcp { raw, .. } => {
                write!(f, "[MCP] {}", raw)
            }
        }
    }
}

#[derive(Debug)]
pub struct LogsState {
    pub list_state: ListState,
    pub results_table: ResultsTableState<'static>,
    pub is_visible: bool,
    /// Structured entries for detail view and rich behavior.
    pub rich_entries: Vec<LogEntry>,
    /// Pretty-print toggle for a single API JSON view /copy.
    pub pretty_json: bool,
    /// Focus flag for rat-focus integration
    pub container_focus: FocusFlag,
    /// Focus flag for the logs list viewport.
    pub f_list: FocusFlag,
    search_input: TextInputState,
    filtered_indices: Vec<usize>,
    search_active: bool,
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
    pub fn append_text_entry_with_level(&mut self, level: Option<LogLevel>, message: String) {
        self.rich_entries.push(LogEntry::Text { level, msg: message });
        self.update_filtered_entries();
    }

    /// Appends an API log entry preserving both raw and structured payloads.
    ///
    /// # Arguments
    ///
    /// * `status` - HTTP status code associated with the log entry.
    /// * `raw` - Raw response text that should appear in the list view.
    /// * `json` - Parsed JSON payload, if available.
    pub fn append_api_entry(&mut self, status: u16, raw: String, json: Option<Value>) {
        self.rich_entries.push(LogEntry::Api { status, raw, json });
        self.update_filtered_entries();
    }

    /// Appends an MCP log entry with an optional structured payload.
    ///
    /// # Arguments
    ///
    /// * `raw` - Raw MCP output for list display.
    /// * `json` - Parsed MCP payload, if available.
    pub fn append_mcp_entry(&mut self, raw: String, json: Option<Value>) {
        self.rich_entries.push(LogEntry::Mcp { raw, json });
        self.update_filtered_entries();
    }

    /// Toggles the visibility of the logs view.
    pub fn toggle_visible(&mut self) {
        self.is_visible = !self.is_visible;
    }

    pub fn activate_search(&mut self) {
        self.search_active = true;
    }

    pub fn deactivate_search(&mut self) {
        self.search_active = false;
    }

    pub fn is_search_active(&self) -> bool {
        self.search_active
    }

    pub fn has_search_query(&self) -> bool {
        !self.search_input.input().trim().is_empty()
    }

    pub fn search_query(&self) -> &str {
        self.search_input.input()
    }

    pub fn search_cursor_columns(&self) -> usize {
        self.search_input.cursor_columns()
    }

    pub fn set_search_cursor_from_column(&mut self, column: u16) {
        let cursor = self.search_input.cursor_index_for_column(column);
        self.search_input.set_cursor(cursor);
    }

    pub fn move_search_cursor_left(&mut self) {
        self.search_input.move_left();
    }

    pub fn move_search_cursor_right(&mut self) {
        self.search_input.move_right();
    }

    pub fn append_search_character(&mut self, character: char) {
        self.search_input.insert_char(character);
        self.update_filtered_entries();
    }

    pub fn remove_search_character(&mut self) {
        self.search_input.backspace();
        self.update_filtered_entries();
    }

    pub fn clear_search_query(&mut self) {
        if self.search_input.input().is_empty() && self.search_input.cursor() == 0 {
            return;
        }
        self.search_input.set_input("");
        self.search_input.set_cursor(0);
        self.update_filtered_entries();
    }

    pub fn filtered_indices(&self) -> &[usize] {
        &self.filtered_indices
    }

    pub fn selected_rich_index(&self) -> Option<usize> {
        let selected_filtered_index = self.list_state.selected()?;
        self.filtered_indices.get(selected_filtered_index).copied()
    }

    fn update_filtered_entries(&mut self) {
        let previous_selected = self.selected_rich_index();
        let query = self.search_input.input();
        self.list_state = self.list_state.with_offset(0);

        if query.trim().is_empty() {
            self.filtered_indices = (0..self.rich_entries.len()).collect();
        } else {
            let mut scored: Vec<(i64, usize)> = self
                .rich_entries
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| fuzzy_score(&entry.to_string(), query).map(|score| (score, index)))
                .collect();
            scored.sort_by(|left, right| right.0.cmp(&left.0));
            self.filtered_indices = scored.into_iter().map(|(_, index)| index).collect();
        }

        if self.filtered_indices.is_empty() {
            self.list_state.select(None);
            return;
        }

        if let Some(previous_selected_index) = previous_selected
            && let Some(filtered_position) = self.filtered_indices.iter().position(|index| *index == previous_selected_index)
        {
            self.list_state.select(Some(filtered_position));
            return;
        }

        self.list_state.select(Some(0));
    }

    /// Processes a general execution result and appends the appropriate log entry.
    ///
    /// # Arguments
    ///
    /// * `execution_outcome` - The execution outcome to process.
    pub(crate) fn process_general_execution_result(&mut self, execution_outcome: &ExecOutcome) {
        match execution_outcome {
            ExecOutcome::Http {
                status_code,
                log_entry,
                payload,
                ..
            } => {
                self.append_api_entry(
                    *status_code,
                    log_entry.clone(),
                    Some(normalize_result_payload_owned(payload.clone())),
                );
            }
            ExecOutcome::Mcp { log_entry, payload, .. } => {
                self.append_mcp_entry(log_entry.clone(), Some(normalize_result_payload_owned(payload.clone())));
            }
            ExecOutcome::PluginDetailLoad { plugin_name, .. } => {
                let message = format!("Plugins: loading details for '{}'", plugin_name);
                self.append_text_entry(message);
            }
            ExecOutcome::Log(text)
            | ExecOutcome::PluginDetail { message: text, .. }
            | ExecOutcome::PluginValidationErr { message: text }
            | ExecOutcome::PluginValidationOk { message: text } => {
                self.append_text_entry(text.clone());
            }
            ExecOutcome::RegistryCatalogGenerated(catalog) => {
                self.append_text_entry(format!("The '{}' catalog was generated successfully", catalog.title))
            }
            ExecOutcome::RegistryCatalogGenerationError(err) | ExecOutcome::RegistryConfigSaveError(err) => {
                self.append_text_entry_with_level(Some(LogLevel::Error), err.to_string())
            }
            ExecOutcome::WorkflowImported { workflow_id, path } => {
                self.append_text_entry(format!("Workflow '{}' imported successfully at '{}'", workflow_id, path.display()));
            }
            ExecOutcome::WorkflowRemoved { workflow_id } => {
                self.append_text_entry(format!("Workflow '{}' removed successfully", workflow_id));
            }
            ExecOutcome::WorkflowOperationError(error) => {
                self.append_text_entry_with_level(Some(LogLevel::Error), error.clone());
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
            pretty_json: true,
            container_focus: FocusFlag::new().with_name("root.logs"),
            f_list: FocusFlag::new().with_name("root.logs.list"),
            results_table: ResultsTableState::default(),
            search_input: TextInputState::new(),
            filtered_indices: Vec::new(),
            search_active: false,
        };
        state.append_text_entry("Welcome to Oatty TUI".to_string());
        state.update_filtered_entries();
        state
    }
}

impl HasFocus for LogsState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.leaf_widget(&self.f_list);
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
