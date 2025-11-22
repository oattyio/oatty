use crate::app::App;
use crate::ui::components::component::Component;
use crate::ui::theme::theme_helpers as th;
use crate::ui::theme::theme_helpers::create_spans_with_match;
use anyhow::{Result, anyhow};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use heroku_engine::WorkflowRunState;
use heroku_types::workflow::RuntimeWorkflow;
use heroku_types::{Effect, Route, validate_candidate_value};
use heroku_util::{HistoryKey, value_contains_secret, workflow_input_uses_history};
use rat_focus::HasFocus;
use ratatui::layout::Position;
use ratatui::widgets::ListItem;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{List, Paragraph, Wrap},
};
use tracing::warn;

/// Renders the workflow picker view, including search, filtered listing, and footer hints.
#[derive(Debug, Default)]
pub struct WorkflowsComponent {
    search_area: Rect,
    list_area: Rect,
    mouse_over_idx: Option<usize>,
}

impl WorkflowsComponent {
    fn handle_search_key(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        // Only handle here when the search field is active, mirroring browser behavior
        if !app.workflows.f_search.get() {
            return Vec::new();
        }

        match key.code {
            // Esc clears the current search query (do not exit search)
            KeyCode::Esc => {
                app.workflows.clear_search();
            }
            KeyCode::Backspace => {
                app.workflows.pop_search_char();
            }
            KeyCode::Left => {
                app.workflows.move_search_left();
            }
            KeyCode::Right => {
                app.workflows.move_search_right();
            }
            KeyCode::Char(character) if !character.is_control() => {
                app.workflows.append_search_char(character);
            }
            _ => {}
        }

        Vec::new()
    }

    fn render_panel(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let workflows_count = app.workflows.total_count();
        let filtered_count = app.workflows.filtered_count();
        let title = if filtered_count == workflows_count {
            format!("Workflows ({workflows_count})")
        } else {
            format!("Workflows ({filtered_count}/{workflows_count})")
        };

        // Match the BrowserComponent layout: dedicated search panel (with its own block)
        // and a list panel (with its own block and title)
        let layout = Layout::vertical([
            Constraint::Length(3), // Search panel area (title and borders)
            Constraint::Min(1),    // List area
        ])
        .split(area);

        self.render_search_bar(frame, layout[0], app);
        self.render_workflow_list(frame, layout[1], app, &title);
    }

    fn render_search_bar(&mut self, frame: &mut Frame, area: Rect, app: &App) {
        let search_query = app.workflows.search_query();
        let theme = &*app.ctx.theme;
        let is_focused = app.workflows.f_search.get();

        // Create a block similar to the browser search panel
        let search_title = Line::from(Span::styled(
            "Search Workflows",
            theme.text_secondary_style().add_modifier(Modifier::BOLD),
        ));
        let mut search_block = th::block(theme, None, is_focused);
        search_block = search_block.title(search_title);
        let inner_area = search_block.inner(area);

        // Show only the query text (or a muted placeholder) inside the block
        let content_line = if is_focused || !search_query.is_empty() {
            Line::from(Span::styled(search_query.to_string(), theme.text_primary_style()))
        } else {
            Line::from(Span::from(""))
        };

        let paragraph = Paragraph::new(content_line).block(search_block).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);

        if is_focused {
            let cursor_byte = app.workflows.search_cursor();
            let prefix = &search_query[..cursor_byte.min(search_query.len())];
            let cursor_cols = prefix.chars().count() as u16;
            let cursor_x = inner_area.x.saturating_add(cursor_cols);
            let cursor_y = inner_area.y;
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }

    fn render_workflow_list(&mut self, frame: &mut Frame, area: Rect, app: &mut App, title: &str) {
        let theme = &*app.ctx.theme;
        let is_focused = app.workflows.list.f_list.get();
        let block = th::block(theme, Some(title), is_focused);
        let list_area = block.inner(area);
        frame.render_widget(block, area);

        let filter_input = app.workflows.search_query();
        let identifier_style = theme.syntax_type_style();
        let summary_style = theme.syntax_string_style();
        let highlight_style = theme.search_highlight_style();
        let (items, filtered_count) = {
            let state = &app.workflows;
            let title_width = state.filtered_title_width().clamp(12, 40);
            let available_summary_width = area.width.saturating_sub(title_width as u16).saturating_sub(4) as usize;

            let items: Vec<ListItem> = state
                .filtered_indices()
                .iter()
                .enumerate()
                .filter_map(|(idx, workflow_index)| {
                    state.workflow_by_index(*workflow_index).map(|workflow| {
                        let identifier_cell = format!(
                            "{:<width$}",
                            workflow.title.as_ref().unwrap_or(&workflow.identifier),
                            width = title_width
                        );
                        let summary = Self::summarize_workflow(workflow, available_summary_width);
                        let needle = filter_input.to_string();
                        let mut spans = create_spans_with_match(needle.clone(), identifier_cell, identifier_style, highlight_style);
                        spans.extend(create_spans_with_match(needle, summary, summary_style, highlight_style));
                        let mut list_item = ListItem::from(Line::from(spans));
                        if self.mouse_over_idx.is_some_and(|mouse_idx| mouse_idx == idx) {
                            list_item = list_item.style(theme.selection_style().add_modifier(Modifier::BOLD));
                        }
                        list_item
                    })
                })
                .collect();
            (items, state.filtered_count())
        };

        if filtered_count == 0 {
            let message = if app.workflows.total_count() == 0 {
                "No workflows are available yet."
            } else {
                "No workflows match the current search."
            };
            let message_paragraph = Paragraph::new(message).style(theme.text_muted_style()).wrap(Wrap { trim: true });
            frame.render_widget(message_paragraph, list_area);
            return;
        }
        let is_list_focused = app.workflows.list.f_list.get();
        let list_state = app.workflows.list_state();
        if !is_list_focused {
            list_state.select(None);
        }
        let list = List::new(items)
            .highlight_style(theme.selection_style().add_modifier(Modifier::BOLD))
            .highlight_symbol(if is_list_focused { "> " } else { "" });

        frame.render_stateful_widget(list, list_area, list_state);
        self.list_area = list_area;
    }

    fn summarize_workflow(workflow: &RuntimeWorkflow, max_width: usize) -> String {
        let summary_source = workflow
            .description
            .as_deref()
            .filter(|value| !value.is_empty())
            .or_else(|| workflow.title.as_deref().filter(|value| !value.is_empty()))
            .unwrap_or("No description provided.");

        if max_width == 0 {
            return summary_source.to_string();
        }

        let mut summary = summary_source.to_string();
        if summary.chars().count() > max_width {
            summary = summary.chars().take(max_width.saturating_sub(3)).collect::<String>();
            summary.push_str("...");
        }
        summary
    }

    /// Populate run state inputs with history-backed defaults when available.
    fn seed_history_defaults(&mut self, app: &mut App, run_state: &mut WorkflowRunState) -> Vec<Effect> {
        let mut effects = Vec::new();
        for (input_name, definition) in &run_state.workflow.inputs {
            if !workflow_input_uses_history(definition) {
                continue;
            }

            let key = HistoryKey::workflow_input(
                app.ctx.history_profile_id.clone(),
                run_state.workflow.identifier.clone(),
                input_name.clone(),
            );

            match app.ctx.history_store.get_latest_value(&key) {
                Ok(Some(stored)) => {
                    if stored.value.is_null() || value_contains_secret(&stored.value) {
                        continue;
                    }
                    if let Some(validation) = &definition.validate
                        && let Err(error) = validate_candidate_value(&stored.value, validation)
                    {
                        let message = format!("History default for '{}' failed validation: {}", input_name, error);
                        warn!(
                            input = %input_name,
                            workflow = %run_state.workflow.identifier,
                            "{}",
                            message
                        );
                        effects.push(Effect::Log(message));
                        continue;
                    }

                    run_state.run_context.inputs.insert(input_name.clone(), stored.value);
                }
                Ok(None) => {}
                Err(error) => {
                    let message = format!("Failed to load history default for '{}': {}", input_name, error);
                    warn!(
                        input = %input_name,
                        workflow = %run_state.workflow.identifier,
                        "{}",
                        message
                    );
                    effects.push(Effect::Log(message));
                }
            }
        }
        effects
    }

    fn hit_test_list(&mut self, app: &mut App, position: Position) -> Option<usize> {
        let offset = app.workflows.list.list_state().offset();
        let idx = (position.y as usize).saturating_sub(self.list_area.y as usize) + offset;
        if app.workflows.list.workflow_by_index(idx).is_some() {
            Some(idx)
        } else {
            None
        }
    }

    /// Open the interactive input view for the selected workflow.
    pub fn open_workflow_inputs(&mut self, app: &mut App) -> Result<()> {
        let Some(workflow) = app.workflows.selected_workflow() else {
            return Err(anyhow!("No workflow selected"));
        };
        let mut run_state = WorkflowRunState::new(workflow.clone());
        self.seed_history_defaults(app, &mut run_state);
        run_state.apply_input_defaults();
        run_state.evaluate_input_providers()?;
        app.workflows.begin_inputs_session(run_state);
        Ok(())
    }
}

impl Component for WorkflowsComponent {
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        // Handle tab/backtab to switch focus between the search field and the list
        match key.code {
            KeyCode::Tab => {
                app.focus.next();
            }
            KeyCode::BackTab => {
                app.focus.prev();
            }
            _ => {}
        }
        let mut effects = Vec::new();
        if let Err(error) = app.workflows.ensure_loaded(&app.ctx.command_registry) {
            app.append_log_message(format!("Failed to load workflows: {error}"));
            return effects;
        }
        // Defer to the search field if it's focused
        let is_search_focused = app.workflows.f_search.get();
        if is_search_focused {
            return self.handle_search_key(app, key);
        }

        // Handle key events for the list
        match key.code {
            // Clear search on Esc (mirrors browser behavior)
            KeyCode::Esc => {
                if !app.workflows.search_query().is_empty() || is_search_focused {
                    app.workflows.clear_search();
                    app.focus.focus(&app.workflows.f_search); // stay in search mode after clearing
                }
            }
            KeyCode::Down => app.workflows.select_next(),
            KeyCode::Up => app.workflows.select_prev(),
            KeyCode::Enter => {
                if app.workflows.selected_workflow().is_some() {
                    if let Err(error) = self.open_workflow_inputs(app) {
                        effects.push(Effect::Log(format!("Failed to open workflow inputs: {error}")));
                    } else {
                        effects.push(Effect::SwitchTo(Route::WorkflowInputs));
                    }
                }
            }
            _ => {}
        }

        effects
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let position = Position {
            x: mouse.column,
            y: mouse.row,
        };

        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            if self.search_area.contains(position) {
                app.focus.focus(&app.workflows.f_search);
            }

            if self.list_area.contains(position)
                && let Some(idx) = self.hit_test_list(app, position)
            {
                app.focus.focus(&app.workflows.list.focus());
                app.workflows.list.set_selected_workflow(idx);
                return self.handle_key_events(app, KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
            }
        }

        if mouse.kind == MouseEventKind::Moved || mouse.kind == MouseEventKind::Up(MouseButton::Left) {
            self.mouse_over_idx = self.hit_test_list(app, position);
        }

        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        if let Err(error) = app.workflows.ensure_loaded(&app.ctx.command_registry) {
            let block = Paragraph::new(format!("Workflows failed to load: {error}"))
                .style(app.ctx.theme.status_error())
                .wrap(Wrap { trim: true });
            frame.render_widget(block, area);
            app.append_log_message(format!("Workflow load error: {error}"));
            return;
        }

        self.render_panel(frame, area, app);
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        let mut hints: Vec<(&str, &str)> = Vec::new();

        let search_focused = app.workflows.f_search.get();
        if search_focused {
            hints.push(("Esc", " Clear search  "));
        } else {
            hints.push(("Shift+Tab", " Focus search  "));
            hints.push(("Esc", " Clear filter  "));
        }

        hints.push(("↑/↓", " Select  "));
        hints.push(("Enter", " Open inputs"));

        th::build_hint_spans(theme, &hints)
    }
}
