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

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
    hint_bar::LogsHintBarComponent,
    state::LogEntry,
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
#[derive(Default)]
pub struct LogsComponent;

impl LogsComponent {
    /// Creates a new LogsComponent instance.
    ///
    /// # Returns
    ///
    /// A new `LogsComponent` with default configuration.
    pub fn new() -> Self {
        Self
    }

    // ============================================================================
    // Styling and Visual Enhancement Methods
    // ============================================================================

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
    /// Initializes the LogsComponent.
    ///
    /// Currently a no-op as the component doesn't require any initialization.
    ///
    /// # Returns
    ///
    /// Always returns `Ok(())` indicating successful initialization.
    fn init(&mut self) -> Result<()> {
        Ok(())
    }

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
    fn handle_key_events(&mut self, app: &mut app::App, key: KeyEvent) -> Option<app::Msg> {
        // Handle keys when detail modal is open
        if app.logs.detail.is_some() {
            return match key.code {
                KeyCode::Esc | KeyCode::Backspace => Some(app::Msg::LogsCloseDetail),
                // TODO: Add messages for scrolling in detail view
                _ => None,
            };
        }

        // Handle main navigation and action keys
        match key.code {
            KeyCode::Up => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    Some(app::Msg::LogsExtendUp)
                } else {
                    Some(app::Msg::LogsUp)
                }
            }
            KeyCode::Down => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    Some(app::Msg::LogsExtendDown)
                } else {
                    Some(app::Msg::LogsDown)
                }
            }
            KeyCode::Enter => Some(app::Msg::LogsOpenDetail),
            KeyCode::Char('c') => Some(app::Msg::LogsCopy),
            KeyCode::Char('v') => Some(app::Msg::LogsTogglePretty),
            _ => None,
        }
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
            if !app.logs.entries.is_empty() {
                list_state.select(Some(app.logs.selection.cursor));
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
                let sel = app.logs.selection.cursor;
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
            let mut hints_comp = LogsHintBarComponent;
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
