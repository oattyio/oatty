use std::cmp::min;

use crate::app::App;
use crate::ui::components::common::ResultsTableView;
use crate::ui::components::component::Component;
use crate::ui::components::results::state::KeyValueEntry;
use crate::ui::components::workflows::collector::manual_entry::ManualEntryComponent;
use crate::ui::components::workflows::collector::{CollectorStagedSelection, CollectorViewState, SelectorStatus};
use crate::ui::components::workflows::view_utils::{classify_json_value, style_for_role};
use crate::ui::theme::Theme;
use crate::ui::theme::theme_helpers::{self as th, ButtonRenderOptions, ButtonType, build_hint_spans};
use crate::ui::utils::render_value;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use oatty_engine::resolve::select_path;
use oatty_types::{Effect, ExecOutcome, Msg, WorkflowProviderErrorPolicy};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Wrap};
use serde_json::{Value as JsonValue, Value};

/// Retained layout metadata capturing screen regions for pointer hit-testing.
#[derive(Debug, Clone, Default)]
struct WorkflowCollectorLayoutState {
    /// Rect covering the filter input at the top of the modal.
    pub filter_panel: Rect,
    /// Rect covering the inner text area for the filter input.
    pub filter_inner_area: Rect,
    /// Rect covering the filter input at the top of the modal.
    pub status_area: Rect,
    /// Rect covering the result results.
    pub table_area: Rect,
    /// Rect covering the detail pane.
    pub detail_area: Rect,
    /// Rect covering the metadata pane.
    pub metadata_area: Rect,
    /// Rect for the Cancel button inside the footer.
    pub cancel_button_area: Rect,
    /// Rect for the Apply button inside the footer.
    pub apply_button_area: Rect,
}

impl From<Vec<Rect>> for WorkflowCollectorLayoutState {
    fn from(value: Vec<Rect>) -> Self {
        Self {
            filter_panel: value[0],
            filter_inner_area: Rect::default(),
            status_area: value[1],

            table_area: value[2],
            detail_area: value[3],
            metadata_area: value[4],

            cancel_button_area: value[5],
            apply_button_area: value[6],
        }
    }
}
/// Component that orchestrates workflow input collection modals (manual entry and selector).
///
/// The collector routes events to the appropriate modal based on the active state inside
/// `WorkflowState` and renders either the provider-backed selector or the manual entry dialog.
#[derive(Debug, Default)]
pub struct WorkflowCollectorComponent {
    manual_entry: ManualEntryComponent,
    results_table_view: ResultsTableView,
    detail_table_view: ResultsTableView,
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

        // Manual entry mode takes precedence when present
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

        // Provider-backed selector handling when present
        if app.workflows.collector_state_mut().is_some() {
            return self.handle_selector_key_events(app, key);
        }

        // Fallback: allow closing if neither manual nor selector is present
        if key.code == KeyCode::Esc {
            effects.push(Effect::CloseModal)
        }
        effects
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        if app.workflows.manual_entry_state().is_some() {
            return self.manual_entry.handle_mouse_events(app, mouse);
        }
        let position = Position::new(mouse.column, mouse.row);
        // Scroll highlighting
        let table = &mut app.workflows.collector_state_mut().expect("results state").table;
        let offset = table.table_state.offset();

        match mouse.kind {
            MouseEventKind::Moved | MouseEventKind::Up(MouseButton::Left) => {
                table.mouse_over_idx = if self.layout.table_area.contains(position) {
                    Some(self.hit_test_results_table(position, offset))
                } else {
                    None
                };
            }
            MouseEventKind::ScrollDown => {
                if self.layout.table_area.contains(position) {
                    table.table_state.scroll_down_by(1);
                }
            }
            MouseEventKind::ScrollUp => {
                if self.layout.table_area.contains(position) {
                    table.table_state.scroll_up_by(1);
                }
            }
            _ => {}
        }

        // Mouse clicks
        if app.workflows.collector_state().is_none() || mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return Vec::new();
        }
        let mut effects = Vec::new();
        let collector = app.workflows.collector_state_mut().unwrap();

        if self.layout.cancel_button_area.contains(position) {
            app.focus.focus(&collector.f_cancel);
            collector.clear_staged_selection();
            effects.push(Effect::CloseModal);
        }
        let mut apply_selection = false;
        if self.layout.apply_button_area.contains(position) {
            app.focus.focus(&collector.f_apply);
            apply_selection = true;
        }

        if self.layout.filter_panel.contains(position) {
            app.focus.focus(&collector.f_filter);
            collector.focus_filter();
            let relative_column = position.x.saturating_sub(self.layout.filter_inner_area.x);
            let cursor_index = collector.filter.cursor_index_for_column(relative_column);
            collector.filter.set_cursor(cursor_index);
        }

        if self.layout.table_area.contains(position) {
            app.focus.focus(&collector.f_table);
            let index = self.hit_test_results_table(position, offset);
            collector.table.table_state.select(Some(index));
            collector.sync_stage_with_selection(Some(index));
            let _ = self.stage_current_row(collector);
        }

        if apply_selection {
            effects.extend(self.apply_selection_to_run_state(app));
        }

        effects
    }

    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        // If a manual entry state exists, render Manual Entry View; else render a selector
        if app.workflows.manual_entry_state().is_some() {
            self.manual_entry.render(frame, rect, app);
            return;
        }

        if app.workflows.collector_state_mut().is_some() {
            self.render_selector(frame, rect, app);
            return;
        }

        // Default placeholder when no state present
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
        let Some(collector) = app.workflows.collector_state() else {
            return Vec::new();
        };
        let detail_required = collector.value_field.is_none();
        let layout_spec = if detail_required {
            vec![
                Constraint::Length(4), // header
                Constraint::Min(6),    // results
                Constraint::Min(5),    // detail
                Constraint::Length(3), // footer
            ]
        } else {
            vec![
                Constraint::Length(4), // header
                Constraint::Min(6),    // results
                Constraint::Length(0), // empty
                Constraint::Length(3), // footer
            ]
        };
        let layout = Layout::vertical(layout_spec).split(area);

        let header_area = layout[0];
        let table_area = layout[1];
        let detail_area = layout[2];
        let footer_area = layout[3];

        let header_layout = Layout::vertical([Constraint::Length(3), Constraint::Length(1)]).split(header_area);

        let detail_layout = Layout::vertical([Constraint::Min(3), Constraint::Length(2)]).split(detail_area);

        let footer_layout = Layout::horizontal([Constraint::Length(24), Constraint::Min(0)]).split(footer_area);

        let button_layout = Layout::horizontal([Constraint::Length(12), Constraint::Length(12)]).split(footer_layout[0]);

        vec![
            header_layout[0], // filter panel
            header_layout[1], // status
            table_area,       // results
            detail_layout[0], // details - empty when !detail_required
            detail_layout[1], // metadata - empty when !detail_required
            button_layout[0], // cancel button
            button_layout[1], // apply button
        ]
    }

    fn on_route_exit(&mut self, app: &mut App) -> Vec<Effect> {
        app.workflows.end_inputs_session();
        Vec::new()
    }
}

impl WorkflowCollectorComponent {
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

    fn hit_test_results_table(&self, mouse_position: Position, offset: usize) -> usize {
        mouse_position.y.saturating_sub(1 + self.layout.table_area.y) as usize + offset
    }
    fn handle_filter_keys(&self, app: &mut App, key: KeyEvent) {
        let Some(collector) = app.workflows.collector_state_mut() else {
            return;
        };
        let theme = &*app.ctx.theme;
        match key.code {
            KeyCode::Left => collector.filter.move_left(),
            KeyCode::Right => collector.filter.move_right(),
            KeyCode::Home => collector.filter.set_cursor(0),
            KeyCode::End => collector.filter.set_cursor(collector.filter.input().len()),
            KeyCode::Backspace => {
                collector.filter.backspace();
                self.apply_filter(collector, theme);
            }
            KeyCode::Char(character) if (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT) && !character.is_control() => {
                collector.filter.insert_char(character);
                self.apply_filter(collector, theme);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                collector.filter.set_input("");
                collector.filter.set_cursor(0);
                self.apply_filter(collector, theme);
            }
            _ => {}
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
        let row_len = collector.table.num_rows();
        let selected = collector.table.table_state.selected().unwrap_or_default();
        let table_state = &mut collector.table.table_state;

        match key.code {
            KeyCode::F(2) => {
                app.workflows.open_manual_for_active_input();
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
            .map(|c| (c.f_cancel.get(), c.f_apply.get()))
            .unwrap_or_default();
        if app.workflows.collector.is_none() {
            return Vec::new();
        }

        let mut effects = Vec::new();
        if key.code != KeyCode::Enter {
            return effects;
        }
        if f_cancel {
            effects.push(Effect::CloseModal);
        }
        if f_apply {
            self.apply_selection_to_run_state(app);
        }
        effects
    }

    fn stage_current_row(&self, collector: &mut CollectorViewState<'_>) -> Result<(), String> {
        let (value, source_field) = self
            .extract_selected_value(collector)
            .ok_or_else(|| "value must be a scalar or value_field missing".to_string())?;
        let idx = collector
            .table
            .table_state
            .selected()
            .ok_or_else(|| "no row selected".to_string())?;
        let row = collector
            .table
            .selected_data(idx)
            .cloned()
            .ok_or_else(|| "no provider row selected".to_string())?;
        let display_key = source_field.clone().unwrap_or_else(|| "value".to_string());
        let display_value = render_value(&display_key, &value, None).into_plain_text();
        collector.set_staged_selection(Some(CollectorStagedSelection::new(value, display_value, source_field, row)));
        collector.error_message = None;
        Ok(())
    }

    fn current_row_is_staged(&self, collector: &CollectorViewState<'_>) -> bool {
        let Some(idx) = collector.table.table_state.selected() else {
            return false;
        };
        if let Some((staged, row)) = collector.staged_selection().zip(collector.table.selected_data(idx)) {
            staged.row == *row
        } else {
            false
        }
    }

    fn apply_selection_to_run_state(&self, app: &mut App) -> Vec<Effect> {
        let Some(collector) = app.workflows.collector_state_mut() else {
            return Vec::new();
        };
        let Some(selection) = collector.take_staged_selection() else {
            collector.error_message = Some("Select a value before applying".into());
            return Vec::new();
        };
        collector.error_message = None;
        let mut effects = Vec::new();
        if let Some(name) = app.workflows.active_input_name() {
            if let Some(run_rc) = app.workflows.active_run_state.clone() {
                let mut run = run_rc.borrow_mut();
                run.run_context.inputs.insert(name, selection.value);
                let _ = run.evaluate_input_providers();
            }
            effects.push(Effect::CloseModal);
        }
        effects
    }

    //-------------------------------------
    // Selector widget handlers
    fn handle_selector_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let mut effects = Vec::new();
        let (f_filter, f_table, f_cancel, f_apply) = app
            .workflows
            .collector
            .as_ref()
            .map(|c| (c.f_filter.get(), c.f_table.get(), c.f_cancel.get(), c.f_apply.get()))
            .unwrap_or_default();
        if f_filter {
            self.handle_filter_keys(app, key);
        }
        if f_table {
            effects.append(&mut self.handle_table_keys(app, key));
        }
        if f_cancel || f_apply {
            let mut new_effects = self.handle_button_keys(app, key);
            effects.append(&mut new_effects);
        }
        let Some(collector) = app.workflows.collector_state_mut() else {
            return effects;
        };
        if key.code == KeyCode::Esc {
            if collector.f_filter.get() && !collector.filter.is_empty() {
                collector.filter.set_input("");
                collector.filter.set_cursor(0);
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

    fn extract_selected_value(&self, collector: &CollectorViewState<'_>) -> Option<(JsonValue, Option<String>)> {
        let idx = collector.table.table_state.selected()?;
        let row = collector.table.selected_data(idx)?;
        if let Some(path) = collector.value_field.as_deref() {
            let value = select_path(row, Some(path))?;
            let field_name = path.split('.').next_back().map(|segment| segment.to_string());
            return match value {
                JsonValue::String(s) => Some((JsonValue::String(s.clone()), field_name)),
                JsonValue::Number(n) => Some((JsonValue::Number(n.clone()), field_name)),
                JsonValue::Bool(b) => Some((JsonValue::Bool(b), field_name)),
                JsonValue::Null => Some((JsonValue::Null, field_name)),
                _ => None,
            };
        }
        match row {
            JsonValue::Object(map) => {
                for (key, value) in map {
                    match value {
                        JsonValue::String(s) => return Some((JsonValue::String(s.clone()), Some(key.clone()))),
                        JsonValue::Number(n) => return Some((JsonValue::Number(n.clone()), Some(key.clone()))),
                        JsonValue::Bool(b) => return Some((JsonValue::Bool(*b), Some(key.clone()))),
                        JsonValue::Null => return Some((JsonValue::Null, Some(key.clone()))),
                        _ => continue,
                    }
                }
                None
            }
            JsonValue::String(s) => Some((JsonValue::String(s.clone()), None)),
            JsonValue::Number(n) => Some((JsonValue::Number(n.clone()), None)),
            JsonValue::Bool(b) => Some((JsonValue::Bool(*b), None)),
            JsonValue::Null => Some((JsonValue::Null, None)),
            _ => None,
        }
    }

    fn render_block(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) -> Rect {
        let Some(collector) = app.workflows.collector_state() else {
            return Rect::default();
        };
        let title = format!("Select one ({})", collector.provider_id);
        let block = th::block(&*app.ctx.theme, Some(title.as_str()), false).padding(Padding::uniform(1));
        let inner = block.inner(rect);
        frame.render_widget(block, rect);

        inner
    }

    fn render_selector(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let inner = self.render_block(frame, rect, app);
        let layout = WorkflowCollectorLayoutState::from(self.get_preferred_layout(app, inner));

        let theme = &*app.ctx.theme;
        let collector = app.workflows.collector_state_mut().expect("selector state");
        let selected_idx = collector.table.table_state.selected();
        if selected_idx.is_none() && collector.table.has_rows() {
            collector.table.table_state.select(Some(0));
        }
        collector.sync_stage_with_selection(selected_idx);

        let filter_inner_area = self.render_filter_panel(frame, layout.filter_panel, collector, theme);
        self.render_status_line(frame, layout.status_area, collector, theme);

        let table_focused = collector.f_table.get();

        self.results_table_view
            .render_results(frame, layout.table_area, &mut collector.table, table_focused, theme);
        // Manual selection of a json field value when one is not specified
        if collector.value_field.is_none() {
            let idx = collector.table.table_state.selected().unwrap_or(0);
            let row_json = collector.table.selected_data(idx).cloned().unwrap_or(Value::Null);
            let entries = collector.table.kv_entries();
            let (_detail_selection, _detail_offset) = self.detail_selection(entries, collector);
            let detail_block = th::block(theme, Some("Details"), table_focused);
            let detail_inner = detail_block.inner(layout.detail_area);
            frame.render_widget(detail_block, layout.detail_area);
            self.detail_table_view
                .render_key_value_or_text(frame, detail_inner, &mut collector.table, &row_json, theme);
            self.render_detail_metadata(frame, layout.metadata_area, collector, theme);
        }
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

        let mut layout = layout;
        layout.filter_inner_area = filter_inner_area;
        self.layout = layout;
    }

    fn render_filter_panel(&self, frame: &mut Frame, area: Rect, collector: &CollectorViewState<'_>, theme: &dyn Theme) -> Rect {
        let filter_block_title = Line::from(Span::styled(
            "Filter Results",
            theme.text_secondary_style().add_modifier(Modifier::BOLD),
        ));
        let is_focused = collector.f_filter.get();
        let mut block = th::block::<String>(theme, None, is_focused);
        block = block.title(filter_block_title);
        let inner_area = block.inner(area);
        let filter_text = collector.filter.input();

        let content_line = if is_focused || !filter_text.is_empty() {
            Line::from(Span::styled(filter_text.to_string(), theme.text_primary_style()))
        } else {
            Line::from(Span::from(""))
        };

        let paragraph = Paragraph::new(content_line).block(block).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);

        if is_focused {
            let cursor_columns = collector.filter.cursor_columns() as u16;
            let cursor_x = inner_area.x.saturating_add(cursor_columns);
            let cursor_y = inner_area.y;
            frame.set_cursor_position((cursor_x, cursor_y));
        }
        inner_area
    }

    fn render_status_line(&self, frame: &mut Frame, area: Rect, selector: &CollectorViewState<'_>, theme: &dyn Theme) {
        let status_line = self.build_status_line(selector, theme);
        frame.render_widget(Paragraph::new(status_line), area);
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

        if let Some(error) = &selector.error_message {
            spans.push(Span::raw("  •  "));
            spans.push(Span::styled(error.clone(), theme.status_error()));
        } else if let Some(staged) = selector.staged_selection() {
            spans.push(Span::styled("  •  Selected: ", theme.text_secondary_style()));
            spans.push(Span::styled("✓ ", theme.status_success()));
            spans.push(Span::styled(
                staged.display_value.clone(),
                style_for_role(classify_json_value(&staged.value), theme),
            ));
            if let Some(field) = &staged.source_field {
                spans.push(Span::styled(format!(" ({field})"), theme.syntax_type_style()));
            }
        }

        Line::from(spans)
    }

    fn detail_selection(&self, entries: &[KeyValueEntry], selector: &CollectorViewState<'_>) -> (Option<usize>, usize) {
        if entries.is_empty() {
            return (None, 0);
        }

        if let Some(staged) = selector.staged_selection()
            && let Some(field) = &staged.source_field
            && let Some(index) = entries.iter().position(|entry| entry.key == *field)
        {
            let offset = min(index, entries.len().saturating_sub(1));
            return (Some(index), offset);
        }

        if let Some(field) = selector.value_field.as_deref() {
            let leaf = field.split('.').next_back().unwrap_or(field);
            if let Some(index) = entries.iter().position(|entry| entry.key == leaf) {
                let offset = min(index, entries.len().saturating_sub(1));
                return (Some(index), offset);
            }
        }

        if let Some(field) = selector.display_field.as_deref()
            && let Some(index) = entries.iter().position(|entry| entry.key == field)
        {
            let offset = min(index, entries.len().saturating_sub(1));
            return (Some(index), offset);
        }

        (None, 0)
    }

    fn render_detail_metadata(&self, frame: &mut Frame, area: Rect, selector: &CollectorViewState<'_>, theme: &dyn Theme) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let mut lines: Vec<Line<'static>> = Vec::new();
        if let Some(field) = self.active_field_key(selector)
            && let Some(metadata) = selector.field_metadata.get(&field)
        {
            let type_label = metadata.json_type.clone().unwrap_or_else(|| "unknown".to_string());
            let mut type_spans = vec![
                Span::styled("Type: ", theme.text_secondary_style()),
                Span::styled(type_label, theme.syntax_type_style()),
            ];
            if metadata.required {
                type_spans.push(Span::styled(" • required", theme.syntax_keyword_style()));
            }
            lines.push(Line::from(type_spans));

            if !metadata.tags.is_empty() {
                let mut tag_spans = Vec::with_capacity(metadata.tags.len() * 2 + 1);
                tag_spans.push(Span::styled("Tags: ", theme.text_secondary_style()));
                for (index, tag) in metadata.tags.iter().enumerate() {
                    if index > 0 {
                        tag_spans.push(Span::styled(" ", theme.text_secondary_style()));
                    }
                    tag_spans.push(Span::styled(format!("#{tag}"), theme.syntax_keyword_style()));
                }
                lines.push(Line::from(tag_spans));
            }

            if !metadata.enum_values.is_empty() {
                let preview_count = min(metadata.enum_values.len(), 5);
                let mut enum_spans = Vec::with_capacity(preview_count * 2 + 2);
                enum_spans.push(Span::styled("Enums: ", theme.text_secondary_style()));
                for (index, value) in metadata.enum_values.iter().take(preview_count).enumerate() {
                    if index > 0 {
                        enum_spans.push(Span::styled(", ", theme.text_secondary_style()));
                    }
                    enum_spans.push(Span::styled(value.clone(), theme.syntax_string_style()));
                }
                if metadata.enum_values.len() > preview_count {
                    enum_spans.push(Span::styled(" …", theme.text_secondary_style()));
                }
                lines.push(Line::from(enum_spans));
            }
        }

        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "Schema metadata unavailable for this selection",
                theme.text_muted_style(),
            )));
        }

        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
    }

    fn active_field_key(&self, selector: &CollectorViewState<'_>) -> Option<String> {
        if let Some(staged) = selector.staged_selection()
            && let Some(field) = &staged.source_field
        {
            return Some(field.clone());
        }
        if let Some(field) = selector.value_field.as_ref() {
            return Some(field.split('.').next_back().unwrap_or(field).to_string());
        }
        if let Some(field) = selector.display_field.as_ref() {
            return Some(field.clone());
        }
        None
    }

    fn selector_hint_spans(theme: &dyn Theme, collector: &CollectorViewState<'_>) -> Vec<Span<'static>> {
        if collector.f_filter.get() {
            return build_hint_spans(
                theme,
                &[
                    ("Esc", " Clear filter  "),
                    ("Enter", " Focus results  "),
                    ("Tab", " Next focus  "),
                    ("Shift+Tab", " Previous focus"),
                ],
            );
        }

        if collector.f_table.get() {
            return build_hint_spans(
                theme,
                &[
                    ("Esc", " Cancel  "),
                    ("↑/↓", " Move  "),
                    ("Space", " Stage selection  "),
                    ("Enter", " Apply/Stage  "),
                    ("R", " Refresh  "),
                    ("F2", " Manual entry  "),
                    ("Tab", " Next focus  "),
                    ("Shift+Tab", " Previous focus"),
                ],
            );
        }

        if collector.f_cancel.get() || collector.f_apply.get() {
            return build_hint_spans(
                theme,
                &[
                    ("Esc", " Cancel  "),
                    ("←/→", " Switch button  "),
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
    use indexmap::IndexMap;
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
            field_metadata: IndexMap::new(),
            staged_selection: None,
            ..Default::default()
        }
    }

    #[test]
    fn detail_selection_prefers_staged_field() {
        let mut selector = base_selector();
        selector.set_staged_selection(Some(CollectorStagedSelection::new(
            JsonValue::String("app-1".into()),
            "app-1".into(),
            Some("name".into()),
            json!({"name": "app-1"}),
        )));

        let entries = vec![
            KeyValueEntry {
                key: "name".into(),
                display_key: "Name".into(),
                display_value: "app-1".into(),
                raw_value: json!("app-1"),
            },
            KeyValueEntry {
                key: "id".into(),
                display_key: "Id".into(),
                display_value: "1".into(),
                raw_value: json!("1"),
            },
        ];

        let component = WorkflowCollectorComponent::default();
        let (selection, offset) = component.detail_selection(&entries, &selector);
        assert_eq!(selection, Some(0));
        assert_eq!(offset, 0);
    }

    #[test]
    fn detail_selection_uses_value_field_leaf_when_unstaged() {
        let mut selector = base_selector();
        selector.value_field = Some("metadata.name".into());
        let entries = vec![
            KeyValueEntry {
                key: "name".into(),
                display_key: "Name".into(),
                display_value: "app-1".into(),
                raw_value: json!("app-1"),
            },
            KeyValueEntry {
                key: "id".into(),
                display_key: "Id".into(),
                display_value: "1".into(),
                raw_value: json!("1"),
            },
        ];

        let component = WorkflowCollectorComponent::default();
        let (selection, offset) = component.detail_selection(&entries, &selector);
        assert_eq!(selection, Some(0));
        assert_eq!(offset, 0);
    }
}
