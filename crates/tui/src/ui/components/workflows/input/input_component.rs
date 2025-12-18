//! Renders the workflow input viewer, handling navigation, validation feedback, and
//! the transitions into manual or provider-backed collectors.

use crate::app::App;
use crate::ui::components::component::Component;
use crate::ui::components::workflows::WorkflowInputViewState;
use crate::ui::components::workflows::input::state::{InputStatus, WorkflowInputRow};
use crate::ui::components::workflows::view_utils::style_for_role;
use crate::ui::theme::{
    roles::Theme,
    theme_helpers::{self as th, ButtonRenderOptions},
};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use oatty_engine::WorkflowRunState;
use oatty_types::{Effect, Modal, Route};
use rat_focus::HasFocus;
use ratatui::layout::Position;
use ratatui::widgets::Block;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Borders, List, ListItem, Paragraph, Wrap},
};
use std::cell::Ref;
use unicode_width::UnicodeWidthStr;

/// Captures layout metadata from the most recent render pass for hit detection.
#[derive(Debug, Default, Clone, Copy)]
pub struct WorkflowInputLayout {
    pub header_area: Rect,
    pub inputs_list_area: Rect,
    pub details_area: Rect,
    pub cancel_button_area: Rect,
    pub run_button_area: Rect,
    pub status_line_area: Rect,
}
impl From<Vec<Rect>> for WorkflowInputLayout {
    fn from(value: Vec<Rect>) -> Self {
        Self {
            header_area: value[0],
            inputs_list_area: value[1],
            details_area: value[2],
            cancel_button_area: value[3],
            run_button_area: value[4],
            status_line_area: value[5],
        }
    }
}
/// Controller for the pre-run workflow input view, coordinating focus, events, and rendering.
#[derive(Debug, Default)]
pub struct WorkflowInputsComponent {
    layout: WorkflowInputLayout,
}

impl Component for WorkflowInputsComponent {
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        if app.workflows.input_view_state().is_none() {
            return Vec::new();
        }

        if let Some(effects) = handle_global_key_event(app, key.code) {
            return effects;
        }

        let focus_snapshot = determine_input_focus(app);

        if focus_snapshot.list_focused {
            return handle_list_focused_key(app, key.code);
        }

        if focus_snapshot.cancel_button_focused {
            return handle_cancel_button_focused_key(app, key.code);
        }

        if focus_snapshot.run_button_focused {
            return handle_run_button_focused_key(app, key.code);
        }

        Vec::new()
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let Some(state) = app.workflows.input_view_state_mut() else {
            return Vec::new();
        };

        let pos = Position {
            x: mouse.column,
            y: mouse.row,
        };

        match mouse.kind {
            MouseEventKind::Moved | MouseEventKind::Up(MouseButton::Left) => {
                state.mouse_over_idx = if self.layout.inputs_list_area.contains(pos) {
                    hit_test_list(
                        pos,
                        state.input_list_state.offset(),
                        &self.layout.inputs_list_area,
                        &state.input_rows,
                    )
                } else {
                    None
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if self.layout.inputs_list_area.contains(pos) {
                    app.focus.focus(&state.f_list);
                    let idx = hit_test_list(
                        pos,
                        state.input_list_state.offset(),
                        &self.layout.inputs_list_area,
                        &state.input_rows,
                    );
                    state.input_list_state.select(idx);
                    if idx.is_some() {
                        return handle_list_focused_key(app, KeyCode::Enter);
                    }
                }

                if self.layout.cancel_button_area.contains(pos) {
                    app.focus.focus(&state.f_cancel_button);
                    return vec![Effect::SwitchTo(Route::Workflows)];
                }

                if self.layout.run_button_area.contains(pos) {
                    app.focus.focus(&state.f_run_button);
                    return app.run_active_workflow();
                }
            }
            _ => {}
        }

        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let block = get_block_container(app);
        let inner = block.inner(area);

        if app.workflows.active_run_state.is_none() || app.workflows.input_view_state().is_none() {
            render_empty(frame, inner, &*app.ctx.theme);
            return;
        };
        let theme = &*app.ctx.theme;
        let layout = WorkflowInputLayout::from(self.get_preferred_layout(app, area));

        let input_view_state = app.workflows.input_view_state_mut().unwrap();
        input_view_state.build_input_rows();

        let run_state_rc = input_view_state.run_state.clone();

        render_header(
            frame,
            layout.header_area,
            run_state_rc.borrow(),
            input_view_state.input_rows.len(),
            theme,
        );
        render_inputs_list(frame, layout.inputs_list_area, input_view_state, theme);
        render_input_details(frame, layout.details_area, input_view_state, theme);
        render_footer(frame, layout, app);
        self.layout = layout;
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

    fn get_preferred_layout(&self, app: &App, area: Rect) -> Vec<Rect> {
        let block = get_block_container(app);
        let inner = block.inner(area);
        let main = Layout::vertical([
            Constraint::Length(2), // header height
            Constraint::Min(6),    // content height
            Constraint::Length(3), // footer height
        ])
        .split(inner);

        let content_layout = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).split(main[1]);
        let layout_areas = Layout::horizontal([
            Constraint::Length(12), // cancel
            Constraint::Length(12), // run
            Constraint::Length(2),  // padding
            Constraint::Min(0),     // status line
        ])
        .split(main[2]);

        vec![
            main[0],           // header
            content_layout[0], // input list
            content_layout[1], // input details
            layout_areas[0],   // cancel button
            layout_areas[1],   // run button
            layout_areas[3],   // status line
        ]
    }

    fn on_route_exit(&mut self, app: &mut App) -> Vec<Effect> {
        if let Some(state) = app.workflows.input_view_state_mut() {
            state.input_list_state.select(None);
        }
        Vec::new()
    }
}

#[derive(Default)]
struct InputFocusSnapshot {
    list_focused: bool,
    cancel_button_focused: bool,
    run_button_focused: bool,
}
fn get_block_container(app: &App) -> Block<'static> {
    let is_focused = app.workflows.input_view_state().map(|s| s.f_list.is_focused()).unwrap_or(false);
    th::block(&*app.ctx.theme, Some("Pre-run Input Viewer"), is_focused)
}
fn handle_global_key_event(app: &mut App, key_code: KeyCode) -> Option<Vec<Effect>> {
    match key_code {
        KeyCode::Tab => {
            app.focus.next();
            Some(Vec::new())
        }
        KeyCode::BackTab => {
            app.focus.prev();
            Some(Vec::new())
        }
        KeyCode::Esc => Some(vec![Effect::SwitchTo(Route::Workflows)]),
        _ => None,
    }
}

fn determine_input_focus(app: &App) -> InputFocusSnapshot {
    let mut snapshot = InputFocusSnapshot::default();
    if let Some(state) = app.workflows.input_view_state() {
        snapshot.list_focused = state.f_list.get();
        snapshot.cancel_button_focused = state.f_cancel_button.get();
        snapshot.run_button_focused = state.f_run_button.get();
    }
    snapshot
}

fn handle_list_focused_key(app: &mut App, key_code: KeyCode) -> Vec<Effect> {
    match key_code {
        KeyCode::Down | KeyCode::Up => {
            if app.workflows.active_run_state.is_some() {
                let direction = if matches!(key_code, KeyCode::Down) {
                    Direction::Forward
                } else {
                    Direction::Backward
                };
                if let Some(state) = app.workflows.input_view_state_mut()
                    && let Some(selected_index) = state.input_list_state.selected()
                    && let Some(next_index) = advance_selection(&state.input_rows, selected_index, direction)
                {
                    state.input_list_state.select(Some(next_index));
                }
            }
            Vec::new()
        }
        KeyCode::Enter => {
            let mut effects = Vec::new();
            if app.workflows.active_run_state.is_some()
                && let Some(reason) = current_row_block_reason(app)
            {
                app.append_log_message(format!("Input blocked: {reason}"));
                return effects;
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
            effects
        }
        KeyCode::F(2) => {
            let mut effects = Vec::new();
            if app.workflows.active_run_state.is_some() && current_row_block_reason(app).is_some() {
                return effects;
            }
            app.workflows.open_manual_for_active_input();
            effects.push(Effect::ShowModal(Modal::WorkflowCollector));
            effects
        }
        _ => Vec::new(),
    }
}

fn handle_cancel_button_focused_key(app: &mut App, key_code: KeyCode) -> Vec<Effect> {
    match key_code {
        KeyCode::Left | KeyCode::Up => {
            focus_input_list(app);
            Vec::new()
        }
        KeyCode::Right => {
            focus_run_button(app);
            Vec::new()
        }
        KeyCode::Enter | KeyCode::Char(' ') => vec![Effect::SwitchTo(Route::Workflows)],
        _ => Vec::new(),
    }
}

fn handle_run_button_focused_key(app: &mut App, key_code: KeyCode) -> Vec<Effect> {
    match key_code {
        KeyCode::Left => {
            focus_cancel_button(app);
            Vec::new()
        }
        KeyCode::Right | KeyCode::Down => {
            focus_input_list(app);
            Vec::new()
        }
        KeyCode::Enter | KeyCode::Char(' ') => app.run_active_workflow(),
        _ => Vec::new(),
    }
}

fn render_empty(frame: &mut Frame, area: Rect, theme: &dyn Theme) {
    let block = th::block(theme, Some("Workflow Inputs"), false);
    frame.render_widget(block, area);
}

fn render_header(frame: &mut Frame, area: Rect, run_state: Ref<WorkflowRunState>, total_inputs: usize, theme: &dyn Theme) {
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

    let unresolved = run_state.unresolved_item_count();
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

fn render_inputs_list(frame: &mut Frame, area: Rect, input_view_state: &mut WorkflowInputViewState, theme: &dyn Theme) {
    let list_focused = input_view_state.f_list.is_focused();
    let rows = &input_view_state.input_rows;
    let block = th::block(theme, Some("Inputs"), list_focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if rows.is_empty() {
        let paragraph = Paragraph::new("This workflow has no declared inputs.")
            .style(theme.text_muted_style())
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, inner);
        return;
    }

    let mut items: Vec<ListItem> = Vec::with_capacity(rows.len());
    let max_len = rows
        .iter()
        .map(|r| UnicodeWidthStr::width(r.name.as_str()) + 2)
        .reduce(|a, b| a.max(b))
        .unwrap_or(20);
    let should_highlight = input_view_state.mouse_over_idx.is_some();
    let mouse_over_idx = input_view_state.mouse_over_idx.unwrap_or(0);
    let highlight_style = theme.selection_style().add_modifier(Modifier::BOLD);
    for (idx, row) in rows.iter().enumerate() {
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

        let mut segments = vec![Span::styled(format!("{:<max_len$}", row.name), name_style), status_span];
        let required_status = if row.required { "[required]" } else { "[optional]" };
        let required_style = if row.required {
            theme.syntax_keyword_style()
        } else {
            theme.text_muted_style()
        };
        segments.push(Span::styled(required_status, required_style));

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
            let message_style = get_message_style(&row.status, theme);
            segments.push(Span::styled(message.to_string(), message_style));
        }

        if row.is_blocked() {
            segments.push(Span::styled(" [disabled]", theme.text_muted_style().add_modifier(Modifier::DIM)));
        }

        let mut line = Line::from(segments);
        if should_highlight && mouse_over_idx == idx {
            line = line.style(highlight_style);
        }
        items.push(ListItem::new(line));
    }
    if input_view_state.input_list_state.selected().is_none() {
        input_view_state.input_list_state.select(first_enabled_index(rows));
    }

    let list = List::new(items)
        .style(theme.text_primary_style())
        .highlight_style(highlight_style)
        .highlight_symbol(if list_focused { "▸ " } else { "" });

    frame.render_stateful_widget(list, inner, &mut input_view_state.input_list_state.clone());
}

fn hit_test_list(pos: Position, offset: usize, list_area: &Rect, rows: &[WorkflowInputRow]) -> Option<usize> {
    let idx = pos.y.saturating_sub(list_area.y + 1) as usize + offset; // sub 1 for block border
    if let Some(row) = rows.get(idx)
        && !row.is_blocked()
    {
        return Some(idx);
    }
    None
}

fn get_message_style(status: &InputStatus, theme: &dyn Theme) -> Style {
    match status {
        InputStatus::Resolved => theme.text_muted_style(),
        InputStatus::Pending => theme.text_muted_style(),
        InputStatus::Error => theme.status_error(),
        InputStatus::Blocked => theme.status_warning(),
    }
}

fn render_input_details(frame: &mut Frame, area: Rect, input_view_state: &WorkflowInputViewState, theme: &dyn Theme) {
    let block = th::block(theme, Some("Workflow Details"), false);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let total_inputs = input_view_state.input_rows.len();
    let unresolved_inputs = input_view_state.run_state.borrow().unresolved_item_count();
    let next_action = find_next_action(&input_view_state.input_rows);

    let mut lines = vec![
        build_ready_line(theme, unresolved_inputs, next_action),
        build_resolved_count_line(theme, total_inputs, unresolved_inputs),
        build_next_action_line(theme, unresolved_inputs, next_action),
        Line::from(""),
        Line::from(Span::styled("Selected values:", theme.text_secondary_style())),
    ];

    lines.extend(build_selected_value_lines(theme, &input_view_state.input_rows));

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Auto-reset note: ", theme.text_secondary_style()),
        Span::styled("downstream steps reset when a prior step edits.", theme.text_muted_style()),
    ]));

    lines.push(Line::from(""));
    let error_notes = collect_error_notes(&input_view_state.input_rows);
    lines.extend(build_error_section(theme, &error_notes));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);
}

fn find_next_action(rows: &[WorkflowInputRow]) -> Option<&str> {
    rows.iter()
        .find(|row| matches!(row.status, InputStatus::Error | InputStatus::Pending | InputStatus::Blocked))
        .map(|row| row.name.as_str())
}

fn build_ready_line(theme: &dyn Theme, unresolved_inputs: usize, next_action: Option<&str>) -> Line<'static> {
    if unresolved_inputs == 0 {
        Line::from(Span::styled("Ready?: ✓ All inputs resolved", theme.status_success()))
    } else {
        let label = next_action.unwrap_or("—");
        Line::from(Span::styled(format!("Ready?: ⚠ Waiting on {label}"), theme.status_warning()))
    }
}

fn build_resolved_count_line(theme: &dyn Theme, total_inputs: usize, unresolved_inputs: usize) -> Line<'static> {
    let resolved_inputs = total_inputs.saturating_sub(unresolved_inputs);
    Line::from(vec![
        Span::styled("Resolved inputs: ", theme.text_secondary_style()),
        Span::styled(format!("{} / {}", resolved_inputs, total_inputs), theme.text_primary_style()),
    ])
}

fn build_next_action_line(theme: &dyn Theme, unresolved_inputs: usize, next_action: Option<&str>) -> Line<'static> {
    let label = next_action.unwrap_or("—").to_owned();
    let style = if unresolved_inputs == 0 {
        theme.text_muted_style()
    } else {
        theme.text_primary_style()
    };
    Line::from(vec![
        Span::styled("Next action: ", theme.text_secondary_style()),
        Span::styled(label, style),
    ])
}

fn build_selected_value_lines(theme: &dyn Theme, rows: &[WorkflowInputRow]) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();
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
            && row.blocked_reason.as_deref() != Some(message)
        {
            let message_style = get_message_style(&row.status, theme);
            line_segments.push(Span::styled(format!(" {message}"), message_style));
        }

        lines.push(Line::from(line_segments));
    }
    lines
}

fn collect_error_notes(rows: &[WorkflowInputRow]) -> Vec<String> {
    rows.iter().filter_map(|row| row.status_message.clone()).collect()
}

fn build_error_section(theme: &dyn Theme, error_notes: &[String]) -> Vec<Line<'static>> {
    if error_notes.is_empty() {
        vec![Line::from(vec![
            Span::styled("Errors & notes: ", theme.text_secondary_style()),
            Span::styled("none", theme.text_muted_style()),
        ])]
    } else {
        let mut lines = Vec::with_capacity(error_notes.len() + 1);
        lines.push(Line::from(Span::styled("Errors & notes:", theme.text_secondary_style())));
        for note in error_notes {
            lines.push(Line::from(vec![
                Span::styled("  • ", theme.text_secondary_style()),
                Span::styled(note.clone(), theme.text_primary_style()),
            ]));
        }
        lines
    }
}

fn render_footer(frame: &mut Frame, layout: WorkflowInputLayout, app: &App) {
    let run_enabled = app.workflows.unresolved_item_count() == 0;
    let theme = &*app.ctx.theme;
    let (cancel_focused, run_focused) = app
        .workflows
        .input_view_state()
        .map(|state| (state.f_cancel_button.get(), state.f_run_button.get()))
        .unwrap_or((false, false));

    let cancel_options = ButtonRenderOptions::new(true, cancel_focused, cancel_focused, Borders::ALL, false);
    th::render_button(frame, layout.cancel_button_area, "Cancel", theme, cancel_options);

    let run_options = ButtonRenderOptions::new(run_enabled, run_focused, run_focused, Borders::ALL, true);
    th::render_button(frame, layout.run_button_area, "Run", theme, run_options);

    let unresolved = app.workflows.unresolved_item_count();
    let status_line = if run_enabled {
        Span::styled("All required inputs resolved — ready to run.", theme.status_success())
    } else {
        Span::styled(
            format!("Resolve {unresolved} required input(s) to enable Run."),
            theme.status_warning(),
        )
    };
    let mut status_layout = layout.status_line_area;
    status_layout.y += 1;
    frame.render_widget(Paragraph::new(Line::from(status_line)).wrap(Wrap { trim: true }), status_layout);
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

fn current_row_block_reason(app: &App) -> Option<String> {
    let state = app.workflows.input_view_state()?;
    let selected_index = state.input_list_state.selected()?;
    state.input_rows.get(selected_index)?.blocked_reason.clone()
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
