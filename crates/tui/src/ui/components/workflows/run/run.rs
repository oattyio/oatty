//! Workflow run view component.
//!
//! This component renders the workflow execution experience, displaying step
//! progress, surfaced outputs, and control actions while reusing the shared
//! results table utilities. Interaction handling covers keyboard and mouse
//! paths, keeping behavior consistent with other workflow views.

use chrono::{Duration, Utc};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use heroku_types::{Effect, WorkflowRunControl};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::App;
use crate::ui::components::workflows::run::state::{RunDetailSource, RunExecutionStatus, RunViewLayout, RunViewMouseTarget, RunViewState};
use crate::ui::components::{
    common::ResultsTableView,
    component::{Component, find_target_index_by_mouse_position},
};
use crate::ui::theme::{
    Theme,
    theme_helpers::{self as th, ButtonRenderOptions, build_hint_spans},
};
use serde_json::Value;

#[derive(Debug, Clone, Copy)]
enum FocusRequest {
    StepsTable,
    OutputsTable,
    DetailPane,
    CancelButton,
    PauseButton,
}

#[derive(Debug, Default)]
pub struct RunViewComponent {
    steps_view: ResultsTableView<'static>,
    outputs_view: ResultsTableView<'static>,
}

impl Component for RunViewComponent {
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Tab => {
                app.focus.next();
                return Vec::new();
            }
            KeyCode::BackTab => {
                app.focus.prev();
                return Vec::new();
            }
            _ => {}
        }

        let mut focus_request: Option<FocusRequest> = None;
        let mut effects: Vec<Effect> = Vec::new();

        let mut log_shortcut_requested = false;

        {
            let Some(run_state) = app.workflows.run_view_state_mut() else {
                return effects;
            };
            let run_id = run_state.run_id().to_string();

            let steps_focused = run_state.steps_focus_flag().get();
            let outputs_focused = run_state.outputs_focus_flag().get();
            let detail_focused = run_state.detail_focus_flag().get();
            let cancel_focused = run_state.cancel_button_focus_flag().get();
            let pause_focused = run_state.pause_button_focus_flag().get();

            match key.code {
                KeyCode::Char('t' | 'T') => {
                    run_state.toggle_wide_mode();
                }
                KeyCode::Char('l' | 'L') => {
                    log_shortcut_requested = true;
                }
                KeyCode::Esc => {
                    if run_state.is_detail_visible() {
                        let source = run_state.detail().map(|detail| detail.source());
                        run_state.hide_detail();
                        match source {
                            Some(RunDetailSource::Steps) => focus_request = Some(FocusRequest::StepsTable),
                            Some(RunDetailSource::Outputs) => focus_request = Some(FocusRequest::OutputsTable),
                            None => {}
                        }
                    }
                }
                _ => {}
            }

            if steps_focused {
                match key.code {
                    KeyCode::Up => run_state.steps_table_mut().reduce_scroll(-1),
                    KeyCode::Down => run_state.steps_table_mut().reduce_scroll(1),
                    KeyCode::PageUp => run_state.steps_table_mut().reduce_scroll(-5),
                    KeyCode::PageDown => run_state.steps_table_mut().reduce_scroll(5),
                    KeyCode::Home => run_state.steps_table_mut().reduce_home(),
                    KeyCode::End => run_state.steps_table_mut().reduce_end(),
                    KeyCode::Enter => {
                        if run_state.is_detail_visible()
                            && matches!(run_state.detail().map(|detail| detail.source()), Some(RunDetailSource::Steps))
                        {
                            run_state.hide_detail();
                        } else {
                            run_state.show_detail(RunDetailSource::Steps);
                            run_state.set_detail_selection(Some(0));
                            focus_request = Some(FocusRequest::DetailPane);
                        }
                    }
                    _ => {}
                }
                run_state.clamp_detail_entries();
            } else if outputs_focused {
                match key.code {
                    KeyCode::Up => run_state.outputs_table_mut().reduce_scroll(-1),
                    KeyCode::Down => run_state.outputs_table_mut().reduce_scroll(1),
                    KeyCode::PageUp => run_state.outputs_table_mut().reduce_scroll(-5),
                    KeyCode::PageDown => run_state.outputs_table_mut().reduce_scroll(5),
                    KeyCode::Home => run_state.outputs_table_mut().reduce_home(),
                    KeyCode::End => run_state.outputs_table_mut().reduce_end(),
                    KeyCode::Enter => {
                        if run_state.is_detail_visible()
                            && matches!(run_state.detail().map(|detail| detail.source()), Some(RunDetailSource::Outputs))
                        {
                            run_state.hide_detail();
                        } else {
                            run_state.show_detail(RunDetailSource::Outputs);
                            run_state.set_detail_selection(Some(0));
                            focus_request = Some(FocusRequest::DetailPane);
                        }
                    }
                    _ => {}
                }
                run_state.clamp_detail_entries();
            } else if detail_focused {
                match key.code {
                    KeyCode::Up => run_state.adjust_detail_selection(-1),
                    KeyCode::Down => run_state.adjust_detail_selection(1),
                    KeyCode::PageUp => run_state.adjust_detail_selection(-5),
                    KeyCode::PageDown => run_state.adjust_detail_selection(5),
                    KeyCode::Home => run_state.set_detail_selection(Some(0)),
                    KeyCode::End => {
                        let entry_count = run_state.current_detail_entries().map(|entries| entries.len()).unwrap_or(0);
                        if entry_count > 0 {
                            run_state.set_detail_selection(Some(entry_count.saturating_sub(1)));
                        }
                    }
                    KeyCode::Esc => {
                        let source = run_state.detail().map(|detail| detail.source());
                        run_state.hide_detail();
                        match source {
                            Some(RunDetailSource::Steps) => focus_request = Some(FocusRequest::StepsTable),
                            Some(RunDetailSource::Outputs) => focus_request = Some(FocusRequest::OutputsTable),
                            None => {}
                        }
                    }
                    _ => {}
                }
                run_state.clamp_detail_entries();
            } else if cancel_focused {
                match key.code {
                    KeyCode::Left | KeyCode::Up => focus_request = Some(FocusRequest::PauseButton),
                    KeyCode::Right | KeyCode::Down => focus_request = Some(FocusRequest::StepsTable),
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if cancel_enabled(run_state.status()) {
                            effects.push(Effect::WorkflowRunControl {
                                run_id: run_id.clone(),
                                command: WorkflowRunControl::Cancel,
                            });
                        }
                    }
                    _ => {}
                }
            } else if pause_focused {
                match key.code {
                    KeyCode::Left | KeyCode::Up => focus_request = Some(FocusRequest::OutputsTable),
                    KeyCode::Right | KeyCode::Down => focus_request = Some(FocusRequest::CancelButton),
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if let Some(command) = pause_command_for_status(run_state.status()) {
                            effects.push(Effect::WorkflowRunControl {
                                run_id: run_id.clone(),
                                command,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        if log_shortcut_requested {
            app.append_log_message("Logs shortcut not yet wired for the run view.");
        }

        if let Some(request) = focus_request
            && let Some(run_state) = app.workflows.run_view_state()
        {
            match request {
                FocusRequest::StepsTable => app.focus.focus(run_state.steps_focus_flag()),
                FocusRequest::OutputsTable => app.focus.focus(run_state.outputs_focus_flag()),
                FocusRequest::DetailPane => app.focus.focus(run_state.detail_focus_flag()),
                FocusRequest::CancelButton => app.focus.focus(run_state.cancel_button_focus_flag()),
                FocusRequest::PauseButton => app.focus.focus(run_state.pause_button_focus_flag()),
            }
        }
        effects
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return Vec::new();
        }

        let layout_snapshot = {
            let Some(run_state) = app.workflows.run_view_state() else {
                return Vec::new();
            };
            run_state.layout().clone()
        };

        let Some(container_area) = layout_snapshot.last_area() else {
            return Vec::new();
        };

        let Some(index) =
            find_target_index_by_mouse_position(&container_area, layout_snapshot.mouse_target_areas(), mouse.column, mouse.row)
        else {
            return Vec::new();
        };

        let Some(target) = layout_snapshot.mouse_target_roles().get(index).copied() else {
            return Vec::new();
        };

        match target {
            RunViewMouseTarget::StepsTable => {
                if let Some(run_state) = app.workflows.run_view_state() {
                    app.focus.focus(run_state.steps_focus_flag());
                }
            }
            RunViewMouseTarget::OutputsTable => {
                if let Some(run_state) = app.workflows.run_view_state() {
                    app.focus.focus(run_state.outputs_focus_flag());
                }
            }
            RunViewMouseTarget::DetailPane => {
                if let Some(run_state) = app.workflows.run_view_state() {
                    app.focus.focus(run_state.detail_focus_flag());
                }
            }
            RunViewMouseTarget::CancelButton => {
                if let Some(run_state) = app.workflows.run_view_state() {
                    app.focus.focus(run_state.cancel_button_focus_flag());
                    if cancel_enabled(run_state.status()) {
                        return vec![Effect::WorkflowRunControl {
                            run_id: run_state.run_id().to_string(),
                            command: WorkflowRunControl::Cancel,
                        }];
                    }
                }
            }
            RunViewMouseTarget::PauseButton => {
                if let Some(run_state) = app.workflows.run_view_state() {
                    app.focus.focus(run_state.pause_button_focus_flag());
                    if let Some(command) = pause_command_for_status(run_state.status()) {
                        return vec![Effect::WorkflowRunControl {
                            run_id: run_state.run_id().to_string(),
                            command,
                        }];
                    }
                }
            }
        }

        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let theme = &*app.ctx.theme;
        let block = Block::default()
            .title(Span::styled("Workflow Run", theme.text_secondary_style()))
            .borders(Borders::ALL)
            .border_style(theme.border_style(true))
            .style(th::panel_style(theme));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let Some(run_state) = app.workflows.run_view_state_mut() else {
            render_empty(frame, inner, theme);
            return;
        };

        let layout_regions = Layout::vertical([Constraint::Length(3), Constraint::Min(6), Constraint::Length(3)]).split(inner);

        let header_area = layout_regions[0];
        let body_area = layout_regions[1];
        let footer_area = layout_regions[2];

        render_header(frame, header_area, theme, run_state);

        let mut layout_state = RunViewLayout::default();
        layout_state.set_last_area(inner);
        layout_state.set_header_area(header_area);

        let mut mouse_targets: Vec<(Rect, RunViewMouseTarget)> = Vec::new();

        if run_state.is_wide_mode() {
            let detail_visible = run_state.is_detail_visible();
            let column_spec: Vec<Constraint> = if detail_visible {
                vec![Constraint::Percentage(45), Constraint::Percentage(35), Constraint::Percentage(20)]
            } else {
                vec![Constraint::Percentage(55), Constraint::Percentage(45)]
            };
            let columns = Layout::horizontal(column_spec).split(body_area);

            if let Some(steps_area) = columns.first().copied() {
                render_steps_table(frame, steps_area, theme, run_state, &mut self.steps_view);
                layout_state.set_steps_area(steps_area);
                mouse_targets.push((steps_area, RunViewMouseTarget::StepsTable));
            }

            if let Some(outputs_area) = columns.get(1).copied() {
                render_outputs_table(frame, outputs_area, theme, run_state, &mut self.outputs_view);
                layout_state.set_outputs_area(outputs_area);
                mouse_targets.push((outputs_area, RunViewMouseTarget::OutputsTable));
            }

            if detail_visible {
                if let Some(area) = columns.get(2).copied() {
                    render_detail_pane(frame, area, theme, run_state);
                    layout_state.set_detail_area(Some(area));
                    mouse_targets.push((area, RunViewMouseTarget::DetailPane));
                }
            } else {
                layout_state.set_detail_area(None);
            }
        } else {
            let detail_visible = run_state.is_detail_visible();
            let body_spec = if detail_visible {
                vec![Constraint::Percentage(55), Constraint::Percentage(45), Constraint::Length(6)]
            } else {
                vec![Constraint::Percentage(60), Constraint::Percentage(40)]
            };
            let rows = Layout::vertical(body_spec).split(body_area);

            if let Some(steps_area) = rows.first().copied() {
                render_steps_table(frame, steps_area, theme, run_state, &mut self.steps_view);
                layout_state.set_steps_area(steps_area);
                mouse_targets.push((steps_area, RunViewMouseTarget::StepsTable));
            }

            if let Some(outputs_area) = rows.get(1).copied() {
                render_outputs_table(frame, outputs_area, theme, run_state, &mut self.outputs_view);
                layout_state.set_outputs_area(outputs_area);
                mouse_targets.push((outputs_area, RunViewMouseTarget::OutputsTable));
            }

            if detail_visible {
                if let Some(detail_area) = rows.get(2).copied() {
                    render_detail_pane(frame, detail_area, theme, run_state);
                    layout_state.set_detail_area(Some(detail_area));
                    mouse_targets.push((detail_area, RunViewMouseTarget::DetailPane));
                }
            } else {
                layout_state.set_detail_area(None);
            }
        }

        let (cancel_area, pause_area) = render_footer(frame, footer_area, theme, run_state, &mut mouse_targets);
        layout_state.set_footer_area(footer_area);
        layout_state.set_cancel_button_area(cancel_area);
        layout_state.set_pause_button_area(pause_area);
        layout_state.set_mouse_targets(mouse_targets);
        run_state.set_layout(layout_state);
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        let Some(run_state) = app.workflows.run_view_state() else {
            return Vec::new();
        };

        if run_state.detail_focus_flag().get() {
            return build_hint_spans(
                theme,
                &[(" Esc", " Close detail "), (" ↑/↓", " Navigate detail "), (" Tab", " Cycle focus ")],
            );
        }

        if run_state.cancel_button_focus_flag().get() {
            return build_hint_spans(
                theme,
                &[(" ←/→", " Switch button "), (" Enter", " Cancel run "), (" Tab", " Cycle focus ")],
            );
        }

        if run_state.pause_button_focus_flag().get() {
            let label = pause_button_label(run_state.status());
            return build_hint_spans(theme, &[(" ←/→", " Switch button "), (" Enter", label), (" Tab", " Cycle focus ")]);
        }

        build_hint_spans(
            theme,
            &[
                (" Esc", " Close detail "),
                (" ↑/↓", " Navigate "),
                (" Enter", " Toggle detail "),
                (" L", " View logs "),
                (" T", " Toggle layout "),
                (" Tab", " Cycle focus "),
            ],
        )
    }

    fn on_route_exit(&mut self, app: &mut App) -> Vec<Effect> {
        app.workflows.end_inputs_session();
        Vec::new()
    }
}

fn render_empty(frame: &mut Frame, area: Rect, theme: &dyn Theme) {
    let placeholder = Paragraph::new("No workflow execution is active.")
        .style(theme.text_muted_style())
        .wrap(Wrap { trim: true });
    frame.render_widget(placeholder, area);
}

fn render_header(frame: &mut Frame, area: Rect, theme: &dyn Theme, run_state: &RunViewState) {
    let now = Utc::now();
    let status_label = format_status_label(run_state.status());
    let elapsed_text = run_state
        .elapsed_since_start(now)
        .map(format_duration)
        .unwrap_or_else(|| "00:00:00".to_string());

    let mut spans = vec![
        Span::styled(
            format!("Workflow: {}", run_state.display_name()),
            theme.text_primary_style().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" • "),
        Span::styled(status_label, theme.text_secondary_style()),
        Span::raw(" • Elapsed: "),
        Span::styled(elapsed_text, theme.text_primary_style()),
        Span::raw(" • Logs forwarded ([L] View)"),
    ];

    if let Some(message) = run_state.status_message() {
        let style = match run_state.status() {
            RunExecutionStatus::Failed => theme.status_error(),
            RunExecutionStatus::Canceled | RunExecutionStatus::CancelRequested => theme.status_warning(),
            _ => theme.status_info(),
        };
        spans.push(Span::raw(" • "));
        spans.push(Span::styled(message.to_string(), style));
    }

    if run_state.is_wide_mode() {
        spans.push(Span::raw(" • Wide mode"));
    }

    let paragraph = Paragraph::new(Line::from(spans)).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn render_steps_table(
    frame: &mut Frame,
    area: Rect,
    theme: &dyn Theme,
    run_state: &mut RunViewState,
    view: &mut ResultsTableView<'static>,
) {
    let steps_focused = run_state.steps_focus_flag().get();
    let inner_block = th::block(theme, Some("Steps"), steps_focused).borders(Borders::NONE);
    let inner_area = inner_block.inner(area);
    frame.render_widget(inner_block, area);

    let visible_rows = inner_area.height.saturating_sub(1).max(1) as usize;
    {
        let table_state = run_state.steps_table_mut();
        table_state.set_visible_rows(visible_rows);
        view.render_results(frame, inner_area, table_state, steps_focused, theme);
    }
}

fn render_outputs_table(
    frame: &mut Frame,
    area: Rect,
    theme: &dyn Theme,
    run_state: &mut RunViewState,
    view: &mut ResultsTableView<'static>,
) {
    let outputs_focused = run_state.outputs_focus_flag().get();
    let inner_block = th::block(theme, Some("Outputs"), outputs_focused).borders(Borders::NONE);
    let inner_area = inner_block.inner(area);
    frame.render_widget(inner_block, area);

    let visible_rows = inner_area.height.saturating_sub(1).max(1) as usize;
    {
        let table_state = run_state.outputs_table_mut();
        table_state.set_visible_rows(visible_rows);
        view.render_results(frame, inner_area, table_state, outputs_focused, theme);
    }
}

fn render_detail_pane(frame: &mut Frame, area: Rect, theme: &dyn Theme, run_state: &RunViewState) {
    let inner_block = th::block(theme, Some("Details"), run_state.detail_focus_flag().get()).borders(Borders::NONE);
    let inner_area = inner_block.inner(area);
    frame.render_widget(inner_block, area);

    let entries = run_state.current_detail_entries().unwrap_or_default();
    let selection = run_state.detail().and_then(|state| state.selection());
    let offset = run_state.detail().map(|state| state.offset()).unwrap_or(0);
    let payload = run_state.current_detail_payload().cloned().unwrap_or(Value::Null);

    ResultsTableView::render_kv_or_text(frame, inner_area, &entries, selection, offset, &payload, theme);
}

fn render_footer(
    frame: &mut Frame,
    area: Rect,
    theme: &dyn Theme,
    run_state: &RunViewState,
    targets: &mut Vec<(Rect, RunViewMouseTarget)>,
) -> (Option<Rect>, Option<Rect>) {
    let footer_block = th::block(theme, None, false).borders(Borders::NONE);
    let inner_area = footer_block.inner(area);
    frame.render_widget(footer_block, area);

    let button_row = Layout::horizontal([Constraint::Length(20), Constraint::Length(20), Constraint::Min(0)]).split(inner_area);
    let cancel_area = button_row[0];
    let pause_area = button_row[1];

    let cancel_enabled = cancel_enabled(run_state.status());
    let cancel_options = ButtonRenderOptions::new(
        cancel_enabled,
        run_state.cancel_button_focus_flag().get(),
        false,
        Borders::ALL,
        false,
    );
    th::render_button(frame, cancel_area, "Cancel", theme, cancel_options);

    let pause_enabled = pause_command_for_status(run_state.status()).is_some();
    let pause_label = pause_button_label(run_state.status());
    let pause_options = ButtonRenderOptions::new(pause_enabled, run_state.pause_button_focus_flag().get(), false, Borders::ALL, true);
    th::render_button(frame, pause_area, pause_label, theme, pause_options);

    if cancel_enabled {
        targets.push((cancel_area, RunViewMouseTarget::CancelButton));
    }
    if pause_enabled {
        targets.push((pause_area, RunViewMouseTarget::PauseButton));
    }

    let status_area = button_row[2];
    let mut line = vec![Span::styled(
        format!("Status: {}", format_status_label(run_state.status())),
        theme.text_secondary_style(),
    )];

    if let Some(message) = run_state.status_message() {
        let style = match run_state.status() {
            RunExecutionStatus::Failed => theme.status_error(),
            RunExecutionStatus::Canceled | RunExecutionStatus::CancelRequested => theme.status_warning(),
            _ => theme.status_info(),
        };
        line.push(Span::raw(" • "));
        line.push(Span::styled(message.to_string(), style));
    }

    let status_paragraph = Paragraph::new(Line::from(line)).wrap(Wrap { trim: true });
    frame.render_widget(status_paragraph, status_area);

    (cancel_enabled.then_some(cancel_area), pause_enabled.then_some(pause_area))
}

fn format_status_label(status: RunExecutionStatus) -> &'static str {
    match status {
        RunExecutionStatus::Pending => "pending",
        RunExecutionStatus::Running => "running",
        RunExecutionStatus::Paused => "paused",
        RunExecutionStatus::CancelRequested => "cancel requested",
        RunExecutionStatus::Succeeded => "succeeded",
        RunExecutionStatus::Failed => "failed",
        RunExecutionStatus::Canceled => "canceled",
    }
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.num_seconds().max(0);
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

fn pause_button_label(status: RunExecutionStatus) -> &'static str {
    match status {
        RunExecutionStatus::Paused => "Continue",
        _ => "Pause",
    }
}

fn cancel_enabled(status: RunExecutionStatus) -> bool {
    !status.is_terminal() && status != RunExecutionStatus::CancelRequested
}

fn pause_command_for_status(status: RunExecutionStatus) -> Option<WorkflowRunControl> {
    match status {
        RunExecutionStatus::Running => Some(WorkflowRunControl::Pause),
        RunExecutionStatus::Paused => Some(WorkflowRunControl::Resume),
        _ => None,
    }
}
