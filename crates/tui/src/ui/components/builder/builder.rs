//! Command builder component for interactive command construction.
//!
//! This module provides a component for rendering the command builder modal,
//! which allows users to interactively build Heroku commands through a
//! multi-panel interface with search, command selection, and parameter input.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::*,
    text::{Line, Span},
    widgets::*,
};

use crate::{
    app, theme,
    ui::{
        components::{builder::layout::BuilderLayout, component::Component},
        utils::IfEmptyStr,
    },
};
use heroku_types::{Field, Focus};

/// Command builder component for interactive command construction.
///
/// This component provides a comprehensive modal interface for building
/// Heroku commands interactively. It includes search functionality,
/// command selection, and parameter input with validation.
///
/// # Features
///
/// - **Search panel**: Filter and search for available commands
/// - **Command list**: Browse and select from available commands
/// - **Input panel**: Fill in command parameters with validation
/// - **Preview panel**: See the generated command in real-time
/// - **Focus management**: Navigate between panels with Tab/Shift+Tab
/// - **Keyboard shortcuts**: Quick access to help, tables, and copy
///
/// # Panel Layout
///
/// The builder modal is divided into three main panels:
///
/// 1. **Search Panel** (top) - Command search and filtering
/// 2. **Command List** (left) - Available commands selection
/// 3. **Input Panel** (center) - Parameter input and validation
/// 4. **Preview Panel** (right) - Generated command preview
///
/// # Key Bindings
///
/// ## Global Shortcuts
/// - **Ctrl+F**: Close builder modal
/// - **Ctrl+H**: Open help modal
/// - **Ctrl+T**: Open table modal
/// - **Ctrl+Y**: Copy current command
///
/// ## Navigation
/// - **Tab**: Move to next panel
/// - **Shift+Tab**: Move to previous panel
/// - **Escape**: Clear search or close modal
///
/// ## Search Panel
/// - **Character input**: Add to search query
/// - **Backspace**: Remove character
/// - **Arrow keys**: Navigate suggestions
/// - **Enter**: Select command
///
/// ## Command List
/// - **Arrow keys**: Navigate commands
/// - **Enter**: Select command
///
/// ## Input Panel
/// - **Arrow keys**: Navigate fields
/// - **Character input**: Edit field values
/// - **Space**: Toggle boolean fields
/// - **Left/Right**: Cycle enum values
/// - **Enter**: Execute command
///
/// # Examples
///
/// ```rust
/// use heroku_tui::ui::components::BuilderComponent;
///
/// let mut builder = BuilderComponent::new();
/// builder.init()?;
/// ```
#[derive(Default)]
pub struct BuilderComponent;

impl BuilderComponent {
    /// Creates a new builder component instance.
    ///
    /// # Returns
    ///
    /// A new BuilderComponent with default state
    pub fn new() -> Self {
        Self
    }

    /// Handle key events for the command builder modal.
    ///
    /// This method processes keyboard input for the builder, handling
    /// navigation between panels, input editing, and special commands.
    /// The behavior varies based on which panel currently has focus.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state to update
    /// * `key` - The key event to process
    ///
    /// # Returns
    ///
    /// `Result<Vec<Effect>>` containing any effects that should be processed
    pub fn handle_key(&mut self, app: &mut app::App, key: KeyEvent) -> Result<Vec<app::Effect>> {
        let mut effects: Vec<app::Effect> = Vec::new();

        // Handle global shortcuts first
        if let Some(effect) = self.handle_global_shortcuts(key) {
            effects.extend(app.update(effect));
            return Ok(effects);
        }

        // Handle focus-specific key events
        match app.builder.selected_focus() {
            Focus::Search => {
                for msg in self.handle_search_keys(key) {
                    effects.extend(app.update(msg));
                }
            }
            Focus::Commands => {
                for msg in self.handle_commands_keys(key) {
                    effects.extend(app.update(msg));
                }
            }
            Focus::Inputs => {
                for msg in self.handle_inputs_keys(key) {
                    effects.extend(app.update(msg));
                }
            }
        }

        Ok(effects)
    }

    /// Handle global shortcuts that work across all panels.
    fn handle_global_shortcuts(&self, key: KeyEvent) -> Option<app::Msg> {
        match key.code {
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(app::Msg::ToggleBuilder),
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(app::Msg::ToggleHelp),
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(app::Msg::ToggleTable),
            KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(app::Msg::CopyCommand),
            _ => None,
        }
    }

    /// Handle key events specific to the search panel.
    fn handle_search_keys(&self, key: KeyEvent) -> Vec<app::Msg> {
        let mut effects = Vec::new();

        match key.code {
            KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
                effects.push(app::Msg::SearchChar(c));
            }
            KeyCode::Backspace => effects.push(app::Msg::SearchBackspace),
            KeyCode::Esc => effects.push(app::Msg::SearchClear),
            KeyCode::Tab => effects.push(app::Msg::FocusNext),
            KeyCode::BackTab => effects.push(app::Msg::FocusPrev),
            KeyCode::Down => effects.push(app::Msg::MoveSelection(1)),
            KeyCode::Up => effects.push(app::Msg::MoveSelection(-1)),
            KeyCode::Enter => effects.push(app::Msg::Enter),
            _ => {}
        }

        effects
    }

    /// Handle key events specific to the commands panel.
    fn handle_commands_keys(&self, key: KeyEvent) -> Vec<app::Msg> {
        let mut effects = Vec::new();

        match key.code {
            KeyCode::Down => effects.push(app::Msg::MoveSelection(1)),
            KeyCode::Up => effects.push(app::Msg::MoveSelection(-1)),
            KeyCode::Enter => effects.push(app::Msg::Enter),
            KeyCode::Tab => effects.push(app::Msg::FocusNext),
            KeyCode::BackTab => effects.push(app::Msg::FocusPrev),
            _ => {}
        }

        effects
    }

    /// Handle key events specific to the inputs panel.
    fn handle_inputs_keys(&self, key: KeyEvent) -> Vec<app::Msg> {
        let mut effects = Vec::new();

        match key.code {
            KeyCode::Tab => effects.push(app::Msg::FocusNext),
            KeyCode::BackTab => effects.push(app::Msg::FocusPrev),
            KeyCode::Up => effects.push(app::Msg::InputsUp),
            KeyCode::Down => effects.push(app::Msg::InputsDown),
            KeyCode::Enter => effects.push(app::Msg::Run),
            KeyCode::Left => effects.push(app::Msg::InputsCycleLeft),
            KeyCode::Right => effects.push(app::Msg::InputsCycleRight),
            KeyCode::Backspace => effects.push(app::Msg::InputsBackspace),
            KeyCode::Char(' ') => effects.push(app::Msg::InputsToggleSpace),
            KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
                effects.push(app::Msg::InputsChar(c));
            }
            _ => {}
        }

        effects
    }
}

impl Component for BuilderComponent {
    /// Renders the command builder modal with all panels.
    ///
    /// This method handles the layout, styling, and content generation for the builder interface.
    ///
    /// # Arguments
    ///
    /// * `f` - The frame to render to
    /// * `rect` - The rectangular area to render in
    /// * `app` - The application state containing builder data
    fn render(&mut self, f: &mut Frame, rect: Rect, app: &mut app::App) {
        use crate::ui::utils::centered_rect;

        let area = centered_rect(96, 90, rect);
        self.render_modal_frame(f, area);

        let inner = self.create_modal_layout(area);
        let chunks = BuilderLayout::vertical_layout(inner);

        // Render search panel
        self.render_search_panel(f, app, chunks[0]);

        // Create and render main panels
        let main = self.create_main_layout(chunks[1]);
        self.render_commands_panel(f, app, main[0]);
        self.render_inputs_panel(f, app, main[1]);
        self.render_preview_panel(f, app, main[2]);

        // Render footer
        self.render_footer(f, chunks[2]);
    }
}

impl BuilderComponent {
    /// Creates the modal frame with title and borders.
    fn render_modal_frame(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(Span::styled(
                "Command Builder  [Esc] Close",
                theme::title_style().fg(theme::ACCENT),
            ))
            .borders(Borders::ALL)
            .border_style(theme::border_style(true));

        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);
    }

    /// Creates the inner layout area for the modal.
    fn create_modal_layout(&self, area: Rect) -> Rect {
        let block = Block::default()
            .title(Span::styled(
                "Command Builder  [Esc] Close",
                theme::title_style().fg(theme::ACCENT),
            ))
            .borders(Borders::ALL)
            .border_style(theme::border_style(true));

        block.inner(area)
    }

    /// Creates the horizontal layout for the main panels.
    fn create_main_layout(&self, area: Rect) -> Vec<Rect> {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30), // Commands
                Constraint::Percentage(35), // Inputs
                Constraint::Percentage(35), // Preview
            ])
            .split(area)
            .to_vec()
    }

    /// Renders the footer with keyboard hints.
    fn render_footer(&self, f: &mut Frame, area: Rect) {
        let footer = Paragraph::new(Line::from(vec![
            Span::styled("Hint: ", theme::text_muted()),
            Span::styled("Ctrl+F", theme::title_style().fg(theme::ACCENT)),
            Span::styled(" close  ", theme::text_muted()),
            Span::styled("Enter", theme::title_style().fg(theme::ACCENT)),
            Span::styled(" apply  ", theme::text_muted()),
            Span::styled("Esc", theme::title_style().fg(theme::ACCENT)),
            Span::styled(" cancel", theme::text_muted()),
        ]))
        .style(theme::text_muted());

        f.render_widget(footer, area);
    }

    /// Renders the search input panel.
    fn render_search_panel(&self, f: &mut Frame, app: &mut app::App, area: Rect) {
        let title = self.create_search_title(app);
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(theme::border_style(app.builder.selected_focus() == Focus::Search));

        let inner = block.inner(area);
        let p = Paragraph::new(app.builder.search_input().as_str())
            .style(theme::text_style())
            .block(block);

        f.render_widget(p, area);
        self.set_search_cursor(f, app, inner);
    }

    /// Creates the search panel title with optional debug badge.
    fn create_search_title(&self, app: &app::App) -> Line<'_> {
        if app.ctx.debug_enabled {
            Line::from(vec![
                Span::styled("Search Commands", theme::title_style()),
                Span::raw("  "),
                Span::styled("[DEBUG]", theme::title_style().fg(theme::ACCENT)),
            ])
        } else {
            Line::from(Span::styled("Search Commands", theme::title_style()))
        }
    }

    /// Sets the cursor position for the search input.
    fn set_search_cursor(&self, f: &mut Frame, app: &app::App, inner: Rect) {
        if app.builder.selected_focus() == Focus::Search {
            let x = inner
                .x
                .saturating_add(app.builder.search_input().chars().count() as u16);
            let y = inner.y;
            f.set_cursor_position((x, y));
        }
    }

    /// Renders the commands list panel.
    fn render_commands_panel(&self, f: &mut Frame, app: &mut app::App, area: Rect) {
        let title = format!("Commands ({})", app.builder.filtered().len());
        let block = Block::default()
            .title(Span::styled(title, theme::title_style()))
            .borders(Borders::ALL)
            .border_style(theme::border_style(app.builder.selected_focus() == Focus::Commands));

        let items = self.create_command_list_items(app);
        let list = List::new(items)
            .block(block)
            .highlight_style(theme::list_highlight_style())
            .highlight_symbol("> ");

        let list_state = &mut app.builder.list_state();
        f.render_stateful_widget(list, area, list_state);
    }

    /// Creates list items for the commands panel.
    fn create_command_list_items(&self, app: &app::App) -> Vec<ListItem<'_>> {
        let filtered = &app.builder.filtered();
        let all_commands = app.builder.all_commands();

        filtered
            .iter()
            .map(|idx| {
                let group = &all_commands[*idx].group;
                let name = &all_commands[*idx].name;
                let display = if name.is_empty() {
                    group.to_string()
                } else {
                    format!("{} {}", group, name)
                };
                ListItem::new(display).style(theme::text_style())
            })
            .collect()
    }

    /// Renders the input fields panel.
    fn render_inputs_panel(&self, f: &mut Frame, app: &mut app::App, area: Rect) {
        let title = self.create_inputs_title(app);
        let block = Block::default()
            .title(Span::styled(title, theme::title_style()))
            .borders(Borders::ALL)
            .border_style(theme::border_style(app.builder.selected_focus() == Focus::Inputs));

        // Draw the block first, then lay out inner area into content + footer rows
        f.render_widget(&block, area);
        let inner = block.inner(area);
        let splits = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);

        let content_rect = splits[0];
        let footer_rect = splits[1];

        let (lines, cursor_pos) = self.create_input_lines(app);
        self.render_input_content(f, app, content_rect, lines);
        self.render_input_footer(f, footer_rect);
        self.set_input_cursor(f, app, content_rect, cursor_pos);
    }

    /// Creates the inputs panel title.
    fn create_inputs_title(&self, app: &app::App) -> String {
        match app.builder.selected_command() {
            Some(s) => {
                let mut split = s.name.splitn(2, ':');
                let group = split.next().unwrap_or("");
                let rest = split.next().unwrap_or("");
                let title = if rest.is_empty() {
                    group.to_string()
                } else {
                    format!("{} {}", group, rest)
                };
                format!("Inputs: {}", title)
            }
            None => "Inputs".into(),
        }
    }

    /// Creates the input field lines and cursor position.
    fn create_input_lines(&self, app: &app::App) -> (Vec<Line<'_>>, Option<(u16, u16)>) {
        let mut lines: Vec<Line> = Vec::new();
        let mut cursor_pos: Option<(u16, u16)> = None;

        for (i, field) in app.builder.input_fields().iter().enumerate() {
            let (line, field_cursor) = self.create_field_line(app, field, i);
            lines.push(line);

            if field_cursor.is_some() {
                cursor_pos = field_cursor;
            }
        }

        // Add missing required fields warning
        let missing: Vec<String> = app.builder.missing_required_fields();
        if app.builder.selected_focus() == Focus::Inputs && !missing.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("Missing required: ", Style::default().fg(theme::WARN)),
                Span::styled(missing.join(", "), theme::text_style()),
            ]));
        }

        (lines, cursor_pos)
    }

    /// Creates a single field line with cursor position.
    fn create_field_line(&self, app: &app::App, field: &Field, field_idx: usize) -> (Line<'_>, Option<(u16, u16)>) {
        let marker = if field.required { "*" } else { "?" };
        let label = format!("{} {}", marker, field.name);
        let hint = self.create_field_hint(field);
        let val = self.create_field_value(field);

        let mut cursor_pos = None;
        if app.builder.selected_focus() == Focus::Inputs && field_idx == app.builder.current_field_idx() {
            let prefix = if hint.is_empty() {
                format!("{}: ", label)
            } else {
                format!("{} {}: ", label, hint)
            };
            let offset = if field.is_bool || !field.enum_values.is_empty() {
                0
            } else {
                field.value.chars().count()
            } as u16;
            cursor_pos = Some((prefix.chars().count() as u16 + offset, field_idx as u16));
        }

        let mut line = Line::from(vec![Span::styled(
            label,
            if field.required {
                theme::text_style()
            } else {
                theme::text_muted()
            },
        )]);

        if !hint.is_empty() {
            line.push_span(Span::raw(" "));
            line.push_span(Span::styled(hint, theme::text_muted()));
        }
        line.push_span(Span::raw(": "));
        line.push_span(Span::styled(val, theme::text_style()));

        if app.builder.selected_focus() == Focus::Inputs && field_idx == app.builder.current_field_idx() {
            line = line.style(theme::highlight_style());
        }

        (line, cursor_pos)
    }

    /// Creates the hint text for a field.
    fn create_field_hint(&self, field: &Field) -> String {
        if field.enum_values.is_empty() {
            return String::new();
        }

        let enum_idx = field.enum_idx.unwrap_or(0);
        let opts = field
            .enum_values
            .iter()
            .enumerate()
            .map(|(i, v)| -> String {
                if enum_idx == i {
                    format!("✓{}", v)
                } else {
                    v.to_string()
                }
            })
            .collect::<Vec<String>>()
            .join("|");

        format!("enum: {}", opts)
    }

    /// Creates the display value for a field.
    fn create_field_value(&self, field: &Field) -> String {
        if field.is_bool {
            if field.value.is_empty() {
                "[ ]".to_string()
            } else {
                "[x]".to_string()
            }
        } else if !field.enum_values.is_empty() {
            field.value.clone().if_empty_then("<choose>".to_string())
        } else {
            field.value.clone()
        }
    }

    /// Renders the input content area.
    fn render_input_content(&self, f: &mut Frame, _app: &mut app::App, area: Rect, lines: Vec<Line>) {
        let p = Paragraph::new(Text::from(lines)).style(theme::text_style());
        f.render_widget(p, area);
    }

    /// Renders the input footer.
    fn render_input_footer(&self, f: &mut Frame, area: Rect) {
        let footer = Paragraph::new("Tab focus  Enter run  Ctrl+H help  Ctrl+C quit").style(theme::text_muted());
        f.render_widget(footer, area);
    }

    /// Sets the cursor position for the input panel.
    fn set_input_cursor(&self, f: &mut Frame, app: &mut app::App, area: Rect, cursor_pos: Option<(u16, u16)>) {
        if app.builder.selected_focus() == Focus::Inputs {
            if let Some((col, row)) = cursor_pos {
                let x = area.x.saturating_add(col);
                let y = area.y.saturating_add(row);
                f.set_cursor_position((x, y));
            }
        }
    }

    /// Renders the preview panel.
    fn render_preview_panel(&self, f: &mut Frame, app: &mut app::App, area: Rect) {
        let block = Block::default()
            .title(Span::styled("Preview", theme::title_style()))
            .borders(Borders::ALL)
            .border_style(theme::border_style(false));

        let content = self.create_preview_content(app);
        let p = Paragraph::new(content).style(theme::text_style()).block(block);

        f.render_widget(p, area);
    }

    /// Creates the preview content.
    fn create_preview_content(&self, app: &app::App) -> String {
        if let Some(spec) = app.builder.selected_command() {
            let mut parts: Vec<String> = Vec::new();
            let mut split = spec.name.splitn(2, ':');
            let group = split.next().unwrap_or("");
            let rest = split.next().unwrap_or("");
            parts.push(group.to_string());
            if !rest.is_empty() {
                parts.push(rest.to_string());
            }

            // Add fields
            for field in app.builder.input_fields() {
                if !field.value.trim().is_empty() {
                    if field.is_bool {
                        parts.push(format!("--{}", field.name));
                    } else {
                        parts.push(format!("--{}", field.name));
                        parts.push(field.value.trim().to_string());
                    }
                }
            }

            format!("heroku {}", parts.join(" "))
        } else {
            "Select a command to see preview".to_string()
        }
    }
}
