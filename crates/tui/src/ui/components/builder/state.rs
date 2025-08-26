
use heroku_types::{Focus, Field, CommandSpec};

#[derive(Debug, Default, Clone)]
pub struct BuilderState {
    is_visible: bool,
    current_focus: Focus,
    selected_command: Option<CommandSpec>,
    input_fields: Vec<Field>,
    current_field_idx: usize,
}

impl BuilderState {
    // Selectors
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    pub fn selected_focus(&self) -> Focus {
        self.current_focus
    }

    pub fn selected_command(&self) -> Option<&CommandSpec> {
        self.selected_command.as_ref()
    }

    pub fn input_fields(&self) -> &[Field] {
        &self.input_fields
    }

    pub fn current_field_idx(&self) -> usize {
        self.current_field_idx
    }

    pub fn count_fields(&self) -> usize {
        self.input_fields.len()
    }

    pub fn selected_field(&self) -> Option<&Field> {
        self.input_fields.get(self.current_field_idx)
    }

    pub fn selected_field_mut(&mut self) -> Option<&mut Field> {
        self.input_fields.get_mut(self.current_field_idx)
    }

    pub fn missing_required_fields(&self) -> Vec<String> {
        self.input_fields
            .iter()
            .filter(|f| f.required && f.value.is_empty())
            .map(|f| f.name.clone())
            .collect()
    }

    pub fn has_selected_command(&self) -> bool {
        self.selected_command.is_some()
    }

    pub fn is_at_debug_field(&self) -> bool {
        self.current_field_idx == self.input_fields.len()
    }

    pub fn can_move_up(&self) -> bool {
        self.current_field_idx > 0
    }

    pub fn can_move_down(&self) -> bool {
        self.current_field_idx < self.input_fields.len().saturating_sub(1)
    }

    // Reducers
    pub fn toggle_visibility(&mut self) {
        self.is_visible = !self.is_visible;
    }

    pub fn apply_visibility(&mut self, visible: bool) {
        self.is_visible = visible;
    }

    pub fn apply_focus(&mut self, focus: Focus) {
        self.current_focus = focus;
    }

    pub fn apply_next_focus(&mut self) {
        self.current_focus = match self.current_focus {
            Focus::Search => Focus::Commands,
            Focus::Commands => Focus::Inputs,
            Focus::Inputs => Focus::Search,
        };
    }

    pub fn apply_previous_focus(&mut self) {
        self.current_focus = match self.current_focus {
            Focus::Search => Focus::Inputs,
            Focus::Commands => Focus::Search,
            Focus::Inputs => Focus::Commands,
        };
    }

    pub fn apply_command_selection(&mut self, command: CommandSpec) {
        self.selected_command = Some(command);
    }

    pub fn apply_fields(&mut self, fields: Vec<Field>) {
        self.input_fields = fields;
    }

    pub fn apply_field_idx(&mut self, idx: usize) {
        self.current_field_idx = idx;
    }

    pub fn reduce_clear_all(&mut self) {
        self.selected_command = None;
        self.input_fields.clear();
        self.current_field_idx = 0;
        self.current_focus = Focus::Search;
    }

    pub fn reduce_move_field_up(&mut self, debug_enabled: bool) {
        if self.can_move_up() {
            self.current_field_idx -= 1;
        } else if debug_enabled {
            self.current_field_idx = self.input_fields.len();
        }
    }

    pub fn reduce_move_field_down(&mut self, debug_enabled: bool) {
        if debug_enabled && self.is_at_debug_field() {
            self.current_field_idx = 0;
        } else if self.can_move_down() {
            self.current_field_idx += 1;
        }
    }

    pub fn reduce_add_char_to_field(&mut self, c: char) {
        if let Some(field) = self.selected_field_mut() {
            if field.is_bool {
                if c == ' ' {
                    field.value = if field.value.is_empty() {
                        "true".into()
                    } else {
                        String::new()
                    };
                }
            } else {
                field.value.push(c);
            }
        }
    }

    pub fn reduce_remove_char_from_field(&mut self) {
        if let Some(field) = self.selected_field_mut() {
            if !field.is_bool {
                field.value.pop();
            }
        }
    }

    pub fn reduce_toggle_boolean_field(&mut self) {
        if let Some(field) = self.selected_field_mut() {
            if field.is_bool {
                field.value = if field.value.is_empty() {
                    "true".into()
                } else {
                    String::new()
                };
            }
        }
    }

    pub fn reduce_cycle_enum_left(&mut self) {
        if let Some(field) = self.selected_field_mut() {
            if !field.enum_values.is_empty() {
                let current = field.enum_idx.unwrap_or(0);
                let new_idx = if current == 0 {
                    field.enum_values.len() - 1
                } else {
                    current - 1
                };
                field.enum_idx = Some(new_idx);
                field.value = field.enum_values[new_idx].clone();
            }
        }
    }

    pub fn reduce_cycle_enum_right(&mut self) {
        if let Some(field) = self.selected_field_mut() {
            if !field.enum_values.is_empty() {
                let current = field.enum_idx.unwrap_or(0);
                let new_idx = (current + 1) % field.enum_values.len();
                field.enum_idx = Some(new_idx);
                field.value = field.enum_values[new_idx].clone();
            }
        }
    }

    // Private setters
    fn set_visibility(&mut self, visible: bool) {
        self.is_visible = visible;
    }

    fn set_focus(&mut self, focus: Focus) {
        self.current_focus = focus;
    }

    fn set_selected_command(&mut self, command: Option<CommandSpec>) {
        self.selected_command = command;
    }

    fn set_fields(&mut self, fields: Vec<Field>) {
        self.input_fields = fields;
    }

    fn set_field_idx(&mut self, idx: usize) {
        self.current_field_idx = idx;
    }
}