use std::cmp::min;

use crate::app::App;
use crate::ui::components::common::ResultsTableView;
use crate::ui::components::component::{Component, find_target_index_by_mouse_position};
use crate::ui::components::table::state::KeyValueEntry;
use crate::ui::components::workflows::collector::manual_entry::ManualEntryComponent;
use crate::ui::components::workflows::collector::{
    SelectorButtonFocus, SelectorStatus, WorkflowCollectorFocus, WorkflowSelectorLayoutState, WorkflowSelectorStagedSelection,
    WorkflowSelectorViewState,
};
use crate::ui::components::workflows::view_utils::{classify_json_value, style_for_role};
use crate::ui::theme::Theme;
use crate::ui::theme::theme_helpers::{self as th, ButtonRenderOptions, build_hint_spans};
use crate::ui::utils::render_value;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use heroku_engine::provider::ProviderRegistry;
use heroku_engine::{ProviderValueResolver, resolve::select_path};
use heroku_types::{Effect, WorkflowProviderErrorPolicy};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Wrap};
use serde_json::{Value as JsonValue, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CollectorMouseTarget {
    ApplyButton,
    CancelButton,
    FilterInput,
    TableBody,
}
/// Component that orchestrates workflow input collection modals (manual entry and selector).
///
/// The collector routes events to the appropriate modal based on the active state inside
/// `WorkflowState` and renders either the provider-backed selector or the manual entry dialog.
#[derive(Debug, Default)]
pub struct WorkflowCollectorComponent {
    manual_entry: ManualEntryComponent,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum SelectorAction {
    #[default]
    None,
    Apply(WorkflowSelectorStagedSelection),
    Refresh {
        provider_id: String,
        resolved_args: serde_json::Map<String, Value>,
    },
    OpenManualEntry,
}

impl Component for WorkflowCollectorComponent {
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let mut effects = Vec::new();

        // Manual entry mode takes precedence when present
        if app.workflows.manual_entry_state().is_some() {
            return self.manual_entry.handle_key_events(app, key);
        }

        // Provider-backed selector handling when present
        if app.workflows.selector_state_mut().is_some() {
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

        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return Vec::new();
        }

        let mut effects = Vec::new();
        let mut action = SelectorAction::None;

        {
            let Some(selector) = app.workflows.selector_state_mut() else {
                return effects;
            };

            let layout = &selector.layout;
            let container = layout.container_area.unwrap_or(Rect::default());
            let mut areas: Vec<Rect> = Vec::new();
            let mut roles: Vec<CollectorMouseTarget> = Vec::new();

            if let Some(area) = layout.apply_button_area {
                areas.push(area);
                roles.push(CollectorMouseTarget::ApplyButton);
            }

            if let Some(area) = layout.cancel_button_area {
                areas.push(area);
                roles.push(CollectorMouseTarget::CancelButton);
            }

            if let Some(area) = layout.filter_area {
                areas.push(area);
                roles.push(CollectorMouseTarget::FilterInput);
            }

            if let Some(area) = layout.table_area {
                areas.push(area);
                roles.push(CollectorMouseTarget::TableBody);
            }

            let Some(index) = find_target_index_by_mouse_position(&container, &areas, mouse.column, mouse.row) else {
                return Vec::new();
            };

            match roles[index] {
                CollectorMouseTarget::ApplyButton => {
                    selector.focus_buttons(SelectorButtonFocus::Apply);
                    if let Some(selection) = selector.take_staged_selection() {
                        selector.error_message = None;
                        action = SelectorAction::Apply(selection);
                    } else {
                        selector.error_message = Some("Select a value before applying".into());
                    }
                }
                CollectorMouseTarget::CancelButton => {
                    selector.focus_buttons(SelectorButtonFocus::Cancel);
                    selector.clear_staged_selection();
                    effects.push(Effect::CloseModal);
                }
                CollectorMouseTarget::FilterInput => {
                    selector.focus_filter();
                    return Vec::new();
                }
                CollectorMouseTarget::TableBody => {
                    if let Some(area) = layout.table_area
                        && let Some(index) = self.row_index_from_position(selector, area, mouse.row)
                    {
                        selector.table.set_selection(index);
                        selector.sync_stage_with_selection();
                        let _ = self.stage_current_row(selector);
                        selector.focus_table();
                    }
                    return Vec::new();
                }
            }
        }

        match action {
            SelectorAction::Refresh {
                provider_id,
                resolved_args,
            } => {
                if let Some(selector) = app.workflows.selector_state_mut() {
                    effects.extend(self.refresh_selector_items(
                        selector,
                        &*app.ctx.theme,
                        &app.ctx.provider_registry,
                        provider_id,
                        resolved_args,
                    ));
                }
            }
            SelectorAction::Apply(selection) => {
                effects.extend(self.apply_selection_to_run_state(app, selection));
            }
            SelectorAction::OpenManualEntry => {
                app.workflows.open_manual_for_active_input();
            }
            SelectorAction::None => {}
        }

        effects
    }

    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        // If manual entry state exists, render Manual Entry View; else render a selector
        if app.workflows.manual_entry_state().is_some() {
            self.manual_entry.render(frame, rect, app);
            return;
        }

        if app.workflows.selector_state_mut().is_some() {
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
            return self.manual_entry.hint_spans(app);
        }
        if let Some(selector) = app.workflows.selector_state() {
            return Self::selector_hint_spans(theme, selector);
        }
        Vec::new()
    }

    fn on_route_exit(&mut self, app: &mut App) -> Vec<Effect> {
        app.workflows.end_inputs_session();
        Vec::new()
    }
}

impl WorkflowCollectorComponent {
    fn handle_filter_keys(&self, selector: &mut WorkflowSelectorViewState<'_>, key: KeyEvent, theme: &dyn Theme) {
        match key.code {
            KeyCode::Enter => {
                selector.focus_table();
            }
            KeyCode::Left => selector.filter.move_left(),
            KeyCode::Right => selector.filter.move_right(),
            KeyCode::Home => selector.filter.set_cursor(0),
            KeyCode::End => selector.filter.set_cursor(selector.filter.input().len()),
            KeyCode::Backspace => {
                selector.filter.backspace();
                self.apply_filter(selector, theme);
            }
            KeyCode::Char(character) if (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT) && !character.is_control() => {
                selector.filter.insert_char(character);
                self.apply_filter(selector, theme);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                selector.filter.set_input("");
                selector.filter.set_cursor(0);
                self.apply_filter(selector, theme);
            }
            _ => {}
        }
    }

    fn handle_table_keys(&self, selector: &mut WorkflowSelectorViewState<'_>, key: KeyEvent) -> (Vec<Effect>, SelectorAction) {
        let mut action = SelectorAction::None;
        match key.code {
            KeyCode::Char('/') => {
                selector.focus_filter();
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                selector.status = SelectorStatus::Loading;
                action = SelectorAction::Refresh {
                    provider_id: selector.provider_id.clone(),
                    resolved_args: selector.resolved_args.clone(),
                };
            }
            KeyCode::F(2) => {
                action = SelectorAction::OpenManualEntry;
            }
            KeyCode::Up => {
                selector.table.reduce_scroll(-1);
                selector.sync_stage_with_selection();
            }
            KeyCode::Down => {
                selector.table.reduce_scroll(1);
                selector.sync_stage_with_selection();
            }
            KeyCode::Home => {
                selector.table.reduce_home();
                selector.sync_stage_with_selection();
            }
            KeyCode::End => {
                selector.table.reduce_end();
                selector.sync_stage_with_selection();
            }
            KeyCode::Enter => {
                if matches!(selector.status, SelectorStatus::Error) && matches!(selector.on_error, Some(WorkflowProviderErrorPolicy::Fail))
                {
                    selector.error_message = Some("provider error: cannot apply (policy: fail)".into());
                } else if self.current_row_is_staged(selector) {
                    if let Some(selection) = selector.take_staged_selection() {
                        selector.error_message = None;
                        action = SelectorAction::Apply(selection);
                    } else {
                        selector.error_message = Some("Select a value before applying".into());
                    }
                } else if let Err(message) = self.stage_current_row(selector) {
                    selector.error_message = Some(message);
                }
            }
            KeyCode::Char(' ') => {
                if let Err(message) = self.stage_current_row(selector) {
                    selector.error_message = Some(message);
                }
            }
            _ => {}
        }
        (Vec::new(), action)
    }

    fn handle_button_keys(&self, selector: &mut WorkflowSelectorViewState<'_>, key: KeyEvent) -> (Vec<Effect>, SelectorAction) {
        let mut effects = Vec::new();
        let mut action = SelectorAction::None;
        match key.code {
            KeyCode::Left => selector.focus_buttons(SelectorButtonFocus::Cancel),
            KeyCode::Right => selector.focus_buttons(SelectorButtonFocus::Apply),
            KeyCode::Enter | KeyCode::Char(' ') => match selector.button_focus() {
                SelectorButtonFocus::Cancel => {
                    selector.clear_staged_selection();
                    effects.push(Effect::CloseModal);
                }
                SelectorButtonFocus::Apply => {
                    if let Some(selection) = selector.take_staged_selection() {
                        selector.error_message = None;
                        action = SelectorAction::Apply(selection);
                    } else {
                        selector.error_message = Some("Select a value before applying".into());
                    }
                }
            },
            _ => {}
        }
        (effects, action)
    }

    fn refresh_selector_items(
        &self,
        selector: &mut WorkflowSelectorViewState<'_>,
        theme: &dyn Theme,
        provider_registry: &ProviderRegistry,
        provider_id: String,
        resolved_args: serde_json::Map<String, serde_json::Value>,
    ) -> Vec<Effect> {
        match provider_registry.fetch_values(&provider_id, &resolved_args) {
            Ok(items_vec) => {
                let json_value = Value::Array(items_vec.clone());
                selector.set_items(items_vec);
                selector.table.apply_result_json(Some(json_value), theme);
                selector.status = SelectorStatus::Loaded;
                selector.error_message = None;
                self.apply_filter(selector, theme);
            }
            Err(error) => {
                selector.status = SelectorStatus::Error;
                selector.error_message = Some(format!("unable to refresh provider data: {error}"));
            }
        }
        Vec::new()
    }

    fn stage_current_row(&self, selector: &mut WorkflowSelectorViewState<'_>) -> Result<(), String> {
        let (value, source_field) = self
            .extract_selected_value(selector)
            .ok_or_else(|| "value must be a scalar or value_field missing".to_string())?;
        let row = selector
            .table
            .selected_data()
            .cloned()
            .ok_or_else(|| "no provider row selected".to_string())?;
        let display_key = source_field.clone().unwrap_or_else(|| "value".to_string());
        let display_value = render_value(&display_key, &value);
        selector.set_staged_selection(Some(WorkflowSelectorStagedSelection::new(value, display_value, source_field, row)));
        selector.error_message = None;
        Ok(())
    }

    fn current_row_is_staged(&self, selector: &WorkflowSelectorViewState<'_>) -> bool {
        if let (Some(staged), Some(row)) = (selector.staged_selection(), selector.table.selected_data()) {
            staged.row == *row
        } else {
            false
        }
    }

    fn apply_selection_to_run_state(&self, app: &mut App, selection: WorkflowSelectorStagedSelection) -> Vec<Effect> {
        let mut effects = Vec::new();
        if let Some(name) = app.workflows.active_input_name() {
            if let Some(run) = app.workflows.active_run_state_mut() {
                run.run_context_mut().inputs.insert(name, selection.value);
                let _ = run.evaluate_input_providers();
            }
            effects.push(Effect::CloseModal);
        }
        effects
    }

    //-------------------------------------
    // Selector widget handlers
    fn handle_selector_key_events(&self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let theme = &*app.ctx.theme;
        let mut effects = Vec::new();
        let mut action = SelectorAction::None;

        {
            let Some(selector) = app.workflows.selector_state_mut() else {
                return effects;
            };

            if (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT)) || key.code == KeyCode::BackTab {
                selector.prev_focus();
                return effects;
            }
            if key.code == KeyCode::Tab && key.modifiers.is_empty() {
                selector.next_focus();
                return effects;
            }

            if key.code == KeyCode::Esc {
                if selector.is_filter_focused() && !selector.filter.is_empty() {
                    selector.filter.set_input("");
                    selector.filter.set_cursor(0);
                    self.apply_filter(selector, theme);
                    selector.focus_filter();
                    return effects;
                }
                selector.clear_staged_selection();
                effects.push(Effect::CloseModal);
                return effects;
            }

            match selector.focus {
                WorkflowCollectorFocus::Filter => {
                    self.handle_filter_keys(selector, key, theme);
                }
                WorkflowCollectorFocus::Table => {
                    let (mut new_effects, next_action) = self.handle_table_keys(selector, key);
                    effects.append(&mut new_effects);
                    action = next_action;
                }
                WorkflowCollectorFocus::Buttons(_) => {
                    let (mut new_effects, next_action) = self.handle_button_keys(selector, key);
                    effects.append(&mut new_effects);
                    action = next_action;
                }
            }
        }

        match action {
            SelectorAction::Refresh {
                provider_id,
                resolved_args,
            } => {
                if let Some(selector) = app.workflows.selector_state_mut() {
                    effects.extend(self.refresh_selector_items(selector, theme, &app.ctx.provider_registry, provider_id, resolved_args));
                }
            }
            SelectorAction::Apply(selection) => {
                effects.extend(self.apply_selection_to_run_state(app, selection));
            }
            SelectorAction::OpenManualEntry => {
                app.workflows.open_manual_for_active_input();
            }
            SelectorAction::None => {}
        }

        effects
    }

    fn apply_filter(&self, selector: &mut WorkflowSelectorViewState<'_>, theme: &dyn Theme) {
        selector.refresh_table(theme);
    }

    fn extract_selected_value(&self, selector: &WorkflowSelectorViewState<'_>) -> Option<(JsonValue, Option<String>)> {
        let row = selector.table.selected_data()?;
        if let Some(path) = selector.value_field.as_deref() {
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

    fn render_selector(&self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let theme = &*app.ctx.theme;
        let selector = app.workflows.selector_state_mut().expect("selector state");
        selector.sync_stage_with_selection();

        let title = format!("Select one ({})", selector.provider_id);
        let block = th::block(theme, Some(title.as_str()), false).padding(Padding::uniform(1));
        let inner = block.inner(rect);
        frame.render_widget(block, rect);

        let detail_required = selector.value_field.is_none();
        let layout_spec = if detail_required {
            vec![Constraint::Length(4), Constraint::Min(6), Constraint::Min(5), Constraint::Length(3)]
        } else {
            vec![Constraint::Length(4), Constraint::Min(8), Constraint::Length(3)]
        };
        let layout = Layout::vertical(layout_spec).split(inner);

        let header_area = layout[0];
        let table_area = layout[1];
        let detail_area = if detail_required { Some(layout[2]) } else { None };
        let footer_area = if detail_required { layout[3] } else { layout[2] };

        let header_layout = Layout::vertical([Constraint::Length(3), Constraint::Length(1)]).split(header_area);
        self.render_filter_panel(frame, header_layout[0], selector, theme);
        self.render_status_line(frame, header_layout[1], selector, theme);

        let mut layout_state = WorkflowSelectorLayoutState {
            container_area: Some(rect),
            filter_area: Some(header_layout[0]),
            table_area: Some(table_area),
            detail_area,
            footer_area: Some(footer_area),
            cancel_button_area: None,
            apply_button_area: None,
        };

        let mut results_view = ResultsTableView::default();
        let table_focused = matches!(selector.focus, WorkflowCollectorFocus::Table);
        let visible_rows = table_area.height.saturating_sub(1).max(1) as usize;
        selector.table.set_visible_rows(visible_rows);

        results_view.render_results(frame, table_area, &selector.table, table_focused, theme);
        layout_state.table_area = Some(table_area);

        if let Some(area) = detail_area {
            let detail_layout = Layout::vertical([Constraint::Min(3), Constraint::Length(2)]).split(area);
            let row_json = selector.table.selected_data().cloned().unwrap_or(Value::Null);
            let entries = selector.table.kv_entries();
            let (detail_selection, detail_offset) = self.detail_selection(entries, selector);
            let detail_block = th::block(theme, Some("Details"), table_focused);
            let detail_inner = detail_block.inner(detail_layout[0]);
            frame.render_widget(detail_block, detail_layout[0]);

            ResultsTableView::render_kv_or_text(frame, detail_inner, entries, detail_selection, detail_offset, &row_json, theme);
            self.render_detail_metadata(frame, detail_layout[1], selector, theme);
            layout_state.detail_area = Some(area);
        } else {
            layout_state.detail_area = None;
        }

        let footer_layout = Layout::horizontal([Constraint::Length(24), Constraint::Min(0)]).split(footer_area);
        let button_layout = Layout::horizontal([Constraint::Length(12), Constraint::Length(12)]).split(footer_layout[0]);
        let buttons_focused = matches!(selector.focus, WorkflowCollectorFocus::Buttons(_));

        let cancel_selected = matches!(selector.button_focus(), SelectorButtonFocus::Cancel);
        let cancel_options = ButtonRenderOptions::new(true, buttons_focused && cancel_selected, cancel_selected, Borders::ALL, false);
        th::render_button(frame, button_layout[0], "Cancel", theme, cancel_options);

        let apply_selected = matches!(selector.button_focus(), SelectorButtonFocus::Apply);
        let apply_options = ButtonRenderOptions::new(
            selector.apply_enabled(),
            buttons_focused && apply_selected,
            apply_selected,
            Borders::ALL,
            true,
        );
        th::render_button(frame, button_layout[1], "Apply", theme, apply_options);

        layout_state.footer_area = Some(footer_area);
        layout_state.cancel_button_area = Some(button_layout[0]);
        layout_state.apply_button_area = Some(button_layout[1]);
        selector.set_layout(layout_state);
    }

    fn render_filter_panel(&self, frame: &mut Frame, area: Rect, selector: &WorkflowSelectorViewState<'_>, theme: &dyn Theme) {
        let filter_block_title = Line::from(Span::styled(
            "Filter Results",
            theme.text_secondary_style().add_modifier(Modifier::BOLD),
        ));
        let is_focused = selector.is_filter_focused();
        let mut block = th::block(theme, None, is_focused);
        block = block.title(filter_block_title);
        let inner_area = block.inner(area);
        let filter_text = selector.filter.input();

        let content_line = if is_focused || !filter_text.is_empty() {
            Line::from(Span::styled(filter_text.to_string(), theme.text_primary_style()))
        } else {
            Line::from(Span::from(""))
        };

        let paragraph = Paragraph::new(content_line).block(block).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);

        if is_focused {
            let cursor_index = selector.filter.cursor().min(filter_text.len());
            let prefix = &filter_text[..cursor_index];
            let cursor_columns = prefix.chars().count() as u16;
            let cursor_x = inner_area.x.saturating_add(cursor_columns);
            let cursor_y = inner_area.y;
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }

    fn render_status_line(&self, frame: &mut Frame, area: Rect, selector: &WorkflowSelectorViewState<'_>, theme: &dyn Theme) {
        let status_line = self.build_status_line(selector, theme);
        frame.render_widget(Paragraph::new(status_line), area);
    }

    fn row_index_from_position(&self, selector: &WorkflowSelectorViewState<'_>, table_area: Rect, mouse_row: u16) -> Option<usize> {
        let total_rows = selector.table.rows().map(|rows| rows.len()).unwrap_or(0);
        if total_rows == 0 {
            return None;
        }

        let inner_top = table_area.y.saturating_add(1);
        let data_start = inner_top.saturating_add(1);
        if mouse_row < data_start {
            return None;
        }

        let visible_index = mouse_row.saturating_sub(data_start) as usize;
        let target_index = selector.table.count_offset().saturating_add(visible_index);
        if target_index < total_rows { Some(target_index) } else { None }
    }

    fn build_status_line(&self, selector: &WorkflowSelectorViewState<'_>, theme: &dyn Theme) -> Line<'static> {
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

    fn detail_selection(&self, entries: &[KeyValueEntry], selector: &WorkflowSelectorViewState<'_>) -> (Option<usize>, usize) {
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

    fn render_detail_metadata(&self, frame: &mut Frame, area: Rect, selector: &WorkflowSelectorViewState<'_>, theme: &dyn Theme) {
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

    fn active_field_key(&self, selector: &WorkflowSelectorViewState<'_>) -> Option<String> {
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

    fn selector_hint_spans(theme: &dyn Theme, selector: &WorkflowSelectorViewState<'_>) -> Vec<Span<'static>> {
        match selector.focus {
            WorkflowCollectorFocus::Filter => build_hint_spans(
                theme,
                &[
                    ("Esc", " Clear filter  "),
                    ("Enter", " Focus table  "),
                    ("Tab", " Next focus  "),
                    ("Shift+Tab", " Previous focus"),
                ],
            ),
            WorkflowCollectorFocus::Buttons(_) => build_hint_spans(
                theme,
                &[
                    ("Esc", " Cancel  "),
                    ("←/→", " Switch button  "),
                    ("Enter", " Activate  "),
                    ("Tab", " Next focus  "),
                    ("Shift+Tab", " Previous focus"),
                ],
            ),
            WorkflowCollectorFocus::Table => build_hint_spans(
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
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::components::common::TextInputState;
    use crate::ui::components::table::TableState;
    use crate::ui::components::workflows::collector::WorkflowSelectorLayoutState;
    use indexmap::IndexMap;
    use serde_json::json;

    fn base_selector() -> WorkflowSelectorViewState<'static> {
        WorkflowSelectorViewState {
            provider_id: "apps list".into(),
            resolved_args: serde_json::Map::new(),
            table: TableState::default(),
            value_field: None,
            display_field: None,
            on_error: None,
            status: SelectorStatus::Loaded,
            error_message: None,
            original_items: None,
            pending_cache_key: None,
            filter: TextInputState::new(),
            focus: WorkflowCollectorFocus::Table,
            field_metadata: IndexMap::new(),
            staged_selection: None,
            layout: WorkflowSelectorLayoutState::default(),
        }
    }

    #[test]
    fn detail_selection_prefers_staged_field() {
        let mut selector = base_selector();
        selector.set_staged_selection(Some(WorkflowSelectorStagedSelection::new(
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
