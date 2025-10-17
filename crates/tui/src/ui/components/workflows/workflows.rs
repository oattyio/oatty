use crate::app::App;
use crate::ui::components::component::Component;
use crate::ui::theme::theme_helpers as th;
use crate::ui::theme::theme_helpers::create_spans_with_match;
use crossterm::event::{KeyCode, KeyEvent};
use heroku_types::Effect::SwitchTo;
use heroku_types::workflow::RuntimeWorkflow;
use heroku_types::{Effect, Route};
use ratatui::widgets::ListItem;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{List, Paragraph, Wrap},
};

/// Renders the workflow picker view, including search, filtered listing, and footer hints.
#[derive(Debug, Default)]
pub struct WorkflowsComponent;

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

    fn render_workflow_list(&self, frame: &mut Frame, area: Rect, app: &mut App, title: &str) {
        let theme = &*app.ctx.theme;
        let is_focused = app.workflows.list.f_list.get();
        let block = th::block(theme, Some(title), is_focused);
        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        let filter_input = app.workflows.search_query();
        let (items, filtered_count) = {
            let state = &app.workflows;
            let identifier_width = state.filtered_identifier_width().clamp(12, 40);
            let available_summary_width = area.width.saturating_sub(identifier_width as u16).saturating_sub(4) as usize;

            let items = state
                .filtered_indices()
                .iter()
                .filter_map(|workflow_index| {
                    state.workflow_by_index(*workflow_index).map(|workflow| {
                        let identifier_cell = format!("{:<width$}", workflow.identifier, width = identifier_width);
                        let summary = Self::summarize_workflow(workflow, available_summary_width);
                        let primary = theme.text_primary_style();
                        let secondary = theme.text_secondary_style();
                        let accent = theme.accent_emphasis_style();
                        let input = filter_input.to_string();
                        let mut spans = create_spans_with_match(input.clone(), identifier_cell, primary, accent);
                        spans.extend(create_spans_with_match(input, summary, secondary, accent));
                        ListItem::from(Line::from(spans))
                    })
                })
                .collect::<Vec<_>>();
            (items, state.filtered_count())
        };

        if filtered_count == 0 {
            let message = if app.workflows.total_count() == 0 {
                "No workflows are available yet."
            } else {
                "No workflows match the current search."
            };
            let message_paragraph = Paragraph::new(message).style(theme.text_muted_style()).wrap(Wrap { trim: true });
            frame.render_widget(message_paragraph, inner_area);
            return;
        }
        let is_list_focused = app.workflows.list.f_list.get();
        let list_state = app.workflows.list_state();
        let list = List::new(items)
            .highlight_style(theme.selection_style().add_modifier(Modifier::BOLD))
            .highlight_symbol(if is_list_focused { "> " } else { "" });

        frame.render_stateful_widget(list, inner_area, list_state);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect, app: &mut App) {
        let theme = &*app.ctx.theme;
        let mut footer_text = String::new();
        if app.workflows.f_search.get() {
            footer_text.push_str("Esc Clear  •  Enter run  •  ↑↓ select");
        } else {
            footer_text.push_str("/ or type to search  •  Enter run  •  ↑↓ select");
        }
        let paragraph = Paragraph::new(footer_text)
            .style(theme.text_secondary_style())
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
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
            app.logs.entries.push(format!("Failed to load workflows: {error}"));
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
                    effects.push(SwitchTo(Route::WorkflowInputs));
                }
            }
            _ => {}
        }

        effects
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        if let Err(error) = app.workflows.ensure_loaded(&app.ctx.command_registry) {
            let block = Paragraph::new(format!("Workflows failed to load: {error}"))
                .style(app.ctx.theme.status_error())
                .wrap(Wrap { trim: true });
            frame.render_widget(block, area);
            app.logs.entries.push(format!("Workflow load error: {error}"));
            return;
        }

        let layout = Layout::vertical([
            Constraint::Min(5),    // Search and list widget panel
            Constraint::Length(1), // Footer
        ])
        .split(area);

        self.render_panel(frame, layout[0], app);
        self.render_footer(frame, layout[1], app);
    }
}
