use crate::app::App;
use crate::ui::components::common::TextInputState;
use crate::ui::components::workflows::collector::manual_entry::state::{
    ManualEntryEnumState, ManualEntryFocus, ManualEntryKind, ManualEntryLayoutState, ManualEntryState,
};
use crate::ui::theme::Theme;
use crate::ui::theme::theme_helpers::{self as th, ButtonRenderOptions, build_hint_spans};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use heroku_types::Effect;
use heroku_types::workflow::validate_candidate_value;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use serde_json::{Number, Value};
use unicode_width::UnicodeWidthChar;

/// Handles rendering and interaction for the manual entry modal.
#[derive(Debug, Default)]
pub struct ManualEntryComponent;

impl ManualEntryComponent {
    pub fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Enter => return self.submit(app),
            KeyCode::Esc => {
                app.workflows.manual_entry = None;
                return vec![Effect::CloseModal];
            }
            _ => {}
        }

        let kind = app.workflows.manual_entry_state().map(|state| state.kind);
        if key.code == KeyCode::Char(' ') && matches!(kind, Some(ManualEntryKind::Enum)) {
            return self.submit(app);
        }

        let Some(kind) = kind else {
            return Vec::new();
        };

        let Some(state) = app.workflows.manual_entry_state_mut() else {
            return Vec::new();
        };

        match kind {
            ManualEntryKind::Text => {
                state.clear_error();
                handle_text_input(state, key);
            }
            ManualEntryKind::Integer | ManualEntryKind::Number => {
                state.clear_error();
                handle_numeric_input(state, key);
            }
            ManualEntryKind::Boolean => {
                state.clear_error();
                handle_boolean_input(state, key);
            }
            ManualEntryKind::Enum => {
                state.clear_error();
                handle_enum_input(state, key);
            }
        }

        Vec::new()
    }

    pub fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return Vec::new();
        }

        let Some(kind) = app.workflows.manual_entry_state().map(|s| s.kind) else {
            return Vec::new();
        };
        let Some(state) = app.workflows.manual_entry_state_mut() else {
            return Vec::new();
        };

        match kind {
            ManualEntryKind::Boolean => {
                state.clear_error();
                if let Some(area) = state.layout.primary_button_area
                    && contains(area, mouse.column, mouse.row)
                {
                    if let Some(flag) = state.value.boolean_mut() {
                        *flag = true;
                    }
                    return Vec::new();
                }
                if let Some(area) = state.layout.secondary_button_area
                    && contains(area, mouse.column, mouse.row)
                {
                    if let Some(flag) = state.value.boolean_mut() {
                        *flag = false;
                    }
                    return Vec::new();
                }
            }
            ManualEntryKind::Enum => {
                state.clear_error();
                if let Some(area) = state.layout.enum_list_area
                    && contains(area, mouse.column, mouse.row)
                {
                    if let Some(enum_state) = state.value.enum_state_mut()
                        && let Some(index) = index_from_list_position(enum_state, area, mouse.row)
                    {
                        enum_state.select(index);
                    }
                    return Vec::new();
                }
            }
            ManualEntryKind::Text | ManualEntryKind::Integer | ManualEntryKind::Number => {
                if let Some(area) = state.layout.value_area
                    && contains(area, mouse.column, mouse.row)
                {
                    state.clear_error();
                    if let Some(buffer) = state.value.text_buffer_mut() {
                        let cursor = position_from_column(buffer, area, mouse.column);
                        buffer.set_cursor(cursor);
                    }
                }
            }
        }

        Vec::new()
    }

    pub fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let Some(state) = app.workflows.manual_entry_state_mut() else {
            return;
        };
        let theme = &*app.ctx.theme;
        let title = format!("Manual entry: {}", state.label);

        frame.render_widget(Clear, rect);

        let block = th::block(theme, Some(title.as_str()), matches!(state.focus, ManualEntryFocus::Value));
        let inner = block.inner(rect);
        frame.render_widget(block, rect);

        let layout = Layout::vertical([Constraint::Length(1), Constraint::Min(2), Constraint::Length(1)]).split(inner);

        let hint_line = build_value_label(state, theme);
        frame.render_widget(Paragraph::new(hint_line).wrap(Wrap { trim: true }), layout[0]);

        state.layout = ManualEntryLayoutState::default();
        let kind = state.kind;
        match kind {
            ManualEntryKind::Text | ManualEntryKind::Integer | ManualEntryKind::Number => {
                render_text_value(frame, layout[1], state, theme);
            }
            ManualEntryKind::Boolean => {
                render_boolean_value(frame, layout[1], state, theme);
            }
            ManualEntryKind::Enum => {
                render_enum_value(frame, layout[1], state, theme);
            }
        }

        if let Some(error) = &state.error {
            frame.render_widget(
                Paragraph::new(error.clone()).style(theme.status_error()).wrap(Wrap { trim: true }),
                layout[2],
            );
        } else if let Some(validation) = state.validation.as_ref()
            && validation.required
            && matches!(state.kind, ManualEntryKind::Enum)
        {
            let message = "Choose a value to continue";
            frame.render_widget(
                Paragraph::new(message).style(theme.text_muted_style()).wrap(Wrap { trim: true }),
                layout[2],
            );
        }
    }

    pub fn hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        let Some(state) = app.workflows.manual_entry_state() else {
            return Vec::new();
        };
        match state.kind {
            ManualEntryKind::Text | ManualEntryKind::Integer | ManualEntryKind::Number => {
                build_hint_spans(theme, &[("Esc", " Cancel  "), ("Enter", " Confirm")])
            }
            ManualEntryKind::Boolean => build_hint_spans(theme, &[("Esc", " Cancel  "), ("Space", " Toggle  "), ("Enter", " Confirm")]),
            ManualEntryKind::Enum => build_hint_spans(
                theme,
                &[
                    ("Esc", " Cancel  "),
                    ("↑/↓", " Move  "),
                    ("Space", " Confirm  "),
                    ("Enter", " Confirm"),
                ],
            ),
        }
    }

    fn submit(&mut self, app: &mut App) -> Vec<Effect> {
        let (candidate, validation) = {
            let Some(state) = app.workflows.manual_entry_state_mut() else {
                return Vec::new();
            };
            match build_candidate_value(state) {
                Ok(value) => {
                    let validation = state.validation.clone();
                    (value, validation)
                }
                Err(message) => {
                    state.set_error(message);
                    return Vec::new();
                }
            }
        };

        if let Some(validation) = validation
            && let Err(message) = validate_candidate_value(&candidate, &validation)
        {
            if let Some(state) = app.workflows.manual_entry_state_mut() {
                state.set_error(message);
            }
            return Vec::new();
        }

        let input_name = app.workflows.active_input_name();
        if let Some(run_state) = app.workflows.active_run_state_mut()
            && let Some(name) = input_name
        {
            run_state.run_context_mut().inputs.insert(name, candidate);
            let _ = run_state.evaluate_input_providers();
        }

        app.workflows.manual_entry = None;
        vec![Effect::CloseModal]
    }
}

fn build_candidate_value(state: &ManualEntryState) -> Result<Value, String> {
    match state.kind {
        ManualEntryKind::Text => {
            let buffer = state.value.text_buffer().expect("text manual entry should provide a buffer");
            Ok(Value::String(buffer.input().to_string()))
        }
        ManualEntryKind::Integer => {
            let buffer = state.value.text_buffer().expect("integer manual entry should provide a buffer");
            let text = buffer.input().trim();
            if text.is_empty() {
                return Err("enter an integer value".to_string());
            }
            let parsed: i64 = text.parse().map_err(|_| "enter a valid integer".to_string())?;
            Ok(Value::Number(Number::from(parsed)))
        }
        ManualEntryKind::Number => {
            let buffer = state.value.text_buffer().expect("number manual entry should provide a buffer");
            let text = buffer.input().trim();
            if text.is_empty() {
                return Err("enter a numeric value".to_string());
            }
            let parsed: f64 = text.parse().map_err(|_| "enter a valid number".to_string())?;
            Number::from_f64(parsed)
                .map(Value::Number)
                .ok_or_else(|| "number is not representable in JSON".to_string())
        }
        ManualEntryKind::Boolean => {
            let value = state.value.boolean().unwrap_or(false);
            Ok(Value::Bool(value))
        }
        ManualEntryKind::Enum => {
            let enum_state = state.value.enum_state().expect("enum kind should expose enum state");
            let Some(index) = enum_state.selected_index() else {
                return Err("select a value before confirming".to_string());
            };
            enum_state
                .options
                .get(index)
                .map(|option| option.value.clone())
                .ok_or_else(|| "select a value before confirming".to_string())
        }
    }
}

fn handle_text_input(state: &mut ManualEntryState, key: KeyEvent) {
    let Some(buffer) = state.value.text_buffer_mut() else { return };
    match key.code {
        KeyCode::Left => buffer.move_left(),
        KeyCode::Right => buffer.move_right(),
        KeyCode::Backspace => buffer.backspace(),
        KeyCode::Char(character) if !character.is_control() => buffer.insert_char(character),
        _ => {}
    }
}

fn handle_numeric_input(state: &mut ManualEntryState, key: KeyEvent) {
    let kind = state.kind;
    let Some(buffer) = state.value.text_buffer_mut() else { return };
    match key.code {
        KeyCode::Left => buffer.move_left(),
        KeyCode::Right => buffer.move_right(),
        KeyCode::Backspace => buffer.backspace(),
        KeyCode::Char(character) => {
            if allow_numeric_char(buffer, character, kind) {
                buffer.insert_char(character);
            }
        }
        _ => {}
    }
}

fn handle_boolean_input(state: &mut ManualEntryState, key: KeyEvent) {
    let Some(value) = state.value.boolean_mut() else { return };
    match key.code {
        KeyCode::Left | KeyCode::Char('0') | KeyCode::Char('f') | KeyCode::Char('F') => *value = false,
        KeyCode::Right | KeyCode::Char('1') | KeyCode::Char('t') | KeyCode::Char('T') => *value = true,
        KeyCode::Char(' ') => *value = !*value,
        _ => {}
    }
}

fn handle_enum_input(state: &mut ManualEntryState, key: KeyEvent) {
    let Some(enum_state) = state.value.enum_state_mut() else { return };
    match key.code {
        KeyCode::Up => enum_state.select_previous(),
        KeyCode::Down => enum_state.select_next(),
        KeyCode::Home => enum_state.select(0),
        KeyCode::End => {
            if !enum_state.options.is_empty() {
                enum_state.select(enum_state.options.len() - 1);
            }
        }
        _ => {}
    }
}

fn render_text_value(frame: &mut Frame, area: Rect, state: &mut ManualEntryState, theme: &dyn Theme) {
    let buffer = state.value.text_buffer().expect("text rendering requires a text buffer");
    let mut spans = Vec::new();
    spans.push(Span::styled("Value: ", theme.text_secondary_style()));
    if buffer.input().is_empty() {
        if let Some(placeholder) = state.placeholder.as_ref() {
            spans.push(Span::styled(placeholder.clone(), theme.text_muted_style()));
        } else {
            spans.push(Span::styled("", theme.text_primary_style()));
        }
    } else {
        spans.push(Span::styled(buffer.input().to_string(), theme.text_primary_style()));
    }

    let paragraph = Paragraph::new(Line::from(spans)).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);

    let cursor_offset = buffer.input()[..buffer.cursor()].chars().count() as u16;
    let cursor_x = area.x + 7 + cursor_offset;
    frame.set_cursor_position((cursor_x, area.y));

    state.layout.value_area = Some(area);
}

fn render_boolean_value(frame: &mut Frame, area: Rect, state: &mut ManualEntryState, theme: &dyn Theme) {
    let button_layout = Layout::horizontal([Constraint::Length(12), Constraint::Length(12)]).split(area);
    let is_true = state.value.boolean().unwrap_or(false);
    let focused = matches!(state.focus, ManualEntryFocus::Value);

    let true_options = ButtonRenderOptions::new(true, focused && is_true, is_true, Borders::ALL, true);
    th::render_button(frame, button_layout[0], "True", theme, true_options);

    let false_options = ButtonRenderOptions::new(true, focused && !is_true, !is_true, Borders::ALL, false);
    th::render_button(frame, button_layout[1], "False", theme, false_options);

    state.layout.primary_button_area = Some(button_layout[0]);
    state.layout.secondary_button_area = Some(button_layout[1]);
    state.layout.value_area = Some(area);
}

fn render_enum_value(frame: &mut Frame, area: Rect, state: &mut ManualEntryState, theme: &dyn Theme) {
    let Some(enum_state) = state.value.enum_state_mut() else { return };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_style(matches!(state.focus, ManualEntryFocus::Value)));

    let items: Vec<ListItem> = enum_state
        .options
        .iter()
        .map(|option| ListItem::new(option.label.clone()))
        .collect();

    let list = List::new(items).block(block).highlight_style(theme.selection_style());
    frame.render_stateful_widget(list, area, &mut enum_state.list_state);

    state.layout.enum_list_area = Some(area);
    state.layout.value_area = Some(area);
}

fn build_value_label(state: &ManualEntryState, theme: &dyn Theme) -> Line<'static> {
    let prompt = match state.kind {
        ManualEntryKind::Text => "Enter a value",
        ManualEntryKind::Integer => "Enter an integer",
        ManualEntryKind::Number => "Enter a number",
        ManualEntryKind::Boolean => "Select true or false",
        ManualEntryKind::Enum => "Choose from the available options",
    };
    Line::from(Span::styled(prompt.to_string(), theme.text_secondary_style()))
}

fn allow_numeric_char(buffer: &mut TextInputState, character: char, kind: ManualEntryKind) -> bool {
    if character.is_ascii_digit() {
        return true;
    }
    if character == '-' {
        return buffer.cursor() == 0 && !buffer.input().starts_with('-');
    }
    if character == '.' && matches!(kind, ManualEntryKind::Number) {
        return !buffer.input().contains('.');
    }
    false
}

fn position_from_column(buffer: &TextInputState, area: Rect, column: u16) -> usize {
    let relative = column.saturating_sub(area.x + 7);
    let text = buffer.input();
    let mut cumulative = 0usize;
    for (byte_index, ch) in text.char_indices() {
        if cumulative as u16 >= relative {
            return byte_index;
        }
        cumulative += ch.width().unwrap_or(1);
    }
    text.len()
}

fn contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x && column < area.x + area.width && row >= area.y && row < area.y + area.height
}

fn index_from_list_position(enum_state: &ManualEntryEnumState, area: Rect, mouse_row: u16) -> Option<usize> {
    if enum_state.options.is_empty() {
        return None;
    }

    let inner_top = area.y.saturating_add(1);
    if mouse_row < inner_top {
        return None;
    }

    let relative = mouse_row.saturating_sub(inner_top) as usize;
    let offset = enum_state.list_state.offset();
    let index = offset + relative;
    if index < enum_state.options.len() { Some(index) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::components::workflows::collector::manual_entry::state::{ManualEntryEnumOption, ManualEntryValueState};
    use serde_json::json;

    #[test]
    fn build_candidate_value_handles_integer_input() {
        let mut state = ManualEntryState {
            kind: ManualEntryKind::Integer,
            ..Default::default()
        };

        let mut buffer = TextInputState::default();
        buffer.set_input("42");
        state.value = ManualEntryValueState::Number(buffer);

        let value = build_candidate_value(&state).expect("integer value");
        assert_eq!(value, Value::Number(Number::from(42)));
    }

    #[test]
    fn build_candidate_value_errors_on_invalid_number() {
        let mut state = ManualEntryState {
            kind: ManualEntryKind::Number,
            ..Default::default()
        };
        let mut buffer = TextInputState::default();
        buffer.set_input("abc");
        state.value = ManualEntryValueState::Number(buffer);

        let error = build_candidate_value(&state).expect_err("should fail to parse number");
        assert!(error.contains("valid number"));
    }

    #[test]
    fn build_candidate_value_returns_selected_enum_option() {
        let mut state = ManualEntryState {
            kind: ManualEntryKind::Enum,
            ..Default::default()
        };
        let options = vec![
            ManualEntryEnumOption {
                label: "alpha".to_string(),
                value: json!("alpha"),
            },
            ManualEntryEnumOption {
                label: "beta".to_string(),
                value: json!("beta"),
            },
        ];
        state.value = ManualEntryValueState::Enum(ManualEntryEnumState::new(options, 1));

        let value = build_candidate_value(&state).expect("enum value");
        assert_eq!(value, json!("beta"));
    }
}
