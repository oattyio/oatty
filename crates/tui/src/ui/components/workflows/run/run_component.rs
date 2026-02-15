//! Workflows run view component.
//!
//! This component renders the workflow execution experience, displaying step
//! progress, surfaced outputs, and control actions while reusing the shared
//! result results utilities. Interaction handling covers keyboard and mouse
//! paths, keeping behavior consistent with other workflow views.

use crate::app::App;
use crate::ui::components::workflows::run::state::{RunExecutionStatus, RunViewState};
use crate::ui::components::{
    common::ResultsTableView,
    component::{Component, find_target_index_by_mouse_position},
};
use crate::ui::theme::{
    Theme,
    theme_helpers::{self as th, ButtonRenderOptions, ButtonType, build_hint_spans},
};
use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use oatty_types::{Effect, ExecOutcome, Modal, Msg, Route, WorkflowRunControl};
use oatty_util::format_duration;
use rat_focus::HasFocus;
use ratatui::layout::Position;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect, Spacing},
    style::Modifier,
    symbols::merge::MergeStrategy,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

/// Captures mouse hit-testing and focus targets for the run view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionRole {
    StepsTable,
    CancelButton,
    PauseButton,
    DoneButton,
    ViewDetailsButton,
}

/// Layout metadata captured during the most recent render pass.
#[derive(Debug, Default, Clone)]
pub struct RunViewLayout {
    container_area: Rect,
    header_area: Rect,
    steps_area: Rect,
    steps_row_count: usize,
    cancel_button_area: Rect,
    pause_button_area: Rect,
    view_details_button_area: Rect,
    done_button_area: Rect,
    status_area: Rect,
    footer_block_area: Rect,
    mouse_target_areas: Vec<Rect>,
    mouse_target_roles: Vec<ActionRole>,
}

impl From<Vec<Rect>> for RunViewLayout {
    fn from(areas: Vec<Rect>) -> Self {
        let mouse_target_areas = vec![areas[2], areas[3], areas[4], areas[6], areas[7]];
        Self {
            container_area: areas[0],
            header_area: areas[1],
            steps_area: areas[2],
            steps_row_count: 0,
            cancel_button_area: areas[3],
            pause_button_area: areas[4],
            status_area: areas[5],
            view_details_button_area: areas[6],
            done_button_area: areas[7],
            footer_block_area: areas[8],
            mouse_target_areas,
            mouse_target_roles: vec![
                ActionRole::StepsTable,
                ActionRole::CancelButton,
                ActionRole::PauseButton,
                ActionRole::ViewDetailsButton,
                ActionRole::DoneButton,
            ],
        }
    }
}

#[derive(Debug, Default)]
pub struct RunViewComponent {
    steps_view: ResultsTableView,
    layout_state: RunViewLayout,
}

impl Component for RunViewComponent {
    fn handle_message(&mut self, app: &mut App, msg: Msg) -> Vec<Effect> {
        if let Msg::Tick = msg
            && let Some(run_state) = app.workflows.run_view_state_mut()
        {
            let theme = &*app.ctx.theme;
            run_state.advance_repeat_animations(theme);
        }
        Vec::new()
    }

    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        // Early exit for global focus-cycling keys (Tab, etc.)
        if Self::handle_focus_cycle_keys(app, key.code) {
            return Vec::new();
        }

        let Some(run_state) = app.workflows.run_view_state_mut() else {
            return Vec::new();
        };

        let run_id = run_state.run_id().to_string();

        match () {
            _ if run_state.steps_table.focus().get() => self.handle_steps_table_keys(run_state, key.code),
            _ if run_state.cancel_button_focus.get() => Self::handle_cancel_button_keys(run_state, &run_id, key.code),
            _ if run_state.pause_button_focus.get() => Self::handle_pause_button_keys(run_state, &run_id, key.code),
            _ if run_state.view_details_button_focus.get() => self.handle_view_details_button_keys(run_state, key.code),
            _ if run_state.done_button_focus.get() => Self::handle_done_button_keys(key.code),
            () => Vec::new(),
        }
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let mut effects = Vec::new();
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind
            && let Some(target) = self.hit_test_mouse_target(mouse)
        {
            let pos = Position::new(mouse.column, mouse.row);
            effects.extend(self.handle_mouse_target(app, target, pos))
        }
        effects
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let theme = &*app.ctx.theme;
        let block = Block::default()
            .title(Span::styled("Workflow Run", theme.text_secondary_style()))
            .borders(Borders::ALL)
            .border_style(theme.border_style(true))
            .style(th::panel_style(theme))
            .merge_borders(MergeStrategy::Exact);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let layout = RunViewLayout::from(self.get_preferred_layout(app, inner));
        let Some(run_state) = app.workflows.run_view_state_mut() else {
            render_empty(frame, inner, theme);
            return;
        };
        if run_state.steps_table.table_state.selected().is_none() && run_state.steps_table.has_rows() {
            run_state.steps_table.table_state.select(Some(0));
        }

        render_header(frame, layout.header_area, theme, run_state);
        render_steps_table(frame, layout.steps_area, theme, run_state, &mut self.steps_view);
        self.render_footer(frame, &layout, theme, run_state);

        let mut layout = layout;
        layout.steps_row_count = run_state.steps_table.num_rows();
        self.layout_state = layout;
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        let Some(run_state) = app.workflows.run_view_state() else {
            return Vec::new();
        };

        if run_state.cancel_button_focus.get() {
            return build_hint_spans(theme, &[(" Enter", " Cancel run ")]);
        }

        if run_state.pause_button_focus.get() {
            let label = pause_button_label(run_state.status());
            return build_hint_spans(theme, &[(" Enter/Space", label)]);
        }

        if run_state.view_details_button_focus.get() {
            return build_hint_spans(theme, &[(" Enter/Space", " View detail ")]);
        }

        if run_state.done_button_focus.get() {
            return build_hint_spans(theme, &[(" Enter/Space", " Close run ")]);
        }

        build_hint_spans(
            theme,
            &[
                (" Esc", " Close detail "),
                (" ↑/↓", " Navigate "),
                (" Enter", " View detail "),
                (" L", " View logs "),
            ],
        )
    }

    fn get_preferred_layout(&self, app: &App, area: Rect) -> Vec<Rect> {
        let main_regions = Layout::vertical([
            Constraint::Length(3), // header
            Constraint::Min(6),    // body
            Constraint::Length(5), // footer (3 rows for bordered buttons + block borders)
        ])
        .spacing(Spacing::Overlap(1))
        .split(area);
        let steps_area = main_regions[1];
        let footer_block = get_footer_block(&*app.ctx.theme);
        let button_row = Layout::horizontal([
            Constraint::Length(12), // cancel button
            Constraint::Length(12), // pause button
            Constraint::Min(0),     // status area
            Constraint::Length(14), // view details button
            Constraint::Length(12), // Done button
        ])
        .split(footer_block.inner(main_regions[2]));
        let cancel_area = button_row[0];
        let pause_area = button_row[1];
        let view_details_area = button_row[3];
        let done_area = button_row[4];

        // Horizontally center the status area.
        let status_area = Rect::new(
            button_row[2].x + 1,
            button_row[2].y + (button_row[2].height) / 2,
            button_row[2].width,
            1,
        );
        vec![
            area,              // entire render area
            main_regions[0],   // header
            steps_area,        // steps results
            cancel_area,       // cancel button
            pause_area,        // pause button
            status_area,       // status area
            view_details_area, // view details button
            done_area,         // Done button
            main_regions[2],   // footer block
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

    fn handle_steps_table_keys(&mut self, run_state: &mut RunViewState, code: KeyCode) -> Vec<Effect> {
        let mut effects = Vec::new();
        let table_state = &mut run_state.steps_table.table_state;
        match code {
            KeyCode::Up => table_state.scroll_up_by(1),
            KeyCode::Down => table_state.scroll_down_by(1),
            KeyCode::PageUp => table_state.scroll_up_by(5),
            KeyCode::PageDown => table_state.scroll_down_by(5),
            KeyCode::Home => table_state.scroll_up_by(u16::MAX),
            KeyCode::End => table_state.scroll_down_by(u16::MAX),
            KeyCode::Enter => {
                effects.extend(self.show_step_output(run_state));
            }
            _ => {}
        }
        effects
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
            _ => {}
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
            _ => {}
        }
        effects
    }

    fn handle_done_button_keys(code: KeyCode) -> Vec<Effect> {
        match code {
            KeyCode::Enter | KeyCode::Char(' ') => vec![Effect::SwitchTo(Route::Workflows)],
            _ => Vec::new(),
        }
    }

    fn handle_view_details_button_keys(&self, run_state: &RunViewState, code: KeyCode) -> Vec<Effect> {
        match code {
            KeyCode::Enter | KeyCode::Char(' ') => self.show_step_output(run_state),
            _ => Vec::new(),
        }
    }

    fn handle_mouse_target(&mut self, app: &mut App, target: ActionRole, pos: Position) -> Vec<Effect> {
        let mut effects = Vec::new();
        let Some(run_state) = app.workflows.run_view_state_mut() else {
            return effects;
        };
        match target {
            ActionRole::StepsTable => {
                let idx = self.hit_test_table(pos, run_state.steps_table.table_state.offset());
                let already_selected = run_state.steps_table.table_state.selected();
                run_state.steps_table.table_state.select(idx);
                app.focus.focus(&run_state.steps_table.focus());
                if idx.is_some() && idx == already_selected {
                    effects.extend(self.show_step_output(run_state));
                }
            }
            ActionRole::CancelButton => {
                app.focus.focus(&run_state.cancel_button_focus);
                if cancel_enabled(run_state.status()) {
                    effects.push(Effect::WorkflowRunControl {
                        run_id: run_state.run_id().to_string(),
                        command: WorkflowRunControl::Cancel,
                    });
                }
            }
            ActionRole::PauseButton => {
                app.focus.focus(&run_state.pause_button_focus);
                if let Some(command) = pause_command_for_status(run_state.status()) {
                    effects.push(Effect::WorkflowRunControl {
                        run_id: run_state.run_id().to_string(),
                        command,
                    });
                }
            }
            ActionRole::ViewDetailsButton => {
                app.focus.focus(&run_state.view_details_button_focus);
                effects.extend(self.show_step_output(run_state));
            }
            ActionRole::DoneButton => {
                app.focus.focus(&run_state.done_button_focus);
                effects.push(Effect::SwitchTo(Route::Workflows));
            }
        }
        effects
    }

    fn show_step_output(&self, run_state: &RunViewState) -> Vec<Effect> {
        let mut effects = Vec::new();
        if let Some(selected_index) = run_state.steps_table.table_state.selected()
            && let Some(value) = run_state.output_by_index(selected_index)
        {
            let outcome = ExecOutcome::Http {
                status_code: 200,
                log_entry: "View step output".to_string(),
                payload: value,
                request_id: 0,
            };
            effects.push(Effect::ShowModal(Modal::Results(Box::new(outcome))));
        }
        effects
    }

    fn hit_test_mouse_target(&self, mouse: MouseEvent) -> Option<ActionRole> {
        let container_area = self.layout_state.container_area;
        let index = find_target_index_by_mouse_position(&container_area, &self.layout_state.mouse_target_areas, mouse.column, mouse.row)?;
        self.layout_state.mouse_target_roles.get(index).copied()
    }

    fn hit_test_table(&self, pos: Position, offset: usize) -> Option<usize> {
        if !self.layout_state.steps_area.contains(pos) {
            return None;
        }
        let table_area = self.layout_state.steps_area;
        let index = pos.y.saturating_sub(table_area.y + 2) as usize + offset; // +2 for the header row and block border
        (index < self.layout_state.steps_row_count).then_some(index)
    }

    fn render_footer(&self, frame: &mut Frame, layout: &RunViewLayout, theme: &dyn Theme, run_state: &RunViewState) {
        let footer_block = get_footer_block(theme);
        frame.render_widget(footer_block, layout.footer_block_area);

        let cancel_enabled = cancel_enabled(run_state.status());
        let cancel_options = ButtonRenderOptions::new(
            cancel_enabled,
            run_state.cancel_button_focus.get(),
            false,
            Borders::ALL,
            ButtonType::Secondary,
        );
        th::render_button(frame, layout.cancel_button_area, "Cancel", theme, cancel_options);

        let pause_enabled = pause_command_for_status(run_state.status()).is_some();
        let pause_label = pause_button_label(run_state.status());
        let pause_options = ButtonRenderOptions::new(
            pause_enabled,
            run_state.pause_button_focus.get(),
            false,
            Borders::ALL,
            ButtonType::Primary,
        );
        th::render_button(frame, layout.pause_button_area, pause_label, theme, pause_options);

        let view_details_enabled = run_state.steps_table.table_state.selected().is_some();
        let view_details_options = ButtonRenderOptions::new(
            view_details_enabled,
            run_state.view_details_button_focus.get(),
            false,
            Borders::ALL,
            ButtonType::Secondary,
        );
        th::render_button(frame, layout.view_details_button_area, "View Details", theme, view_details_options);

        let done_enabled = run_state.status() == RunExecutionStatus::Succeeded;
        let done_options = ButtonRenderOptions::new(
            done_enabled,
            run_state.done_button_focus.get(),
            false,
            Borders::ALL,
            ButtonType::Secondary,
        );
        th::render_button(frame, layout.done_button_area, "Done", theme, done_options);

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
        frame.render_widget(status_paragraph, layout.status_area);
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

    let spans = vec![
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

    let paragraph = Paragraph::new(Line::from(spans)).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn render_steps_table(frame: &mut Frame, area: Rect, theme: &dyn Theme, run_state: &mut RunViewState, view: &mut ResultsTableView) {
    let steps_focused = run_state.steps_table.focus().get();
    let inner_block = th::block(theme, Some("Steps"), steps_focused).merge_borders(MergeStrategy::Exact);
    let inner_area = inner_block.inner(area);
    frame.render_widget(inner_block, area);

    let table_state = run_state.steps_table_mut();
    view.render_results(frame, inner_area, table_state, steps_focused, theme);
}

fn get_footer_block(theme: &dyn Theme) -> Block<'_> {
    th::block::<String>(theme, None, false).merge_borders(MergeStrategy::Exact)
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
