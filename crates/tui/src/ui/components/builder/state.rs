use std::sync::Arc;

use heroku_types::{CommandSpec, Field, Focus};
use heroku_util::fuzzy_score;
use ratatui::widgets::ListState;

#[derive(Debug, Default, Clone)]
pub struct BuilderState {
    is_visible: bool,
    current_focus: Focus,
    selected_command: Option<CommandSpec>,
    input_fields: Vec<Field>,
    current_field_idx: usize,

    search_input: String,
    all_commands: Arc<[CommandSpec]>,
    filtered: Vec<usize>,
    selected: usize,
    list_state: ListState,
}

impl BuilderState {
    // =======================
    // Visibility & Focus API
    // =======================
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }
    pub fn toggle_visibility(&mut self) {
        self.is_visible = !self.is_visible;
    }
    pub fn apply_visibility(&mut self, visible: bool) {
        self.is_visible = visible;
    }

    pub fn selected_focus(&self) -> Focus {
        self.current_focus
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

    // ========================
    // Search & Filtered List
    // ========================
    pub fn search_input(&self) -> &String {
        &self.search_input
    }
    pub fn search_input_push(&mut self, ch: char) {
        self.search_input.push(ch);
        self.update_browser_filtered();
    }
    pub fn search_input_pop(&mut self) {
        self.search_input.pop();
        self.update_browser_filtered();
    }
    pub fn search_input_clear(&mut self) {
        self.search_input.clear();
        self.update_browser_filtered();
    }

    pub fn filtered(&self) -> &Vec<usize> {
        &self.filtered
    }
    pub fn list_state(&mut self) -> &mut ListState {
        &mut self.list_state
    }
    pub fn all_commands(&self) -> Arc<[CommandSpec]> {
        self.all_commands.clone()
    }
    pub fn set_all_commands(&mut self, commands: Arc<[CommandSpec]>) {
        self.all_commands = commands;
    }

    /// Updates the filtered command list based on the current search query.
    ///
    /// This method filters the available commands using fuzzy matching
    /// and updates the filtered indices and selection state.
    pub fn update_browser_filtered(&mut self) {
        if self.search_input.is_empty() {
            self.filtered = (0..self.all_commands.len()).collect();
        } else {
            let mut items: Vec<(i64, usize)> = self
                .all_commands
                .iter()
                .enumerate()
                .filter_map(|(i, command)| {
                    let group = &command.group;
                    let name = &command.name;
                    let exec = if name.is_empty() {
                        group.to_string()
                    } else {
                        format!("{} {}", group, name)
                    };
                    if let Some(score) = fuzzy_score(&exec, &self.search_input) {
                        return Some((score, i));
                    }
                    None
                })
                .collect();
            items.sort_by(|a, b| b.0.cmp(&a.0));

            self.filtered = items.iter().map(|x| x.1).collect();
        }
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
        self.list_state.select(Some(self.selected));
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.filtered.is_empty() {
            return;
        }

        let mut selected = self.selected;
        let new_selected = if delta > 0 {
            selected.saturating_add(delta as usize)
        } else {
            selected.saturating_sub((-delta) as usize)
        };
        selected = new_selected.min(self.filtered.len().saturating_sub(1));
        self.list_state.select(Some(selected));

        let idx = self.filtered[selected];
        let command = self.all_commands[idx].clone();
        self.selected = selected;
        self.apply_command_selection(command);
    }

    // ======================
    // Command & Field State
    // ======================
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
    
    /// Gets the available range fields for the selected command
    pub fn available_ranges(&self) -> Vec<String> {
        self.selected_command
            .as_ref()
            .map(|cmd| cmd.ranges.clone())
            .unwrap_or_default()
    }

    /// Handle Enter within the builder context: focus Inputs when a selection exists.
    pub fn apply_enter(&mut self) {
        if !self.filtered.is_empty() {
            self.apply_focus(Focus::Inputs);
        }
    }

    // Internal helpers for managing field/selection state
    fn apply_command_selection(&mut self, command: CommandSpec) {
        let CommandSpec {
            flags, positional_args, ranges, ..
        } = &command;
        let mut fields: Vec<Field> = Vec::with_capacity(flags.len() + positional_args.len());

        positional_args.iter().for_each(|a| {
            fields.push(Field {
                name: a.name.clone(),
                required: true,
                is_bool: false,
                value: String::new(),
                enum_values: vec![],
                enum_idx: None,
            });
        });

        flags.iter().for_each(|f| {
            fields.push(Field {
                name: f.name.clone(),
                required: f.required,
                is_bool: f.r#type == "boolean",
                value: f.default_value.clone().unwrap_or_default(),
                enum_values: f.enum_values.clone(),
                enum_idx: None,
            });
        });
        
        // Add range fields if available
        if !ranges.is_empty() {
            // Add range field selection
            fields.push(Field {
                name: "range-field".to_string(),
                required: false,
                is_bool: false,
                value: ranges.first().unwrap_or(&String::new()).clone(),
                enum_values: ranges.clone(),
                enum_idx: Some(0),
            });
            
            // Add range start field
            fields.push(Field {
                name: "range-start".to_string(),
                required: false,
                is_bool: false,
                value: String::new(),
                enum_values: vec![],
                enum_idx: None,
            });
            
            // Add range end field
            fields.push(Field {
                name: "range-end".to_string(),
                required: false,
                is_bool: false,
                value: String::new(),
                enum_values: vec![],
                enum_idx: None,
            });
        }
        
        self.set_input_fields(fields);
        self.apply_field_idx(0);
        self.selected_command = Some(command);
    }

    fn set_input_fields(&mut self, fields: Vec<Field>) {
        self.input_fields = fields;
    }
    fn apply_field_idx(&mut self, idx: usize) {
        self.current_field_idx = idx;
    }

    // =================
    // Field Navigation
    // =================
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

    // =================
    // Field Editing
    // =================
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
        if let Some(field) = self.selected_field_mut()
            && !field.is_bool
        {
            field.value.pop();
        }
    }

    pub fn reduce_toggle_boolean_field(&mut self) {
        if let Some(field) = self.selected_field_mut()
            && field.is_bool
        {
            field.value = if field.value.is_empty() {
                "true".into()
            } else {
                String::new()
            };
        }
    }

    pub fn reduce_cycle_enum_left(&mut self) {
        if let Some(field) = self.selected_field_mut()
            && !field.enum_values.is_empty()
        {
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

    pub fn reduce_cycle_enum_right(&mut self) {
        if let Some(field) = self.selected_field_mut()
            && !field.enum_values.is_empty()
        {
            let current = field.enum_idx.unwrap_or(0);
            let new_idx = (current + 1) % field.enum_values.len();
            field.enum_idx = Some(new_idx);
            field.value = field.enum_values[new_idx].clone();
        }
    }

    // ================
    // Private helpers
    // ================
    fn selected_field_mut(&mut self) -> Option<&mut Field> {
        self.input_fields.get_mut(self.current_field_idx)
    }
    fn is_at_debug_field(&self) -> bool {
        self.current_field_idx == self.input_fields.len()
    }
    fn can_move_up(&self) -> bool {
        self.current_field_idx > 0
    }
    fn can_move_down(&self) -> bool {
        self.current_field_idx < self.input_fields.len().saturating_sub(1)
    }
}
