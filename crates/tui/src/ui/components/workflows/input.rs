use crossterm::event::{KeyCode, KeyEvent};
use heroku_engine::ProviderBindingOutcome;
use heroku_types::Effect;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use crate::app::App;
use crate::ui::components::component::Component;
use crate::ui::components::workflows::view_utils::{ProviderCacheSummary, format_cache_summary, format_preview};
use crate::ui::theme::{roles::Theme, theme_helpers as th};
use serde_json::Value as JsonValue;

#[derive(Debug, Default)]
pub struct WorkflowInputsComponent;

impl Component for WorkflowInputsComponent {
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        if app.workflows.input_view_state().is_none() {
            return Vec::new();
        };

        match key.code {
            KeyCode::Esc => {
                app.close_workflow_inputs();
                Vec::new()
            }
            KeyCode::Down => {
                if let Some(total) = active_input_count(app) {
                    app.workflows.input_view_state_mut().unwrap().select_next(total);
                }
                Vec::new()
            }
            KeyCode::Up => {
                if let Some(total) = active_input_count(app) {
                    app.workflows.input_view_state_mut().unwrap().select_prev(total);
                }
                Vec::new()
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if let Some(state) = app.workflows.active_run_state_mut() {
                    if let Err(err) = state.evaluate_input_providers() {
                        app.logs.entries.push(format!("Provider evaluation error: {err}"));
                    } else {
                        app.workflows.observe_provider_refresh_current();
                    }
                }
                Vec::new()
            }
            KeyCode::Enter => match app.execute_workflow_from_inputs() {
                Ok(effects) => effects,
                Err(err) => {
                    app.logs.entries.push(format!("Workflow execution error: {err}"));
                    Vec::new()
                }
            },
            _ => Vec::new(),
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        if app.workflows.active_run_state().is_none() {
            render_empty(frame, area, &*app.ctx.theme);
            return;
        };

        let rows = build_input_rows(app);
        if let Some(state) = app.workflows.input_view_state_mut() {
            state.clamp_selection(rows.len());
        }

        let header_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(area);

        render_header(frame, header_area[0], app, rows.len());

        let content_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(header_area[1]);

        render_inputs_list(frame, content_layout[0], app, &rows);
        render_input_details(frame, content_layout[1], app, &rows);
    }
}

fn render_empty(frame: &mut Frame, area: Rect, theme: &dyn Theme) {
    let block = th::block(theme, Some("Workflow Inputs"), false);
    frame.render_widget(block, area);
}

fn render_header(frame: &mut Frame, area: Rect, app: &mut App, total_inputs: usize) {
    let run_state = app.workflows.active_run_state().unwrap();
    let theme = &*app.ctx.theme;
    let workflow = &run_state.workflow;
    let title = workflow
        .title
        .as_deref()
        .filter(|title| !title.is_empty())
        .unwrap_or(&workflow.identifier);

    let subtitle = format!("{} inputs • {} steps", total_inputs, workflow.steps.len());

    let block = th::block(theme, Some(title), true);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = vec![
        Line::from(Span::styled(title, theme.text_primary_style().add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(subtitle, theme.text_secondary_style())),
        Line::from(Span::styled(
            "↑↓ Navigate  •  Enter Run  •  r Refresh providers  •  Esc Back",
            theme.text_muted_style(),
        )),
    ];

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);
}

fn render_inputs_list(frame: &mut Frame, area: Rect, app: &App, rows: &[WorkflowInputRow]) {
    let theme = &*app.ctx.theme;
    let block = th::block(theme, Some("Inputs"), matches!(app.workflows.input_view_state(), Some(_)));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if rows.is_empty() {
        let paragraph = Paragraph::new("This workflow has no declared inputs.")
            .style(theme.text_muted_style())
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, inner);
        return;
    }

    let selected = app.workflows.input_view_state().map(|state| state.selected()).unwrap_or(0);

    let mut lines = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let marker = if index == selected { "▸" } else { " " };
        let status_span = match row.status {
            InputStatus::Resolved => Span::styled("[resolved]", theme.status_success()),
            InputStatus::Pending => Span::styled("[pending]", theme.status_warning()),
            InputStatus::Error => Span::styled("[error]", theme.status_error()),
        };

        let mut segments = vec![Span::raw(format!("{marker} {}", row.name)), Span::raw("  "), status_span];

        if let Some(provider) = &row.provider_label {
            segments.push(Span::raw("  "));
            segments.push(Span::styled(format!("[provider: {provider}]"), theme.text_secondary_style()));
        }

        if row.required {
            segments.push(Span::raw("  "));
            segments.push(Span::styled("[required]", theme.text_secondary_style()));
        }

        lines.push(Line::from(segments));

        if let Some(message) = &row.status_message {
            lines.push(Line::from(vec![Span::styled(format!("    {message}"), theme.text_muted_style())]));
        }
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true }).style(theme.text_primary_style());
    frame.render_widget(paragraph, inner);
}

fn render_input_details(frame: &mut Frame, area: Rect, app: &App, rows: &[WorkflowInputRow]) {
    let theme = &*app.ctx.theme;
    let block = th::block(theme, Some("Details"), false);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(view_state) = app.workflows.input_view_state() else {
        return;
    };

    let Some(row) = rows.get(view_state.selected()) else {
        let paragraph = Paragraph::new("Select an input to inspect details.")
            .style(theme.text_muted_style())
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, inner);
        return;
    };

    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("Input: ", theme.text_secondary_style()),
        Span::styled(&row.name, theme.text_primary_style().add_modifier(Modifier::BOLD)),
    ]));

    if let Some(description) = &row.description {
        lines.push(Line::from(Span::styled(description, theme.text_secondary_style())));
    }

    if let Some(value) = &row.current_value {
        lines.push(Line::from(vec![
            Span::styled("Current value: ", theme.text_secondary_style()),
            Span::styled(value, theme.text_primary_style()),
        ]));
    }

    if let Some(provider) = &row.provider_label {
        lines.push(Line::from(vec![
            Span::styled("Provider: ", theme.text_secondary_style()),
            Span::styled(provider, theme.text_primary_style()),
        ]));
    }

    match row.status {
        InputStatus::Resolved => {
            lines.push(Line::from(Span::styled("Status: resolved", theme.status_success())));
        }
        InputStatus::Pending => {
            lines.push(Line::from(Span::styled("Status: pending", theme.status_warning())));
        }
        InputStatus::Error => {
            lines.push(Line::from(Span::styled("Status: error", theme.status_error())));
        }
    }

    if let Some(message) = &row.status_message {
        lines.push(Line::from(Span::styled(message, theme.text_muted_style())));
    }

    if let Some(summary) = &row.cache_summary {
        lines.push(Line::from(vec![
            Span::styled("Cache: ", theme.text_secondary_style()),
            Span::styled(format_cache_summary(summary), theme.text_primary_style()),
        ]));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);
}

fn active_input_count(app: &App) -> Option<usize> {
    app.workflows.active_run_state().map(|state| state.workflow.inputs.len())
}

fn build_input_rows(app: &App) -> Vec<WorkflowInputRow> {
    let mut rows = Vec::new();
    let run_state = app.workflows.active_run_state().unwrap();
    for (name, definition) in run_state.workflow.inputs.iter() {
        rows.push(build_input_row(app, run_state, name, definition));
    }

    rows
}

fn build_input_row(
    app: &App,
    run_state: &heroku_engine::WorkflowRunState,
    name: &str,
    definition: &heroku_types::workflow::WorkflowInputDefinition,
) -> WorkflowInputRow {
    let required = definition.validate.as_ref().map(|validate| validate.required).unwrap_or(false);

    let provider_label = definition.provider.as_ref().map(|provider| match provider {
        heroku_types::workflow::WorkflowValueProvider::Id(id) => id.clone(),
        heroku_types::workflow::WorkflowValueProvider::Detailed(detail) => detail.id.clone(),
    });

    let provider_key = provider_label.as_ref().map(|id| format!("{name}:{id}"));

    let provider_state = run_state.provider_state_for(name);
    let mut status = InputStatus::Pending;
    let mut status_message = None;

    if let Some(state) = provider_state {
        for outcome_state in state.argument_outcomes.values() {
            match &outcome_state.outcome {
                ProviderBindingOutcome::Error(error) => {
                    status = InputStatus::Error;
                    status_message = Some(error.message.clone());
                    break;
                }
                ProviderBindingOutcome::Prompt(prompt) => {
                    status = InputStatus::Pending;
                    status_message = Some(prompt.reason.message.clone());
                }
                ProviderBindingOutcome::Skip(skip) => {
                    status = InputStatus::Pending;
                    status_message = Some(skip.reason.message.clone());
                }
                ProviderBindingOutcome::Resolved(_) => {
                    if !matches!(status, InputStatus::Error) {
                        status = InputStatus::Resolved;
                    }
                }
            }
        }
    } else if run_state.run_context.inputs.get(name).is_some() {
        status = InputStatus::Resolved;
    } else if !required {
        status = InputStatus::Pending;
        status_message = Some("Optional value not provided.".to_string());
    }

    let current_value = run_state.run_context.inputs.get(name).map(|value| format_preview(value));

    let description = definition.description.clone();

    let cache_summary = provider_key
        .as_deref()
        .and_then(|key| app.workflows.provider_snapshot(key))
        .map(ProviderCacheSummary::from_snapshot);

    WorkflowInputRow {
        name: name.to_string(),
        required,
        provider_label,
        status,
        status_message,
        description,
        current_value,
        cache_summary,
    }
}

#[derive(Debug)]
struct WorkflowInputRow {
    name: String,
    required: bool,
    provider_label: Option<String>,
    status: InputStatus,
    status_message: Option<String>,
    description: Option<String>,
    current_value: Option<String>,
    cache_summary: Option<ProviderCacheSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputStatus {
    Resolved,
    Pending,
    Error,
}
