use crate::app::App;
use crate::ui::components::common::ResultsTableView;
use crate::ui::components::workflows::state::{SelectorStatus, WorkflowSelectorViewState};
use crate::ui::theme::Theme;
use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use heroku_engine::ProviderValueResolver;
use heroku_engine::resolve::select_path;
use heroku_types::{Effect, WorkflowProviderErrorPolicy};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use serde_json::{Value as JsonValue, Value};
use crate::{
    ui::{
        components::{component::Component},
        theme::theme_helpers as th,
    },
};
#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct WorkflowCollectorComponent {}

impl Component for WorkflowCollectorComponent {
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let mut effects = Vec::new();

        // Manual entry mode takes precedence when present
        if app.workflows.manual_entry_state().is_some() {
            return self.handle_manual_entry_key_events(app, key);
        }

        // Provider-backed selector handling when present
        if app.workflows.selector_state_mut().is_some() {
            return self.handle_selector_key_events(app, key);
        }

        // Fallback: allow closing if neither manual nor selector is present
        match key.code {
            KeyCode::Esc => effects.push(Effect::CloseModal),
            _ => {}
        }
        effects
    }

    fn handle_mouse_events(&mut self, _app: &mut App, _mouse: MouseEvent) -> Vec<Effect> {
        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        // If manual entry state exists, render Manual Entry View; else render a selector
        if app.workflows.manual_entry_state().is_some() {
            self.render_manual_entry(frame, rect, app);
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

        let layout = self.preferred_layout(inner_area);

        for area in layout.into_iter() {
            frame.render_widget(
                Paragraph::new("No selector state").block(Block::default().borders(Borders::ALL)),
                area,
            );
        }
    }
}

impl WorkflowCollectorComponent {
    //------------------------------------------
    // Manual entry widget handlers
    fn handle_manual_entry_key_events(&self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let mut effects = Vec::new();
        // pull this branch out of the matcher to avoid a borrow checker error
        if key.code == KeyCode::Enter {
            effects.extend(self.validate_and_insert_manual_value(app));
        }
        let view_state = app.workflows.manual_entry_state_mut().expect("manual entry state");
        match key.code {
            KeyCode::Esc => {
                app.workflows.manual_entry = None;
                effects.push(Effect::CloseModal);
            }
            KeyCode::Left => view_state.text.move_left(),
            KeyCode::Right => view_state.text.move_right(),
            KeyCode::Backspace => view_state.text.backspace(),
            KeyCode::Char(c) => view_state.text.insert_char(c),
            _ => {}
        }
        effects
    }

    fn validate_and_insert_manual_value(&self, app: &mut App) -> Vec<Effect> {
        let view_state = app.workflows.manual_entry_state_mut().expect("manual entry state");
        let mut effects = Vec::new();
        // Validate against enum (pattern deferred to follow-up if regex available)
        let candidate = view_state.text.input().to_string();
        if let Some(validate) = &view_state.validation {
            if !validate.allowed_values.is_empty() {
                let ok = validate
                    .allowed_values
                    .iter()
                    .any(|v| v.as_str() == Some(candidate.as_str()) || v.to_string() == candidate);
                if !ok {
                    view_state.error = Some("value is not in allowed set".into());
                    return effects;
                }
            }
        }

        let input_name = app.workflows.active_input_name();
        if let Some(run_state) = app.workflows.active_run_state_mut() {
            if let Some(name) = input_name {
                run_state
                    .run_context_mut()
                    .inputs
                    .insert(name, Value::String(candidate));
                let _ = run_state.evaluate_input_providers();
            }
        }
        app.workflows.manual_entry = None;
        effects.push(Effect::CloseModal);
        effects
    }

    //-------------------------------------
    // Selector widget handlers
    fn handle_selector_key_events(&self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let mut effects = Vec::new();
        let sel = app.workflows.selector_state_mut().expect("selector state");
        // If filter is active, route editing keys
        if sel.filter_active {
            match key.code {
                KeyCode::Esc => {
                    sel.filter_active = false;
                    sel.filter.set_input("");
                    self.apply_filter(sel, &*app.ctx.theme);
                }
                KeyCode::Enter => {
                    sel.filter_active = false;
                    self.apply_filter(sel, &*app.ctx.theme);
                }
                KeyCode::Left => sel.filter.move_left(),
                KeyCode::Right => sel.filter.move_right(),
                KeyCode::Backspace => sel.filter.backspace(),
                KeyCode::Char(c) => sel.filter.insert_char(c),
                _ => {}
            }
            return effects;
        }

        match key.code {
            KeyCode::Esc => effects.push(Effect::CloseModal),
            KeyCode::Char('/') => {
                sel.filter_active = true;
            }
            KeyCode::Char('r') => {
                // Force refresh now
                if let Ok(items) = app.ctx.provider_registry.fetch_values(&sel.provider_id, &sel.resolved_args) {
                    sel.original_items = Some(items.clone());
                    let json = Value::Array(items);
                    sel.table.apply_result_json(Some(json), &*app.ctx.theme);
                    sel.status = SelectorStatus::Loaded;
                    sel.error_message = None;
                    self.apply_filter(sel, &*app.ctx.theme); // reapply active filter if any
                } else {
                    sel.status = SelectorStatus::Error;
                }
            }
            KeyCode::Up => sel.table.reduce_scroll(-1),
            KeyCode::Down => sel.table.reduce_scroll(1),
            KeyCode::Home => sel.table.reduce_home(),
            KeyCode::End => sel.table.reduce_end(),
            KeyCode::Enter => {
                if matches!(sel.status, SelectorStatus::Error)
                    && matches!(sel.on_error, Some(WorkflowProviderErrorPolicy::Fail))
                {
                    // Fail policy: do not allow apply
                    sel.error_message = Some("provider error: cannot apply (policy: fail)".into());
                    return effects;
                }
                if let Some(value) = self.extract_selected_value(sel) {
                    if let Some(name) = app.workflows.active_input_name() {
                        if let Some(run) = app.workflows.active_run_state_mut() {
                            run.run_context_mut().inputs.insert(name, value);
                            let _ = run.evaluate_input_providers();
                        }
                        effects.push(Effect::CloseModal);
                    }
                } else {
                    sel.error_message = Some("value must be a scalar or value_field missing".into());
                }
            }
            KeyCode::F(2) => {
                // Fallback to manual
                app.workflows.open_manual_for_active_input();
            }
            _ => {}
        }
        effects
    }


    fn apply_filter(&self, sel: &mut WorkflowSelectorViewState<'_>, theme: &dyn Theme) {
        let Some(items) = sel.original_items.clone() else {
            return;
        };
        let query = sel.filter.input().trim().to_lowercase();
        if query.is_empty() {
            let json = Value::Array(items);
            sel.table.apply_result_json(Some(json), theme);
            sel.table.normalize();
            return;
        }
        let filtered: Vec<serde_json::Value> = items
            .into_iter()
            .filter(|item| match item {
                Value::Object(map) => {
                    if let Some(df) = sel.display_field.as_deref() {
                        if let Some(v) = map.get(df) {
                            if let Some(s) = v.as_str() {
                                return s.to_lowercase().starts_with(&query);
                            }
                        }
                    }
                    // fallback: any string field contains substring
                    map.values()
                        .any(|v| v.as_str().map(|s| s.to_lowercase().contains(&query)).unwrap_or(false))
                }
                Value::String(s) => s.to_lowercase().contains(&query),
                _ => false,
            })
            .collect();
        let json = Value::Array(filtered);
        sel.table.apply_result_json(Some(json), theme);
        sel.table.normalize();
    }

    fn extract_selected_value(&self, sel: &WorkflowSelectorViewState<'_>) -> Option<JsonValue> {
        let row = sel.table.selected_data()?;
        if let Some(path) = sel.value_field.as_deref() {
            let v = select_path(row, Some(path))?;
            return match v {
                JsonValue::String(s) => Some(JsonValue::String(s.clone())),
                JsonValue::Number(n) => Some(JsonValue::Number(n.clone())),
                JsonValue::Bool(b) => Some(JsonValue::Bool(b)),
                JsonValue::Null => Some(JsonValue::Null),
                _ => None,
            };
        }
        match row {
            JsonValue::Object(map) => {
                for (_k, v) in map {
                    if let JsonValue::String(s) = v {
                        return Some(JsonValue::String(s.clone()));
                    }
                    if let JsonValue::Number(n) = v {
                        return Some(JsonValue::Number(n.clone()));
                    }
                    if let JsonValue::Bool(b) = v {
                        return Some(JsonValue::Bool(*b));
                    }
                }
                None
            }
            JsonValue::String(s) => Some(JsonValue::String(s.clone())),
            JsonValue::Number(n) => Some(JsonValue::Number(n.clone())),
            JsonValue::Bool(b) => Some(JsonValue::Bool(*b)),
            JsonValue::Null => Some(JsonValue::Null),
            _ => None,
        }
    }

    fn render_manual_entry(&self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let view_state = app.workflows.manual_entry_state().expect("manual entry state");
        let label = format!("Manual entry: {}", view_state.label);
        let block = th::block(&*app.ctx.theme, Some(label.as_str()), false);
        let inner = block.inner(rect);
        frame.render_widget(block, rect);

        let layout = Layout::vertical([
            Constraint::Length(1), // value line
            Constraint::Length(1), // error line
            Constraint::Min(1),    // hints
        ])
            .split(inner);

        // Value line
        let value_text = format!("Value: {}", view_state.text.input());
        frame.render_widget(Paragraph::new(value_text), layout[0]);

        // Error line
        if let Some(err) = &view_state.error {
            frame.render_widget(Paragraph::new(err.clone()).wrap(Wrap { trim: true }), layout[1]);
        }

        // Hints
        frame.render_widget(Paragraph::new("Esc cancel  •  Enter confirm"), layout[2]);

        // Cursor position (UTF-8 safe by counting chars up to cursor)
        let cursor_chars = view_state.text.input()[..view_state.text.cursor()].chars().count();
        let x = inner.x + 7 + cursor_chars as u16; // "Value: " == 7 chars
        let y = inner.y;
        frame.set_cursor_position((x, y));
    }
    
    fn render_selector(&self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let sel = app.workflows.selector_state_mut().expect("selector state");
        // Non-blocking initial load: try cached values or enqueue a background fetch.
        if matches!(sel.status, SelectorStatus::Loading) {
            if let Some(items) = app
                .ctx
                .provider_registry
                .cached_values_or_enqueue(&sel.provider_id, sel.resolved_args.clone())
            {
                sel.original_items = Some(items.clone());
                let json = Value::Array(items);
                sel.table.apply_result_json(Some(json), &*app.ctx.theme);
                sel.table.normalize();
                sel.status = SelectorStatus::Loaded;
                sel.error_message = None;
                // If filter already has text, apply it
                self.apply_filter(sel, &*app.ctx.theme);
            }
        }

        let title = format!("Select one ({})", sel.provider_id);
        let block = th::block(&*app.ctx.theme, Some(title.as_str()), false);
        let inner = block.inner(rect);
        frame.render_widget(block, rect);

        let layout = Layout::vertical([
            Constraint::Length(1), // Header with status
            Constraint::Min(6),    // Table
            Constraint::Length(1), // Hints
        ])
            .split(inner);

        // Header line: Filter (placeholder) + Status
        let status_label = match sel.status {
            SelectorStatus::Loading => "loading…",
            SelectorStatus::Loaded => "loaded",
            SelectorStatus::Error => "error",
        };
        let header = Line::from(vec![
            Span::raw(if sel.filter_active { "Filter*: [" } else { "Filter: [" }),
            Span::raw(sel.filter.input()),
            Span::raw("]   "),
            Span::raw("Status: "),
            Span::raw(status_label),
            if let Some(err) = &sel.error_message {
                Span::raw(format!("  •  {}", err))
            } else {
                Span::raw("")
            },
        ]);
        frame.render_widget(Paragraph::new(header), layout[0]);

        // Table grid
        let mut results_view = ResultsTableView::default();
        results_view.render_results(frame, layout[1], &sel.table, true, &*app.ctx.theme);

        // Hints
        let hints = Paragraph::new(Line::from(vec![Span::raw(
            "Esc cancel  •  ↑↓ move  •  Enter confirm  •  r refresh  •  F2 manual",
        )]));
        frame.render_widget(hints, layout[2]);
    }

    fn preferred_layout(&self, area: Rect) -> Vec<Rect> {
        if area.width >= 141 {
            let two_col = Layout::horizontal([Constraint::Percentage(65), Constraint::Min(20)]).split(area);

            let constraints = [Constraint::Percentage(100), Constraint::Min(1)];

            let left_areas = Layout::vertical(constraints).split(two_col[0]);

            vec![left_areas[0], two_col[1], left_areas[1]]
        } else {
            let constraints = [Constraint::Percentage(40), Constraint::Min(1)];

            Layout::vertical(constraints).split(area).to_vec()
        }
    }
}
