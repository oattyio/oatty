use crate::ui::components::common::TextInputState;
use crate::ui::components::common::manual_entry_modal::state::{ManualEntryEnumState, ManualEntryKind, ManualEntryState};
use crate::ui::theme::Theme;
use crate::ui::theme::theme_helpers::{self as th, ButtonRenderOptions, ButtonType, build_hint_spans};
use anyhow::{Result, anyhow};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use oatty_types::Effect;
use oatty_types::workflow::validate_candidate_value;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use serde_json::{Number, Value};
use unicode_width::UnicodeWidthChar;

/// Tracks rendered rectangles for pointer hit-testing.
#[derive(Debug, Default, Clone)]
pub struct ManualEntryLayoutState {
    pub value_label_area: Rect,
    pub value_area: Rect,
    pub message_area: Rect,
    pub primary_button_area: Rect,
    pub secondary_button_area: Rect,
}
impl From<Vec<Rect>> for ManualEntryLayoutState {
    fn from(value: Vec<Rect>) -> Self {
        ManualEntryLayoutState {
            value_label_area: value[0],
            value_area: value[1],
            message_area: value[2],
            primary_button_area: value[3],
            secondary_button_area: value[4],
        }
    }
}
/// Handles rendering and interaction for the manual entry modal.
#[derive(Debug, Default)]
pub struct ManualEntryView {
    layout: ManualEntryLayoutState,
}

impl ManualEntryView {
    fn render_block(&self, frame: &mut Frame, area: Rect, theme: &dyn Theme, state: &mut ManualEntryState) -> Rect {
        let block = th::block(theme, Some(&state.title), state.f_input.get());
        let inner = block.inner(area);
        frame.render_widget(block, area);
        inner
    }

    fn submit(&mut self, state: &mut ManualEntryState) -> Result<Option<Value>> {
        let candidate = build_candidate_value(state)?;
        if let Some(validation) = state.validation.as_ref() {
            validate_candidate_value(&candidate, validation)?;
        }
        Ok(Some(candidate))
    }

    pub fn handle_key_events(&mut self, state: &mut ManualEntryState, key: KeyEvent) -> Result<Option<Value>> {
        match key.code {
            KeyCode::Enter => return self.submit(state),
            KeyCode::Char(' ') if matches!(state.kind, ManualEntryKind::Enum) => return self.submit(state),
            _ => {}
        }

        let kind = state.kind;
        if key.code == KeyCode::Char(' ') && matches!(kind, ManualEntryKind::Enum) {
            return self.submit(state);
        }

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

        Ok(None)
    }

    pub fn handle_mouse_events(&mut self, state: &mut ManualEntryState, mouse: MouseEvent) -> Vec<Effect> {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return Vec::new();
        }

        let pos = Position {
            x: mouse.column,
            y: mouse.row,
        };
        match state.kind {
            ManualEntryKind::Boolean => {
                state.clear_error();
                if self.layout.primary_button_area.contains(pos) {
                    if let Some(flag) = state.value.boolean_mut() {
                        *flag = true;
                    }
                    return Vec::new();
                }
                if self.layout.secondary_button_area.contains(pos) {
                    if let Some(flag) = state.value.boolean_mut() {
                        *flag = false;
                    }
                    return Vec::new();
                }
            }
            ManualEntryKind::Enum => {
                state.clear_error();
                if self.layout.value_area.contains(pos) {
                    if let Some(enum_state) = state.value.enum_state_mut()
                        && let Some(index) = index_from_list_position(enum_state, self.layout.value_area, mouse.row)
                    {
                        enum_state.select(index);
                    }
                    return Vec::new();
                }
            }
            ManualEntryKind::Text | ManualEntryKind::Integer | ManualEntryKind::Number => {
                if self.layout.value_area.contains(pos) {
                    state.clear_error();
                    if let Some(buffer) = state.value.text_buffer_mut() {
                        let cursor = position_from_column(buffer, self.layout.value_area, mouse.column);
                        buffer.set_cursor(cursor);
                    }
                }
            }
        }

        Vec::new()
    }

    pub fn render_with_state(&mut self, frame: &mut Frame, rect: Rect, theme: &dyn Theme, state: &mut ManualEntryState) {
        let inner = self.render_block(frame, rect, theme, state);
        let layout = ManualEntryLayoutState::from(self.get_preferred_layout(inner));

        let hint_line = build_value_label(state, theme);
        frame.render_widget(Paragraph::new(hint_line).wrap(Wrap { trim: true }), layout.value_label_area);

        let kind = state.kind;
        match kind {
            ManualEntryKind::Text | ManualEntryKind::Integer | ManualEntryKind::Number => {
                render_text_value(frame, layout.value_area, state, theme);
            }
            ManualEntryKind::Boolean => {
                render_boolean_value(frame, &layout, state, theme);
            }
            ManualEntryKind::Enum => {
                render_enum_value(frame, layout.value_area, state, theme);
            }
        }

        if let Some(error) = &state.error {
            frame.render_widget(
                Paragraph::new(error.clone()).style(theme.status_error()).wrap(Wrap { trim: true }),
                layout.message_area,
            );
        } else if let Some(validation) = state.validation.as_ref()
            && validation.required
            && matches!(state.kind, ManualEntryKind::Enum)
        {
            let message = "Choose a value to continue";
            frame.render_widget(
                Paragraph::new(message).style(theme.text_muted_style()).wrap(Wrap { trim: true }),
                layout.message_area,
            );
        }

        self.layout = layout;
    }

    pub fn get_hint_spans(&self, theme: &dyn Theme, state: &ManualEntryState) -> Vec<Span<'_>> {
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

    pub fn get_preferred_layout(&self, area: Rect) -> Vec<Rect> {
        let main_layout = Layout::vertical([
            Constraint::Length(1), // value label
            Constraint::Min(2),    // value
            Constraint::Length(1), // error message
        ])
        .split(area)
        .to_vec();

        let button_layout = Layout::horizontal([
            Constraint::Length(12), // primary button
            Constraint::Length(12), // secondary button
        ])
        .split(main_layout[1]);

        vec![main_layout[0], main_layout[1], main_layout[2], button_layout[0], button_layout[1]]
    }
}

fn build_candidate_value(state: &ManualEntryState) -> Result<Value> {
    match state.kind {
        ManualEntryKind::Text => {
            let buffer = state.value.text_buffer().expect("text manual entry should provide a buffer");
            Ok(Value::String(buffer.input().to_string()))
        }
        ManualEntryKind::Integer => {
            let buffer = state.value.text_buffer().expect("integer manual entry should provide a buffer");
            let text = buffer.input().trim();
            if text.is_empty() {
                return Err(anyhow!("enter an integer value"));
            }
            let parsed: i64 = text.parse().map_err(|_| anyhow!("enter a valid integer"))?;
            Ok(Value::Number(Number::from(parsed)))
        }
        ManualEntryKind::Number => {
            let buffer = state.value.text_buffer().expect("number manual entry should provide a buffer");
            let text = buffer.input().trim();
            if text.is_empty() {
                return Err(anyhow!("enter a numeric value"));
            }
            let parsed: f64 = text.parse().map_err(|_| anyhow!("enter a valid number"))?;
            Number::from_f64(parsed)
                .map(Value::Number)
                .ok_or_else(|| anyhow!("number is not representable in JSON"))
        }
        ManualEntryKind::Boolean => {
            let value = state.value.boolean().unwrap_or(false);
            Ok(Value::Bool(value))
        }
        ManualEntryKind::Enum => {
            let enum_state = state.value.enum_state().expect("enum kind should expose enum state");
            let Some(index) = enum_state.selected_index() else {
                return Err(anyhow!("select a value before confirming"));
            };
            enum_state
                .options
                .get(index)
                .map(|option| option.value.clone())
                .ok_or_else(|| anyhow!("select a value before confirming"))
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

fn render_text_value(frame: &mut Frame, area: Rect, state: &ManualEntryState, theme: &dyn Theme) {
    let buffer = state.value.text_buffer().expect("text rendering requires a text buffer");
    let mut spans = Vec::new();
    spans.push(Span::styled("Value: ", theme.text_primary_style()));
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

    let cursor_offset = buffer.cursor_columns() as u16;
    let cursor_x = area.x + 7 + cursor_offset;
    frame.set_cursor_position((cursor_x, area.y));
}

fn render_boolean_value(frame: &mut Frame, layout: &ManualEntryLayoutState, state: &ManualEntryState, theme: &dyn Theme) {
    let is_true = state.value.boolean().unwrap_or(false);
    let focused = state.f_input.get();

    let true_options = ButtonRenderOptions::new(true, focused && is_true, is_true, Borders::ALL, ButtonType::Primary);
    th::render_button(frame, layout.primary_button_area, "True", theme, true_options);

    let false_options = ButtonRenderOptions::new(true, focused && !is_true, !is_true, Borders::ALL, ButtonType::Secondary);
    th::render_button(frame, layout.secondary_button_area, "False", theme, false_options);
}

fn render_enum_value(frame: &mut Frame, area: Rect, state: &mut ManualEntryState, theme: &dyn Theme) {
    let Some(enum_state) = state.value.enum_state_mut() else { return };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_style(state.f_input.get()));

    let items: Vec<ListItem> = enum_state
        .options
        .iter()
        .map(|option| ListItem::new(option.label.clone()))
        .collect();

    let list = List::new(items).block(block).highlight_style(theme.selection_style());
    frame.render_stateful_widget(list, area, &mut enum_state.list_state);
}

fn build_value_label(state: &ManualEntryState, theme: &dyn Theme) -> Line<'static> {
    if let Some(label) = state.label.as_ref() {
        return Line::from(Span::styled(label.clone(), theme.text_secondary_style()));
    }
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
    use crate::ui::components::common::manual_entry_modal::state::{ManualEntryEnumOption, ManualEntryValueState};

    use super::*;
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
        assert!(error.to_string().contains("valid number"));
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
