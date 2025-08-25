//! Command builder component for interactive command construction.
//!
//! This module provides a component for rendering the command builder modal,
//! which allows users to interactively build Heroku commands through a
//! multi-panel interface with search, command selection, and parameter input.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::*,
    text::{Line, Span},
    widgets::*,
    Frame,
};

use crate::{app, component::Component, theme};

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
        match app.builder.focus {
            app::Focus::Search => match key.code {
                KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::ToggleTable))
                }
                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::ToggleBuilder))
                }
                KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::ToggleHelp))
                }
                KeyCode::Char(c)
                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
                {
                    effects.extend(app.update(app::Msg::SearchChar(c)))
                }
                KeyCode::Backspace => effects.extend(app.update(app::Msg::SearchBackspace)),
                KeyCode::Esc => effects.extend(app.update(app::Msg::SearchClear)),
                KeyCode::Tab => effects.extend(app.update(app::Msg::FocusNext)),
                KeyCode::BackTab => effects.extend(app.update(app::Msg::FocusPrev)),
                KeyCode::Down => effects.extend(app.update(app::Msg::MoveSelection(1))),
                KeyCode::Up => effects.extend(app.update(app::Msg::MoveSelection(-1))),
                KeyCode::Enter => effects.extend(app.update(app::Msg::Enter)),
                KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::CopyCommand))
                }
                _ => {}
            },
            app::Focus::Commands => match key.code {
                KeyCode::Char('t') => effects.extend(app.update(app::Msg::ToggleTable)),
                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::ToggleBuilder))
                }
                KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::ToggleHelp))
                }
                KeyCode::Down => effects.extend(app.update(app::Msg::MoveSelection(1))),
                KeyCode::Up => effects.extend(app.update(app::Msg::MoveSelection(-1))),
                KeyCode::Enter => effects.extend(app.update(app::Msg::Enter)),
                KeyCode::Tab => effects.extend(app.update(app::Msg::FocusNext)),
                KeyCode::BackTab => effects.extend(app.update(app::Msg::FocusPrev)),
                KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::CopyCommand))
                }
                _ => {}
            },
            app::Focus::Inputs => match key.code {
                KeyCode::Char('t') => effects.extend(app.update(app::Msg::ToggleTable)),
                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::ToggleBuilder))
                }
                KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::ToggleHelp))
                }
                KeyCode::Tab => effects.extend(app.update(app::Msg::FocusNext)),
                KeyCode::BackTab => effects.extend(app.update(app::Msg::FocusPrev)),
                KeyCode::Up => effects.extend(app.update(app::Msg::InputsUp)),
                KeyCode::Down => effects.extend(app.update(app::Msg::InputsDown)),
                KeyCode::Enter => effects.extend(app.update(app::Msg::Run)),
                KeyCode::Left => effects.extend(app.update(app::Msg::InputsCycleLeft)),
                KeyCode::Right => effects.extend(app.update(app::Msg::InputsCycleRight)),
                KeyCode::Backspace => effects.extend(app.update(app::Msg::InputsBackspace)),
                KeyCode::Char(' ') => effects.extend(app.update(app::Msg::InputsToggleSpace)),
                KeyCode::Char(c)
                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
                {
                    effects.extend(app.update(app::Msg::InputsChar(c)))
                }
                KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::CopyCommand))
                }
                _ => {}
            },
        }
        Ok(effects)
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
        let block = Block::default()
            .title(Span::styled(
                "Command Builder  [Esc] Close",
                theme::title_style().fg(theme::ACCENT),
            ))
            .borders(Borders::ALL)
            .border_style(theme::border_style(true));
        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);
        let inner = block.inner(area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(1),
            ])
            .split(inner);

        // Search panel
        self.render_search_panel(f, app, chunks[0]);

        let main = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Percentage(35),
                Constraint::Percentage(35),
            ])
            .split(chunks[1]);

        // Command list panel
        self.render_commands_panel(f, app, main[0]);

        // Input fields panel
        self.render_inputs_panel(f, app, main[1]);

        // Preview panel
        self.render_preview_panel(f, app, main[2]);

        // Footer hint for builder modal
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
        f.render_widget(footer, chunks[2]);
    }
}

impl BuilderComponent {
    /// Renders the search input panel.
    fn render_search_panel(&self, f: &mut Frame, app: &mut app::App, area: Rect) {
        // Title with optional DEBUG badge
        let title = if app.ctx.debug_enabled {
            Line::from(vec![
                Span::styled("Search Commands", theme::title_style()),
                Span::raw("  "),
                Span::styled("[DEBUG]", theme::title_style().fg(theme::ACCENT)),
            ])
        } else {
            Line::from(Span::styled("Search Commands", theme::title_style()))
        };
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(theme::border_style(app.builder.focus == app::Focus::Search));
        let inner = block.inner(area);
        let p = Paragraph::new(app.browser.search.as_str())
            .style(theme::text_style())
            .block(block);
        f.render_widget(p, area);
        if app.builder.focus == app::Focus::Search {
            let x = inner
                .x
                .saturating_add(app.browser.search.chars().count() as u16);
            let y = inner.y;
            f.set_cursor_position((x, y));
        }
    }

    /// Renders the commands list panel.
    fn render_commands_panel(&self, f: &mut Frame, app: &mut app::App, area: Rect) {
        let title = format!("Commands ({})", app.browser.filtered.len());
        let block = Block::default()
            .title(Span::styled(title, theme::title_style()))
            .borders(Borders::ALL)
            .border_style(theme::border_style(
                app.builder.focus == app::Focus::Commands,
            ));

        let filtered = &app.browser.filtered;
        let all_commands = &app.browser.all_commands;
        let items: Vec<ListItem> = filtered
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
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(theme::list_highlight_style())
            .highlight_symbol("> ");
        let list_state = &mut app.browser.list_state;
        f.render_stateful_widget(list, area, list_state);
    }

    /// Renders the input fields panel.
    fn render_inputs_panel(&self, f: &mut Frame, app: &mut app::App, area: Rect) {
        use crate::ui::utils::IfEmptyStr;

        let title = match &app.builder.picked {
            Some(s) => {
                let mut split = s.name.splitn(2, ':');
                let group = split.next().unwrap_or("");
                let rest = split.next().unwrap_or("");
                let disp = if rest.is_empty() {
                    group.to_string()
                } else {
                    format!("{} {}", group, rest)
                };
                format!("Inputs: {}", disp)
            }
            None => "Inputs".into(),
        };
        let block = Block::default()
            .title(Span::styled(title, theme::title_style()))
            .borders(Borders::ALL)
            .border_style(theme::border_style(app.builder.focus == app::Focus::Inputs));

        // Draw the block first, then lay out inner area into content + footer rows
        f.render_widget(block.clone(), area);
        let inner = block.inner(area);
        let splits = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);
        let content_rect = splits[0];
        let footer_rect = splits[1];

        let mut lines: Vec<Line> = Vec::new();
        let mut cursor_row: Option<u16> = None;
        let mut cursor_col: Option<u16> = None;
        for (i, field) in app.builder.fields.iter().enumerate() {
            let marker = if field.required { "*" } else { "?" };
            let label = format!("{} {}", marker, field.name);
            let mut hint = String::new();
            if !field.enum_values.is_empty() {
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
                hint = format!("enum: {}", opts);
            }
            let val = if field.is_bool {
                if field.value.is_empty() {
                    "[ ]".to_string()
                } else {
                    "[x]".to_string()
                }
            } else if !field.enum_values.is_empty() {
                field.value.clone().if_empty_then("<choose>".to_string())
            } else {
                field.value.clone()
            };

            if app.builder.focus == app::Focus::Inputs && i == app.builder.field_idx {
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
                cursor_col = Some(prefix.chars().count() as u16 + offset);
                cursor_row = Some(i as u16);
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

            if app.builder.focus == app::Focus::Inputs && i == app.builder.field_idx {
                line = line.style(theme::highlight_style());
            }
            lines.push(line);
        }

        let missing: Vec<String> = app.missing_required();
        if app.builder.focus == app::Focus::Inputs && !missing.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("Missing required: ", Style::default().fg(theme::WARN)),
                Span::styled(missing.join(", "), theme::text_style()),
            ]));
        }

        // Render content lines inside the content_rect (no block so it uses inner area)
        let p = Paragraph::new(Text::from(lines as Vec<Line>)).style(theme::text_style());
        f.render_widget(p, content_rect);

        // Footer anchored at the base of the inputs pane
        let footer = Paragraph::new("Tab focus  Enter run  Ctrl+H help  Ctrl+C quit")
            .style(theme::text_muted());
        f.render_widget(footer, footer_rect);

        if app.builder.focus == app::Focus::Inputs {
            if let (Some(row), Some(col)) = (cursor_row, cursor_col) {
                let x = content_rect.x.saturating_add(col);
                let y = content_rect.y.saturating_add(row);
                f.set_cursor_position((x, y));
            }
        }
    }

    /// Renders the preview panel.
    fn render_preview_panel(&self, f: &mut Frame, app: &mut app::App, area: Rect) {
        // This would need to be implemented based on the preview widget logic
        // For now, we'll use a placeholder
        let block = Block::default()
            .title(Span::styled("Preview", theme::title_style()))
            .borders(Borders::ALL)
            .border_style(theme::border_style(false));

        let content = if let Some(spec) = &app.builder.picked {
            let mut parts: Vec<String> = Vec::new();
            let mut split = spec.name.splitn(2, ':');
            let group = split.next().unwrap_or("");
            let rest = split.next().unwrap_or("");
            parts.push(group.to_string());
            if !rest.is_empty() {
                parts.push(rest.to_string());
            }
            // Add fields
            for field in &app.builder.fields {
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
        };

        let p = Paragraph::new(content)
            .style(theme::text_style())
            .block(block);
        f.render_widget(p, area);
    }
}
