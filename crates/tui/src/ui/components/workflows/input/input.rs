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
use crate::ui::components::workflows::state::validate_candidate_value;
use crate::ui::components::workflows::view_utils::format_preview;
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
            KeyCode::Enter => {
                if let Some(def) = app.workflows.active_input_definition() {
                    // Route: provider present → selector (collector modal); else → manual entry
                    if def.provider.is_some() {
                        app.workflows.open_selector_for_active_input();
                        effects.extend(app.prepare_selector_fetch());
                    } else {
                        app.workflows.open_manual_for_active_input();
                    }
                    effects.push(Effect::ShowModal(Modal::WorkflowCollector));
                }
            }
            KeyCode::F(2) => {
                // F2 fallback to manual entry regardless of provider presence
                app.workflows.open_manual_for_active_input();
                effects.push(Effect::ShowModal(Modal::WorkflowCollector));
            }
            _ => {}
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

        let splits = Layout::vertical([
            Constraint::Length(2),       // header height
            Constraint::Percentage(100), // content height
        ])
        .split(inner);

        render_header(frame, splits[0], app, rows.len());

        let content_layout = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).split(splits[1]);

        render_inputs_list(frame, content_layout[0], app, &rows);
        render_input_details(frame, content_layout[1], app, &rows);
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        [
            Span::styled("Esc", theme.accent_emphasis_style()),
            Span::styled(" Back", theme.text_muted_style()),
            Span::styled(" ↑/↓", theme.accent_emphasis_style()),
            Span::styled(" Navigate", theme.text_muted_style()),
            Span::styled(" Enter", theme.accent_emphasis_style()),
            Span::styled(" Pick input", theme.text_muted_style()),
        ]
        .to_vec()
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

    // Build list items without a manual marker; the List widget will handle highlight/marker
    let mut items: Vec<ListItem> = Vec::with_capacity(rows.len());
    for row in rows.iter() {
        let status_span = match row.status {
            InputStatus::Resolved => Span::styled(format!("{:<15}", "✓ Looks good!"), theme.status_success()),
            InputStatus::Pending => Span::styled(format!("{:<15}", "⚠ No value"), theme.status_warning()),
            InputStatus::Error => Span::styled(format!("{:<15}", "X error"), theme.status_error()),
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
            segments.push(Span::styled(format!("{message}"), theme.text_muted_style()));
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
    let block = th::block(theme, Some("Workflow Details"), false);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.workflows.active_run_state().is_none() {
        return;
    }

    // Aggregate readiness information
    let total = rows.len();
    let unresolved = app.workflows.unresolved_item_count();
    let resolved = total.saturating_sub(unresolved);

    // Next action = first unresolved or error entry
    let mut next_action: Option<&str> = None;
    for row in rows {
        if matches!(row.status, InputStatus::Error | InputStatus::Pending) {
            next_action = Some(&row.name);
            break;
        }
    }

    // Selected values list
    let mut selected_lines: Vec<Line> = Vec::new();
    for row in rows {
        let value_display = row
            .current_value
            .as_deref()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "— pending —".to_string());
        selected_lines.push(Line::from(vec![
            Span::styled("  • ", theme.text_secondary_style()),
            Span::styled(format!("{:<14}", row.name), theme.text_primary_style()),
            Span::styled(" → ", theme.text_secondary_style()),
            if row.current_value.is_some() {
                Span::styled(value_display, theme.status_success())
            } else {
                Span::styled(value_display, theme.text_muted_style())
            },
        ]));
    }

    // Cache age summary (best-effort: show provider labels; age unknown if not tracked)
    let mut cache_lines: Vec<Line> = Vec::new();
    {
        use std::collections::BTreeMap;
        let mut providers: BTreeMap<&str, Option<String>> = BTreeMap::new();
        for row in rows {
            if let Some(label) = row.provider_label.as_deref() {
                // Age unknown in the current model; show placeholder "—"
                providers.entry(label).or_insert(None);
            }
        }
        if providers.is_empty() {
            cache_lines.push(Line::from(Span::styled("Cache age: —", theme.text_muted_style())));
        } else {
            cache_lines.push(Line::from(vec![Span::styled("Cache age:", theme.text_secondary_style())]));
            for (label, _age) in providers.into_iter() {
                cache_lines.push(Line::from(vec![
                    Span::styled("  • ", theme.text_secondary_style()),
                    Span::styled(format!("{:<12}", label), theme.text_primary_style()),
                    Span::styled(" ", theme.text_secondary_style()),
                    Span::styled("—", theme.text_muted_style()),
                ]));
            }
        }
    }

    // Errors & notes aggregation
    let mut error_notes: Vec<String> = Vec::new();
    for row in rows {
        if let Some(message) = &row.status_message {
            error_notes.push(message.clone());
        }
    }

    // Build final lines according to spec layout
    let mut lines: Vec<Line> = Vec::new();
    // Ready?
    let ready_label = if unresolved == 0 {
        Span::styled("Ready?: ✓ All inputs resolved", theme.status_success())
    } else {
        Span::styled(
            format!("Ready?: ⚠ Waiting on {}", next_action.unwrap_or("—")),
            theme.status_warning(),
        )
    };
    lines.push(Line::from(ready_label));

    // Resolved count
    lines.push(Line::from(vec![
        Span::styled("Resolved inputs: ", theme.text_secondary_style()),
        Span::styled(format!("{} / {}", resolved, total), theme.text_primary_style()),
    ]));

    // Next action
    lines.push(Line::from(vec![
        Span::styled("Next action: ", theme.text_secondary_style()),
        Span::styled(
            next_action.unwrap_or("—"),
            if unresolved == 0 {
                theme.text_muted_style()
            } else {
                theme.text_primary_style()
            },
        ),
    ]));

    // Spacer
    lines.push(Line::from(""));

    // Selected values header
    lines.push(Line::from(Span::styled("Selected values:", theme.text_secondary_style())));
    lines.extend(selected_lines);

    // Spacer and auto-reset note
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Auto-reset note: ", theme.text_secondary_style()),
        Span::styled("downstream steps reset when a prior step edits.", theme.text_muted_style()),
    ]));

    // Spacer and cache ages
    lines.push(Line::from(""));
    lines.extend(cache_lines);

    // Spacer and errors
    lines.push(Line::from(""));
    if error_notes.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Errors & notes: ", theme.text_secondary_style()),
            Span::styled("none", theme.text_muted_style()),
        ]));
    } else {
        lines.push(Line::from(Span::styled("Errors & notes:", theme.text_secondary_style())));
        for note in error_notes {
            lines.push(Line::from(vec![
                Span::styled("  • ", theme.text_secondary_style()),
                Span::styled(note, theme.text_primary_style()),
            ]));
        }
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
        rows.push(build_input_row(run_state, name, definition));
    }

    rows
}

fn build_input_row(run_state: &WorkflowRunState, name: &str, definition: &WorkflowInputDefinition) -> WorkflowInputRow {
    let required = definition.is_required();

    let provider_label = definition.provider.as_ref().map(|provider| match provider {
        WorkflowValueProvider::Id(id) => id.clone(),
        WorkflowValueProvider::Detailed(detail) => detail.id.clone(),
    });

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
                ProviderBindingOutcome::Resolved(_) => {}
            }
        }
    }

    let raw_value = run_state.run_context.inputs.get(name);
    let has_value = raw_value.map_or(false, has_meaningful_input_value);

    if matches!(status, InputStatus::Error) {
        // Preserve error state and explanatory message coming from provider resolution.
    } else if has_value {
        if let Some(value) = raw_value {
            if let Some(validation) = &definition.validate {
                match value {
                    serde_json::Value::String(text) => match validate_candidate_value(text, validation) {
                        Ok(()) => {
                            status = InputStatus::Resolved;
                            status_message = None;
                        }
                        Err(message) => {
                            status = InputStatus::Error;
                            status_message = Some(message);
                        }
                    },
                    other => {
                        if !validation.allowed_values.is_empty() {
                            let matches_allowed = validation.allowed_values.iter().any(|allowed| allowed == other);
                            if matches_allowed {
                                status = InputStatus::Resolved;
                                status_message = None;
                            } else {
                                status = InputStatus::Error;
                                status_message = Some("value is not in the allowed set".to_string());
                            }
                        } else if validation.pattern.is_some() || validation.min_length.is_some() || validation.max_length.is_some() {
                            status = InputStatus::Error;
                            status_message = Some("value must be text to satisfy validation rules".to_string());
                        } else {
                            status = InputStatus::Resolved;
                            status_message = None;
                        }
                    }
                }
            } else {
                status = InputStatus::Resolved;
                status_message = None;
            }
        }
    } else {
        status = InputStatus::Pending;
        if !required && status_message.is_none() {
            status_message = Some("[optional]".to_string());
        }
    }

    let current_value = raw_value.map(format_preview);
    let description = definition.description.clone();

    WorkflowInputRow {
        name: name.to_string(),
        required,
        provider_label,
        status,
        status_message,
        description,
        current_value,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputStatus {
    Resolved,
    Pending,
    Error,
}

/// Returns `true` when the input value contains data that should mark the input as resolved.
fn has_meaningful_input_value(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::String(text) => !text.trim().is_empty(),
        serde_json::Value::Array(elements) => !elements.is_empty(),
        serde_json::Value::Object(properties) => !properties.is_empty(),
        _ => true,
    }
}
