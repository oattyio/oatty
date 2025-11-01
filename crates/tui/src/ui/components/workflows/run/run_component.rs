//! Workflow run view component.
//!
//! This component renders the workflow execution experience, displaying step
//! progress, surfaced outputs, and control actions while reusing the shared
//! result table utilities. Interaction handling covers keyboard and mouse
//! paths, keeping behavior consistent with other workflow views.

use chrono::{Duration, Utc};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use heroku_types::{Effect, Msg, WorkflowRunControl};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::App;
use crate::ui::components::workflows::run::state::{
    DetailPaneVisibilityChange, RunDetailSource, RunExecutionStatus, RunViewLayout, RunViewMouseTarget, RunViewState,
};
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
    steps_view: ResultsTableView,
    outputs_view: ResultsTableView,
    detail_view: ResultsTableView,
}

#[derive(Debug, Default)]
struct RunViewLayoutState {
    footer_area: Rect,
    cancel_button_area: Rect,
    pause_button_area: Rect,
    mouse_targets: Vec<(Rect, RunViewMouseTarget)>,
}

impl Component for RunViewComponent {
    fn handle_message(&mut self, app: &mut App, msg: &Msg) -> Vec<Effect> {
        if let Msg::Tick = msg
            && let Some(run_state) = app.workflows.run_view_state_mut()
        {
            let theme = &*app.ctx.theme;
            run_state.advance_repeat_animations(theme);
        }
        Vec::new()
    }

    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        if Self::handle_focus_cycle_keys(app, key.code) {
            return Vec::new();
        }

        let (focus_request, log_shortcut_requested, effects) = {
            let Some(run_state) = app.workflows.run_view_state_mut() else {
                return Vec::new();
            };

            let mut focus_request: Option<FocusRequest> = None;
            let mut effects: Vec<Effect> = Vec::new();
            let mut log_shortcut_requested = false;

            let run_id = run_state.run_id().to_string();
            Self::handle_global_shortcuts(run_state, key.code, &mut focus_request, &mut log_shortcut_requested);

            let focus_snapshot = run_state.focus_snapshot();
            if focus_snapshot.steps_table_focused {
                focus_request = focus_request.or(self.handle_steps_table_keys(run_state, key.code));
            } else if focus_snapshot.outputs_table_focused {
                focus_request = focus_request.or(self.handle_outputs_table_keys(run_state, key.code));
            } else if focus_snapshot.detail_pane_focused {
                focus_request = focus_request.or(self.handle_detail_keys(run_state, key.code));
            } else if focus_snapshot.cancel_button_focused {
                let (request, control_effects) = Self::handle_cancel_button_keys(run_state, &run_id, key.code);
                focus_request = focus_request.or(request);
                effects.extend(control_effects);
            } else if focus_snapshot.pause_button_focused {
                let (request, control_effects) = Self::handle_pause_button_keys(run_state, &run_id, key.code);
                focus_request = focus_request.or(request);
                effects.extend(control_effects);
            }

            (focus_request, log_shortcut_requested, effects)
        };

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

        let Some(target) = Self::hit_test_mouse_target(app, mouse) else {
            return Vec::new();
        };

        Self::handle_mouse_target(app, target)
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

        let layout_regions = Layout::vertical([
            Constraint::Length(3), // header
            Constraint::Min(6),    // body
            Constraint::Length(3), // footer
        ])
        .split(inner);

        let header_area = layout_regions[0];
        let body_area = layout_regions[1];
        let footer_area = layout_regions[2];

        render_header(frame, header_area, theme, run_state);

        let mut layout_state = Self::prepare_layout_state(inner, header_area);
        let mut mouse_targets: Vec<(Rect, RunViewMouseTarget)> = Vec::new();
        self.render_body(frame, body_area, theme, run_state, &mut layout_state, &mut mouse_targets);

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

    fn get_preferred_layout(&self, app: &App, area: Rect) -> Vec<Rect> {
        let (detail_visible, is_wide_mode) = app
            .workflows
            .run_view_state()
            .map(|state| (state.is_detail_visible(), state.is_wide_mode()))
            .unwrap_or_default();

        let column_spec: Vec<Constraint> = if detail_visible {
            if is_wide_mode {
                // wide mode - detail visible
                vec![Constraint::Percentage(45), Constraint::Percentage(35), Constraint::Percentage(20)]
            } else {
                // compact mode - detail visible
                vec![Constraint::Percentage(55), Constraint::Percentage(45), Constraint::Length(6)]
            }
        } else {
            if is_wide_mode {
                // wide mode - detail hidden
                vec![Constraint::Percentage(55), Constraint::Percentage(45)]
            } else {
                // compact mode - detail hidden
                vec![Constraint::Percentage(60), Constraint::Percentage(40)]
            }
        };
        Layout::horizontal(column_spec).split(area).to_vec()
    }

    fn on_route_exit(&mut self, app: &mut App) -> Vec<Effect> {
        app.workflows.end_inputs_session();
        Vec::new()
    }
}

impl RunViewComponent {
    fn handle_focus_cycle_keys(app: &mut App, code: KeyCode) -> bool {
        match code {
            KeyCode::Tab => {
                app.focus.next();
                true
            }
            KeyCode::BackTab => {
                app.focus.prev();
                true
            }
            _ => false,
        }
    }

    fn handle_global_shortcuts(
        run_state: &mut RunViewState,
        code: KeyCode,
        focus_request: &mut Option<FocusRequest>,
        log_shortcut_requested: &mut bool,
    ) {
        match code {
            KeyCode::Char('t' | 'T') => run_state.toggle_wide_mode(),
            KeyCode::Char('l' | 'L') => *log_shortcut_requested = true,
            KeyCode::Esc => {
                if let Some(request) = Self::close_detail(run_state) {
                    *focus_request = Some(request);
                }
            }
            _ => {}
        }
    }

    fn close_detail(run_state: &mut RunViewState) -> Option<FocusRequest> {
        if !run_state.is_detail_visible() {
            return None;
        }
        let source = run_state.detail().map(|detail| detail.source());
        run_state.hide_detail();
        match source {
            Some(RunDetailSource::Steps) => Some(FocusRequest::StepsTable),
            Some(RunDetailSource::Outputs) => Some(FocusRequest::OutputsTable),
            None => None,
        }
    }

    fn handle_steps_table_keys(&mut self, run_state: &mut RunViewState, code: KeyCode) -> Option<FocusRequest> {
        let mut focus_request = None;
        match code {
            KeyCode::Up => self.steps_view.table_state.scroll_up_by(1),
            KeyCode::Down => self.steps_view.table_state.scroll_down_by(1),
            KeyCode::PageUp => self.steps_view.table_state.scroll_up_by(5),
            KeyCode::PageDown => self.steps_view.table_state.scroll_down_by(5),
            KeyCode::Home => self.steps_view.table_state.scroll_up_by(u16::MAX),
            KeyCode::End => self.steps_view.table_state.scroll_down_by(u16::MAX),
            KeyCode::Enter => {
                let change = run_state.toggle_detail_for(RunDetailSource::Steps);
                focus_request = Self::handle_detail_toggle_result(RunDetailSource::Steps, change);
            }
            _ => {}
        }
        focus_request
    }

    fn handle_outputs_table_keys(&mut self, run_state: &mut RunViewState, code: KeyCode) -> Option<FocusRequest> {
        let mut focus_request = None;
        match code {
            KeyCode::Up => self.outputs_view.table_state.scroll_up_by(1),
            KeyCode::Down => self.outputs_view.table_state.scroll_down_by(1),
            KeyCode::PageUp => self.outputs_view.table_state.scroll_up_by(5),
            KeyCode::PageDown => self.outputs_view.table_state.scroll_down_by(5),
            KeyCode::Home => self.outputs_view.table_state.scroll_up_by(u16::MAX),
            KeyCode::End => self.outputs_view.table_state.scroll_down_by(u16::MAX),
            KeyCode::Enter => {
                let change = run_state.toggle_detail_for(RunDetailSource::Outputs);
                focus_request = Self::handle_detail_toggle_result(RunDetailSource::Outputs, change);
            }
            _ => {}
        }
        focus_request
    }

    fn handle_detail_keys(&mut self, run_state: &mut RunViewState, code: KeyCode) -> Option<FocusRequest> {
        match code {
            KeyCode::Up => self.detail_view.table_state.scroll_up_by(1),
            KeyCode::Down => self.detail_view.table_state.scroll_down_by(1),
            KeyCode::PageUp => self.detail_view.table_state.scroll_up_by(5),
            KeyCode::PageDown => self.detail_view.table_state.scroll_down_by(5),
            KeyCode::Home => self.detail_view.table_state.scroll_up_by(u16::MAX),
            KeyCode::End => self.detail_view.table_state.scroll_down_by(u16::MAX),
            KeyCode::Esc => return Self::close_detail(run_state),
            _ => {}
        }
        None
    }

    fn handle_cancel_button_keys(run_state: &RunViewState, run_id: &str, code: KeyCode) -> (Option<FocusRequest>, Vec<Effect>) {
        match code {
            KeyCode::Left | KeyCode::Up => (Some(FocusRequest::PauseButton), Vec::new()),
            KeyCode::Right | KeyCode::Down => (Some(FocusRequest::StepsTable), Vec::new()),
            KeyCode::Enter | KeyCode::Char(' ') => {
                if cancel_enabled(run_state.status()) {
                    (
                        None,
                        vec![Effect::WorkflowRunControl {
                            run_id: run_id.to_string(),
                            command: WorkflowRunControl::Cancel,
                        }],
                    )
                } else {
                    (None, Vec::new())
                }
            }
            _ => (None, Vec::new()),
        }
    }

    fn handle_pause_button_keys(run_state: &RunViewState, run_id: &str, code: KeyCode) -> (Option<FocusRequest>, Vec<Effect>) {
        match code {
            KeyCode::Left | KeyCode::Up => (Some(FocusRequest::OutputsTable), Vec::new()),
            KeyCode::Right | KeyCode::Down => (Some(FocusRequest::CancelButton), Vec::new()),
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(command) = pause_command_for_status(run_state.status()) {
                    (
                        None,
                        vec![Effect::WorkflowRunControl {
                            run_id: run_id.to_string(),
                            command,
                        }],
                    )
                } else {
                    (None, Vec::new())
                }
            }
            _ => (None, Vec::new()),
        }
    }

    fn handle_detail_toggle_result(source: RunDetailSource, change: DetailPaneVisibilityChange) -> Option<FocusRequest> {
        match change {
            DetailPaneVisibilityChange::BecameVisible => Some(FocusRequest::DetailPane),
            DetailPaneVisibilityChange::BecameHidden => match source {
                RunDetailSource::Steps => Some(FocusRequest::StepsTable),
                RunDetailSource::Outputs => Some(FocusRequest::OutputsTable),
            },
        }
    }

    fn hit_test_mouse_target(app: &App, mouse: MouseEvent) -> Option<RunViewMouseTarget> {
        let layout_snapshot = app.workflows.run_view_state().map(|state| state.layout().clone())?;
        let container_area = layout_snapshot.last_area()?;
        let index = find_target_index_by_mouse_position(&container_area, layout_snapshot.mouse_target_areas(), mouse.column, mouse.row)?;
        layout_snapshot.mouse_target_roles().get(index).copied()
    }

    fn handle_mouse_target(app: &mut App, target: RunViewMouseTarget) -> Vec<Effect> {
        let Some(run_state) = app.workflows.run_view_state() else {
            return Vec::new();
        };
        match target {
            RunViewMouseTarget::StepsTable => {
                app.focus.focus(run_state.steps_focus_flag());
                Vec::new()
            }
            RunViewMouseTarget::OutputsTable => {
                app.focus.focus(run_state.outputs_focus_flag());
                Vec::new()
            }
            RunViewMouseTarget::DetailPane => {
                app.focus.focus(run_state.detail_focus_flag());
                Vec::new()
            }
            RunViewMouseTarget::CancelButton => {
                app.focus.focus(run_state.cancel_button_focus_flag());
                if cancel_enabled(run_state.status()) {
                    vec![Effect::WorkflowRunControl {
                        run_id: run_state.run_id().to_string(),
                        command: WorkflowRunControl::Cancel,
                    }]
                } else {
                    Vec::new()
                }
            }
            RunViewMouseTarget::PauseButton => {
                app.focus.focus(run_state.pause_button_focus_flag());
                if let Some(command) = pause_command_for_status(run_state.status()) {
                    vec![Effect::WorkflowRunControl {
                        run_id: run_state.run_id().to_string(),
                        command,
                    }]
                } else {
                    Vec::new()
                }
            }
        }
    }

    fn prepare_layout_state(container_area: Rect, header_area: Rect) -> RunViewLayout {
        let mut layout_state = RunViewLayout::default();
        layout_state.set_last_area(container_area);
        layout_state.set_header_area(header_area);
        layout_state
    }

    fn render_body(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        theme: &dyn Theme,
        run_state: &mut RunViewState,
        layout_state: &mut RunViewLayout,
        mouse_targets: &mut Vec<(Rect, RunViewMouseTarget)>,
    ) {
        let detail_visible = run_state.is_detail_visible();
        let column_spec: Vec<Constraint> = if detail_visible {
            vec![Constraint::Percentage(45), Constraint::Percentage(35), Constraint::Percentage(20)]
        } else {
            vec![Constraint::Percentage(55), Constraint::Percentage(45)]
        };
        let columns = Layout::horizontal(column_spec).split(area);

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
                self.render_detail_pane(frame, area, theme, run_state);
                layout_state.set_detail_area(Some(area));
                mouse_targets.push((area, RunViewMouseTarget::DetailPane));
            }
        } else {
            layout_state.set_detail_area(None);
        }
    }

    fn render_compact_body(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        theme: &dyn Theme,
        run_state: &mut RunViewState,
        layout_state: &mut RunViewLayout,
        mouse_targets: &mut Vec<(Rect, RunViewMouseTarget)>,
    ) {
        let detail_visible = run_state.is_detail_visible();
        let body_spec = if detail_visible {
            vec![Constraint::Percentage(55), Constraint::Percentage(45), Constraint::Length(6)]
        } else {
            vec![Constraint::Percentage(60), Constraint::Percentage(40)]
        };
        let rows = Layout::vertical(body_spec).split(area);

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
                self.render_detail_pane(frame, detail_area, theme, run_state);
                layout_state.set_detail_area(Some(detail_area));
                mouse_targets.push((detail_area, RunViewMouseTarget::DetailPane));
            }
        } else {
            layout_state.set_detail_area(None);
        }
    }

    fn render_detail_pane(&mut self, frame: &mut Frame, area: Rect, theme: &dyn Theme, run_state: &RunViewState) {
        let inner_block = th::block(theme, Some("Details"), run_state.detail_focus_flag().get()).borders(Borders::NONE);
        let inner_area = inner_block.inner(area);
        frame.render_widget(inner_block, area);
        let idx = self.detail_view.table_state.selected().unwrap_or(0);
        let entries = run_state.current_detail_entries(idx).unwrap_or_default();
        let payload = run_state.current_detail_payload(idx).cloned().unwrap_or(Value::Null);

        self.detail_view.render_kv_or_text(frame, inner_area, &entries, &payload, theme);
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

fn render_steps_table(frame: &mut Frame, area: Rect, theme: &dyn Theme, run_state: &mut RunViewState, view: &mut ResultsTableView) {
    let steps_focused = run_state.steps_focus_flag().get();
    let inner_block = th::block(theme, Some("Steps"), steps_focused).borders(Borders::NONE);
    let inner_area = inner_block.inner(area);
    frame.render_widget(inner_block, area);

    let table_state = run_state.steps_table_mut();
    view.render_results(frame, inner_area, table_state, steps_focused, theme);
}

fn render_outputs_table(frame: &mut Frame, area: Rect, theme: &dyn Theme, run_state: &mut RunViewState, view: &mut ResultsTableView) {
    let outputs_focused = run_state.outputs_focus_flag().get();
    let inner_block = th::block(theme, Some("Outputs"), outputs_focused).borders(Borders::NONE);
    let inner_area = inner_block.inner(area);
    frame.render_widget(inner_block, area);

    let table_state = run_state.outputs_table_mut();
    view.render_results(frame, inner_area, table_state, outputs_focused, theme);
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
