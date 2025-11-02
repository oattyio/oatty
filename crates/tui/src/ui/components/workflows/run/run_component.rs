//! Workflow run view component.
//!
//! This component renders the workflow execution experience, displaying step
//! progress, surfaced outputs, and control actions while reusing the shared
//! result table utilities. Interaction handling covers keyboard and mouse
//! paths, keeping behavior consistent with other workflow views.

use chrono::{Duration, Utc};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use rat_focus::HasFocus;
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
    RunDetailSource, RunExecutionStatus, RunViewState,
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

/// Captures mouse hit-testing targets for the run view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunViewMouseTarget {
    StepsTable,
    OutputsTable,
    DetailPane,
    CancelButton,
    PauseButton,
}

/// Layout metadata captured during the most recent render pass.
#[derive(Debug, Default, Clone)]
pub struct RunViewLayout {
    container_area: Rect,
    header_area: Rect,
    steps_area: Rect,
    outputs_area: Rect,
    detail_area: Rect,
    footer_area: Rect,
    cancel_button_area: Rect,
    pause_button_area: Rect,
    footer_block_area: Rect,
    status_area: Rect,
    mouse_target_areas: Vec<Rect>,
    mouse_target_roles: Vec<RunViewMouseTarget>,
}

impl From<Vec<Rect>> for RunViewLayout {
    fn from(areas: Vec<Rect>) -> Self {
        Self {
            container_area: areas[0],
            header_area: areas[1],
            steps_area: areas[2],
            outputs_area: areas[3],
            detail_area: areas[4],
            cancel_button_area: areas[5],
            pause_button_area: areas[6],
            footer_block_area: areas[7],
            status_area: areas[8],
            footer_area: areas[9],
            mouse_target_areas: areas[2..7].to_vec(),
            mouse_target_roles: vec![
                RunViewMouseTarget::StepsTable,
                RunViewMouseTarget::OutputsTable,
                RunViewMouseTarget::DetailPane,
                RunViewMouseTarget::CancelButton,
                RunViewMouseTarget::PauseButton,
            ],
        }
    }
}

#[derive(Debug, Default)]
pub struct RunViewComponent {
    steps_view: ResultsTableView,
    outputs_view: ResultsTableView,
    detail_view: ResultsTableView,
    layout_state: RunViewLayout,
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

            let Some(run_state) = app.workflows.run_view_state_mut() else {
                return Vec::new();
            };

            let mut effects: Vec<Effect> = Vec::new();

            let run_id = run_state.run_id().to_string();
            Self::handle_global_shortcuts(run_state, key.code);

            if run_state.steps_table.focus().get() {
                self.handle_steps_table_keys(run_state, key.code);
            } else if run_state.outputs_table.focus().get() {
                self.handle_outputs_table_keys(run_state, key.code);
            } else if run_state.detail_focus.get() {
                self.handle_detail_keys(run_state, key.code);
            } else if run_state.cancel_button_focus.get() {
                effects.extend(Self::handle_cancel_button_keys(run_state, &run_id, key.code));
            } else if run_state.pause_button_focus.get() {
                effects.extend(Self::handle_pause_button_keys(run_state, &run_id, key.code));
            }

        effects
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return Vec::new();
        }

        let Some(target) = self.hit_test_mouse_target(mouse) else {
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

        let layout = RunViewLayout::from(self.get_preferred_layout(app, inner));
        let Some(run_state) = app.workflows.run_view_state_mut() else {
            render_empty(frame, inner, theme);
            return;
        };

        render_header(frame, layout.header_area, theme, run_state);
        self.render_body(frame, theme, run_state, &layout);
        render_footer(frame, layout.footer_area, theme, run_state);

        self.layout_state = layout;
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        let Some(run_state) = app.workflows.run_view_state() else {
            return Vec::new();
        };

        if run_state.detail_focus.get() {
            return build_hint_spans(
                theme,
                &[(" Esc", " Close detail "), (" ↑/↓", " Navigate detail "), (" Tab", " Cycle focus ")],
            );
        }

        if run_state.cancel_button_focus.get() {
            return build_hint_spans(
                theme,
                &[(" ←/→", " Switch button "), (" Enter", " Cancel run "), (" Tab", " Cycle focus ")],
            );
        }

        if run_state.pause_button_focus.get() {
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

        let main_regions = Layout::vertical([
            Constraint::Length(3), // header
            Constraint::Min(6),    // body
            Constraint::Length(3), // footer
        ]).split(area);

        let column_spec: Vec<Constraint> = if detail_visible {
            if is_wide_mode {
                // wide mode - detail visible
                vec![
                    Constraint::Percentage(45), // steps
                    Constraint::Percentage(35), // outputs
                    Constraint::Percentage(20), // detail
                ]
            } else {
                // compact mode - detail visible
                vec![
                    Constraint::Percentage(55), // steps
                    Constraint::Percentage(45), // outputs
                    Constraint::Length(6)       // detail
                ]
            }
        } else {
            if is_wide_mode {
                // wide mode - detail hidden
                vec![
                    Constraint::Percentage(55), // steps
                    Constraint::Percentage(45), // outputs
                    Constraint::Length(0)       // detail - empty when not shown
                ]
            } else {
                // compact mode - detail hidden
                vec![
                    Constraint::Percentage(60), // steps
                    Constraint::Percentage(40), // outputs
                    Constraint::Length(0)       // detail - empty when not shown
                ]
            }
        };
        let body_area = Layout::horizontal(column_spec).split(main_regions[1]);

        let footer_block = get_footer_block(&*app.ctx.theme);
        let button_row = Layout::horizontal([
            Constraint::Length(20),
            Constraint::Length(20),
            Constraint::Min(0)]).split(footer_block.inner(main_regions[2]));
        let cancel_area = button_row[0];
        let pause_area = button_row[1];
        let status_area = Rect::new(button_row[2].x + 1, (button_row[2].y.saturating_sub(button_row[2].height)) / 2, button_row[2].width, 1);

        vec![
            area,
            main_regions[0], // header
            body_area[0],    // steps
            body_area[1],    // outputs
            body_area[2],    // detail
            cancel_area,     // cancel button
            pause_area,      // pause button
            status_area,     // status area
            main_regions[2], // footer block
        ]
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

    fn handle_global_shortcuts(run_state: &mut RunViewState, code: KeyCode) {
        match code {
            KeyCode::Char('t' | 'T') => run_state.toggle_wide_mode(),
            KeyCode::Esc => {
                run_state.hide_detail();
            }
            _ => {}
        }
    }

    fn handle_steps_table_keys(&mut self, run_state: &mut RunViewState, code: KeyCode) {
        match code {
            KeyCode::Up => self.steps_view.table_state.scroll_up_by(1),
            KeyCode::Down => self.steps_view.table_state.scroll_down_by(1),
            KeyCode::PageUp => self.steps_view.table_state.scroll_up_by(5),
            KeyCode::PageDown => self.steps_view.table_state.scroll_down_by(5),
            KeyCode::Home => self.steps_view.table_state.scroll_up_by(u16::MAX),
            KeyCode::End => self.steps_view.table_state.scroll_down_by(u16::MAX),
            KeyCode::Enter => {
                run_state.toggle_detail_for(RunDetailSource::Steps);
            }
            _ => {}
        }
    }

    fn handle_outputs_table_keys(&mut self, run_state: &mut RunViewState, code: KeyCode) {
        match code {
            KeyCode::Up => self.outputs_view.table_state.scroll_up_by(1),
            KeyCode::Down => self.outputs_view.table_state.scroll_down_by(1),
            KeyCode::PageUp => self.outputs_view.table_state.scroll_up_by(5),
            KeyCode::PageDown => self.outputs_view.table_state.scroll_down_by(5),
            KeyCode::Home => self.outputs_view.table_state.scroll_up_by(u16::MAX),
            KeyCode::End => self.outputs_view.table_state.scroll_down_by(u16::MAX),
            KeyCode::Enter => {
                run_state.toggle_detail_for(RunDetailSource::Outputs);
            }
            _ => {}
        }
    }

    fn handle_detail_keys(&mut self, run_state: &mut RunViewState, code: KeyCode) {
        match code {
            KeyCode::Up => self.detail_view.table_state.scroll_up_by(1),
            KeyCode::Down => self.detail_view.table_state.scroll_down_by(1),
            KeyCode::PageUp => self.detail_view.table_state.scroll_up_by(5),
            KeyCode::PageDown => self.detail_view.table_state.scroll_down_by(5),
            KeyCode::Home => self.detail_view.table_state.scroll_up_by(u16::MAX),
            KeyCode::End => self.detail_view.table_state.scroll_down_by(u16::MAX),
            KeyCode::Esc => run_state.hide_detail(),
            _ => {}
        }
    }

    fn handle_cancel_button_keys(run_state: &RunViewState, run_id: &str, code: KeyCode) -> Vec<Effect> {
        let mut effects = Vec::new();
        match code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                if cancel_enabled(run_state.status()) {
                    effects.push(Effect::WorkflowRunControl {
                        run_id: run_id.to_string(),
                        command: WorkflowRunControl::Cancel,
                    })
                }
            }
            _ => {},
        }
        effects
    }

    fn handle_pause_button_keys(run_state: &RunViewState, run_id: &str, code: KeyCode) -> Vec<Effect> {
        let mut effects = Vec::new();
        match code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(command) = pause_command_for_status(run_state.status()) {
                        effects.push(Effect::WorkflowRunControl {
                            run_id: run_id.to_string(),
                            command,
                        })
                }
            }
            _ =>  {},
        }
       effects
    }

    fn handle_mouse_target(app: &mut App, target: RunViewMouseTarget) -> Vec<Effect> {
        let mut effects = Vec::new();
        let Some(run_state) = app.workflows.run_view_state() else {
            return effects;
        };
        match target {
            RunViewMouseTarget::StepsTable => {
                app.focus.focus(&run_state.steps_table.focus());
            }
            RunViewMouseTarget::OutputsTable => {
                app.focus.focus(&run_state.outputs_table.focus());
            }
            RunViewMouseTarget::DetailPane => {
                app.focus.focus(&run_state.detail_focus);
            }
            RunViewMouseTarget::CancelButton => {
                app.focus.focus(&run_state.cancel_button_focus);
                if cancel_enabled(run_state.status()) {
                   effects.push(Effect::WorkflowRunControl {
                        run_id: run_state.run_id().to_string(),
                        command: WorkflowRunControl::Cancel,
                    });
                }
            }
            RunViewMouseTarget::PauseButton => {
                app.focus.focus(&run_state.pause_button_focus);
                if let Some(command) = pause_command_for_status(run_state.status()) {
                    effects.push(Effect::WorkflowRunControl {
                        run_id: run_state.run_id().to_string(),
                        command,
                    });
                }
            }
        }
        effects
    }

    fn render_body(
        &mut self,
        frame: &mut Frame,
        theme: &dyn Theme,
        run_state: &mut RunViewState,
        layout_state: &RunViewLayout,
    ) {
        let detail_visible = run_state.is_detail_visible();
        render_steps_table(frame, layout_state.steps_area, theme, run_state, &mut self.steps_view);
        render_outputs_table(frame, layout_state.outputs_area, theme, run_state, &mut self.outputs_view);
        if detail_visible {
        self.render_detail_pane(frame, layout_state.detail_area, theme, run_state);
        }
    }

    fn render_detail_pane(&mut self, frame: &mut Frame, area: Rect, theme: &dyn Theme, run_state: &RunViewState) {
        let inner_block = th::block(theme, Some("Details"), run_state.detail_focus.get()).borders(Borders::NONE);
        let inner_area = inner_block.inner(area);
        frame.render_widget(inner_block, area);
        let idx = self.detail_view.table_state.selected().unwrap_or(0);
        let entries = run_state.current_detail_entries(idx).unwrap_or_default();
        let payload = run_state.current_detail_payload(idx).cloned().unwrap_or(Value::Null);

        self.detail_view.render_kv_or_text(frame, inner_area, &entries, &payload, theme);
    }

    fn hit_test_mouse_target(&self, mouse: MouseEvent) -> Option<RunViewMouseTarget> {
        let container_area = self.layout_state.footer_area;
        let index = find_target_index_by_mouse_position(&container_area, &self.layout_state.mouse_target_areas, mouse.column, mouse.row)?;
        self.layout_state.mouse_target_roles.get(index).copied()
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
    let steps_focused = run_state.steps_table.focus().get();
    let inner_block = th::block(theme, Some("Steps"), steps_focused).borders(Borders::NONE);
    let inner_area = inner_block.inner(area);
    frame.render_widget(inner_block, area);

    let table_state = run_state.steps_table_mut();
    view.render_results(frame, inner_area, table_state, steps_focused, theme);
}

fn render_outputs_table(frame: &mut Frame, area: Rect, theme: &dyn Theme, run_state: &mut RunViewState, view: &mut ResultsTableView) {
    let outputs_focused = run_state.outputs_table.focus().get();
    let inner_block = th::block(theme, Some("Outputs"), outputs_focused).borders(Borders::NONE);
    let inner_area = inner_block.inner(area);
    frame.render_widget(inner_block, area);

    let table_state = run_state.outputs_table_mut();
    view.render_results(frame, inner_area, table_state, outputs_focused, theme);
}
fn get_footer_block(theme: &dyn Theme) -> Block {
    th::block(theme, None, false).borders(Borders::NONE)
}

fn render_footer(
    frame: &mut Frame,
    area: Rect,
    theme: &dyn Theme,
    run_state: &RunViewState,
) {
    let footer_block = get_footer_block(theme);
    let inner_area = footer_block.inner(area);
    frame.render_widget(footer_block, area);

    let button_row = Layout::horizontal([Constraint::Length(20), Constraint::Length(20), Constraint::Min(0)]).split(inner_area);
    let cancel_area = button_row[0];
    let pause_area = button_row[1];
    let status_area = button_row[2];

    let cancel_enabled = cancel_enabled(run_state.status());
    let cancel_options = ButtonRenderOptions::new(
        cancel_enabled,
        run_state.cancel_button_focus.get(),
        false,
        Borders::ALL,
        false,
    );
    th::render_button(frame, cancel_area, "Cancel", theme, cancel_options);

    let pause_enabled = pause_command_for_status(run_state.status()).is_some();
    let pause_label = pause_button_label(run_state.status());
    let pause_options = ButtonRenderOptions::new(pause_enabled, run_state.pause_button_focus.get(), false, Borders::ALL, true);
    th::render_button(frame, pause_area, pause_label, theme, pause_options);

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
