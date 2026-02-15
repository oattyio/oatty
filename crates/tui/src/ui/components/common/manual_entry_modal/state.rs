use std::cmp::min;

use indexmap::IndexSet;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use ratatui::widgets::ListState;
use serde_json::Value as JsonValue;

use crate::ui::components::common::TextInputState;
use crate::ui::utils::render_value;
use oatty_types::workflow::{WorkflowInputDefinition, WorkflowInputValidation};

/// Identifies the editing mode used by the manual entry modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManualEntryKind {
    Text,
    Integer,
    Number,
    Boolean,
    Enum,
}

/// Represents a single selectable literal for enum-style manual entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ManualEntryEnumOption {
    pub label: String,
    pub value: JsonValue,
}

/// Maintains list state for enum selections, including scroll offset.
#[derive(Debug, Default, Clone)]
pub struct ManualEntryEnumState {
    pub options: Vec<ManualEntryEnumOption>,
    pub list_state: ListState,
}

impl ManualEntryEnumState {
    pub fn new(options: Vec<ManualEntryEnumOption>, selected: usize) -> Self {
        let mut state = ListState::default();
        if !options.is_empty() {
            state.select(Some(min(selected, options.len().saturating_sub(1))));
        }
        Self {
            options,
            list_state: state,
        }
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub fn select(&mut self, index: usize) {
        if self.options.is_empty() {
            self.list_state.select(None);
            return;
        }
        let bounded = min(index, self.options.len().saturating_sub(1));
        self.list_state.select(Some(bounded));
    }

    pub fn select_next(&mut self) {
        if self.options.is_empty() {
            return;
        }
        let current = self.selected_index().unwrap_or(0);
        let next = min(current.saturating_add(1), self.options.len().saturating_sub(1));
        self.select(next);
    }

    pub fn select_previous(&mut self) {
        if self.options.is_empty() {
            return;
        }
        let current = self.selected_index().unwrap_or(0);
        let next = current.saturating_sub(1);
        self.select(next);
    }
}

/// Captures the mutable value backing the manual entry view.
#[derive(Debug, Clone)]
pub enum ManualEntryValueState {
    Text(TextInputState),
    Number(TextInputState),
    Boolean(bool),
    Enum(ManualEntryEnumState),
}

impl ManualEntryValueState {
    pub fn text_buffer(&self) -> Option<&TextInputState> {
        match self {
            ManualEntryValueState::Text(buffer) | ManualEntryValueState::Number(buffer) => Some(buffer),
            _ => None,
        }
    }

    pub fn text_buffer_mut(&mut self) -> Option<&mut TextInputState> {
        match self {
            ManualEntryValueState::Text(buffer) | ManualEntryValueState::Number(buffer) => Some(buffer),
            _ => None,
        }
    }

    pub fn boolean(&self) -> Option<bool> {
        match self {
            ManualEntryValueState::Boolean(value) => Some(*value),
            _ => None,
        }
    }

    pub fn boolean_mut(&mut self) -> Option<&mut bool> {
        match self {
            ManualEntryValueState::Boolean(value) => Some(value),
            _ => None,
        }
    }

    pub fn enum_state(&self) -> Option<&ManualEntryEnumState> {
        match self {
            ManualEntryValueState::Enum(state) => Some(state),
            _ => None,
        }
    }

    pub fn enum_state_mut(&mut self) -> Option<&mut ManualEntryEnumState> {
        match self {
            ManualEntryValueState::Enum(state) => Some(state),
            _ => None,
        }
    }
}

/// Complete mutable state for the manual entry modal.
#[derive(Debug, Clone)]
pub struct ManualEntryState {
    pub title: String,
    pub label: Option<String>,
    pub placeholder: Option<String>,
    pub hint: Option<String>,
    pub example: Option<String>,
    pub error: Option<String>,
    pub validation: Option<WorkflowInputValidation>,
    pub kind: ManualEntryKind,
    pub value: ManualEntryValueState,
    /// Container and widget focus.
    pub container_focus: FocusFlag,
    pub f_input: FocusFlag,
}

impl Default for ManualEntryState {
    fn default() -> Self {
        Self {
            title: String::new(),
            label: None,
            placeholder: None,
            hint: None,
            example: None,
            error: None,
            validation: None,
            kind: ManualEntryKind::Text,
            value: ManualEntryValueState::Text(TextInputState::default()),
            container_focus: FocusFlag::default(),
            f_input: FocusFlag::default(),
        }
    }
}

impl HasFocus for ManualEntryState {
    fn build(&self, builder: &mut FocusBuilder) {
        let start = builder.start(self);
        builder.leaf_widget(&self.f_input);
        builder.end(start);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}

impl ManualEntryState {
    /// Builds manual entry state for the provided workflow input definition.
    pub fn from_definition(definition: &WorkflowInputDefinition, label: &str, existing: Option<&JsonValue>) -> Self {
        let mut state = ManualEntryState {
            label: Some(label.to_string()),
            placeholder: definition.placeholder.clone(),
            hint: definition.hint.clone(),
            example: definition.example.clone(),
            validation: definition.validate.clone(),
            ..ManualEntryState::default()
        };

        let enumerated = merge_enumerations(definition);
        if !enumerated.is_empty() {
            state.kind = ManualEntryKind::Enum;
            state.value = ManualEntryValueState::Enum(build_enum_state(&enumerated, existing));
            return state;
        }

        state.kind = inferred_kind(definition);
        state.value = match state.kind {
            ManualEntryKind::Boolean => ManualEntryValueState::Boolean(parse_boolean(existing).unwrap_or(false)),
            ManualEntryKind::Text | ManualEntryKind::Integer | ManualEntryKind::Number => {
                let mut buffer = TextInputState::default();
                let prefill = existing.and_then(render_existing_scalar).unwrap_or_default();
                if !prefill.is_empty() {
                    buffer.set_input(prefill.clone());
                    buffer.set_cursor(prefill.len());
                }
                match state.kind {
                    ManualEntryKind::Text => ManualEntryValueState::Text(buffer),
                    ManualEntryKind::Integer | ManualEntryKind::Number => ManualEntryValueState::Number(buffer),
                    ManualEntryKind::Boolean | ManualEntryKind::Enum => unreachable!(),
                }
            }
            ManualEntryKind::Enum => unreachable!(),
        };

        state
    }

    pub fn clear_error(&mut self) {
        self.error = None;
    }

    pub fn set_error<S: Into<String>>(&mut self, message: S) {
        self.error = Some(message.into());
    }
}

fn inferred_kind(definition: &WorkflowInputDefinition) -> ManualEntryKind {
    match definition.r#type.as_deref().map(str::to_lowercase) {
        Some(ref ty) if ty == "boolean" => ManualEntryKind::Boolean,
        Some(ref ty) if ty == "integer" => ManualEntryKind::Integer,
        Some(ref ty) if ty == "number" || ty == "float" || ty == "double" => ManualEntryKind::Number,
        _ => ManualEntryKind::Text,
    }
}

fn merge_enumerations(definition: &WorkflowInputDefinition) -> Vec<JsonValue> {
    let mut merged: IndexSet<JsonValue> = IndexSet::new();
    for value in &definition.enumerated_values {
        merged.insert(value.clone());
    }
    if let Some(validation) = &definition.validate {
        for value in &validation.allowed_values {
            merged.insert(value.clone());
        }
    }
    merged.into_iter().collect()
}

fn build_enum_state(options: &[JsonValue], existing: Option<&JsonValue>) -> ManualEntryEnumState {
    let mut rendered_options = Vec::new();
    let mut selected_index = 0usize;
    for (idx, option) in options.iter().enumerate() {
        if let Some(current) = existing
            && values_match(option, current)
        {
            selected_index = idx;
        }
        rendered_options.push(ManualEntryEnumOption {
            label: render_value("", option, None).into_plain_text(),
            value: option.clone(),
        });
    }
    ManualEntryEnumState::new(rendered_options, selected_index)
}

fn render_existing_scalar(existing: &JsonValue) -> Option<String> {
    match existing {
        JsonValue::String(text) => Some(text.clone()),
        JsonValue::Number(number) => Some(number.to_string()),
        JsonValue::Bool(flag) => Some(flag.to_string()),
        JsonValue::Null => None,
        other => Some(render_value("", other, None).into_plain_text()),
    }
}

fn parse_boolean(existing: Option<&JsonValue>) -> Option<bool> {
    existing.and_then(|value| {
        value.as_bool().or_else(|| {
            if let JsonValue::String(text) = value {
                match text.to_lowercase().as_str() {
                    "true" | "1" | "yes" => Some(true),
                    "false" | "0" | "no" => Some(false),
                    _ => None,
                }
            } else {
                None
            }
        })
    })
}

fn values_match(expected: &JsonValue, candidate: &JsonValue) -> bool {
    if expected == candidate {
        return true;
    }
    match (expected, candidate) {
        (_, JsonValue::String(text)) => &expected.to_string() == text,
        (JsonValue::String(expected_text), other) => expected_text == &other.to_string(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn builder_uses_enum_kind_when_enumerations_present() {
        let definition = WorkflowInputDefinition {
            enumerated_values: vec![json!("alpha"), json!("beta")],
            ..Default::default()
        };

        let state = ManualEntryState::from_definition(&definition, "option", None);
        assert!(matches!(state.kind, ManualEntryKind::Enum));
        let enum_state = state.value.enum_state().expect("enum state available");
        assert_eq!(enum_state.options.len(), 2);
        assert_eq!(enum_state.options[0].value, json!("alpha"));
        assert_eq!(enum_state.options[0].label, "alpha");
    }

    #[test]
    fn builder_prefills_boolean_from_existing_value() {
        let definition = WorkflowInputDefinition {
            r#type: Some("boolean".to_string()),
            ..Default::default()
        };

        let state = ManualEntryState::from_definition(&definition, "flag", Some(&Value::Bool(true)));
        assert!(matches!(state.kind, ManualEntryKind::Boolean));
        assert_eq!(state.value.boolean(), Some(true));
    }

    #[test]
    fn builder_prefills_number_buffer_from_existing_value() {
        let definition = WorkflowInputDefinition {
            r#type: Some("number".to_string()),
            ..Default::default()
        };

        let state = ManualEntryState::from_definition(&definition, "count", Some(&json!(42.5)));
        assert!(matches!(state.kind, ManualEntryKind::Number));
        let buffer = state.value.text_buffer().expect("buffer present for number");
        assert_eq!(buffer.input(), "42.5");
    }

    #[test]
    fn builder_carries_hint_and_example_metadata() {
        let definition = WorkflowInputDefinition {
            hint: Some("Use the full repo URL".to_string()),
            example: Some("https://github.com/acme/service".to_string()),
            placeholder: Some("owner/repo".to_string()),
            ..Default::default()
        };

        let state = ManualEntryState::from_definition(&definition, "repo", None);
        assert_eq!(state.hint.as_deref(), Some("Use the full repo URL"));
        assert_eq!(state.example.as_deref(), Some("https://github.com/acme/service"));
        assert_eq!(state.placeholder.as_deref(), Some("owner/repo"));
    }
}
