use crossterm::event::{KeyCode, KeyEvent};
use heroku_engine::RuntimeWorkflow;
use heroku_types::Effect;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph, Wrap},
};

use crate::app::App;
use crate::ui::components::component::Component;
use crate::ui::theme::theme_helpers as th;

/// Renders the workflow picker view, including search, filtered listing, and footer hints.
#[derive(Debug, Default)]
pub struct WorkflowsComponent {
    /// Indicates whether the search field is currently active for text entry.
    search_active: bool,
}

impl WorkflowsComponent {
    fn handle_search_key(&mut self, app: &mut App, key: KeyEvent) -> Option<Vec<Effect>> {
        if !self.search_active {
            return None;
        }

        match key.code {
            KeyCode::Esc => {
                self.search_active = false;
                return Some(Vec::new());
            }
            KeyCode::Enter => {
                self.search_active = false;
                return Some(Vec::new());
            }
            KeyCode::Backspace => {
                app.workflows.pop_search_char();
                return Some(Vec::new());
            }
            KeyCode::Char(character) if !character.is_control() => {
                app.workflows.append_search_char(character);
                return Some(Vec::new());
            }
            _ => {}
        }

        None
    }

    fn render_panel(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let workflows_count = app.workflows.total_count();
        let filtered_count = app.workflows.filtered_count();
        let title = if filtered_count == workflows_count {
            format!("Workflows ({workflows_count})")
        } else {
            format!("Workflows ({filtered_count}/{workflows_count})")
        };
        let is_focused = app.workflows.is_focused();

        let theme = &*app.ctx.theme;
        let outer_block = th::block(theme, Some(&title), is_focused);
        let inner_area = outer_block.inner(area);
        frame.render_widget(outer_block, area);

        let inner_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(inner_area);

        self.render_search_bar(frame, inner_layout[0], app);
        self.render_workflow_list(frame, inner_layout[1], app);
    }

    fn render_search_bar(&mut self, frame: &mut Frame, area: Rect, app: &App) {
        let search_query = app.workflows.search_query();
        let theme = &*app.ctx.theme;
        let search_line = if self.search_active || !search_query.is_empty() {
            Line::from(vec![
                Span::styled("Search: ", theme.text_secondary_style()),
                Span::styled("[", theme.text_secondary_style()),
                Span::styled(search_query.to_string(), theme.text_primary_style()),
                Span::styled("]", theme.text_secondary_style()),
            ])
        } else {
            Line::from(vec![
                Span::styled("Search: ", theme.text_secondary_style()),
                Span::styled("[", theme.text_secondary_style()),
                Span::styled("type / to filter", theme.text_muted_style()),
                Span::styled("]", theme.text_secondary_style()),
            ])
        };

        let paragraph = Paragraph::new(search_line).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);

        if self.search_active {
            let prefix = "Search: [";
            let cursor_x = area
                .x
                .saturating_add(prefix.chars().count() as u16)
                .saturating_add(search_query.chars().count() as u16);
            frame.set_cursor_position((cursor_x, area.y));
        }
    }

    fn render_workflow_list(&self, frame: &mut Frame, area: Rect, app: &mut App) {
        let theme = &*app.ctx.theme;
        let (items, filtered_count) = {
            let state = &app.workflows;
            let identifier_width = state.filtered_identifier_width().clamp(12, 40);
            let available_summary_width = area.width.saturating_sub(identifier_width as u16).saturating_sub(4) as usize;

            let items = state
                .filtered_indices()
                .iter()
                .enumerate()
                .filter_map(|(row_index, workflow_index)| {
                    state.workflow_by_index(*workflow_index).map(|workflow| {
                        let identifier_cell = format!("{:<width$}", workflow.identifier, width = identifier_width);
                        let summary = Self::summarize_workflow(workflow, available_summary_width);
                        let line = Line::from(vec![
                            Span::styled(identifier_cell, theme.text_primary_style()),
                            Span::raw("  "),
                            Span::styled(summary, theme.text_secondary_style()),
                        ]);
                        let row_style = th::table_row_style(theme, row_index);
                        ListItem::new(line).style(row_style)
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
            frame.render_widget(message_paragraph, area);
            return;
        }

        let list_state = app.workflows.list_state();

        let list = List::new(items)
            .highlight_style(theme.selection_style().add_modifier(Modifier::BOLD))
            .highlight_symbol("▸ ");

        frame.render_stateful_widget(list, area, list_state);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect, app: &mut App) {
        let theme = &*app.ctx.theme;
        let mut footer_text = "↑↓ select  •  / search  •  Enter run  •  Esc back".to_string();
        if self.search_active {
            footer_text.push_str("  •  Enter close search");
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
        if let Err(error) = app.workflows.ensure_loaded(&app.ctx.registry) {
            app.logs.entries.push(format!("Failed to load workflows: {error}"));
            return Vec::new();
        }

        if let Some(effects) = self.handle_search_key(app, key) {
            return effects;
        }

        match key.code {
            KeyCode::Char('/') => {
                self.search_active = true;
                return Vec::new();
            }
            KeyCode::Down => app.workflows.select_next(),
            KeyCode::Up => app.workflows.select_prev(),
            KeyCode::Enter => {
                if let Some(workflow) = app.workflows.selected_workflow().cloned() {
                    if let Err(error) = app.open_workflow_inputs(&workflow) {
                        app.logs.entries.push(format!("Failed to open workflow inputs: {error}"));
                    }
                }
            }
            _ => {}
        }

        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        if let Err(error) = app.workflows.ensure_loaded(&app.ctx.registry) {
            let block = Paragraph::new(format!("Workflows failed to load: {error}"))
                .style(app.ctx.theme.status_error())
                .wrap(Wrap { trim: true });
            frame.render_widget(block, area);
            app.logs.entries.push(format!("Workflow load error: {error}"));
            return;
        }

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(1)])
            .split(area);

        self.render_panel(frame, layout[0], app);
        self.render_footer(frame, layout[1], app);
    }
}
