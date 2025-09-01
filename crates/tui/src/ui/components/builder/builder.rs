//! Command builder component for interactive command construction.
//!
//! This module provides a component for rendering the command builder modal,
//! which allows users to interactively build Heroku commands through a
//! multi-panel interface with search, command selection, and parameter input.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use heroku_types::Field;
use rat_focus::{FocusBuilder, HasFocus};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::*,
    text::{Line, Span},
    widgets::*,
};

use crate::{
    app,
    ui::{
        components::{builder::layout::BuilderLayout, component::Component},
        focus,
        theme::helpers as th,
        utils::IfEmptyStr,
    },
};

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
/// ```rust,ignore
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
    ///
    /// Applies local state updates directly to `app.builder`.
    fn handle_search_keys(&self, app: &mut app::App, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
                app.builder.search_input_push(c);
            },
            KeyCode::Backspace => app.builder.search_input_pop(),
            KeyCode::Esc => app.builder.search_input_clear(),
            KeyCode::Tab | KeyCode::BackTab => {
                let focus_ring = app.builder.focus_ring();
                let _ = if key.code == KeyCode::Tab {
                    focus_ring.next();
                } else {
                    focus_ring.prev();
                };
            },
            KeyCode::Down => app.builder.move_selection(1),
            KeyCode::Up => app.builder.move_selection(-1),
            KeyCode::Enter => {
                app.builder.apply_enter();
                // move focus to inputs via rat-focus ring
                app.builder.inputs_flag.set(true);
                app.builder.search_flag.set(false);
                app.builder.commands_flag.set(false);
            },
            _ => {},
        }
    }

    /// Handle key events specific to the commands panel.
    ///
    /// Applies local state updates directly to `app.builder`.
    fn handle_commands_keys(&self, app: &mut app::App, key: KeyEvent) {
        match key.code {
            KeyCode::Down => app.builder.move_selection(1),
            KeyCode::Up => app.builder.move_selection(-1),
            KeyCode::Enter => {
                app.builder.apply_enter();
                app.builder.inputs_flag.set(true);
                app.builder.search_flag.set(false);
                app.builder.commands_flag.set(false);
            },
            KeyCode::Tab | KeyCode::BackTab => {
                let f = app.builder.focus_ring();
                if key.code == KeyCode::Tab {
                    let _ = f.next();
                } else {
                    let _ = f.prev();
                }
            },
            _ => {},
        }
    }

    /// Handle key events specific to the inputs panel.
    ///
    /// Applies local state updates directly to `app.builder`.
    fn handle_inputs_keys(&self, app: &mut app::App, key: KeyEvent) {
        match key.code {
            KeyCode::Tab | KeyCode::BackTab => {
                let f = app.builder.focus_ring();
                if key.code == KeyCode::Tab {
                    let _ = f.next();
                } else {
                    let _ = f.prev();
                }
            },
            KeyCode::Up => app.builder.reduce_move_field_up(app.ctx.debug_enabled),
            KeyCode::Down => app.builder.reduce_move_field_down(app.ctx.debug_enabled),
            // Note: Enter in builder is handled at the top-level input loop to
            // close the modal and populate the palette; no direct run here.
            KeyCode::Left => app.builder.reduce_cycle_enum_left(),
            KeyCode::Right => app.builder.reduce_cycle_enum_right(),
            KeyCode::Backspace => app.builder.reduce_remove_char_from_field(),
            KeyCode::Char(' ') => app.builder.reduce_toggle_boolean_field(),
            KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
                app.builder.reduce_add_char_to_field(c);
            },
            _ => {},
        }
    }
}

impl Component for BuilderComponent {
    /// Renders the command builder modal with all panels.
    ///
    /// This method handles the layout, styling, and content generation for the
    /// builder interface.
    ///
    /// # Arguments
    ///
    /// * `f` - The frame to render to
    /// * `rect` - The rectangular area to render in
    /// * `app` - The application state containing builder data
    fn render(&mut self, f: &mut Frame, rect: Rect, app: &mut app::App) {
        use crate::ui::utils::centered_rect;

        let area = centered_rect(96, 90, rect);
        self.render_modal_frame(f, area, app);

        let inner = self.create_modal_layout(area, app);
        let chunks = BuilderLayout::vertical_layout(inner);

        // Render search panel
        self.render_search_panel(f, app, chunks[0]);

        // Create and render main panels
        let main = self.create_main_layout(chunks[1]);
        self.render_commands_panel(f, app, main[0]);
        self.render_inputs_panel(f, app, main[1]);
        self.render_preview_panel(f, app, main[2]);

        // Render footer
        self.render_footer(f, app, chunks[2]);
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
    ///
    /// Local/UI state updates are applied directly to `app` here (no App::Msg),
    /// mirroring the palette's local-first handling. Only global actions
    /// escalate via `app.update(..)` to produce side effects.
    fn handle_key_events(&mut self, app: &mut app::App, key: KeyEvent) -> Vec<app::Effect> {
        let mut effects: Vec<app::Effect> = Vec::new();

        // Handle global shortcuts first
        if let Some(effect) = self.handle_global_shortcuts(key) {
            effects.extend(app.update(effect));
            return effects;
        }

        // Handle focus-specific key events (local updates applied in-place)
        if app.builder.search_flag.get() {
            self.handle_search_keys(app, key);
        } else if app.builder.commands_flag.get() {
            self.handle_commands_keys(app, key);
        } else if app.builder.inputs_flag.get() {
            self.handle_inputs_keys(app, key);
        }

        effects
    }
}

impl BuilderComponent {
    /// Creates the modal frame with title and borders.
    fn render_modal_frame(&self, f: &mut Frame, area: Rect, app: &app::App) {
        let block = th::block(&*app.ctx.theme, Some("Command Builder  [Esc] Close"), true);

        f.render_widget(Clear, area);
        f.render_widget(block.clone(), area);
    }

    /// Creates the inner layout area for the modal.
    fn create_modal_layout(&self, area: Rect, app: &app::App) -> Rect {
        // Use the same modal block to compute inner area
        let block = th::block(&*app.ctx.theme, Some("Command Builder  [Esc] Close"), true);
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
    fn render_footer(&self, frame: &mut Frame, app: &app::App, area: Rect) {
        let theme = &*app.ctx.theme;
        let footer = Paragraph::new(Line::from(vec![
            Span::styled("Hint: ", theme.text_muted_style()),
            Span::styled("Ctrl+F", theme.accent_emphasis_style()),
            Span::styled(" close  ", theme.text_muted_style()),
            Span::styled("Enter", theme.accent_emphasis_style()),
            Span::styled(" apply  ", theme.text_muted_style()),
            Span::styled("Esc", theme.accent_emphasis_style()),
            Span::styled(" cancel", theme.text_muted_style()),
        ]))
        .style(theme.text_muted_style());

        frame.render_widget(footer, area);
    }

    /// Renders the search input panel.
    fn render_search_panel(&self, f: &mut Frame, app: &mut app::App, area: Rect) {
        let title = self.create_search_title(app);
        let focused = app.builder.search_flag.get();
        let mut block = th::block(&*app.ctx.theme, None, focused);
        block = block.title(title);

        let inner = block.inner(area);
        let p = Paragraph::new(app.builder.search_input().as_str())
            .style(app.ctx.theme.text_primary_style())
            .block(block);

        f.render_widget(p, area);
        self.set_search_cursor(f, app, inner);
    }

    /// Creates the search panel title with optional debug badge.
    fn create_search_title(&self, app: &app::App) -> Line<'_> {
        if app.ctx.debug_enabled {
            let t = &*app.ctx.theme;
            Line::from(vec![
                Span::styled("Search Commands", t.text_secondary_style().add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::styled("[DEBUG]", t.accent_emphasis_style()),
            ])
        } else {
            let t = &*app.ctx.theme;
            Line::from(Span::styled(
                "Search Commands",
                t.text_secondary_style().add_modifier(Modifier::BOLD),
            ))
        }
    }

    /// Sets the cursor position for the search input.
    fn set_search_cursor(&self, f: &mut Frame, app: &app::App, inner: Rect) {
        if app.builder.search_flag.get() {
            let x = inner
                .x
                .saturating_add(app.builder.search_input().chars().count() as u16);
            let y = inner.y;
            f.set_cursor_position((x, y));
        }
    }

    /// Renders the commands list panel.
    fn render_commands_panel(&self, frame: &mut Frame, app: &mut app::App, area: Rect) {
        let title = format!("Commands ({})", app.builder.filtered().len());
        let focused = app.builder.commands_flag.get();
        let block = th::block(&*app.ctx.theme, Some(&title), focused);

        let items = self.create_command_list_items(app);
        let list = List::new(items)
            .block(block)
            .highlight_style(app.ctx.theme.selection_style().add_modifier(Modifier::BOLD))
            .highlight_symbol("> ");

        let list_state = &mut app.builder.list_state();
        frame.render_stateful_widget(list, area, list_state);
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
                ListItem::new(display).style(app.ctx.theme.text_primary_style())
            })
            .collect()
    }

    /// Renders the input fields panel.
    fn render_inputs_panel(&self, f: &mut Frame, app: &mut app::App, area: Rect) {
        let title = self.create_inputs_title(app);
        let focused = app.builder.inputs_flag.get();
        let block = th::block(&*app.ctx.theme, Some(&title), focused);

        // Draw the block first, then lay out inner area into content + footer rows
        f.render_widget(&block, area);
        let inner = block.inner(area);

        let (input_lines, cursor_pos) = self.create_input_lines(app);
        self.render_input_content(f, app, inner, input_lines);
        self.set_input_cursor(f, app, inner, cursor_pos);
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
            },
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
        if app.builder.inputs_flag.get() && !missing.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("Missing required: ", Style::default().fg(app.ctx.theme.roles().warning)),
                Span::styled(missing.join(", "), app.ctx.theme.text_primary_style()),
            ]));
        }

        (lines, cursor_pos)
    }

    /// Creates a single field line with cursor position.
    fn create_field_line(&self, app: &app::App, field: &Field, field_idx: usize) -> (Line<'_>, Option<(u16, u16)>) {
        let marker = if field.required { "*" } else { "?" };
        let label = format!("{} {}", marker, field.name);
        let hint = self.create_field_hint(app, field);
        let val = self.create_field_value(app, field);

        let mut cursor_pos = None;
        if app.builder.inputs_flag.get() && field_idx == app.builder.current_field_idx() {
            let prefix = if let Some((ref hint_text, _)) = hint {
                format!("{} {}: ", label, hint_text)
            } else {
                format!("{}: ", label)
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
                app.ctx.theme.text_primary_style()
            } else {
                app.ctx.theme.text_muted_style()
            },
        )]);

        if let Some((_, hspans)) = hint {
            line.push_span(Span::raw(" "));
            for s in hspans {
                line.push_span(s);
            }
        }
        line.push_span(Span::raw(": "));
        line.push_span(val);

        if app.builder.inputs_flag.get() && field_idx == app.builder.current_field_idx() {
            line = line.style(app.ctx.theme.selection_style());
        }

        (line, cursor_pos)
    }

    /// Creates the hint text for a field.
    /// Returns both a plain text version (for cursor math) and styled spans
    /// with the selected enum value highlighted in green with a checkmark.
    fn create_field_hint(&self, app: &app::App, field: &Field) -> Option<(String, Vec<Span<'static>>)> {
        if field.enum_values.is_empty() {
            return None;
        }

        let t = &*app.ctx.theme;
        let enum_idx = field.enum_idx.unwrap_or(0);

        // Build plain text (for measuring) and styled spans (for rendering)
        let mut plain = String::from("enum: ");
        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.push(Span::styled("enum: ", t.text_muted_style()));

        for (i, v) in field.enum_values.iter().enumerate() {
            let (p, s) = if enum_idx == i {
                (format!("✓{}", v), Span::styled(format!("✓{}", v), t.status_success()))
            } else {
                (v.to_string(), Span::styled(v.to_string(), t.text_muted_style()))
            };
            plain.push_str(&p);
            spans.push(s);
            if i + 1 < field.enum_values.len() {
                plain.push('|');
                spans.push(Span::styled("|", t.text_muted_style()));
            }
        }

        Some((plain, spans))
    }

    /// Creates the display value for a field.
    ///
    /// For boolean fields with a non-empty value, returns a green check mark.
    /// Otherwise returns the appropriate text styled with the primary text
    /// color.
    fn create_field_value(&self, app: &app::App, field: &Field) -> Span<'static> {
        let t = &*app.ctx.theme;
        if field.is_bool {
            if field.value.trim().is_empty() {
                Span::styled("[ ]".to_string(), t.text_primary_style())
            } else {
                Span::styled("[✓]", t.status_success())
            }
        } else if !field.enum_values.is_empty() {
            let value = field.value.clone().if_empty_then("<choose>".to_string());
            Span::styled(value, t.text_primary_style())
        } else {
            Span::styled(field.value.clone(), t.text_primary_style())
        }
    }

    /// Renders the input content area.
    fn render_input_content(&self, f: &mut Frame, app: &mut app::App, area: Rect, lines: Vec<Line>) {
        // Use theme primary text for content
        let p = Paragraph::new(Text::from(lines)).style(app.ctx.theme.text_primary_style());
        f.render_widget(p, area);
    }

    /// Sets the cursor position for the input panel.
    fn set_input_cursor(&self, f: &mut Frame, app: &mut app::App, area: Rect, cursor_pos: Option<(u16, u16)>) {
        if app.builder.inputs_flag.get()
            && let Some((col, row)) = cursor_pos
        {
            let x = area.x.saturating_add(col);
            let y = area.y.saturating_add(row);
            f.set_cursor_position((x, y));
        }
    }

    /// Renders the preview panel.
    fn render_preview_panel(&self, f: &mut Frame, app: &mut app::App, area: Rect) {
        let block = th::block(&*app.ctx.theme, Some("Preview"), false);

        let content = self.create_preview_content(app);
        let p = Paragraph::new(content)
            .style(app.ctx.theme.text_primary_style())
            .block(block);

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
