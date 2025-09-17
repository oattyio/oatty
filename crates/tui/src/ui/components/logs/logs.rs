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
use heroku_types::Effect;
use heroku_util::redact_sensitive;
use once_cell::sync::Lazy;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::*,
};
use regex::Regex;
use serde_json::Value;

use super::{
    hint_bar::LogsHintBar,
    state::{LogDetailView, LogEntry},
};
use crate::{
    app,
    ui::{
        components::{TableComponent, component::Component},
        theme::{helpers as th, roles::Theme as UiTheme},
        utils::centered_rect,
    },
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
    fn selected_index(&self, app: &app::App) -> Option<usize> {
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
    fn move_cursor(&self, app: &mut app::App, delta: isize) {
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
    fn extend_selection(&self, app: &mut app::App, delta: isize) {
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
    /// Returns the selected log entry if exactly one item is selected and it's
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
    fn is_single_api(&self, app: &app::App) -> Option<LogEntry> {
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
    fn choose_detail(&self, app: &mut app::App) {
        // Multi-selection always shows text view
        if !app.logs.selection.is_single() {
            app.logs.detail = Some(LogDetailView::Text);
            app.logs.cached_detail_index = None;
            app.logs.cached_redacted_json = None;
            return;
        }

        let idx = app.logs.selection.cursor;
        match app.logs.rich_entries.get(idx) {
            Some(LogEntry::Api { json: Some(j), .. }) => {
                if self.json_has_array(j) {
                    // Route array JSON to the global Table modal for better UX
                    let redacted = Self::redact_json(j);
                    app.table.apply_result_json(Some(redacted), &*app.ctx.theme);
                    app.table.normalize();
                    // Clear logs detail modal state since we're using table
                    app.logs.detail = None;
                    app.logs.cached_detail_index = None;
                    app.logs.cached_redacted_json = None;
                } else {
                    // Show formatted JSON in logs detail modal
                    app.logs.detail = Some(LogDetailView::Text);
                    app.logs.cached_detail_index = Some(idx);
                    app.logs.cached_redacted_json = Some(Self::redact_json(j));
                }
            }
            _ => {
                // Non-API entries or API without JSON show as text
                app.logs.detail = Some(LogDetailView::Text);
                app.logs.cached_detail_index = None;
                app.logs.cached_redacted_json = None;
            }
        }
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

    /// Recursively redacts sensitive data from JSON values.
    ///
    /// This method traverses the JSON structure and applies redaction to all
    /// string values while preserving the overall structure. Used for security
    /// when displaying JSON data in the UI.
    ///
    /// # Arguments
    ///
    /// * `v` - The JSON value to redact
    ///
    /// # Returns
    ///
    /// A new JSON value with all string content redacted
    fn redact_json(v: &Value) -> Value {
        match v {
            Value::String(s) => Value::String(redact_sensitive(s)),
            Value::Array(arr) => Value::Array(arr.iter().map(Self::redact_json).collect()),
            Value::Object(map) => {
                let mut out = serde_json::Map::new();
                for (k, val) in map.iter() {
                    out.insert(k.clone(), Self::redact_json(val));
                }
                Value::Object(out)
            }
            other => other.clone(),
        }
    }

    // ============================================================================
    // Copy and Text Processing Methods
    // ============================================================================

    /// Builds the text content to be copied to clipboard based on current
    /// selection.
    ///
    /// This method handles different copy scenarios:
    ///
    /// - **Single API entry with JSON**: Returns formatted JSON if pretty mode
    ///   enabled
    /// - **Single API entry without JSON**: Returns raw log content
    /// - **Multi-selection**: Returns concatenated log entries
    ///
    /// All output is automatically redacted for security.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state containing logs and selection
    ///
    /// # Returns
    ///
    /// A redacted string containing the selected log content
    fn build_copy_text(&self, app: &app::App) -> String {
        if app.logs.entries.is_empty() {
            return String::new();
        }
        let (start, end) = app.logs.selection.range();
        if start >= app.logs.entries.len() {
            return String::new();
        }

        // Handle single selection with special JSON formatting
        if start == end
            && let Some(LogEntry::Api { json, raw, .. }) = app.logs.rich_entries.get(start)
        {
            if let Some(j) = json
                && app.logs.pretty_json
            {
                let red = Self::redact_json(j);
                return serde_json::to_string_pretty(&red).unwrap_or_else(|_| redact_sensitive(raw));
            }
            return redact_sensitive(raw);
        }

        // Multi-select or text fallback: concatenate visible strings
        let mut buf = String::new();
        for i in start..=end.min(app.logs.entries.len() - 1) {
            let line = app.logs.entries.get(i).cloned().unwrap_or_default();
            if !buf.is_empty() {
                buf.push('\n');
            }
            buf.push_str(&line);
        }
        redact_sensitive(&buf)
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
    /// A styled `Line` with appropriate color coding
    fn styled_line<'a>(&self, theme: &dyn UiTheme, line: &'a str) -> Line<'a> {
        // Compiled regex patterns for performance
        static TS_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^\[?\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2}(?:\.\d+)?Z?\]?").unwrap());
        static UUID_RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-5][0-9a-fA-F]{3}-[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}\b")
                .unwrap()
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
        let mut last = 0usize;
        for m in UUID_RE.find_iter(rest).chain(HEXID_RE.find_iter(rest)) {
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
    /// Handles keyboard input events for the logs component.
    ///
    /// This method processes various key combinations to provide navigation,
    /// selection, and interaction functionality:
    ///
    /// ## Navigation Keys
    /// - **↑/↓**: Move cursor up/down
    /// - **Shift + ↑/↓**: Extend selection range
    ///
    /// ## Detail View Keys (when detail modal is open)
    /// - **Esc/Backspace**: Close detail modal
    /// - **↑/↓**: Scroll within table detail view
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
    fn handle_key_events(&mut self, app: &mut app::App, key: KeyEvent) -> Vec<Effect> {
        let mut effects = Vec::new();

        // Handle keys when detail modal is open
        if let Some(detail) = app.logs.detail {
            match key.code {
                KeyCode::Esc | KeyCode::Backspace => {
                    // Close detail modal
                    app.logs.detail = None;
                    return effects;
                }
                KeyCode::Up => {
                    // Scroll up in table detail view
                    if let LogDetailView::Table { offset } = detail {
                        app.logs.detail = Some(LogDetailView::Table {
                            offset: offset.saturating_sub(1),
                        });
                    }
                    return effects;
                }
                KeyCode::Down => {
                    // Scroll down in table detail view
                    if let LogDetailView::Table { offset } = detail {
                        app.logs.detail = Some(LogDetailView::Table {
                            offset: offset.saturating_add(1),
                        });
                    }
                    return effects;
                }
                _ => {}
            }
        }

        // Handle main navigation and action keys
        match key.code {
            KeyCode::Up => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    // Extend selection upward
                    self.extend_selection(app, -1);
                } else {
                    // Move cursor up
                    self.move_cursor(app, -1);
                }
            }
            KeyCode::Down => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    // Extend selection downward
                    self.extend_selection(app, 1);
                } else {
                    // Move cursor down
                    self.move_cursor(app, 1);
                }
            }
            KeyCode::Enter => {
                // Open detail view for selected entry
                self.choose_detail(app);
            }
            KeyCode::Char('c') => {
                // Copy selected content to clipboard
                let text = self.build_copy_text(app);
                effects.push(Effect::CopyLogsRequested(text));
            }
            KeyCode::Char('v') => {
                // Toggle JSON pretty-printing (API entries only)
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
    /// This method handles the complete rendering of the logs interface
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
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut app::App) {
        let focused = app.logs.focus.get();
        let title = format!("Logs ({})", app.logs.entries.len());
        let block = th::block(&*app.ctx.theme, Some(&title), focused);
        let inner = block.inner(rect);

        // Create list items with syntax highlighting
        // Note: Entries are pre-redacted when appended for safety
        let items: Vec<ListItem> = app
            .logs
            .entries
            .iter()
            .map(|l| ListItem::new(self.styled_line(&*app.ctx.theme, l)))
            .collect();

        // Configure the main log list widget
        let list = List::new(items)
            .block(block)
            .highlight_style(app.ctx.theme.selection_style().add_modifier(Modifier::BOLD))
            .style(th::panel_style(&*app.ctx.theme))
            .highlight_symbol(if focused { "► " } else { "" });

        // Set up list state for selection highlighting
        let mut list_state = ListState::default();
        if focused {
            if let Some(sel) = self.selected_index(app) {
                list_state.select(Some(sel));
            }
        } else {
            list_state.select(None);
        }
        frame.render_stateful_widget(list, rect, &mut list_state);

        // Render scrollbar when focused to show position within log list
        if focused {
            use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};
            let content_len = app.logs.entries.len();
            if content_len > 0 {
                let visible = rect.height.saturating_sub(2) as usize; // Account for borders
                let sel = self.selected_index(app).unwrap_or(0);
                let max_top = content_len.saturating_sub(visible);
                let top = sel.min(max_top);
                let mut sb_state = ScrollbarState::new(content_len).position(top);
                let sb = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_style(Style::default().fg(app.ctx.theme.roles().scrollbar_thumb))
                    .track_style(Style::default().fg(app.ctx.theme.roles().scrollbar_track));
                frame.render_stateful_widget(sb, rect, &mut sb_state);
            }
        }

        // Render hint bar at bottom when focused
        if focused && inner.height >= 1 {
            let hint_area = Rect::new(inner.x, inner.y + inner.height.saturating_sub(1), inner.width, 1);
            let mut hints_comp = LogsHintBar;
            hints_comp.render(frame, hint_area, app);
        }

        // Render detail modal overlay when open
        if focused && app.logs.detail.is_some() {
            let area = centered_rect(90, 85, rect);
            let title = "Log Details";
            let block = th::block(&*app.ctx.theme, Some(title), true);

            // Clear the modal area and render the border
            frame.render_widget(Clear, area);
            frame.render_widget(&block, area);
            let inner = block.inner(area);

            // Split modal into content and footer areas
            let splits = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(inner);

            // Render the main detail content
            self.render_detail_content(frame, splits[0], app);

            // Render footer with keyboard hints
            let footer = Paragraph::new(Line::from(vec![
                Span::styled("Hint: ", app.ctx.theme.text_muted_style()),
                Span::styled("Esc", app.ctx.theme.accent_emphasis_style()),
                Span::styled(" close  ", app.ctx.theme.text_muted_style()),
                Span::styled("c", app.ctx.theme.accent_emphasis_style()),
                Span::styled(" copy  ", app.ctx.theme.text_muted_style()),
            ]))
            .style(app.ctx.theme.text_muted_style());
            frame.render_widget(footer, splits[1]);
        }
    }
}

impl LogsComponent {
    /// Renders the content of the detail modal for selected log entries.
    ///
    /// This method handles different rendering modes based on the selection:
    ///
    /// - **Single API entry with JSON**: Renders formatted JSON using
    ///   TableComponent
    /// - **Single non-API entry**: Renders plain text with word wrapping
    /// - **Multi-selection**: Renders concatenated text from all selected
    ///   entries
    ///
    /// All content is automatically redacted for security before display.
    ///
    /// # Arguments
    ///
    /// * `f` - The terminal frame to render to
    /// * `area` - The rectangular area allocated for the detail content
    /// * `app` - The application state containing logs and selection
    fn render_detail_content(&self, f: &mut Frame, area: Rect, app: &mut app::App) {
        let (start, end) = app.logs.selection.range();

        // Handle single selection
        if start == end {
            if let Some(LogEntry::Api { json: Some(j), .. }) = app.logs.rich_entries.get(start) {
                // Use cached redacted JSON if available, otherwise redact on-the-fly
                // Note: Only non-array JSON renders here; arrays are routed to global table
                // modal
                let red_ref: &Value = match app.logs.cached_detail_index {
                    Some(i) if i == start => app.logs.cached_redacted_json.as_ref().unwrap_or(j),
                    _ => j,
                };

                // Render formatted JSON using TableComponent for better presentation
                let table = TableComponent::default();
                table.render_kv_or_text(f, area, red_ref, &*app.ctx.theme);
                return;
            }

            // Handle single non-API entry or API without JSON
            let s = app.logs.entries.get(start).cloned().unwrap_or_default();
            let p = Paragraph::new(redact_sensitive(&s))
                .block(Block::default().borders(Borders::NONE))
                .wrap(Wrap { trim: false })
                .style(app.ctx.theme.text_primary_style());
            f.render_widget(p, area);
            return;
        }

        // Handle multi-selection: concatenate all selected log entries
        let mut buf = String::new();
        let max = app.logs.entries.len().saturating_sub(1);
        for i in start..=end.min(max) {
            if !buf.is_empty() {
                buf.push('\n');
            }
            buf.push_str(app.logs.entries.get(i).map(|s| s.as_str()).unwrap_or(""));
        }

        // Render concatenated text with word wrapping
        let p = Paragraph::new(redact_sensitive(&buf))
            .block(Block::default().borders(Borders::NONE))
            .wrap(Wrap { trim: false })
            .style(app.ctx.theme.text_primary_style());
        f.render_widget(p, area);
    }
}
