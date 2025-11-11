use std::sync::{Arc, Mutex};

use crate::ui::components::common::TextInputState;
use heroku_types::{CommandSpec, Field};
use heroku_util::fuzzy_score;
use rat_focus::{FocusBuilder, FocusFlag, HasFocus};
use ratatui::layout::Rect;
use ratatui::widgets::ListState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorDirection {
    Up,
    Down,
    None,
}
#[derive(Debug, Clone)]
pub struct BrowserState {
    pub registry: Arc<Mutex<heroku_registry::CommandRegistry>>,
    pub list_state: ListState,
    selected_command: Option<CommandSpec>,
    input_fields: Vec<Field>,
    current_field_idx: usize,
    // rat-focus flags for panels
    container_focus: FocusFlag,
    pub f_search: FocusFlag,
    pub f_commands: FocusFlag,
    pub f_help: FocusFlag,

    search_input: TextInputState,
    filtered: Vec<usize>,
    viewport_rows: usize,
}

impl BrowserState {
    pub fn new(registry: Arc<Mutex<heroku_registry::CommandRegistry>>) -> Self {
        Self {
            registry,
            selected_command: None,
            input_fields: vec![],
            current_field_idx: 0,
            container_focus: FocusFlag::named("browser"),
            f_search: FocusFlag::named("browser.search"),
            f_commands: FocusFlag::named("browser.commands"),
            f_help: FocusFlag::named("browser.help"),
            search_input: TextInputState::new(),
            filtered: vec![],
            list_state: ListState::default(),
            viewport_rows: 0,
        }
    }
}

impl BrowserState {
    // ========================
    // Search & Filtered List
    // ========================
    pub fn search_query(&self) -> &str {
        self.search_input.input()
    }

    pub fn search_cursor(&self) -> usize {
        self.search_input.cursor()
    }

    pub fn move_search_cursor_left(&mut self) {
        self.search_input.move_left();
    }

    pub fn move_search_cursor_right(&mut self) {
        self.search_input.move_right();
    }

    pub fn append_search_character(&mut self, character: char) {
        self.search_input.insert_char(character);
        self.update_browser_filtered();
    }

    pub fn remove_search_character(&mut self) {
        self.search_input.backspace();
        self.update_browser_filtered();
    }

    pub fn clear_search_query(&mut self) {
        if self.search_input.input().is_empty() && self.search_input.cursor() == 0 {
            return;
        }
        self.search_input.set_input("");
        self.search_input.set_cursor(0);
        self.update_browser_filtered();
    }

    pub fn filtered(&self) -> &Vec<usize> {
        &self.filtered
    }

    pub fn set_viewport_rows(&mut self, rows: usize) {
        let normalized = rows.max(1);
        self.viewport_rows = normalized;
    }

    /// Updates the filtered command list based on the current search query.
    pub fn update_browser_filtered(&mut self) {
        // Rebuild the filtered list.
        {
            let Some(registry_lock) = self.registry.lock().ok() else {
                return;
            };
            let all_commands = &registry_lock.commands;
            let query = self.search_input.input();
            if query.trim().is_empty() {
                self.filtered = (0..all_commands.len()).collect();
            } else {
                let mut scored: Vec<(i64, usize)> = all_commands
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, command)| {
                        let exec = format!("{} {}", command.canonical_id(), command.summary);
                        fuzzy_score(&exec, query).map(|score| (score, idx))
                    })
                    .collect();

                scored.sort_by(|a, b| b.0.cmp(&a.0));
                self.filtered = scored.into_iter().map(|(_, idx)| idx).collect();
            }
        }

        self.move_selection(CursorDirection::None);
    }

    pub fn move_selection(&mut self, direction: CursorDirection) {
        if self.filtered.is_empty() {
            return;
        }
        match direction {
            CursorDirection::Up => {
                self.list_state.select_previous();
            }
            CursorDirection::Down => {
                self.list_state.select_next();
            }
            CursorDirection::None => {}
        }
        self.commit_selection();
    }

    pub fn commit_selection(&mut self) {
        let selected_idx = self.list_state.selected().unwrap_or(0);
        let idx = *self.filtered.get(selected_idx).unwrap_or(&0usize);
        let maybe_command = {
            let Some(registry_lock) = self.registry.lock().ok() else {
                return;
            };
            registry_lock.commands.get(idx).cloned()
        };

        if let Some(command) = maybe_command {
            self.apply_command_selection(command);
        }
    }

    // ======================
    // Command & Field State
    // ======================
    pub fn selected_command(&self) -> Option<&CommandSpec> {
        self.selected_command.as_ref()
    }

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
        builder.leaf_widget(&self.f_help);
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heroku_registry::CommandRegistry;
    use std::sync::{Arc, Mutex};

    fn build_state() -> BrowserState {
        let registry = Arc::new(Mutex::new(CommandRegistry::default()));
        BrowserState::new(registry)
    }

    #[test]
    fn insert_characters_respects_cursor_position() {
        let mut state = build_state();
        state.append_search_character('a');
        state.append_search_character('b');
        state.move_search_cursor_left();
        state.append_search_character('c');

        assert_eq!(state.search_query(), "acb");
        assert_eq!(state.search_cursor(), 2);
    }

    #[test]
    fn clear_search_query_resets_buffer_and_cursor() {
        let mut state = build_state();
        state.append_search_character('a');
        state.append_search_character('b');
        state.move_search_cursor_left();
        state.clear_search_query();

        assert_eq!(state.search_query(), "");
        assert_eq!(state.search_cursor(), 0);
    }
}
