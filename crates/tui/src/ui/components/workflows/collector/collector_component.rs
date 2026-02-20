use crate::app::App;
use crate::ui::components::common::ResultsTableView;
use crate::ui::components::common::TextInputState;
use crate::ui::components::component::Component;
use crate::ui::components::workflows::collector::manual_entry::ManualEntryComponent;
use crate::ui::components::workflows::collector::{
    CollectorApplyTarget, CollectorSelectionSource, CollectorStagedSelection, CollectorViewState, SelectorStatus,
};
use crate::ui::components::workflows::view_utils::{classify_json_value, style_for_role};
use crate::ui::theme::Theme;
use crate::ui::theme::theme_helpers::{self as th, ButtonRenderOptions, ButtonType, build_hint_spans};
use crate::ui::utils::{KeyScoreContext, get_scored_keys_with_context, render_value};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use oatty_engine::field_paths::{missing_details_from_json_row, nested_scalar_leaf_candidates_from_json, non_scalar_runtime_message};
use oatty_engine::resolve::select_path;
use oatty_types::{Effect, ExecOutcome, Modal, Msg, WorkflowProviderErrorPolicy};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::Modifier;
use ratatui::symbols::merge::MergeStrategy;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Wrap};
use serde_json::Value as JsonValue;

/// Retained layout metadata capturing screen regions for pointer hit-testing.
#[derive(Debug, Clone, Default)]
struct WorkflowCollectorLayoutState {
    /// Rect covering the filter input at the top of the modal.
    pub filter_panel: Rect,
    /// Rect covering the inner text area for the filter input.
    pub filter_inner_area: Rect,
    /// Rect covering the status row under the filter.
    pub status_area: Rect,
    /// Rect covering the results table body.
    pub table_area: Rect,
    /// Rect covering the inline manual override input panel.
    pub manual_panel_area: Rect,
    /// Rect covering the inner text area for manual override input.
    pub manual_inner_area: Rect,
    /// Rect for the Cancel button inside the footer.
    pub cancel_button_area: Rect,
    /// Rect for the Apply button inside the footer.
    pub apply_button_area: Rect,
}

/// Rendering parameters for a single-line text input panel.
#[derive(Debug, Clone, Copy)]
struct TextInputPanelRenderSpec<'a> {
    title: &'a str,
    is_focused: bool,
    text: &'a str,
    placeholder: Option<&'a str>,
    cursor_columns: usize,
}

impl From<Vec<Rect>> for WorkflowCollectorLayoutState {
    fn from(value: Vec<Rect>) -> Self {
        Self {
            filter_panel: value[0],
            filter_inner_area: Rect::default(),
            status_area: value[1],
            table_area: value[2],
            manual_panel_area: value[3],
            manual_inner_area: Rect::default(),
            cancel_button_area: value[4],
            apply_button_area: value[5],
        }
    }
}

/// Component that orchestrates workflow input collection modals (manual entry and selector).
#[derive(Debug, Default)]
pub struct WorkflowCollectorComponent {
    manual_entry: ManualEntryComponent,
    results_table_view: ResultsTableView,
    layout: WorkflowCollectorLayoutState,
}

impl Component for WorkflowCollectorComponent {
    fn handle_message(&mut self, app: &mut App, message: Msg) -> Vec<Effect> {
        if let Msg::ExecCompleted(outcome) = message
            && let ExecOutcome::Log(log_message) = outcome.as_ref()
        {
            self.handle_provider_fetch_failure(app, log_message);
        }
        Vec::new()
    }

    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let mut effects = Vec::new();

        if app.workflows.manual_entry_state().is_some() {
            return self.manual_entry.handle_key_events(app, key);
        }

        match key.code {
            KeyCode::BackTab => {
                app.focus.prev();
            }
            KeyCode::Tab => {
                app.focus.next();
            }
            _ => {}
        }

        if app.workflows.collector_state_mut().is_some() {
            return self.handle_selector_key_events(app, key);
        }

        if key.code == KeyCode::Esc {
            effects.push(Effect::CloseModal)
        }
        effects
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        if app.workflows.manual_entry_state().is_some() {
            return self.manual_entry.handle_mouse_events(app, mouse);
        }

        if app.workflows.collector_state().is_none() {
            return Vec::new();
        }

        let position = Position::new(mouse.column, mouse.row);
        let table = &mut app.workflows.collector_state_mut().expect("selector state").table;
        let offset = if table.has_rows() {
            table.table_state.offset()
        } else {
            table.list_state.offset()
        };
        let row_count = if table.has_rows() {
            table.num_rows()
        } else {
            table.kv_entries().len()
        };

        match mouse.kind {
            MouseEventKind::Moved | MouseEventKind::Up(MouseButton::Left) => {
                table.mouse_over_idx = if self.layout.table_area.contains(position) {
                    if table.has_rows() {
                        self.hit_test_results_table(position, offset, row_count)
                    } else {
                        self.hit_test_key_value_rows(position, offset, row_count)
                    }
                } else {
                    None
                };
            }
            MouseEventKind::ScrollDown => {
                if self.layout.table_area.contains(position) {
                    if table.has_rows() {
                        table.table_state.scroll_down_by(1);
                    } else {
                        table.list_state.scroll_down_by(1);
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if self.layout.table_area.contains(position) {
                    if table.has_rows() {
                        table.table_state.scroll_up_by(1);
                    } else {
                        table.list_state.scroll_up_by(1);
                    }
                }
            }
            _ => {}
        }

        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return Vec::new();
        }

        let mut effects = Vec::new();
        let collector = app.workflows.collector_state_mut().expect("selector state");

        if self.layout.cancel_button_area.contains(position) {
            app.focus.focus(&collector.f_cancel);
            collector.clear_staged_selection();
            effects.push(Effect::CloseModal);
        }

        if self.layout.apply_button_area.contains(position) {
            app.focus.focus(&collector.f_apply);
            effects.extend(self.apply_selection_to_run_state(app));
            return effects;
        }

        if self.layout.filter_panel.contains(position) {
            app.focus.focus(&collector.f_filter);
            collector.focus_filter();
            let relative_column = position.x.saturating_sub(self.layout.filter_inner_area.x);
            let cursor_index = collector.filter.cursor_index_for_column(relative_column);
            collector.filter.set_cursor(cursor_index);
        }

        if self.layout.manual_panel_area.contains(position) {
            app.focus.focus(&collector.f_manual);
            collector.set_selection_source(CollectorSelectionSource::Manual);
            let relative_column = position.x.saturating_sub(self.layout.manual_inner_area.x);
            let cursor_index = collector.manual_override.cursor_index_for_column(relative_column);
            collector.manual_override.set_cursor(cursor_index);
        }

        if self.layout.table_area.contains(position) {
            app.focus.focus(&collector.f_table);
            if collector.table.has_rows() {
                if let Some(index) = self.hit_test_results_table(position, offset, collector.table.num_rows()) {
                    self.select_table_cell_from_mouse(collector, index, position);
                    if let Err(message) = self.stage_current_row(collector) {
                        collector.error_message = Some(message);
                    }
                }
            } else if let Some(index) = self.hit_test_key_value_rows(position, offset, collector.table.kv_entries().len()) {
                collector.table.list_state.select(Some(index));
                if let Err(message) = self.stage_current_row(collector) {
                    collector.error_message = Some(message);
                }
            }
        }

        effects
    }

    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        if app.workflows.manual_entry_state().is_some() {
            self.manual_entry.render(frame, rect, app);
            return;
        }

        if app.workflows.collector_state_mut().is_some() {
            self.render_selector(frame, rect, app);
            return;
        }

        let block = Block::bordered().title("Collector");
        let inner_area = block.inner(rect);
        frame.render_widget(block, rect);
        frame.render_widget(
            Paragraph::new("No selector state").block(Block::default().borders(Borders::ALL)),
            inner_area,
        );
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        if app.workflows.manual_entry_state().is_some() {
            return self.manual_entry.get_hint_spans(app);
        }
        if let Some(selector) = app.workflows.collector_state() {
            return Self::selector_hint_spans(theme, selector);
        }
        Vec::new()
    }

    fn get_preferred_layout(&self, app: &App, area: Rect) -> Vec<Rect> {
        let Some(_collector) = app.workflows.collector_state() else {
            return Vec::new();
        };

        let layout = Layout::vertical([
            Constraint::Length(4), // header
            Constraint::Min(6),    // results
            Constraint::Length(3), // manual override
            Constraint::Length(3), // footer
        ])
        .split(area);

        let header_layout = Layout::vertical([Constraint::Length(3), Constraint::Length(1)]).split(layout[0]);
        let footer_layout = Layout::horizontal([Constraint::Length(24), Constraint::Min(0)]).split(layout[3]);
        let button_layout = Layout::horizontal([Constraint::Length(12), Constraint::Length(12)]).split(footer_layout[0]);

        vec![
            header_layout[0], // filter panel
            header_layout[1], // status
            layout[1],        // results
            layout[2],        // manual override
            button_layout[0], // cancel button
            button_layout[1], // apply button
        ]
    }

    fn on_route_exit(&mut self, app: &mut App) -> Vec<Effect> {
        app.workflows.handle_route_exit_cleanup();
        Vec::new()
    }
}

impl WorkflowCollectorComponent {
    const MAX_CELL_SCALAR_DEPTH: usize = 3;

    fn apply_text_input_key(buffer: &mut TextInputState, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Left => {
                buffer.move_left();
                true
            }
            KeyCode::Right => {
                buffer.move_right();
                true
            }
            KeyCode::Home => {
                buffer.set_cursor(0);
                true
            }
            KeyCode::End => {
                buffer.set_cursor(buffer.input().len());
                true
            }
            KeyCode::Backspace => {
                buffer.backspace();
                true
            }
            KeyCode::Delete => {
                buffer.delete();
                true
            }
            KeyCode::Char(character) if (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT) && !character.is_control() => {
                buffer.insert_char(character);
                true
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                buffer.clear();
                true
            }
            _ => false,
        }
    }

    fn render_text_input_panel(&self, frame: &mut Frame, area: Rect, theme: &dyn Theme, spec: TextInputPanelRenderSpec<'_>) -> Rect {
        let panel_title = Line::from(Span::styled(
            spec.title.to_string(),
            theme.text_secondary_style().add_modifier(Modifier::BOLD),
        ));
        let mut block = th::block::<String>(theme, None, spec.is_focused).merge_borders(MergeStrategy::Exact);
        block = block.title(panel_title);
        let inner_area = block.inner(area);

        let content_line = if spec.is_focused || !spec.text.is_empty() {
            Line::from(Span::styled(spec.text.to_string(), theme.text_primary_style()))
        } else if let Some(placeholder_text) = spec.placeholder {
            Line::from(Span::styled(placeholder_text.to_string(), theme.text_muted_style()))
        } else {
            Line::from(Span::from(""))
        };

        frame.render_widget(Paragraph::new(content_line).block(block).wrap(Wrap { trim: true }), area);

        if spec.is_focused {
            frame.set_cursor_position((inner_area.x.saturating_add(spec.cursor_columns as u16), inner_area.y));
        }

        inner_area
    }

    fn value_to_palette_string(selected_value: JsonValue) -> String {
        match selected_value {
            JsonValue::String(text) => text,
            JsonValue::Number(number) => number.to_string(),
            JsonValue::Bool(boolean) => boolean.to_string(),
            JsonValue::Null => "null".to_string(),
            other => other.to_string(),
        }
    }

    fn handle_provider_fetch_failure(&self, app: &mut App, log_message: &str) {
        if !log_message.starts_with("Provider fetch failed:") {
            return;
        }

        let Some(collector) = app.workflows.collector_state_mut() else {
            return;
        };

        let has_pending_fetch = collector.pending_cache_key.is_some() || matches!(collector.status, SelectorStatus::Loading);
        if !has_pending_fetch {
            return;
        }

        collector.status = SelectorStatus::Error;
        collector.pending_cache_key = None;
        collector.error_message = Some(
            log_message
                .strip_prefix("Provider fetch failed:")
                .map(str::trim)
                .filter(|detail| !detail.is_empty())
                .map(|detail| format!("Provider selector failed: {detail}"))
                .unwrap_or_else(|| "Provider selector failed.".to_string()),
        );
    }

    fn hit_test_results_table(&self, mouse_position: Position, offset: usize, row_count: usize) -> Option<usize> {
        let first_data_row_y = self.layout.table_area.y.saturating_add(2);
        let row_index = mouse_position.y.saturating_sub(first_data_row_y) as usize + offset;
        (row_index < row_count).then_some(row_index)
    }

    fn hit_test_key_value_rows(&self, mouse_position: Position, offset: usize, row_count: usize) -> Option<usize> {
        let first_data_row_y = self.layout.table_area.y;
        let row_index = mouse_position.y.saturating_sub(first_data_row_y) as usize + offset;
        (row_index < row_count).then_some(row_index)
    }

    fn select_table_cell_from_mouse(&self, collector: &mut CollectorViewState<'_>, row_index: usize, mouse_position: Position) {
        let relative_x = mouse_position.x.saturating_sub(self.layout.table_area.x);
        let selected_column = collector
            .table
            .hit_test_column(relative_x, self.layout.table_area.width)
            .unwrap_or_else(|| collector.table.table_state.selected_column().unwrap_or(0));
        collector
            .table
            .select_cell(row_index, selected_column, self.layout.table_area.width);
        collector.set_selection_source(CollectorSelectionSource::Table);
    }

    fn handle_filter_keys(&self, app: &mut App, key: KeyEvent) {
        let Some(collector) = app.workflows.collector_state_mut() else {
            return;
        };
        if Self::apply_text_input_key(&mut collector.filter, key) {
            self.apply_filter(collector, &*app.ctx.theme);
        }
    }

    fn handle_manual_override_keys(&self, app: &mut App, key: KeyEvent) {
        let Some(collector) = app.workflows.collector_state_mut() else {
            return;
        };

        if Self::apply_text_input_key(&mut collector.manual_override, key) {
            collector.set_selection_source(CollectorSelectionSource::Manual);
            collector.error_message = None;
        }
    }

    fn handle_table_keys(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        if matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R')) {
            if let Some(collector) = app.workflows.collector_state_mut() {
                collector.status = SelectorStatus::Loading;
                collector.error_message = None;
            }
            return app.prepare_selector_fetch();
        }

        let collector = app.workflows.collector_state_mut().expect("selector state");
        if !collector.table.has_rows() {
            let list_state = &mut collector.table.list_state;
            match key.code {
                KeyCode::Up => list_state.scroll_up_by(1),
                KeyCode::Down => list_state.scroll_down_by(1),
                KeyCode::PageUp => list_state.scroll_up_by(10),
                KeyCode::PageDown => list_state.scroll_down_by(10),
                KeyCode::Home => list_state.scroll_up_by(u16::MAX),
                KeyCode::End => list_state.scroll_down_by(u16::MAX),
                KeyCode::Enter => {
                    if matches!(collector.status, SelectorStatus::Error)
                        && matches!(collector.on_error, Some(WorkflowProviderErrorPolicy::Fail))
                    {
                        collector.error_message = Some("provider error: cannot apply (policy: fail)".into());
                    } else if collector.table.drill_into_selection(&*app.ctx.theme) {
                        collector.clear_staged_selection();
                        collector.error_message = None;
                    } else if let Err(message) = self.stage_current_row(collector) {
                        collector.error_message = Some(message);
                    } else {
                        return self.apply_selection_to_run_state(app);
                    }
                }
                KeyCode::Char(' ') => {
                    if let Err(message) = self.stage_current_row(collector) {
                        collector.error_message = Some(message);
                    }
                }
                _ => {}
            }
            return Vec::new();
        }

        let row_len = collector.table.num_rows();
        let selected = collector.table.table_state.selected().unwrap_or_default();
        let table_state = &mut collector.table.table_state;

        match key.code {
            KeyCode::Left => {
                collector.table.move_selected_column_left(self.layout.table_area.width);
                if let Err(message) = self.stage_current_row(collector) {
                    collector.error_message = Some(message);
                }
            }
            KeyCode::Right => {
                collector.table.move_selected_column_right(self.layout.table_area.width);
                if let Err(message) = self.stage_current_row(collector) {
                    collector.error_message = Some(message);
                }
            }
            KeyCode::Up => table_state.select_previous(),
            KeyCode::Down => {
                if selected < row_len {
                    table_state.select_next();
                }
            }
            KeyCode::PageUp => table_state.scroll_up_by(5),
            KeyCode::PageDown => table_state.scroll_down_by(5),
            KeyCode::Home => table_state.scroll_up_by(u16::MAX),
            KeyCode::End => table_state.scroll_down_by(u16::MAX),
            KeyCode::Enter => {
                if matches!(collector.status, SelectorStatus::Error)
                    && matches!(collector.on_error, Some(WorkflowProviderErrorPolicy::Fail))
                {
                    collector.error_message = Some("provider error: cannot apply (policy: fail)".into());
                } else if collector.table.drill_into_selection(&*app.ctx.theme) {
                    collector.clear_staged_selection();
                    collector.error_message = None;
                } else if self.current_row_is_staged(collector) {
                    return self.apply_selection_to_run_state(app);
                } else if let Err(message) = self.stage_current_row(collector) {
                    collector.error_message = Some(message);
                }
            }
            KeyCode::Char(' ') => {
                if let Err(message) = self.stage_current_row(collector) {
                    collector.error_message = Some(message);
                }
            }
            _ => {}
        }
        Vec::new()
    }

    fn handle_button_keys(&self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let (f_cancel, f_apply) = app
            .workflows
            .collector
            .as_ref()
            .map(|collector| (collector.f_cancel.get(), collector.f_apply.get()))
            .unwrap_or_default();
        if app.workflows.collector.is_none() || key.code != KeyCode::Enter {
            return Vec::new();
        }

        let mut effects = Vec::new();
        if f_cancel {
            effects.push(Effect::CloseModal);
        }
        if f_apply {
            effects.extend(self.apply_selection_to_run_state(app));
        }
        effects
    }

    fn stage_current_row(&self, collector: &mut CollectorViewState<'_>) -> Result<(), String> {
        let (value, source_field) = self.extract_selected_value(collector)?;
        let row = if collector.table.has_rows() {
            let index = collector
                .table
                .table_state
                .selected()
                .ok_or_else(|| "no row selected".to_string())?;
            collector
                .table
                .selected_data(index)
                .cloned()
                .ok_or_else(|| "no provider row selected".to_string())?
        } else {
            let index = collector
                .table
                .list_state
                .selected()
                .ok_or_else(|| "no key/value row selected".to_string())?;
            collector
                .table
                .selected_kv_entry(index)
                .map(|entry| entry.raw_value.clone())
                .ok_or_else(|| "no key/value entry selected".to_string())?
        };
        let display_key = source_field.clone().unwrap_or_else(|| "value".to_string());
        let display_value = render_value(&display_key, &value, None).into_plain_text();
        collector.set_staged_selection(Some(CollectorStagedSelection::new(value, display_value, source_field, row)));
        collector.set_selection_source(CollectorSelectionSource::Table);
        collector.error_message = None;
        Ok(())
    }

    fn current_row_is_staged(&self, collector: &CollectorViewState<'_>) -> bool {
        if !collector.table.has_rows() {
            let Some(index) = collector.table.list_state.selected() else {
                return false;
            };
            let Some((staged, entry)) = collector.staged_selection().zip(collector.table.selected_kv_entry(index)) else {
                return false;
            };
            return staged.row == entry.raw_value && staged.source_field == Some(entry.key.clone());
        }

        let Some(index) = collector.table.table_state.selected() else {
            return false;
        };
        if let Some((staged, row)) = collector.staged_selection().zip(collector.table.selected_data(index)) {
            if collector.value_field.is_some() {
                staged.row == *row
            } else {
                staged.row == *row && staged.source_field == self.current_selected_source_field(collector)
            }
        } else {
            false
        }
    }

    fn apply_selection_to_run_state(&self, app: &mut App) -> Vec<Effect> {
        let Some(collector) = app.workflows.collector_state_mut() else {
            return Vec::new();
        };

        let selected_value = match collector.selection_source {
            CollectorSelectionSource::Table => {
                let Some(selection) = collector.take_staged_selection() else {
                    collector.error_message = Some("Select a table row/column value or use Manual Override".into());
                    return Vec::new();
                };
                selection.value
            }
            CollectorSelectionSource::Manual => {
                let Some(value) = self.parse_manual_override_value(&collector.manual_override) else {
                    collector.error_message = Some("Enter a manual value before applying".into());
                    return Vec::new();
                };
                value
            }
        };

        collector.error_message = None;
        let mut effects = Vec::new();

        match collector.apply_target {
            CollectorApplyTarget::WorkflowInput => {
                if let Some(name) = app.workflows.active_input_name() {
                    if let Some(run_rc) = app.workflows.active_run_state.clone() {
                        let mut run = run_rc.borrow_mut();
                        run.run_context.inputs.insert(name, selected_value);
                        let _ = run.evaluate_input_providers();
                    }
                    effects.push(Effect::CloseModal);
                }
            }
            CollectorApplyTarget::PaletteInput { positional } => {
                let value = Self::value_to_palette_string(selected_value);
                if positional {
                    app.palette.apply_accept_positional_suggestion(&value);
                } else {
                    app.palette.apply_accept_non_command_suggestion(&value);
                }
                app.palette.reduce_clear_suggestions();
                app.palette.set_is_suggestions_open(false);
                effects.push(Effect::CloseModal);
            }
        }

        effects
    }

    fn parse_manual_override_value(&self, manual_state: &TextInputState) -> Option<JsonValue> {
        let input = manual_state.input().trim();
        if input.is_empty() {
            return None;
        }

        serde_json::from_str::<JsonValue>(input)
            .ok()
            .or_else(|| Some(JsonValue::String(input.to_string())))
    }

    fn handle_selector_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let mut effects = Vec::new();
        let (f_filter, f_table, f_manual, f_cancel, f_apply) = app
            .workflows
            .collector
            .as_ref()
            .map(|collector| {
                (
                    collector.f_filter.get(),
                    collector.f_table.get(),
                    collector.f_manual.get(),
                    collector.f_cancel.get(),
                    collector.f_apply.get(),
                )
            })
            .unwrap_or_default();

        if f_filter {
            self.handle_filter_keys(app, key);
        }
        if f_table {
            effects.append(&mut self.handle_table_keys(app, key));
        }
        if f_manual {
            if key.code == KeyCode::Enter {
                effects.extend(self.apply_selection_to_run_state(app));
            } else if key.code == KeyCode::Char('o') && key.modifiers.contains(KeyModifiers::CONTROL) {
                if let Some(collector) = app.workflows.collector_state_mut()
                    && matches!(collector.apply_target, CollectorApplyTarget::WorkflowInput)
                {
                    collector.pending_manual_file_pick = true;
                    collector.error_message = None;
                    effects.push(Effect::ShowModal(Modal::FilePicker(vec!["json"])));
                }
            } else {
                self.handle_manual_override_keys(app, key);
            }
        }
        if f_cancel || f_apply {
            effects.append(&mut self.handle_button_keys(app, key));
        }

        let Some(collector) = app.workflows.collector_state_mut() else {
            return effects;
        };

        if key.code == KeyCode::Esc {
            if collector.table.drill_up(&*app.ctx.theme) {
                collector.error_message = None;
                return effects;
            }
            if collector.f_filter.get() && !collector.filter.is_empty() {
                collector.filter.clear();
                self.apply_filter(collector, &*app.ctx.theme);
                collector.focus_filter();
                return effects;
            }
            collector.clear_staged_selection();
            effects.push(Effect::CloseModal);
        }

        effects
    }

    fn apply_filter(&self, selector: &mut CollectorViewState<'_>, theme: &dyn Theme) {
        selector.refresh_table(theme);
    }

    fn extract_selected_value(&self, collector: &CollectorViewState<'_>) -> Result<(JsonValue, Option<String>), String> {
        if !collector.table.has_rows() {
            let index = collector
                .table
                .list_state
                .selected()
                .ok_or_else(|| "no key/value row selected".to_string())?;
            let entry = collector
                .table
                .selected_kv_entry(index)
                .ok_or_else(|| "no key/value entry selected".to_string())?;
            return self.scalar_value_from_json(&entry.raw_value, Some(entry.key.clone()), &entry.key);
        }

        let index = collector
            .table
            .table_state
            .selected()
            .ok_or_else(|| "no row selected".to_string())?;
        let row = collector
            .table
            .selected_data(index)
            .ok_or_else(|| "no provider row selected".to_string())?;

        if let Some(field_name) = self.current_selected_source_field(collector)
            && let JsonValue::Object(map) = row
        {
            let value = map
                .get(field_name.as_str())
                .ok_or_else(|| format!("selected field '{field_name}' missing from provider row"))?;
            if let Ok(scalar) = self.scalar_value_from_json(value, Some(field_name.clone()), &field_name) {
                return Ok(scalar);
            }
            if let Some((scalar, source_path)) = self.resolve_display_scalar_value(value, &field_name, 0) {
                return Ok((scalar, Some(source_path)));
            }
            return Err(non_scalar_runtime_message(&field_name));
        }

        if let Some(path) = collector.value_field.as_deref() {
            let leaf = path.split('.').next_back().unwrap_or(path);
            if let Some(value) = select_path(row, Some(path)) {
                return self.scalar_value_from_json(&value, Some(leaf.to_string()), path);
            }

            if let JsonValue::Object(map) = row
                && let Some(value) = map.get(leaf)
            {
                return self.scalar_value_from_json(value, Some(leaf.to_string()), path);
            }

            let nested_leaf_candidates = nested_scalar_leaf_candidates_from_json(row, leaf);
            if nested_leaf_candidates.len() == 1 {
                let (resolved_path, resolved_value) = nested_leaf_candidates.first().expect("single candidate");
                return Ok((resolved_value.clone(), Some(resolved_path.clone())));
            }
            if nested_leaf_candidates.len() > 1 {
                let candidates = nested_leaf_candidates
                    .iter()
                    .take(6)
                    .map(|(candidate_path, _)| candidate_path.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(format!(
                    "value_field '{path}' not found directly; found multiple nested '{leaf}' candidates ({candidates}). Set select.value_field to an explicit path or use Manual Override."
                ));
            }

            let missing_details = missing_details_from_json_row(row, path, 12);
            return Err(missing_details.runtime_message());
        }

        match row {
            JsonValue::Object(map) => {
                for (key, value) in map {
                    if let Ok((scalar, _)) = self.scalar_value_from_json(value, Some(key.clone()), key) {
                        return Ok((scalar, Some(key.clone())));
                    }
                }
                Err("provider row has no scalar values to select".to_string())
            }
            JsonValue::String(_) | JsonValue::Number(_) | JsonValue::Bool(_) | JsonValue::Null => {
                self.scalar_value_from_json(row, None, "row")
            }
            _ => Err("selected row is not scalar and no value_field was provided".to_string()),
        }
    }

    fn scalar_value_from_json(
        &self,
        value: &JsonValue,
        source_field: Option<String>,
        source_path: &str,
    ) -> Result<(JsonValue, Option<String>), String> {
        match value {
            JsonValue::String(text) => Ok((JsonValue::String(text.clone()), source_field)),
            JsonValue::Number(number) => Ok((JsonValue::Number(number.clone()), source_field)),
            JsonValue::Bool(boolean) => Ok((JsonValue::Bool(*boolean), source_field)),
            JsonValue::Null => Ok((JsonValue::Null, source_field)),
            JsonValue::Array(_) | JsonValue::Object(_) => Err(non_scalar_runtime_message(source_path)),
        }
    }

    fn resolve_display_scalar_value(&self, value: &JsonValue, path_prefix: &str, depth: usize) -> Option<(JsonValue, String)> {
        if depth >= Self::MAX_CELL_SCALAR_DEPTH {
            return None;
        }

        match value {
            JsonValue::String(_) | JsonValue::Number(_) | JsonValue::Bool(_) | JsonValue::Null => {
                Some((value.clone(), path_prefix.to_string()))
            }
            JsonValue::Object(map) => {
                let best_key = get_scored_keys_with_context(map, KeyScoreContext::ValueSelection).first()?.clone();
                let nested_value = map.get(&best_key)?;
                let next_path = format!("{path_prefix}.{best_key}");
                self.resolve_display_scalar_value(nested_value, &next_path, depth + 1)
            }
            JsonValue::Array(values) => values.iter().enumerate().find_map(|(index, entry)| match entry {
                JsonValue::Object(_) | JsonValue::Array(_) => None,
                _ => {
                    let next_path = format!("{path_prefix}[{index}]");
                    self.resolve_display_scalar_value(entry, &next_path, depth + 1)
                }
            }),
        }
    }

    fn current_selected_source_field(&self, collector: &CollectorViewState<'_>) -> Option<String> {
        collector.table.selected_column_key()
    }

    fn render_block(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) -> Rect {
        let Some(collector) = app.workflows.collector_state() else {
            return Rect::default();
        };
        let title = format!("Select one ({})", collector.provider_id);
        let block = th::block(&*app.ctx.theme, Some(title.as_str()), false)
            .merge_borders(MergeStrategy::Exact)
            .padding(Padding::uniform(1));
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        inner
    }

    fn render_selector(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let inner = self.render_block(frame, rect, app);
        let mut layout = WorkflowCollectorLayoutState::from(self.get_preferred_layout(app, inner));

        let theme = &*app.ctx.theme;
        let collector = app.workflows.collector_state_mut().expect("selector state");
        if collector.table.table_state.selected().is_none() && collector.table.has_rows() {
            collector.table.table_state.select(Some(0));
        }
        if collector.value_field.is_none() {
            collector.table.ensure_column_selected();
            collector.table.ensure_selected_column_visible(layout.table_area.width);
        }
        let selected_index = if collector.table.has_rows() {
            collector.table.table_state.selected()
        } else {
            collector.table.list_state.selected()
        };
        collector.sync_stage_with_selection(selected_index);

        layout.filter_inner_area = self.render_filter_panel(frame, layout.filter_panel, collector, theme);
        self.render_status_line(frame, layout.status_area, collector, theme);

        let table_focused = collector.f_table.get();
        let results_block = th::block(theme, Some("Results"), table_focused).merge_borders(MergeStrategy::Exact);
        let results_inner = results_block.inner(layout.table_area);
        frame.render_widget(results_block, layout.table_area);
        self.results_table_view
            .render_results(frame, results_inner, &mut collector.table, table_focused, theme);
        layout.table_area = results_inner;

        layout.manual_inner_area = self.render_manual_override_panel(frame, layout.manual_panel_area, collector, theme);

        let cancel_options = ButtonRenderOptions::new(true, collector.f_cancel.get(), false, Borders::ALL, ButtonType::Secondary);
        th::render_button(frame, layout.cancel_button_area, "Cancel", theme, cancel_options);

        let apply_options = ButtonRenderOptions::new(
            collector.apply_enabled(),
            collector.f_apply.get(),
            false,
            Borders::ALL,
            ButtonType::Primary,
        );
        th::render_button(frame, layout.apply_button_area, "Apply", theme, apply_options);

        self.layout = layout;
    }

    fn render_filter_panel(&self, frame: &mut Frame, area: Rect, collector: &CollectorViewState<'_>, theme: &dyn Theme) -> Rect {
        self.render_text_input_panel(
            frame,
            area,
            theme,
            TextInputPanelRenderSpec {
                title: "Filter Results",
                is_focused: collector.f_filter.get(),
                text: collector.filter.input(),
                placeholder: None,
                cursor_columns: collector.filter.cursor_columns(),
            },
        )
    }

    fn render_manual_override_panel(&self, frame: &mut Frame, area: Rect, collector: &CollectorViewState<'_>, theme: &dyn Theme) -> Rect {
        self.render_text_input_panel(
            frame,
            area,
            theme,
            TextInputPanelRenderSpec {
                title: "Manual Override (optional)",
                is_focused: collector.f_manual.get(),
                text: collector.manual_override.input(),
                placeholder: Some("Type any value. JSON literals are parsed automatically."),
                cursor_columns: collector.manual_override.cursor_columns(),
            },
        )
    }

    fn render_status_line(&self, frame: &mut Frame, area: Rect, selector: &CollectorViewState<'_>, theme: &dyn Theme) {
        frame.render_widget(Paragraph::new(self.build_status_line(selector, theme)), area);
    }

    fn build_status_line(&self, selector: &CollectorViewState<'_>, theme: &dyn Theme) -> Line<'static> {
        let mut spans = Vec::new();
        let (indicator, label, style) = match selector.status {
            SelectorStatus::Loading => ("⟳", "Loading…", theme.status_info()),
            SelectorStatus::Loaded => ("✓", "Ready", theme.status_success()),
            SelectorStatus::Error => ("✖", "Error", theme.status_error()),
        };

        spans.push(Span::styled("Status ", theme.text_secondary_style()));
        spans.push(Span::styled(format!("{indicator} {label}"), style));

        match selector.selection_source {
            CollectorSelectionSource::Table => {
                spans.push(Span::styled("  •  Apply: table", theme.text_secondary_style()));
                if let Some(staged) = selector.staged_selection() {
                    spans.push(Span::styled("  •  ", theme.text_secondary_style()));
                    spans.push(Span::styled(
                        staged.display_value.clone(),
                        style_for_role(classify_json_value(&staged.value), theme),
                    ));
                    if let Some(field) = &staged.source_field {
                        spans.push(Span::styled(format!(" ({field})"), theme.syntax_type_style()));
                    }
                }
            }
            CollectorSelectionSource::Manual => {
                spans.push(Span::styled("  •  Apply: manual", theme.text_secondary_style()));
                if let Some(value) = self.parse_manual_override_value(&selector.manual_override) {
                    let display_value = render_value("manual", &value, None).into_plain_text();
                    spans.push(Span::styled("  •  ", theme.text_secondary_style()));
                    spans.push(Span::styled(display_value, style_for_role(classify_json_value(&value), theme)));
                } else {
                    spans.push(Span::styled("  •  (empty)", theme.text_muted_style()));
                }
            }
        }

        if let Some(error) = &selector.error_message {
            spans.push(Span::raw("  •  "));
            spans.push(Span::styled(error.clone(), theme.status_error()));
        }

        Line::from(spans)
    }

    fn selector_hint_spans(theme: &dyn Theme, collector: &CollectorViewState<'_>) -> Vec<Span<'static>> {
        if collector.f_filter.get() {
            return build_hint_spans(
                theme,
                &[
                    ("Esc", " Clear filter  "),
                    ("Ctrl+U", " Clear line  "),
                    ("Tab", " Next focus  "),
                    ("Shift+Tab", " Previous focus"),
                ],
            );
        }

        if collector.f_table.get() {
            let mut hints = vec![
                ("Esc", " Cancel  "),
                ("↑/↓", " Row  "),
                ("Space", " Stage selection  "),
                ("Enter", " Apply/Stage  "),
                ("R", " Refresh  "),
                ("Tab", " Next focus  "),
                ("Shift+Tab", " Previous focus"),
            ];
            if collector.value_field.is_none() {
                hints.insert(2, ("←/→", " Cell  "));
            }
            return build_hint_spans(theme, &hints);
        }

        if collector.f_manual.get() {
            let mut hints = vec![
                ("Esc", " Cancel  "),
                ("Enter", " Apply  "),
                ("Ctrl+U", " Clear line  "),
                ("Tab", " Next focus  "),
                ("Shift+Tab", " Previous focus"),
            ];
            if matches!(collector.apply_target, CollectorApplyTarget::WorkflowInput) {
                hints.insert(2, ("Ctrl+O", " Select file  "));
            }
            return build_hint_spans(theme, &hints);
        }

        if collector.f_cancel.get() || collector.f_apply.get() {
            return build_hint_spans(
                theme,
                &[
                    ("Esc", " Cancel  "),
                    ("Enter", " Activate  "),
                    ("Tab", " Next focus  "),
                    ("Shift+Tab", " Previous focus"),
                ],
            );
        }

        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::components::common::TextInputState;
    use crate::ui::components::results::ResultsTableState;
    use serde_json::json;

    fn base_selector() -> CollectorViewState<'static> {
        CollectorViewState {
            provider_id: "apps list".into(),
            resolved_args: serde_json::Map::new(),
            table: ResultsTableState::default(),
            value_field: None,
            display_field: None,
            on_error: None,
            status: SelectorStatus::Loaded,
            error_message: None,
            original_items: None,
            pending_cache_key: None,
            filter: TextInputState::new(),
            staged_selection: None,
            ..Default::default()
        }
    }

    #[test]
    fn extract_selected_value_uses_leaf_fallback_when_value_field_path_misses() {
        let mut selector = base_selector();
        selector.value_field = Some("metadata.id".into());
        selector.table.apply_result_json(
            Some(json!([{ "id": "app-1", "name": "alpha" }])),
            &crate::ui::theme::dracula::DraculaTheme::new(),
            true,
        );
        selector.table.table_state.select(Some(0));

        let component = WorkflowCollectorComponent::default();
        let (value, source_field) = component.extract_selected_value(&selector).expect("leaf fallback should resolve");
        assert_eq!(value, json!("app-1"));
        assert_eq!(source_field.as_deref(), Some("id"));
    }

    #[test]
    fn extract_selected_value_reports_clear_message_when_value_field_missing() {
        let mut selector = base_selector();
        selector.value_field = Some("metadata.id".into());
        selector.table.apply_result_json(
            Some(json!([{ "project": "app-1", "name": "alpha" }])),
            &crate::ui::theme::dracula::DraculaTheme::new(),
            true,
        );
        selector.table.table_state.select(Some(0));

        let component = WorkflowCollectorComponent::default();
        let message = component
            .extract_selected_value(&selector)
            .expect_err("missing value field should fail");
        assert!(message.contains("value_field 'metadata.id' not found"));
        assert!(message.contains("Update select.value_field"));
    }

    #[test]
    fn extract_selected_value_uses_unique_nested_leaf_candidate() {
        let mut selector = base_selector();
        selector.value_field = Some("id".into());
        selector.table.apply_result_json(
            Some(json!([{ "owner": { "id": "owner-1" }, "name": "alpha" }])),
            &crate::ui::theme::dracula::DraculaTheme::new(),
            true,
        );
        selector.table.table_state.select(Some(0));

        let component = WorkflowCollectorComponent::default();
        let (value, source_field) = component.extract_selected_value(&selector).expect("nested fallback should resolve");
        assert_eq!(value, json!("owner-1"));
        assert_eq!(source_field.as_deref(), Some("owner.id"));
    }

    #[test]
    fn extract_selected_value_reports_ambiguous_nested_leaf_candidates() {
        let mut selector = base_selector();
        selector.value_field = Some("id".into());
        selector.table.apply_result_json(
            Some(json!([{ "owner": { "id": "owner-1" }, "team": { "id": "team-1" } }])),
            &crate::ui::theme::dracula::DraculaTheme::new(),
            true,
        );
        selector.table.table_state.select(Some(0));

        let component = WorkflowCollectorComponent::default();
        let message = component
            .extract_selected_value(&selector)
            .expect_err("ambiguous nested id should error");
        assert!(message.contains("multiple nested 'id' candidates"));
        assert!(message.contains("owner.id"));
        assert!(message.contains("team.id"));
    }

    #[test]
    fn extract_selected_value_prefers_explicit_cell_selection_over_value_field_path() {
        let mut selector = base_selector();
        selector.value_field = Some("service".into());
        selector.table.apply_result_json(
            Some(json!([{ "id": "srv-1", "service": { "id": "srv-1", "name": "api" } }])),
            &crate::ui::theme::dracula::DraculaTheme::new(),
            true,
        );
        selector.table.table_state.select(Some(0));
        let id_column_index = (0..selector.table.column_count())
            .find(|index| {
                selector.table.table_state.select_column(Some(*index));
                selector.table.selected_column_key().as_deref() == Some("id")
            })
            .expect("id column present");
        selector.table.table_state.select_column(Some(id_column_index));

        let component = WorkflowCollectorComponent::default();
        let (value, source_field) = component.extract_selected_value(&selector).expect("cell selection should win");
        assert_eq!(value, json!("srv-1"));
        assert_eq!(source_field.as_deref(), Some("id"));
    }

    #[test]
    fn extract_selected_value_uses_prioritized_scalar_for_selected_object_cell() {
        let mut selector = base_selector();
        selector.table.apply_result_json(
            Some(json!([{ "service": { "id": "srv-1", "name": "api-service" } }])),
            &crate::ui::theme::dracula::DraculaTheme::new(),
            true,
        );
        selector.table.table_state.select(Some(0));

        let service_column_index = (0..selector.table.column_count())
            .find(|index| {
                selector.table.table_state.select_column(Some(*index));
                selector.table.selected_column_key().as_deref() == Some("service")
            })
            .expect("service column present");
        selector.table.table_state.select_column(Some(service_column_index));

        let component = WorkflowCollectorComponent::default();
        let (value, source_field) = component
            .extract_selected_value(&selector)
            .expect("selected object cell should resolve to displayed scalar");
        assert_eq!(value, json!("srv-1"));
        assert_eq!(source_field.as_deref(), Some("service.id"));
    }

    #[test]
    fn parse_manual_override_value_parses_json_literal_then_fallback_string() {
        let component = WorkflowCollectorComponent::default();
        let mut input = TextInputState::new();

        input.set_input("true");
        assert_eq!(component.parse_manual_override_value(&input), Some(JsonValue::Bool(true)));

        input.set_input("owner-1");
        assert_eq!(
            component.parse_manual_override_value(&input),
            Some(JsonValue::String("owner-1".into()))
        );
    }
}
