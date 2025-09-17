use std::sync::Arc;

use heroku_types::{CommandSpec, Field};
use heroku_util::fuzzy_score;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::{layout::Rect, widgets::ListState};

#[derive(Debug, Clone)]
pub struct BrowserState {
    selected_command: Option<CommandSpec>,
    input_fields: Vec<Field>,
    current_field_idx: usize,
    // rat-focus flags for panels
    container_focus: FocusFlag,
    pub f_search: FocusFlag,
    pub f_commands: FocusFlag,

    search_input: String,
    all_commands: Arc<[CommandSpec]>,
    filtered: Vec<usize>,
    selected: usize,
    list_state: ListState,
}

impl Default for BrowserState {
    fn default() -> Self {
        Self {
            selected_command: None,
            input_fields: vec![],
            current_field_idx: 0,
            container_focus: FocusFlag::named("browser"),
            f_search: FocusFlag::named("browser.search"),
            f_commands: FocusFlag::named("browser.commands"),
            search_input: String::new(),
            all_commands: Arc::from([]),
            filtered: vec![],
            selected: 0,
            list_state: ListState::default(),
        }
    }
}

impl BrowserState {
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
                    fuzzy_score(&exec, &self.search_input).map(|score| (score, i))
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

    /// Handle Enter within the browser context (noop for now).
    pub fn apply_enter(&mut self) {}

    // Internal helpers for managing field/selection state
    fn apply_command_selection(&mut self, command: CommandSpec) {
        let CommandSpec {
            flags, positional_args, ..
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
}

impl HasFocus for BrowserState {
    fn build(&self, builder: &mut FocusBuilder) {
        let tag = builder.start(self);
        builder.leaf_widget(&self.f_search);
        builder.leaf_widget(&self.f_commands);
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
