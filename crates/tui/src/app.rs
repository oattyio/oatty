use ratatui::widgets::ListState;

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub required: bool,
    pub is_bool: bool,
    pub value: String,
    pub enum_values: Vec<String>,
    pub enum_idx: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Search,
    Commands,
    Inputs,
}

pub struct App {
    pub registry: heroku_registry::Registry,
    pub focus: Focus,
    pub search: String,
    pub all_commands: Vec<heroku_registry::CommandSpec>,
    pub filtered: Vec<usize>,
    pub selected: usize,

    pub picked: Option<heroku_registry::CommandSpec>,
    pub fields: Vec<Field>,
    pub field_idx: usize,

    pub logs: Vec<String>,
    pub show_help: bool,
    pub help_spec: Option<heroku_registry::CommandSpec>,
    pub list_state: ListState,
    pub dry_run: bool,
    pub debug_enabled: bool,
    pub result_json: Option<serde_json::Value>,
    pub show_table: bool,
    pub table_offset: usize,
    pub show_builder: bool,
    pub palette: crate::palette::PaletteState,
    pub providers: Vec<Box<dyn crate::palette::ValueProvider>>,
    pub executing: bool,
    pub throbber_idx: usize,
    pub exec_rx: Option<std::sync::mpsc::Receiver<ExecOutcome>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Msg {
    ToggleHelp,
    ToggleTable,
    ToggleBuilder,
    CloseModal,
    FocusNext,
    FocusPrev,
    MoveSelection(isize),
    Enter,
    SearchChar(char),
    SearchBackspace,
    SearchClear,
    InputsUp,
    InputsDown,
    InputsChar(char),
    InputsBackspace,
    InputsToggleSpace,
    InputsCycleLeft,
    InputsCycleRight,
    Run,
    CopyCommand,
    TableScroll(isize),
    TableHome,
    TableEnd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    CopyCommandRequested,
}

impl App {
    pub fn new(registry: heroku_registry::Registry) -> Self {
        let all = registry.commands.clone();
        let mut app = Self {
            registry,
            focus: Focus::Search,
            search: String::new(),
            filtered: (0..all.len()).collect(),
            selected: 0,
            picked: None,
            fields: Vec::new(),
            field_idx: 0,
            logs: vec!["Welcome to Heroku TUI".into()],
            show_help: false,
            help_spec: None,
            all_commands: all,
            list_state: ListState::default(),
            dry_run: false,
            debug_enabled: std::env::var("DEBUG")
                .map(|v| !v.is_empty() && v != "0" && v.to_lowercase() != "false")
                .unwrap_or(false),
            result_json: None,
            show_table: false,
            table_offset: 0,
            show_builder: false,
            palette: Default::default(),
            providers: Default::default(),
            executing: false,
            throbber_idx: 0,
            exec_rx: None,
        };
        // Debug: add a static provider for apps:info positional <app>
        if app.debug_enabled {
            app.providers.push(Box::new(crate::palette::StaticValuesProvider {
                command_key: "apps:info".into(),
                field: "app".into(),
                values: vec!["demo".into(), "api".into(), "web".into(), "my-app".into()],
            }));
        }
        app.rebuild_fields_for_current_selection();
        if !app.filtered.is_empty() {
            app.list_state.select(Some(0));
        }
        app
    }

    pub fn filter_commands(&mut self) {
        let q = self.search.to_lowercase();
        self.filtered = self
            .all_commands
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                if q.is_empty() {
                    return true;
                }
                let name = c.name.as_str();
                let lower_name = name.to_lowercase();
                // Display form: group + rest
                let mut split = name.splitn(2, ':');
                let group = split.next().unwrap_or("");
                let rest = split.next().unwrap_or("");
                let display = if rest.is_empty() {
                    group.to_string()
                } else {
                    format!("{} {}", group, rest)
                };
                let display_lower = display.to_lowercase();
                // Flat form: replace ':' with space for queries like "apps list"
                let flat_lower = lower_name.replace(':', " ");
                lower_name.contains(&q) || display_lower.contains(&q) || flat_lower.contains(&q)
            })
            .map(|(i, _)| i)
            .collect();
        self.selected = 0;
        self.rebuild_fields_for_current_selection();
        if self.filtered.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.filtered.is_empty() {
            return;
        }
        let len = self.filtered.len() as isize;
        let cur = self.selected as isize;
        let next = (cur + delta).rem_euclid(len);
        self.selected = next as usize;
        self.rebuild_fields_for_current_selection();
        self.list_state.select(Some(self.selected));
    }

    pub fn pick_current_command(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        // Fields are already synced with selection; Enter moves focus to Inputs
        self.focus = Focus::Inputs;
    }

    pub fn inputs_prev(&mut self) {
        if self.field_idx > 0 {
            self.field_idx -= 1;
        }
    }
    // Allow selecting a pseudo-field at index == fields.len() for Dry-run toggle when DEBUG enabled
    pub fn inputs_next(&mut self) {
        let max_index = if self.debug_enabled {
            self.fields.len()
        } else {
            self.fields.len().saturating_sub(1)
        };
        if self.field_idx < max_index {
            self.field_idx += 1;
        }
    }

    pub fn edit_current_field<F: FnOnce(&mut String)>(&mut self, f: F) {
        if self.field_idx >= self.fields.len() {
            return;
        }
        if let Some(field) = self.fields.get_mut(self.field_idx) {
            f(&mut field.value);
        }
    }

    pub fn cycle_enum_current(&mut self, delta: isize) {
        if self.field_idx >= self.fields.len() {
            return;
        }
        if let Some(field) = self.fields.get_mut(self.field_idx) {
            if field.enum_values.is_empty() {
                return;
            }
            if field.enum_idx.is_none() {
                field.enum_idx = Some(0);
            }
            let len = field.enum_values.len() as isize;
            let idx = field.enum_idx.unwrap() as isize;
            let next = (idx + delta).rem_euclid(len);
            field.enum_idx = Some(next as usize);
            field.value = field.enum_values[next as usize].clone();
        }
    }

    pub fn missing_required(&self) -> Vec<String> {
        self.fields
            .iter()
            .filter(|f| f.required && f.value.trim().is_empty() && !f.is_bool)
            .map(|f| f.name.clone())
            .collect()
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub fn run(&mut self) {
        if self.picked.is_none() {
            return;
        }
        let spec = self.picked.clone().unwrap();
        let mut pos_map = std::collections::HashMap::new();
        let mut body = serde_json::Map::new();

        // Fill from fields
        for f in &self.fields {
            if spec.positional_args.iter().any(|p| p == &f.name) {
                pos_map.insert(f.name.clone(), f.value.clone());
            } else {
                if f.is_bool {
                    // in this simple UI, non-empty toggles true
                    if !f.value.is_empty() {
                        body.insert(f.name.clone(), serde_json::Value::Bool(true));
                    }
                } else if !f.value.is_empty() {
                    body.insert(f.name.clone(), serde_json::Value::String(f.value.clone()));
                }
            }
        }
        let path = crate::preview::resolve_path(&spec.path, &pos_map);
        let cli = crate::preview::cli_preview(&spec, &self.fields);
        let req = crate::preview::request_preview(&spec, &path, &body);
        if self.dry_run {
            self.logs.push(format!("Dry-run:\n{}\n{}", cli, req));
        } else {
            self.logs.push(format!("Run (simulated):\n{}\n{}", cli, req));
            // DEBUG: populate a small sample JSON result for table preview on GET list-like
            if self.debug_enabled && spec.method == "GET" {
                // naive: if path ends with a collection (no trailing placeholder), show demo
                if !spec.path.ends_with('}') {
                    self.result_json = Some(crate::tables::sample_apps());
                    self.show_table = true; // prefer modal view by default when results are available
                    self.table_offset = 0;
                }
            }
        }
        if self.logs.len() > 500 {
            self.logs.drain(0..self.logs.len() - 500);
        }
    }

    fn rebuild_fields_for_current_selection(&mut self) {
        if self.filtered.is_empty() {
            self.picked = None;
            self.fields.clear();
            self.field_idx = 0;
            return;
        }
        let idx = self.filtered[self.selected];
        let spec = self.all_commands[idx].clone();
        self.picked = Some(spec.clone());
        self.fields.clear();
        // positional args first
        for pa in &spec.positional_args {
            self.fields.push(Field {
                name: pa.clone(),
                required: true,
                is_bool: false,
                value: String::new(),
                enum_values: vec![],
                enum_idx: None,
            });
        }
        // flags
        for f in &spec.flags {
            let mut field = Field {
                name: f.name.clone(),
                required: f.required,
                is_bool: f.r#type == "boolean",
                value: String::new(),
                enum_values: f.enum_values.clone(),
                enum_idx: None,
            };
            if !field.enum_values.is_empty() {
                if let Some(def) = f.default_value.clone() {
                    if let Some(pos) = field.enum_values.iter().position(|v| v == &def) {
                        field.enum_idx = Some(pos);
                        field.value = field.enum_values[pos].clone();
                    } else {
                        field.enum_idx = Some(0);
                        field.value = field.enum_values[0].clone();
                    }
                } else {
                    field.enum_idx = Some(0);
                    field.value = field.enum_values[0].clone();
                }
            } else if field.is_bool {
                if let Some(def) = &f.default_value {
                    if def == "true" || def == "1" {
                        field.value.push('x');
                    }
                }
            } else if let Some(def) = &f.default_value {
                field.value = def.clone();
            }
            self.fields.push(field);
        }
        self.field_idx = 0;
    }
}

#[derive(Debug)]
pub struct ExecOutcome {
    pub log: String,
    pub result_json: Option<serde_json::Value>,
    pub open_table: bool,
}

pub fn update(app: &mut App, msg: Msg) -> Option<Effect> {
    match msg {
        Msg::ToggleHelp => app.toggle_help(),
        Msg::ToggleTable => app.show_table = !app.show_table,
        Msg::ToggleBuilder => {
            if app.show_builder {
                app.show_builder = false;
            } else {
                app.show_builder = true;
                // Seed the builder's search with the palette's current command token
                let first = app.palette.input.split_whitespace().next().unwrap_or("");
                app.search = first.to_string();
                app.filter_commands();
                app.focus = Focus::Search;
                // Hide palette suggestions behind the modal
                app.palette.popup_open = false;
            }
        }
        Msg::CloseModal => {
            if app.show_help { app.toggle_help(); }
            if app.show_table { app.show_table = false; }
            if app.show_builder { app.show_builder = false; }
            app.help_spec = None;
        }
        Msg::FocusNext => {
            app.focus = match app.focus {
                Focus::Search => Focus::Commands,
                Focus::Commands => Focus::Inputs,
                Focus::Inputs => Focus::Search,
            };
        }
        Msg::FocusPrev => {
            app.focus = match app.focus {
                Focus::Search => Focus::Inputs,
                Focus::Commands => Focus::Search,
                Focus::Inputs => Focus::Commands,
            };
        }
        Msg::MoveSelection(d) => app.move_selection(d),
        Msg::Enter => match app.focus {
            Focus::Commands | Focus::Search => app.pick_current_command(),
            Focus::Inputs => app.run(),
        },
        Msg::SearchChar(c) => {
            app.search.push(c);
            app.filter_commands();
        }
        Msg::SearchBackspace => {
            app.search.pop();
            app.filter_commands();
        }
        Msg::SearchClear => {
            app.search.clear();
            app.filter_commands();
        }
        Msg::InputsUp => app.inputs_prev(),
        Msg::InputsDown => app.inputs_next(),
        Msg::InputsChar(c) => app.edit_current_field(|s| s.push(c)),
        Msg::InputsBackspace => app.edit_current_field(|s| {
            s.pop();
        }),
        Msg::InputsToggleSpace => {
            if app.debug_enabled && app.field_idx == app.fields.len() {
                app.dry_run = !app.dry_run;
            } else if let Some(field) = app.fields.get(app.field_idx) {
                if field.is_bool {
                    app.edit_current_field(|s| {
                        if s.is_empty() {
                            s.push('x');
                        } else {
                            s.clear();
                        }
                    });
                }
            }
        }
        Msg::InputsCycleLeft => app.cycle_enum_current(-1),
        Msg::InputsCycleRight => app.cycle_enum_current(1),
        Msg::Run => app.run(),
        Msg::CopyCommand => return Some(Effect::CopyCommandRequested),
        Msg::TableScroll(delta) => {
            // Basic vertical scrolling; clamp to non-negative
            let off = app.table_offset as isize + delta;
            app.table_offset = off.max(0) as usize;
        }
        Msg::TableHome => { app.table_offset = 0; }
        Msg::TableEnd => { app.table_offset = usize::MAX / 2; } // clamped during render
    }
    None
}
