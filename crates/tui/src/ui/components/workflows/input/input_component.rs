use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use heroku_engine::{ProviderBindingOutcome, WorkflowRunState};
use heroku_types::{Effect, Modal, Route, WorkflowInputDefinition, WorkflowProviderArgumentValue, WorkflowValueProvider};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::app::App;
use crate::ui::components::component::Component;
use crate::ui::components::workflows::input::state::WorkflowInputLayout;
use crate::ui::components::workflows::view_utils::{JsonSyntaxRole, classify_json_value, format_preview, style_for_role};
use crate::ui::theme::{
    roles::Theme,
    theme_helpers::{self as th, ButtonRenderOptions},
};
use heroku_types::workflow::validate_candidate_value;

#[derive(Debug, Default)]
pub struct WorkflowInputsComponent;

impl Component for WorkflowInputsComponent {
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let mut effects = Vec::new();
        if app.workflows.input_view_state().is_none() {
            return effects;
        };

        match key.code {
            KeyCode::Tab => {
                app.focus.next();
                return effects;
            }
            KeyCode::BackTab => {
                app.focus.prev();
                return effects;
            }
            KeyCode::Esc => {
                effects.push(Effect::SwitchTo(Route::Workflows));
                return effects;
            }
            _ => {}
        }

        let list_focused = app.workflows.input_view_state().map(|state| state.f_list.get()).unwrap_or(false);
        let cancel_focused = app
            .workflows
            .input_view_state()
            .map(|state| state.f_cancel_button.get())
            .unwrap_or(false);
        let run_focused = app
            .workflows
            .input_view_state()
            .map(|state| state.f_run_button.get())
            .unwrap_or(false);

        if list_focused {
            match key.code {
                KeyCode::Down => {
                    if app.workflows.active_run_state().is_some() {
                        let rows = build_input_rows(app);
                        if let Some(state) = app.workflows.input_view_state_mut()
                            && let Some(next) = advance_selection(&rows, state.selected(), Direction::Forward)
                        {
                            state.set_selected(next);
                        }
                    }
                }
                KeyCode::Up => {
                    if app.workflows.active_run_state().is_some() {
                        let rows = build_input_rows(app);
                        if let Some(state) = app.workflows.input_view_state_mut()
                            && let Some(prev) = advance_selection(&rows, state.selected(), Direction::Backward)
                        {
                            state.set_selected(prev);
                        }
                    }
                }
                KeyCode::Enter => {
                    if app.workflows.active_run_state().is_some() {
                        let rows = build_input_rows(app);
                        if let Some(reason) = current_row_block_reason(app, &rows) {
                            app.append_log_message(format!("Input blocked: {reason}"));
                            return effects;
                        }
                    }
                    if let Some(definition) = app.workflows.active_input_definition() {
                        if definition.provider.is_some() {
                            app.workflows.open_selector_for_active_input(&app.ctx.command_registry);
                            effects.extend(app.prepare_selector_fetch());
                        } else {
                            app.workflows.open_manual_for_active_input();
                        }
                        effects.push(Effect::ShowModal(Modal::WorkflowCollector));
                    }
                }
                KeyCode::F(2) => {
                    if app.workflows.active_run_state().is_some() {
                        let rows = build_input_rows(app);
                        if current_row_block_reason(app, &rows).is_some() {
                            return effects;
                        }
                    }
                    app.workflows.open_manual_for_active_input();
                    effects.push(Effect::ShowModal(Modal::WorkflowCollector));
                }
                _ => {}
            }
            return effects;
        }

        if cancel_focused {
            match key.code {
                KeyCode::Left | KeyCode::Up => focus_input_list(app),
                KeyCode::Right => focus_run_button(app),
                KeyCode::Enter | KeyCode::Char(' ') => effects.push(Effect::SwitchTo(Route::Workflows)),
                _ => {}
            }
            return effects;
        }

        if run_focused {
            match key.code {
                KeyCode::Left => focus_cancel_button(app),
                KeyCode::Right | KeyCode::Down => focus_input_list(app),
                KeyCode::Enter | KeyCode::Char(' ') => effects.extend(app.run_active_workflow()),
                _ => {}
            }
            return effects;
        }

        effects
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return Vec::new();
        }

        let (layout, cancel_flag, run_flag) = if let Some(state) = app.workflows.input_view_state() {
            (*state.layout(), state.f_cancel_button.clone(), state.f_run_button.clone())
        } else {
            return Vec::new();
        };

        if let Some(area) = layout.cancel_button_area
            && rect_contains(area, mouse.column, mouse.row)
        {
            app.focus.focus(&cancel_flag);
            return vec![Effect::SwitchTo(Route::Workflows)];
        }

        if let Some(area) = layout.run_button_area
            && rect_contains(area, mouse.column, mouse.row)
        {
            app.focus.focus(&run_flag);
            return app.run_active_workflow();
        }

        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let block = th::block(&*app.ctx.theme, Some("Pre-run Input Viewer"), true);
        let inner = block.inner(area);

        if app.workflows.active_run_state().is_none() {
            render_empty(frame, inner, &*app.ctx.theme);
            return;
        };

        let rows = build_input_rows(app);
        let run_enabled = app.workflows.unresolved_item_count() == 0;
        if let Some(state) = app.workflows.input_view_state_mut() {
            state.clamp_selection(rows.len());
            if let Some(selected) = rows.get(state.selected())
                && selected.is_blocked()
                && let Some(first) = first_enabled_index(&rows)
            {
                state.set_selected(first);
            }
        }

        let splits = Layout::vertical([
            Constraint::Length(2), // header height
            Constraint::Min(6),    // content height
            Constraint::Length(3), // footer height
        ])
        .split(inner);

        render_header(frame, splits[0], app, rows.len());

        let content_layout = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).split(splits[1]);

        render_inputs_list(frame, content_layout[0], app, &rows);
        render_input_details(frame, content_layout[1], app, &rows);
        render_footer(frame, splits[2], app, run_enabled);
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        if let Some(state) = app.workflows.input_view_state() {
            if state.f_run_button.get() {
                return th::build_hint_spans(
                    theme,
                    &[
                        ("Esc", " Cancel"),
                        ("←/→", " Switch"),
                        ("Enter", " Run workflow"),
                        ("Tab", " Cycle focus"),
                    ],
                );
            }
            if state.f_cancel_button.get() {
                return th::build_hint_spans(
                    theme,
                    &[
                        ("Esc", " Cancel"),
                        ("←/→", " Switch"),
                        ("Enter", " Close inputs"),
                        ("Tab", " Cycle focus"),
                    ],
                );
            }
        }
        th::build_hint_spans(
            theme,
            &[
                ("Esc", " Cancel"),
                (" ↑/↓", " Navigate"),
                (" Enter", " Collect input"),
                (" F2", " Manual entry"),
            ],
        )
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
            InputStatus::Blocked => Span::styled(format!("{:<15}", "Waiting..."), theme.status_warning().add_modifier(Modifier::BOLD)),
        };

        let mut name_style: Style = theme.syntax_type_style();
        if row.is_blocked() {
            name_style = name_style.add_modifier(Modifier::DIM);
        }

        let mut segments = vec![Span::styled(format!("{:<20}", row.name), name_style), status_span];
        if row.required {
            segments.push(Span::styled("[required]", theme.syntax_keyword_style()));
        }

        if row.provider_label.is_some() {
            segments.push(Span::styled(format!("{:>3}", "⇄"), theme.text_secondary_style()));
        }

        if let Some(reason) = &row.blocked_reason {
            segments.push(Span::styled(
                format!(" [{}]", reason),
                theme.status_warning().add_modifier(Modifier::BOLD),
            ));
        }

        if let Some(message) = &row.status_message
            && row.blocked_reason.as_ref() != Some(message)
        {
            let message_style = match row.status {
                InputStatus::Resolved => theme.text_muted_style(),
                InputStatus::Pending => theme.text_muted_style(),
                InputStatus::Error => theme.status_error(),
                InputStatus::Blocked => theme.status_warning(),
            };
            segments.push(Span::styled(message.to_string(), message_style));
        }

        if row.is_blocked() {
            segments.push(Span::styled(" [disabled]", theme.text_muted_style().add_modifier(Modifier::DIM)));
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
        if matches!(row.status, InputStatus::Error | InputStatus::Pending | InputStatus::Blocked) {
            next_action = Some(&row.name);
            break;
        }
    }

    // Selected values list
    let mut selected_lines: Vec<Line> = Vec::new();
    for row in rows {
        let mut line_segments = vec![Span::styled("  • ", theme.text_secondary_style())];

        let mut name_style: Style = theme.syntax_type_style();
        if row.is_blocked() {
            name_style = name_style.add_modifier(Modifier::DIM);
        }
        line_segments.push(Span::styled(format!("{:<14}", row.name), name_style));
        line_segments.push(Span::styled(" → ", theme.syntax_keyword_style()));

        match row.status {
            InputStatus::Resolved => {
                line_segments.push(Span::styled("✓ ", theme.status_success()));
                if let Some(preview) = &row.current_value {
                    line_segments.push(Span::styled(preview.text.clone(), style_for_role(preview.role, theme)));
                } else {
                    line_segments.push(Span::styled("— pending —", theme.text_muted_style()));
                }
            }
            InputStatus::Pending => {
                line_segments.push(Span::styled("— pending —", theme.text_muted_style()));
            }
            InputStatus::Error => {
                line_segments.push(Span::styled("✖ ", theme.status_error()));
            }
            InputStatus::Blocked => {
                line_segments.push(Span::styled("— blocked —", theme.status_warning()));
            }
        }

        if let Some(reason) = &row.blocked_reason {
            line_segments.push(Span::styled(format!(" [{}]", reason), theme.status_warning()));
        }

        if let Some(message) = &row.status_message
            && row.blocked_reason.as_ref() != Some(message)
        {
            let message_style = match row.status {
                InputStatus::Resolved => theme.text_muted_style(),
                InputStatus::Pending => theme.text_muted_style(),
                InputStatus::Error => theme.status_error(),
                InputStatus::Blocked => theme.status_warning(),
            };
            line_segments.push(Span::styled(format!(" {message}"), message_style));
        }

        selected_lines.push(Line::from(line_segments));
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

fn render_footer(frame: &mut Frame, area: Rect, app: &mut App, run_enabled: bool) {
    if area.height == 0 || area.width == 0 {
        if let Some(state) = app.workflows.input_view_state_mut() {
            state.set_layout(WorkflowInputLayout::default());
        }
        return;
    }

    let theme = &*app.ctx.theme;
    let (cancel_focused, run_focused) = app
        .workflows
        .input_view_state()
        .map(|state| (state.f_cancel_button.get(), state.f_run_button.get()))
        .unwrap_or((false, false));

    let layout_areas = Layout::horizontal([
        Constraint::Length(12), // cancel
        Constraint::Length(12), // run
        Constraint::Length(2),  // padding
        Constraint::Min(0),     // status line
    ])
    .split(area);

    let cancel_options = ButtonRenderOptions::new(true, cancel_focused, cancel_focused, Borders::ALL, false);
    th::render_button(frame, layout_areas[0], "Cancel", theme, cancel_options);

    let run_options = ButtonRenderOptions::new(run_enabled, run_focused, run_focused, Borders::ALL, true);
    th::render_button(frame, layout_areas[1], "Run", theme, run_options);

    let unresolved = app.workflows.unresolved_item_count();
    let status_line = if run_enabled {
        Span::styled("All required inputs resolved — ready to run.", theme.status_success())
    } else {
        Span::styled(
            format!("Resolve {unresolved} required input(s) to enable Run."),
            theme.status_warning(),
        )
    };
    let mut status_layout = layout_areas[3];
    status_layout.y += 1;
    frame.render_widget(Paragraph::new(Line::from(status_line)).wrap(Wrap { trim: true }), status_layout);

    if let Some(state) = app.workflows.input_view_state_mut() {
        state.set_layout(WorkflowInputLayout {
            cancel_button_area: Some(layout_areas[0]),
            run_button_area: Some(layout_areas[1]),
        });
    }
}

fn build_input_rows(app: &App) -> Vec<WorkflowInputRow> {
    let mut rows = Vec::new();
    let run_state = app.workflows.active_run_state().unwrap();
    for (name, definition) in run_state.workflow.inputs.iter() {
        rows.push(build_input_row(run_state, name, definition));
    }

    rows
}

fn friendly_input_label(run_state: &WorkflowRunState, identifier: &str) -> String {
    run_state
        .workflow
        .inputs
        .get(identifier)
        .map(|definition| definition.display_name(identifier).into_owned())
        .unwrap_or_else(|| identifier.to_string())
}

fn build_input_row(run_state: &WorkflowRunState, name: &str, definition: &WorkflowInputDefinition) -> WorkflowInputRow {
    let required = definition.is_required();

    let provider_label = definition.provider.as_ref().map(|provider| match provider {
        WorkflowValueProvider::Id(id) => id.clone(),
        WorkflowValueProvider::Detailed(detail) => detail.id.clone(),
    });
    let display_name = definition.display_name(name).into_owned();

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
    let has_value = raw_value.is_some_and(has_meaningful_input_value);

    if matches!(status, InputStatus::Error) {
        // Preserve error state and explanatory message coming from provider resolution.
    } else if has_value {
        if let Some(value) = raw_value {
            if let Some(validation) = &definition.validate {
                match validate_candidate_value(value, validation) {
                    Ok(()) => {
                        status = InputStatus::Resolved;
                        status_message = None;
                    }
                    Err(message) => {
                        status = InputStatus::Error;
                        status_message = Some(message);
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

    let blocked_reason = dependency_block_reason(run_state, definition);
    if blocked_reason.is_some() && !matches!(status, InputStatus::Error) {
        status = InputStatus::Blocked;
        if status_message.is_none() || matches!(status_message.as_deref(), Some("[optional]")) {
            status_message = blocked_reason.clone();
        }
    }

    let current_value = raw_value.map(|value| WorkflowValuePreview::new(format_preview(value), classify_json_value(value)));
    WorkflowInputRow {
        name: display_name,
        required,
        provider_label,
        status,
        status_message,
        current_value,
        blocked_reason,
    }
}

#[derive(Debug, Clone)]
struct WorkflowValuePreview {
    text: String,
    role: JsonSyntaxRole,
}

impl WorkflowValuePreview {
    fn new(text: String, role: JsonSyntaxRole) -> Self {
        Self { text, role }
    }
}

#[derive(Debug)]
struct WorkflowInputRow {
    name: String,
    required: bool,
    provider_label: Option<String>,
    status: InputStatus,
    status_message: Option<String>,
    current_value: Option<WorkflowValuePreview>,
    blocked_reason: Option<String>,
}

impl WorkflowInputRow {
    fn is_blocked(&self) -> bool {
        self.blocked_reason.is_some()
    }
}

#[derive(Debug, Clone, Copy)]
enum Direction {
    Forward,
    Backward,
}

fn advance_selection(rows: &[WorkflowInputRow], current: usize, direction: Direction) -> Option<usize> {
    if rows.is_empty() {
        return None;
    }

    let len = rows.len();
    let mut index = current;
    for _ in 0..len {
        index = match direction {
            Direction::Forward => (index + 1) % len,
            Direction::Backward => (index + len - 1) % len,
        };
        if !rows[index].is_blocked() {
            return Some(index);
        }
    }
    None
}

fn first_enabled_index(rows: &[WorkflowInputRow]) -> Option<usize> {
    rows.iter().position(|row| !row.is_blocked())
}

fn current_row_block_reason(app: &App, rows: &[WorkflowInputRow]) -> Option<String> {
    let state = app.workflows.input_view_state()?;
    rows.get(state.selected())?.blocked_reason.clone()
}

fn dependency_block_reason(run_state: &WorkflowRunState, definition: &WorkflowInputDefinition) -> Option<String> {
    if definition.depends_on.is_empty() {
        return None;
    }

    for value in definition.depends_on.values() {
        match value {
            WorkflowProviderArgumentValue::Binding(binding) => {
                if let Some(input_name) = binding.from_input.as_deref()
                    && !run_state.run_context.inputs.get(input_name).is_some_and(has_meaningful_input_value)
                {
                    let label = friendly_input_label(run_state, input_name);
                    return Some(format!("Waiting on input '{label}'"));
                }
                if let Some(step_id) = binding.from_step.as_deref()
                    && !run_state.run_context.steps.get(step_id).is_some_and(has_meaningful_input_value)
                {
                    return Some(format!("Waiting on step '{step_id}'"));
                }
            }
            WorkflowProviderArgumentValue::Literal(template) => {
                for input_name in extract_template_inputs(template) {
                    if !run_state
                        .run_context
                        .inputs
                        .get(&input_name)
                        .is_some_and(has_meaningful_input_value)
                    {
                        let label = friendly_input_label(run_state, &input_name);
                        return Some(format!("Waiting on input '{label}'"));
                    }
                }
                for step_id in extract_template_steps(template) {
                    if !run_state.run_context.steps.get(&step_id).is_some_and(has_meaningful_input_value) {
                        return Some(format!("Waiting on step '{step_id}'"));
                    }
                }
            }
        }
    }

    None
}

fn extract_template_inputs(template: &str) -> Vec<String> {
    extract_template_identifiers(template, "inputs.")
}

fn extract_template_steps(template: &str) -> Vec<String> {
    extract_template_identifiers(template, "steps.")
}

fn extract_template_identifiers(template: &str, prefix: &str) -> Vec<String> {
    let mut results: Vec<String> = Vec::new();
    let mut remaining = template;
    while let Some(start) = remaining.find("${{") {
        let after = &remaining[start + 3..];
        if let Some(end) = after.find("}}") {
            let expression = after[..end].trim();
            if let Some(rest) = expression.strip_prefix(prefix)
                && let Some(identifier) = parse_identifier(rest)
                && !results.contains(&identifier)
            {
                results.push(identifier.clone());
            }
            remaining = &after[end + 2..];
        } else {
            break;
        }
    }
    results
}

fn parse_identifier(fragment: &str) -> Option<String> {
    let mut identifier = String::new();
    for ch in fragment.chars() {
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            identifier.push(ch);
        } else {
            break;
        }
    }
    if identifier.is_empty() { None } else { Some(identifier) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputStatus {
    Resolved,
    Pending,
    Error,
    Blocked,
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

fn focus_input_list(app: &mut App) {
    if let Some(state) = app.workflows.input_view_state() {
        app.focus.focus(&state.f_list);
    }
}

fn focus_cancel_button(app: &mut App) {
    if let Some(state) = app.workflows.input_view_state() {
        app.focus.focus(&state.f_cancel_button);
    }
}

fn focus_run_button(app: &mut App) {
    if let Some(state) = app.workflows.input_view_state() {
        app.focus.focus(&state.f_run_button);
    }
}

fn rect_contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x && column < area.x + area.width && row >= area.y && row < area.y + area.height
}
