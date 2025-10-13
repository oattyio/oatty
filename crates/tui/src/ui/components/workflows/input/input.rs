use crossterm::event::{KeyCode, KeyEvent};
use heroku_engine::{ProviderBindingOutcome, WorkflowRunState};
use heroku_types::{Effect, Modal, Route, WorkflowInputDefinition, WorkflowValueProvider};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::App;
use crate::ui::components::component::Component;
use crate::ui::components::workflows::view_utils::{ProviderCacheSummary, format_cache_summary, format_preview, summarize_values};
use crate::ui::theme::{roles::Theme, theme_helpers as th};

#[derive(Debug, Default)]
pub struct WorkflowInputsComponent;

impl Component for WorkflowInputsComponent {
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let mut effects = Vec::new();
        if app.workflows.input_view_state().is_none() {
            return Vec::new();
        };

        match key.code {
            KeyCode::Esc => {
                app.close_workflow_inputs();
                effects.push(Effect::SwitchTo(Route::Workflows));
            }
            KeyCode::Down => {
                if let Some(total) = active_input_count(app) {
                    app.workflows.input_view_state_mut().unwrap().select_next(total);
                }
            }
            KeyCode::Up => {
                if let Some(total) = active_input_count(app) {
                    app.workflows.input_view_state_mut().unwrap().select_prev(total);
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if let Some(state) = app.workflows.active_run_state_mut() {
                    if let Err(err) = state.evaluate_input_providers() {
                        app.logs.entries.push(format!("Provider evaluation error: {err}"));
                    } else {
                        app.workflows.observe_provider_refresh_current();
                    }
                }
            }
            KeyCode::Enter => {
                // If the selected input is unresolved, open the Guided Input Collector modal.
                if app.workflows.active_input_definition().is_some() {
                    effects.push(Effect::ShowModal(Modal::WorkflowCollector));
                }
            }
            _ => {},
        }
        effects
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let block = th::block(&*app.ctx.theme, Some("Pre-run Input Viewer"), true);
        let inner = block.inner(area);

        if app.workflows.active_run_state().is_none() {
            render_empty(frame, inner, &*app.ctx.theme);
            return;
        };

        let rows = build_input_rows(app);
        if let Some(state) = app.workflows.input_view_state_mut() {
            state.clamp_selection(rows.len());
        }

        let _selected_status = app
            .workflows
            .input_view_state()
            .and_then(|s| rows.get(s.selected()))
            .map(|r| r.status);

        let splits = Layout::vertical([
            Constraint::Length(2),       // header height
            Constraint::Percentage(100), // content height
            Constraint::Min(1),          // Hints bar height
        ])
        .split(inner);

        render_header(frame, splits[0], app, rows.len());

        let content_layout = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).split(splits[1]);

        render_inputs_list(frame, content_layout[0], app, &rows);
        render_input_details(frame, content_layout[1], app, &rows);

        let hint_spans = self.get_hint_spans(app, true);
        let hints_widget = Paragraph::new(Line::from(hint_spans)).style(app.ctx.theme.text_muted_style());
        frame.render_widget(hints_widget, splits[2]);
    }

    fn get_hint_spans(&self, app: &App, is_root: bool) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        let mut spans = Vec::new();
        if is_root {
            spans.push(Span::styled("Hints: ", theme.text_muted_style()));
        }
        spans.extend([
            Span::styled("Esc", theme.accent_emphasis_style()),
            Span::styled(" Back", theme.text_muted_style()),
            Span::styled(" ↑/↓", theme.accent_emphasis_style()),
            Span::styled(" Navigate", theme.text_muted_style()),
            Span::styled(" Enter", theme.accent_emphasis_style()),
            Span::styled(" Pick input", theme.text_muted_style()),
            Span::styled(" Ctrl+R", theme.accent_emphasis_style()),
            Span::styled(" Refresh", theme.text_muted_style()),
        ]);
        spans
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
    let description = workflow
        .description
        .as_deref()
        .filter(|description| !description.is_empty())
        .unwrap_or(&workflow.identifier);

    let unresolved = app.workflows.unresolved_item_count();
    let subtitle = if unresolved == 0 {
        format!("{} inputs • {} steps", total_inputs, workflow.steps.len())
    } else {
        format!(
            "{} inputs • {} steps • {} unresolved",
            total_inputs,
            workflow.steps.len(),
            unresolved
        )
    };

    let spans = vec![
        Span::styled(
            format!("Workflow: {} - {} • ", title, description),
            theme.text_primary_style().add_modifier(Modifier::BOLD),
        ),
        Span::styled(subtitle, theme.text_secondary_style()),
    ];

    let paragraph = Paragraph::new(Line::from(spans)).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn render_inputs_list(frame: &mut Frame, area: Rect, app: &App, rows: &[WorkflowInputRow]) {
    let theme = &*app.ctx.theme;
    let block = th::block(theme, Some("Inputs"), app.workflows.input_view_state().is_some());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(view_state) = app.workflows.input_view_state() else {
        return;
    };

    if rows.is_empty() {
        let paragraph = Paragraph::new("This workflow has no declared inputs.")
            .style(theme.text_muted_style())
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, inner);
        return;
    }

    let selected = view_state.selected();

    // Build list items without manual marker; the List widget will handle highlight/marker
    let mut items: Vec<ListItem> = Vec::with_capacity(rows.len());
    for row in rows.iter() {
        let status_span = match row.status {
            InputStatus::Resolved => Span::styled(format!("{:<12}", "✓ Looks good!"), theme.status_success()),
            InputStatus::Pending => Span::styled(format!("{:<12}", "⚠ No value"), theme.status_warning()),
            InputStatus::Error => Span::styled(format!("{:<12}", "X error"), theme.status_error()),
        };

        let mut segments = vec![Span::raw(format!("{:<20}", row.name)), status_span];

        let provider_label = row.provider_label.as_ref().map(|label| format!("[provider: {label}]"));
        segments.push(Span::styled(
            format!("{:<30}", provider_label.unwrap_or_default()),
            theme.text_secondary_style(),
        ));

        if row.required {
            segments.push(Span::styled("[required]", theme.text_secondary_style()));
        }
        if let Some(message) = &row.status_message {
            segments.push(Span::styled(format!("    {message}"), theme.text_muted_style()));
        }

        let line = Line::from(segments);
        items.push(ListItem::new(line));
    }

    let list = List::new(items)
        .style(theme.text_primary_style())
        .highlight_style(theme.selection_style().add_modifier(Modifier::BOLD))
        .highlight_symbol("▸ ");

    let mut list_state = ListState::default();
    list_state.select(Some(selected));

    frame.render_stateful_widget(list, inner, &mut list_state);
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

    let run_state = app.workflows.active_run_state().unwrap();
    let definition = run_state.workflow.inputs.get(&row.name).cloned().unwrap_or_default();

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

    if let Some(input_type) = definition.r#type.as_deref() {
        lines.push(Line::from(vec![
            Span::styled("Type: ", theme.text_secondary_style()),
            Span::styled(input_type, theme.text_primary_style()),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled("Mode: ", theme.text_secondary_style()),
        Span::styled(
            match definition.mode {
                heroku_types::workflow::WorkflowInputMode::Single => "single",
                heroku_types::workflow::WorkflowInputMode::Multiple => "multiple",
            },
            theme.text_primary_style(),
        ),
    ]));

    if let Some(select) = definition.select.as_ref() {
        let mut select_parts = Vec::new();
        if let Some(field) = select.display_field.as_deref() {
            select_parts.push(format!("display={field}"));
        }
        if let Some(field) = select.value_field.as_deref() {
            select_parts.push(format!("value={field}"));
        }
        if let Some(field) = select.id_field.as_deref() {
            select_parts.push(format!("id={field}"));
        }
        if !select_parts.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Select: ", theme.text_secondary_style()),
                Span::styled(select_parts.join(" • "), theme.text_primary_style()),
            ]));
        }
    }

    if !definition.enumerated_values.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Enumerated: ", theme.text_secondary_style()),
            Span::styled(summarize_values(&definition.enumerated_values, 8), theme.text_primary_style()),
        ]));
    }

    if let Some(validate) = definition.validate.as_ref() {
        if !validate.allowed_values.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Allowed: ", theme.text_secondary_style()),
                Span::styled(summarize_values(&validate.allowed_values, 8), theme.text_primary_style()),
            ]));
        }
        if let Some(pattern) = validate.pattern.as_deref() {
            lines.push(Line::from(vec![
                Span::styled("Pattern: ", theme.text_secondary_style()),
                Span::styled(pattern, theme.text_primary_style()),
            ]));
        }
    }

    if let Some(policy) = definition.on_error {
        lines.push(Line::from(vec![
            Span::styled("on_error: ", theme.text_secondary_style()),
            Span::styled(
                match policy {
                    heroku_types::workflow::WorkflowProviderErrorPolicy::Manual => "manual",
                    heroku_types::workflow::WorkflowProviderErrorPolicy::Cached => "cached",
                    heroku_types::workflow::WorkflowProviderErrorPolicy::Fail => "fail",
                },
                theme.text_primary_style(),
            ),
        ]));
    }

    if !definition.provider_args.is_empty() {
        let arg_list = definition.provider_args.keys().cloned().collect::<Vec<_>>().join(", ");
        lines.push(Line::from(vec![
            Span::styled("Provider args: ", theme.text_secondary_style()),
            Span::styled(arg_list, theme.text_primary_style()),
        ]));
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

fn build_input_row(app: &App, run_state: &WorkflowRunState, name: &str, definition: &WorkflowInputDefinition) -> WorkflowInputRow {
    let required = definition.validate.as_ref().map(|validate| validate.required).unwrap_or(false);

    let provider_label = definition.provider.as_ref().map(|provider| match provider {
        WorkflowValueProvider::Id(id) => id.clone(),
        WorkflowValueProvider::Detailed(detail) => detail.id.clone(),
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
        status_message = Some("(This field is optional)".to_string());
    }

    let current_value = run_state.run_context.inputs.get(name).map(format_preview);

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
