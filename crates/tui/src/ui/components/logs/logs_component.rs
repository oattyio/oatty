//! Logs component for displaying and interacting with application logs.
//!
//! This component provides a comprehensive interface for viewing, navigating,
//! and interacting with log entries. It supports:
//!
//! - **Multi-selection**: Users can select single or multiple log entries
//! - **Detail views**: JSON logs can be viewed in formatted detail modals
//! - **Table integration**: Array JSON data is routed to the global table
//!   component
//! - **Copy functionality**: Selected logs can be copied to clipboard
//! - **Syntax highlighting**: Timestamps, UUIDs, and hex IDs are styled
//! - **Security**: Sensitive data is automatically redacted
//!
//! The component follows the TEA (The Elm Architecture) pattern and integrates
//! with the application's focus management system.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use heroku_types::{Effect, ExecOutcome, Modal, Msg};
use heroku_util::redact_json;
use once_cell::sync::Lazy;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::*,
};
use regex::Regex;
use serde_json::Value;

use super::{
    LogDetailsComponent,
    state::{LogDetailView, LogEntry},
};
use crate::app::App;
use crate::ui::{
    components::component::Component,
    theme::{roles::Theme as UiTheme, theme_helpers as th},
    utils::build_copy_text,
};

/// Component for displaying and interacting with application logs.
///
/// The LogsComponent provides a rich interface for viewing log entries with
/// support for selection, detail views, and various interaction modes. It
/// automatically handles data redaction for security and provides visual
/// enhancements for better readability.
#[derive(Debug, Default)]
pub struct LogsComponent;

impl LogsComponent {
    // ============================================================================
    // Selection and Navigation Methods
    // ============================================================================

    /// Gets the currently selected log entry index.
    ///
    /// Returns `None` if there are no log entries, otherwise returns the cursor
    /// position clamped to valid bounds.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state containing log entries and selection
    ///
    /// # Returns
    ///
    /// * `Some(usize)` - The selected index if entries exist
    /// * `None` - If no entries are available
    fn selected_index(&self, app: &App) -> Option<usize> {
        if app.logs.entries.is_empty() {
            None
        } else {
            Some(app.logs.selection.cursor.min(app.logs.entries.len() - 1))
        }
    }

    /// Moves the cursor by the specified delta and updates the selection
    /// anchor.
    ///
    /// This method handles single-item selection mode where the cursor and
    /// anchor are synchronized. The cursor is clamped to valid bounds.
    ///
    /// # Arguments
    ///
    /// * `app` - Mutable reference to application state
    /// * `delta` - Number of positions to move (positive for down, negative for
    ///   up)
    fn move_cursor(&self, app: &mut App, delta: isize) {
        if app.logs.entries.is_empty() {
            return;
        }
        let len = app.logs.entries.len() as isize;
        let cur = app.logs.selection.cursor as isize;
        let next = (cur + delta).clamp(0, len - 1) as usize;
        app.logs.selection.cursor = next;
        app.logs.selection.anchor = next;
    }

    /// Extends the selection by the specified delta without changing the
    /// anchor.
    ///
    /// This method is used for multi-selection mode where the user holds Shift
    /// to extend the selection range. Only the cursor position is updated.
    ///
    /// # Arguments
    ///
    /// * `app` - Mutable reference to application state
    /// * `delta` - Number of positions to extend (positive for down, negative
    ///   for up)
    fn extend_selection(&self, app: &mut App, delta: isize) {
        if app.logs.entries.is_empty() {
            return;
        }
        let len = app.logs.entries.len() as isize;
        let cur = app.logs.selection.cursor as isize;
        let next = (cur + delta).clamp(0, len - 1) as usize;
        app.logs.selection.cursor = next;
    }

    /// Checks if a single API log entry is currently selected.
    ///
    /// Returns the selected log entry if exactly one item is selected, and it's
    /// an API entry. Used for determining available actions like JSON
    /// formatting.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state containing selection and log entries
    ///
    /// # Returns
    ///
    /// * `Some(LogEntry)` - The selected API log entry if single selection
    /// * `None` - If no selection, multi-selection, or non-API entry
    fn is_single_api(&self, app: &App) -> Option<LogEntry> {
        if app.logs.selection.is_single() {
            let idx = app.logs.selection.cursor;
            return app.logs.rich_entries.get(idx).cloned();
        }
        None
    }

    // ============================================================================
    // Detail View and JSON Processing Methods
    // ============================================================================

    /// Determines and opens the appropriate detail view for the current
    /// selection.
    ///
    /// This method handles the logic for choosing between different detail view
    /// modes based on the selected log entry type and content:
    ///
    /// - **Multi-selection**: Always shows text view
    /// - **API with array JSON**: Routes to global table component
    /// - **API with object JSON**: Shows formatted JSON in detail modal
    /// - **Other entries**: Shows plain text view
    ///
    /// # Arguments
    ///
    /// * `app` - Mutable reference to application state
    fn choose_detail(&self, app: &mut App) -> Vec<Effect> {
        let mut modal_to_open = Modal::LogDetails;

        if !app.logs.selection.is_single() {
            app.logs.detail = Some(LogDetailView::Text);
            app.logs.cached_detail_index = None;
            app.logs.cached_redacted_json = None;
        } else {
            let selected_index = app.logs.selection.cursor;
            match app.logs.rich_entries.get(selected_index) {
                Some(LogEntry::Api {
                    status,
                    raw,
                    json: Some(json_value),
                    ..
                }) if self.json_has_array(json_value) => {
                    app.logs.detail = None;
                    app.logs.cached_detail_index = None;
                    app.logs.cached_redacted_json = None;
                    modal_to_open = Modal::Results(Box::new(ExecOutcome::Http(*status, raw.to_string(), json_value.clone(), None, 0)));
                }
                Some(LogEntry::Api {
                    json: Some(json_value), ..
                }) => {
                    app.logs.detail = Some(LogDetailView::Text);
                    app.logs.cached_detail_index = Some(selected_index);
                    app.logs.cached_redacted_json = Some(redact_json(json_value));
                }
                _ => {
                    app.logs.detail = Some(LogDetailView::Text);
                    app.logs.cached_detail_index = None;
                    app.logs.cached_redacted_json = None;
                }
            }
        }

        vec![Effect::ShowModal(modal_to_open)]
    }

    /// Checks if a JSON value contains array data suitable for table display.
    ///
    /// Returns `true` if the JSON contains arrays that would benefit from
    /// tabular presentation rather than formatted text display.
    ///
    /// # Arguments
    ///
    /// * `v` - The JSON value to check
    ///
    /// # Returns
    ///
    /// * `true` - If the value contains non-empty arrays
    /// * `false` - If the value contains no arrays or only empty arrays
    fn json_has_array(&self, v: &Value) -> bool {
        match v {
            Value::Array(a) => !a.is_empty(),
            Value::Object(m) => m.values().any(|v| matches!(v, Value::Array(_))),
            _ => false,
        }
    }

    // ============================================================================
    // Styling and Visual Enhancement Methods
    // ============================================================================

    /// Applies syntax highlighting to log lines for better readability.
    ///
    /// This method identifies and styles different parts of log entries:
    ///
    /// - **Timestamps**: Styled with secondary accent color
    /// - **UUIDs**: Styled with emphasis color for easy identification
    /// - **Hex IDs**: Styled with emphasis color for long hexadecimal strings
    /// - **Regular text**: Uses primary text color
    ///
    /// # Arguments
    ///
    /// * `theme` - The UI theme providing color schemes
    /// * `line` - The log line text to style
    ///
    /// # Returns
    ///
    /// A styled `Line` with the appropriate color coding
    fn styled_line<'a>(&self, theme: &dyn UiTheme, line: &'a str) -> Line<'a> {
        // Compiled regex patterns for performance
        static TS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\[?\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2}(?:\.\d+)?Z?]?").unwrap());
        static UUID_RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-5][0-9a-fA-F]{3}-[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}\b").unwrap()
        });
        static HEXID_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b[0-9a-fA-F]{12,}\b").unwrap());

        let mut spans: Vec<Span> = Vec::new();
        let mut i = 0usize;

        // Style timestamp at the beginning of the line
        if let Some(m) = TS_RE.find(line)
            && m.start() == 0
            && m.end() > 0
        {
            spans.push(Span::styled(
                &line[m.start()..m.end()],
                Style::default().fg(theme.roles().accent_secondary),
            ));
            i = m.end();
        }

        // Style remaining text with UUID/hex ID highlighting
        let rest = &line[i..];
        let mut matches: Vec<_> = UUID_RE.find_iter(rest).chain(HEXID_RE.find_iter(rest)).collect();
        matches.sort_by_key(|m| m.start());

        let mut last = 0usize;
        for m in matches {
            if m.start() < last {
                continue; // Skip overlapping matches
            }
            // Add text before the match
            if m.start() > last {
                spans.push(Span::styled(&rest[last..m.start()], theme.text_primary_style()));
            }
            // Style the UUID/hex ID
            spans.push(Span::styled(&rest[m.start()..m.end()], theme.accent_emphasis_style()));
            last = m.end();
        }

        // Add remaining text
        if last < rest.len() {
            spans.push(Span::styled(&rest[last..], theme.text_primary_style()));
        }

        Line::from(spans)
    }
}

impl Component for LogsComponent {
    fn handle_message(&mut self, app: &mut App, msg: &Msg) -> Vec<Effect> {
        if let Msg::ExecCompleted(outcome) = msg {
            app.logs.process_general_execution_result(outcome)
        }
        Vec::new()
    }

    /// Handles keyboard input events for the log component.
    ///
    /// This method processes various key combinations to provide navigation,
    /// selection, and interaction functionality:
    ///
    /// ## Navigation Keys
    /// - **↑/↓**: Move cursor up/down
    /// - **Shift + ↑/↓**: Extend selection range
    ///
    /// ## Action Keys
    /// - **Enter**: Open detail view for selected entry
    /// - **c**: Copy selected content to clipboard
    /// - **v**: Toggle JSON pretty-printing (API entries only)
    ///
    /// # Arguments
    ///
    /// * `app` - Mutable reference to application state
    /// * `key` - The key event to process
    ///
    /// # Returns
    ///
    /// A vector of effects to be processed by the application
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        if app.logs.detail.is_some() {
            let mut detail_component = LogDetailsComponent::default();
            return detail_component.handle_key_events(app, key);
        }

        let mut effects = Vec::new();

        // Handle main navigation and action keys
        match key.code {
            // tab navigation
            KeyCode::BackTab => {
                app.focus.prev();
            }
            KeyCode::Tab => {
                app.focus.next();
            }
            KeyCode::Up => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.extend_selection(app, -1);
                } else {
                    self.move_cursor(app, -1);
                }
            }
            KeyCode::Down => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.extend_selection(app, 1);
                } else {
                    self.move_cursor(app, 1);
                }
            }
            KeyCode::Enter => {
                // Open detail view for selected entry
                return self.choose_detail(app);
            }
            KeyCode::Char('c') => {
                let text = build_copy_text(app);
                effects.push(Effect::CopyLogsRequested(text));
            }
            KeyCode::Char('v') => {
                if matches!(self.is_single_api(app), Some(LogEntry::Api { .. })) {
                    app.logs.pretty_json = !app.logs.pretty_json;
                }
                return effects;
            }
            _ => {}
        }
        effects
    }

    /// Renders the logs component to the terminal frame.
    ///
    /// This method handles the complete rendering of the logs interface,
    /// including:
    ///
    /// - **Main log list**: Displays all log entries with syntax highlighting
    /// - **Selection highlighting**: Shows current selection with visual
    ///   indicators
    /// - **Scrollbar**: Indicates position within the log list when focused
    /// - **Hint bar**: Shows available keyboard shortcuts when focused
    /// - **Detail modal**: Overlays detailed view for selected entries
    ///
    /// The component adapts its appearance based on focus state and provides
    /// visual feedback for user interactions.
    ///
    /// # Arguments
    ///
    /// * `frame` - The terminal frame to render to
    /// * `rect` - The rectangular area allocated for this component
    /// * `app` - The application state containing logs and UI state
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let focused = app.logs.container_focus.get();
        let title = format!("Logs ({})", app.logs.entries.len());
        let block = th::block(&*app.ctx.theme, Some(&title), focused);

        // Create list items with syntax highlighting
        // Note: Entries are pre-redacted when appended for safety
        let items: Vec<ListItem> = app
            .logs
            .entries
            .iter()
            .map(|l| ListItem::from(self.styled_line(&*app.ctx.theme, l)))
            .collect();

        // Configure the main log list widget
        let list = List::new(items)
            .block(block)
            .highlight_style(app.ctx.theme.selection_style().add_modifier(Modifier::BOLD))
            .style(th::panel_style(&*app.ctx.theme))
            .highlight_symbol(if focused { "> " } else { "" });

        // Set up the list state for selection highlighting
        let mut list_state = ListState::default();
        if focused {
            if let Some(sel) = self.selected_index(app) {
                list_state.select(Some(sel));
            }
        } else {
            list_state.select(None);
        }
        frame.render_stateful_widget(list, rect, &mut list_state);

        // Render scrollbar when focused to show position within a log list
        if focused {
            let content_len = app.logs.entries.len();
            if content_len > 0 {
                let visible = rect.height.saturating_sub(2) as usize; // Account for borders
                if visible > 0 && content_len > visible {
                    let sel = self.selected_index(app).unwrap_or(0);
                    let max_top = content_len.saturating_sub(visible);
                    let top = sel.min(max_top);
                    let scrollable_range = content_len.saturating_sub(visible).saturating_add(1);
                    let mut sb_state = ScrollbarState::new(scrollable_range).position(top).viewport_content_length(visible);
                    let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .thumb_style(Style::default().fg(app.ctx.theme.roles().scrollbar_thumb))
                        .track_style(Style::default().fg(app.ctx.theme.roles().scrollbar_track));
                    frame.render_stateful_widget(sb, rect, &mut sb_state);
                }
            }
        }
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        // Only render when logs are focused (rat-focus)
        if !app.logs.container_focus.get() {
            return vec![];
        }

        // Decide if we should show the pretty/raw toggle hint
        let mut show_pretty_toggle = false;
        if app.logs.selection.is_single() {
            let idx = app.logs.selection.cursor;
            if let Some(LogEntry::Api { json: Some(_), .. }) = app.logs.rich_entries.get(idx) {
                show_pretty_toggle = true;
            }
        }

        let theme = &*app.ctx.theme;
        let mut spans = th::build_hint_spans(
            theme,
            &[
                ("↑/↓", " Move  "),
                ("Shift+↑/↓", " Range  "),
                ("Enter", " Open  "),
                ("C", " Copy  "),
            ],
        );
        if show_pretty_toggle {
            spans.push(Span::styled("V ", theme.accent_emphasis_style()));
            // Show current mode with green highlight
            if app.logs.pretty_json {
                spans.push(Span::styled("pretty", Style::default().fg(theme.roles().success)));
                spans.push(Span::styled("/raw  ", theme.text_muted_style()));
            } else {
                spans.push(Span::styled("pretty/", theme.text_muted_style()));
                spans.push(Span::styled("raw  ", Style::default().fg(theme.roles().success)));
            }
        }

        spans
    }
}
