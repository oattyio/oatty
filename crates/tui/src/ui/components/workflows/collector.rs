use crossterm::event::{KeyCode, KeyEvent};
use heroku_types::Effect;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::app::App;
use crate::ui::components::component::Component;
use crate::ui::components::workflows::{ProviderCacheSummary, WorkflowProviderSnapshot, format_cache_summary, format_preview};
use crate::ui::utils::centered_rect;
use heroku_engine::{BindingSource, ProviderBindingOutcome, WorkflowRunState};
use heroku_types::workflow::{WorkflowInputDefinition, WorkflowValueProvider};
use serde_json::Value as JsonValue;
use std::collections::HashSet;

const MAX_CANDIDATES: usize = 24;

#[derive(Clone, Debug)]
struct UnresolvedItem {
    input: String,
    argument: String,
    detail: String,
    source: Option<BindingSource>,
    required: bool,
    path: Option<String>,
    outcome: ProviderBindingOutcome,
}

#[derive(Clone, Debug)]
struct CandidateItem {
    label: String,
    value: JsonValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum CollectorFocus {
    #[default]
    UnresolvedList,
    Candidates,
    Manual,
}

impl CollectorFocus {
    fn next(self) -> Self {
        match self {
            CollectorFocus::UnresolvedList => CollectorFocus::Candidates,
            CollectorFocus::Candidates => CollectorFocus::Manual,
            CollectorFocus::Manual => CollectorFocus::UnresolvedList,
        }
    }

    fn previous(self) -> Self {
        match self {
            CollectorFocus::UnresolvedList => CollectorFocus::Manual,
            CollectorFocus::Candidates => CollectorFocus::UnresolvedList,
            CollectorFocus::Manual => CollectorFocus::Candidates,
        }
    }
}

/// Guided Input Collector modal implementation.
#[derive(Debug, Default)]
pub struct WorkflowCollectorComponent {
    /// Active free-text filter for unresolved inputs.
    filter_query: String,
    /// Manual input capture buffer.
    manual_value: String,
    /// Indicates that the manual buffer was edited by the user and should not be auto-overwritten.
    manual_dirty: bool,
    /// Currently selected unresolved item index (after filtering).
    selected_index: usize,
    /// Currently selected candidate within the detail pane.
    selected_candidate: usize,
    /// Focus area for keyboard routing.
    focus: CollectorFocus,
    /// When true, keystrokes modify the filter rather than control focus navigation.
    search_active: bool,
}

impl Component for WorkflowCollectorComponent {
    /// Handles key event inputs and updates the internal state of the collector or application accordingly.
    ///
    /// # Parameters
    /// - `app`: A mutable reference to the application state (`App`). Used to update or retrieve data from the app context.
    /// - `key`: The `KeyEvent` to be processed, representing the key input received by the application.
    ///
    /// # Returns
    /// A vector of `Effect` instances, representing actions or changes triggered by the key event.
    ///
    /// # Behavior
    /// - If `search_active` is `true`, calls `handle_filter_input` to handle the key input and returns its result.
    /// - Otherwise, processes the key input based on the pressed key and the current focus.
    /// - Specific key handling:
    ///   - `Esc`: Closes the modal and returns an `Effect::CloseModal`.
    ///   - `Tab`: Moves focus to the next item, synchronizes manual value with the app, and returns no effect.
    ///   - `Shift+Tab` (BackTab): Moves focus to the previous item, synchronizes manual value with the app, and returns no effect.
    ///   - `/` (when focus is on `CollectorFocus::UnresolvedList`): Activates search mode and clears the filter query.
    ///   - `r` or `R`: Evaluates input providers if applicable, logs errors if any, observes refresh events, and returns no effect.
    ///   - `F2`: Sets focus to `CollectorFocus::Manual`, disables search mode, and synchronizes manual value with the app.
    ///   - `Up`/`Down`: Calls `handle_vertical_navigation` to perform vertical navigation in the UI and returns the result.
    ///   - `Enter`: Handles the Enter key and performs appropriate actions by calling `handle_enter`.
    ///   - Other character keys (in `CollectorFocus::Manual`): Updates the manual value buffer (if non-control characters) and marks it as dirty.
    ///   - `Backspace` (in `CollectorFocus::Manual`): Removes the last character from the manual value buffer if not empty and marks it as dirty.
    ///   - Any unhandled keys: Returns an empty vector of effects.
    ///
    /// # Notes
    /// - The method dynamically switches behavior based on `search_active`, `focus`, and the provided `key.code`.
    /// - Internal methods such as `sync_manual_value`, `handle_filter_input`, `handle_vertical_navigation`, and `handle_enter` are used to process specific tasks.
    /// - Logging is performed when evaluating input providers (`r`/`R` key) if any errors occur.
    /// - UI focus and manual value states are managed depending on various key inputs.
    ///
    /// # Side Effects
    /// - Updates the `search_active`, `manual_value`, `manual_dirty`, and `focus` states as needed.
    /// - May modify the application state (`app`) as part of key handling operations.
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        if self.search_active {
            return self.handle_filter_input(key);
        }

        match key.code {
            KeyCode::Esc => vec![Effect::CloseModal],
            KeyCode::Tab => {
                self.focus = self.focus.next();
                self.sync_manual_value(app);
                Vec::new()
            }
            KeyCode::BackTab => {
                self.focus = self.focus.previous();
                self.sync_manual_value(app);
                Vec::new()
            }
            KeyCode::Char('/') if matches!(self.focus, CollectorFocus::UnresolvedList) => {
                self.search_active = true;
                self.filter_query.clear();
                Vec::new()
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                let mut refreshed = false;
                if let Some(state) = app.workflows.active_run_state_mut() {
                    if let Err(err) = state.evaluate_input_providers() {
                        app.logs.entries.push(format!("Provider evaluation error: {err}"));
                    } else {
                        refreshed = true;
                    }
                }
                if refreshed {
                    app.workflows.observe_provider_refresh_current();
                }
                Vec::new()
            }
            KeyCode::F(2) => {
                self.focus = CollectorFocus::Manual;
                self.search_active = false;
                self.sync_manual_value(app);
                Vec::new()
            }
            KeyCode::Up => self.handle_vertical_navigation(app, true),
            KeyCode::Down => self.handle_vertical_navigation(app, false),
            KeyCode::Enter => self.handle_enter(app),
            KeyCode::Char(c) if matches!(self.focus, CollectorFocus::Manual) && !c.is_control() => {
                self.manual_dirty = true;
                self.manual_value.push(c);
                Vec::new()
            }
            KeyCode::Backspace if matches!(self.focus, CollectorFocus::Manual) => {
                if !self.manual_value.is_empty() {
                    self.manual_value.pop();
                    self.manual_dirty = true;
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// Renders the modal interface for the Guided Input Collector.
    ///
    /// This function handles the rendering of various components of the Guided Input Collector
    /// on the terminal UI. It divides the modal into areas including a header, body, candidate list,
    /// manual input controls, and footer, and renders corresponding widgets into these areas.
    ///
    /// # Arguments
    ///
    /// * `frame` - A mutable reference to the `Frame`, representing the current terminal frame where
    ///   widgets are drawn.
    /// * `area` - A `Rect` object defining the area available for rendering.
    /// * `app` - A mutable reference to the `App` object, which provides context and state for rendering
    ///   and application logic.
    ///
    /// # Details
    ///
    /// 1. **Modal Layout**:
    ///    - Calculates the modal area based on proportions relative to the provided terminal area (`area`).
    ///    - Clears the modal's area before drawing contents to ensure no artifacts remain.
    ///
    /// 2. **Chunks Layout**:
    ///    - Divides the modal area into four vertical chunks for the header, body, candidate list with
    ///      manual input controls, and footer.
    ///    - Further splits the body chunk horizontally into two sections: unresolved items list (on the left)
    ///      and detailed information (on the right).
    ///
    /// 3. **ViewModel Construction and State Synchronization**:
    ///    - Builds a `CollectorViewModel` object based on the current application state.
    ///    - Ensures the selected indexes (`selected_index` for unresolved items
    ///      and `selected_candidate` for candidate items) are valid, resetting them if necessary.
    ///    - Calculates the unresolved item count for display in the header.
    ///
    /// 4. **Rendering Components**:
    ///    - **Header**:
    ///        - Displays the header with the workflow identifier and unresolved item count.
    ///        - Uses a bordered block with a formatted title.
    ///    - **Body**:
    ///        - Renders the list of unresolved items on the left (`render_unresolved_list`).
    ///        - Renders details of the selected item on the right (`render_details`).
    ///    - **Candidates & Manual Input**:
    ///        - Renders the candidate list alongside manual controls (`render_manual_and_candidates`).
    ///    - **Footer**:
    ///        - Provides keybinding hints using styled text enclosed within a bordered block.
    ///
    /// # Key Bindings Display in Footer:
    /// - `[Esc]`: Close modal.
    /// - `[Enter]`: Apply changes.
    /// - `[/]`: Filter items.
    /// - `[r]`: Refresh data.
    /// - `[Tab]`: Cycle focus between elements.
    /// - `[F2]`: Toggle manual input interface.
    ///
    /// This function ensures a responsive and organized layout for the Guided Input Collector,
    /// maintaining consistency and presenting state-related information to the user effectively.
    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        // Centered modal area proportions
        let modal_area = centered_rect(90, 80, area);

        // Clear area under the modal
        frame.render_widget(Clear, modal_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // header
                Constraint::Min(6),    // body
                Constraint::Length(5), // candidates + manual controls
                Constraint::Length(1), // footer
            ])
            .split(modal_area);

        let theme = &*app.ctx.theme;

        let mut view_model = CollectorViewModel::build(app, &self.filter_query, self.selected_index);

        if !view_model.unresolved_items.is_empty() {
            let max_index = view_model.unresolved_items.len() - 1;
            if self.selected_index > max_index {
                self.selected_index = max_index;
                view_model = CollectorViewModel::build(app, &self.filter_query, self.selected_index);
            }
        } else {
            self.selected_index = 0;
        }

        if view_model.candidate_items.is_empty() {
            self.selected_candidate = 0;
        } else if self.selected_candidate >= view_model.candidate_items.len() {
            self.selected_candidate = 0;
        }
        let unresolved_count = view_model.unresolved_items.len();

        let header_title = format!(
            "Guided Input Collector — {} — Unresolved: {}",
            view_model.workflow_identifier, unresolved_count
        );
        let header_block = Block::default().title(header_title).borders(Borders::ALL);
        frame.render_widget(header_block, chunks[0]);

        let body_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
            .split(chunks[1]);

        self.render_unresolved_list(frame, body_chunks[0], app, &view_model);
        self.render_details(frame, body_chunks[1], app, &view_model);
        self.render_manual_and_candidates(frame, chunks[2], app, &view_model);

        let footer = Paragraph::new(Line::from(vec![
            Span::styled("[Esc] Close  ", theme.text_secondary_style().add_modifier(Modifier::BOLD)),
            Span::raw("[Enter] Apply  [/] Filter  [r] Refresh  [Tab] Cycle focus  [F2] Manual"),
        ]))
        .block(Block::default().borders(Borders::ALL));
        frame.render_widget(footer, chunks[3]);
    }
}

impl WorkflowCollectorComponent {
    fn handle_filter_input(&mut self, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Esc => {
                self.search_active = false;
                Vec::new()
            }
            KeyCode::Enter => {
                self.search_active = false;
                self.manual_dirty = false;
                Vec::new()
            }
            KeyCode::Backspace => {
                self.filter_query.pop();
                self.selected_index = 0;
                self.selected_candidate = 0;
                self.manual_dirty = false;
                Vec::new()
            }
            KeyCode::Char(character) if !character.is_control() => {
                self.filter_query.push(character);
                self.selected_index = 0;
                self.selected_candidate = 0;
                self.manual_dirty = false;
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn handle_vertical_navigation(&mut self, app: &App, moving_up: bool) -> Vec<Effect> {
        let view_model = CollectorViewModel::build(app, &self.filter_query, self.selected_index);

        match self.focus {
            CollectorFocus::UnresolvedList => {
                let len = view_model.unresolved_items.len();
                if len == 0 {
                    return Vec::new();
                }
                if moving_up {
                    if self.selected_index == 0 {
                        self.selected_index = len.saturating_sub(1);
                    } else {
                        self.selected_index = self.selected_index.saturating_sub(1);
                    }
                } else {
                    self.selected_index = (self.selected_index + 1) % len;
                }
                self.selected_candidate = 0;
                self.manual_dirty = false;
                self.sync_manual_value(app);
            }
            CollectorFocus::Candidates => {
                let len = view_model.candidate_items.len();
                if len == 0 {
                    return Vec::new();
                }
                if moving_up {
                    if self.selected_candidate == 0 {
                        self.selected_candidate = len.saturating_sub(1);
                    } else {
                        self.selected_candidate = self.selected_candidate.saturating_sub(1);
                    }
                } else {
                    self.selected_candidate = (self.selected_candidate + 1) % len;
                }
            }
            CollectorFocus::Manual => {}
        }

        Vec::new()
    }

    fn handle_enter(&mut self, app: &mut App) -> Vec<Effect> {
        let view_model = CollectorViewModel::build(app, &self.filter_query, self.selected_index);

        match self.focus {
            CollectorFocus::UnresolvedList => {
                if view_model.unresolved_items.is_empty() {
                    return match app.execute_workflow_from_collector() {
                        Ok(effects) => effects,
                        Err(err) => {
                            app.logs.entries.push(format!("Workflow execution error: {err}"));
                            Vec::new()
                        }
                    };
                }

                if view_model.candidate_items.is_empty() {
                    self.focus = CollectorFocus::Manual;
                    self.sync_manual_value(app);
                } else {
                    self.focus = CollectorFocus::Candidates;
                }

                Vec::new()
            }
            CollectorFocus::Candidates => {
                if let Some(item) = view_model.unresolved_items.get(self.selected_index).cloned()
                    && let Some(candidate) = view_model.candidate_items.get(self.selected_candidate)
                {
                    return self.apply_value(app, &item, candidate.value.clone());
                }

                Vec::new()
            }
            CollectorFocus::Manual => {
                if let Some(item) = view_model.unresolved_items.get(self.selected_index).cloned() {
                    let value = JsonValue::String(self.manual_value.clone());
                    return self.apply_value(app, &item, value);
                }
                Vec::new()
            }
        }
    }

    fn sync_manual_value(&mut self, app: &App) {
        if self.manual_dirty {
            return;
        }

        let view_model = CollectorViewModel::build(app, &self.filter_query, self.selected_index);
        if let Some(item) = view_model.unresolved_items.get(self.selected_index) {
            if let Some(state) = app.workflows.active_run_state() {
                if let Some(value) = state.run_context.inputs.get(&item.input) {
                    self.manual_value = format_json_value(value);
                } else {
                    self.manual_value.clear();
                }
            }
        } else {
            self.manual_value.clear();
        }
    }

    fn apply_value(&mut self, app: &mut App, item: &UnresolvedItem, value: JsonValue) -> Vec<Effect> {
        {
            let Some(state) = app.workflows.active_run_state_mut() else {
                app.logs.entries.push("No workflow run state available".into());
                return Vec::new();
            };

            apply_value_to_state(state, item, value);

            if let Err(err) = state.evaluate_input_providers() {
                app.logs.entries.push(format!("Provider evaluation error: {err}"));
                return Vec::new();
            }
        }

        app.workflows.observe_provider_refresh_current();

        self.manual_dirty = false;
        self.sync_manual_value(app);

        let model_after = CollectorViewModel::build(app, &self.filter_query, self.selected_index);
        if model_after.unresolved_items.is_empty() {
            match app.execute_workflow_from_collector() {
                Ok(effects) => effects,
                Err(err) => {
                    app.logs.entries.push(format!("Workflow execution error: {err}"));
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        }
    }

    fn render_unresolved_list(&mut self, frame: &mut Frame, area: Rect, app: &App, view_model: &CollectorViewModel) {
        let theme = &*app.ctx.theme;
        let mut inner_lines = Vec::new();
        let highlight_index = self.selected_index.min(view_model.unresolved_items.len().saturating_sub(1));

        for (index, item) in view_model.unresolved_items.iter().enumerate() {
            let prefix = if index == highlight_index { "▸" } else { " " };
            let required_badge = if item.required { " [required]" } else { "" };
            let detail = format!(
                "{prefix} {input}.{argument}{required} — {detail}",
                prefix = prefix,
                input = item.input,
                argument = item.argument,
                required = required_badge,
                detail = item.detail
            );
            inner_lines.push(Line::from(Span::styled(detail, theme.text_primary_style())));
        }

        if inner_lines.is_empty() {
            inner_lines.push(Line::from(Span::styled(
                "All inputs resolved. Press Enter to run.",
                theme.text_secondary_style(),
            )));
        }

        let filter_line = if self.filter_query.is_empty() {
            Line::from(vec![
                Span::styled("Filter: ", theme.text_secondary_style()),
                Span::styled("[type / to search]", theme.text_muted_style()),
            ])
        } else {
            Line::from(vec![
                Span::styled("Filter: ", theme.text_secondary_style()),
                Span::styled(self.filter_query.clone(), theme.text_primary_style()),
            ])
        };

        let mut all_lines = Vec::with_capacity(inner_lines.len() + 2);
        all_lines.push(filter_line);
        all_lines.push(Line::from(Span::raw("")));
        all_lines.extend(inner_lines);

        let block = Block::default()
            .title("Unresolved Inputs")
            .borders(Borders::ALL)
            .border_style(theme.border_style(matches!(self.focus, CollectorFocus::UnresolvedList)));

        let paragraph = Paragraph::new(all_lines).wrap(Wrap { trim: true }).block(block);
        frame.render_widget(paragraph, area);
    }

    fn render_details(&self, frame: &mut Frame, area: Rect, app: &App, view_model: &CollectorViewModel) {
        let theme = &*app.ctx.theme;
        let block = Block::default()
            .title("Details")
            .borders(Borders::ALL)
            .border_style(theme.border_style(false));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if let Some(item) = view_model.unresolved_items.get(self.selected_index) {
            let provider_label = view_model.provider_label.as_deref().unwrap_or("-");
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Input: ", theme.text_secondary_style()),
                    Span::styled(&item.input, theme.text_primary_style()),
                ]),
                Line::from(vec![
                    Span::styled("Argument: ", theme.text_secondary_style()),
                    Span::styled(&item.argument, theme.text_primary_style()),
                ]),
                Line::from(vec![
                    Span::styled("Provider: ", theme.text_secondary_style()),
                    Span::styled(provider_label, theme.text_primary_style()),
                ]),
                Line::from(vec![
                    Span::styled("Reason: ", theme.text_secondary_style()),
                    Span::styled(&item.detail, theme.status_warning()),
                ]),
            ];

            if let Some(path) = &item.path {
                lines.push(Line::from(vec![
                    Span::styled("Path: ", theme.text_secondary_style()),
                    Span::styled(path, theme.text_primary_style()),
                ]));
            }

            if let Some(definition) = &view_model.selected_input_definition {
                if let Some(description) = definition.description.as_deref() {
                    lines.push(Line::from(Span::styled(description, theme.text_muted_style())));
                }
            }

            let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
            frame.render_widget(paragraph, inner);
        } else {
            let placeholder = Paragraph::new("Select an unresolved item or press Enter to run.")
                .style(theme.text_muted_style())
                .wrap(Wrap { trim: true });
            frame.render_widget(placeholder, inner);
        }
    }

    fn render_manual_and_candidates(&self, frame: &mut Frame, area: Rect, app: &App, view_model: &CollectorViewModel) {
        let theme = &*app.ctx.theme;
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(area);

        // Candidates panel
        let candidate_title = if let Some(summary) = &view_model.provider_snapshot {
            format!("Candidates — {}", format_cache_summary(summary))
        } else {
            "Candidates".to_string()
        };

        let candidate_block = Block::default()
            .title(candidate_title)
            .borders(Borders::ALL)
            .border_style(theme.border_style(matches!(self.focus, CollectorFocus::Candidates)));
        let candidate_inner = candidate_block.inner(layout[0]);
        frame.render_widget(candidate_block, layout[0]);

        if view_model.candidate_items.is_empty() {
            let message = Paragraph::new("No candidates available. Use manual entry.")
                .style(theme.text_muted_style())
                .wrap(Wrap { trim: true });
            frame.render_widget(message, candidate_inner);
        } else {
            let items: Vec<ListItem> = view_model
                .candidate_items
                .iter()
                .enumerate()
                .map(|(index, candidate)| {
                    let label = if index == self.selected_candidate {
                        Span::styled(candidate.label.clone(), theme.selection_style())
                    } else {
                        Span::styled(candidate.label.clone(), theme.text_primary_style())
                    };
                    ListItem::new(Line::from(label))
                })
                .collect();

            let mut list_state = ratatui::widgets::ListState::default();
            list_state.select(Some(self.selected_candidate.min(items.len().saturating_sub(1))));

            let list = List::new(items).highlight_symbol("▸ ");
            frame.render_stateful_widget(list, candidate_inner, &mut list_state);
        }

        // Manual entry panel
        let manual_block = Block::default()
            .title("Manual Entry")
            .borders(Borders::ALL)
            .border_style(theme.border_style(matches!(self.focus, CollectorFocus::Manual)));
        let manual_inner = manual_block.inner(layout[1]);
        frame.render_widget(manual_block, layout[1]);

        let manual_value_line = if self.manual_value.is_empty() {
            Line::from(Span::styled("<type value>", theme.text_muted_style()))
        } else {
            Line::from(Span::styled(self.manual_value.clone(), theme.text_primary_style()))
        };

        let mut manual_lines = vec![manual_value_line];
        if let Some(summary) = &view_model.provider_snapshot {
            manual_lines.push(Line::from(Span::styled(format_cache_summary(summary), theme.text_muted_style())));
        }

        let manual_paragraph = Paragraph::new(manual_lines).wrap(Wrap { trim: true });
        frame.render_widget(manual_paragraph, manual_inner);
    }
}

#[derive(Debug, Default)]
struct CollectorViewModel {
    workflow_identifier: String,
    unresolved_items: Vec<UnresolvedItem>,
    selected_input_definition: Option<WorkflowInputDefinition>,
    provider_label: Option<String>,
    candidate_items: Vec<CandidateItem>,
    provider_snapshot: Option<ProviderCacheSummary>,
}

impl CollectorViewModel {
    fn build(app: &App, filter: &str, selected_index: usize) -> Self {
        let mut model = Self::default();

        let Some(state) = app.workflows.active_run_state() else {
            return model;
        };

        model.workflow_identifier = state.workflow.identifier.clone();

        let filter_lower = filter.to_lowercase();
        let mut unresolved = Vec::new();

        for (input_name, _definition) in state.workflow.inputs.iter() {
            if let Some(provider_state) = state.provider_state_for(input_name) {
                for (argument_name, outcome_state) in provider_state.argument_outcomes.iter() {
                    match &outcome_state.outcome {
                        ProviderBindingOutcome::Prompt(prompt) => {
                            if !filter_matches(&filter_lower, input_name, argument_name, &prompt.reason.message) {
                                continue;
                            }
                            unresolved.push(UnresolvedItem {
                                input: input_name.clone(),
                                argument: argument_name.clone(),
                                detail: prompt.reason.message.clone(),
                                source: Some(prompt.source.clone()),
                                required: prompt.required,
                                path: prompt.reason.path.clone(),
                                outcome: ProviderBindingOutcome::Prompt(prompt.clone()),
                            });
                        }
                        ProviderBindingOutcome::Error(error) => {
                            if !filter_matches(&filter_lower, input_name, argument_name, &error.message) {
                                continue;
                            }
                            unresolved.push(UnresolvedItem {
                                input: input_name.clone(),
                                argument: argument_name.clone(),
                                detail: error.message.clone(),
                                source: error.source.clone(),
                                required: false,
                                path: None,
                                outcome: ProviderBindingOutcome::Error(error.clone()),
                            });
                        }
                        ProviderBindingOutcome::Skip(decision) => {
                            if !filter_matches(&filter_lower, input_name, argument_name, &decision.reason.message) {
                                continue;
                            }
                            unresolved.push(UnresolvedItem {
                                input: input_name.clone(),
                                argument: argument_name.clone(),
                                detail: decision.reason.message.clone(),
                                source: Some(decision.source.clone()),
                                required: false,
                                path: decision.reason.path.clone(),
                                outcome: ProviderBindingOutcome::Skip(decision.clone()),
                            });
                        }
                        ProviderBindingOutcome::Resolved(_) => {}
                    }
                }
            }
        }

        let selected_item = unresolved.get(selected_index).cloned();
        let selected_definition = selected_item
            .as_ref()
            .and_then(|item| state.workflow.inputs.get(&item.input).cloned());
        let provider_label = selected_definition
            .as_ref()
            .and_then(|definition| definition.provider.as_ref())
            .map(|provider| match provider {
                WorkflowValueProvider::Id(id) => id.clone(),
                WorkflowValueProvider::Detailed(detail) => detail.id.clone(),
            });
        let provider_cache_key = selected_item
            .as_ref()
            .and_then(|item| provider_label.as_ref().map(|label| format!("{}:{label}", item.input)));
        let provider_snapshot = provider_cache_key
            .as_deref()
            .and_then(|key| app.workflows.provider_snapshot(key))
            .map(|snapshot| ProviderCacheSummary::from_snapshot(snapshot));
        let candidate_items = selected_item
            .as_ref()
            .map(|item| collect_candidates(state, item))
            .unwrap_or_default();

        model.unresolved_items = unresolved;
        model.selected_input_definition = selected_definition;
        model.provider_label = provider_label;
        model.candidate_items = candidate_items;
        model.provider_snapshot = provider_snapshot;
        model
    }
}

fn filter_matches(filter: &str, input_name: &str, argument_name: &str, detail: &str) -> bool {
    if filter.is_empty() {
        return true;
    }

    let filter = filter.trim();
    if filter.is_empty() {
        return true;
    }

    let filter = filter.to_lowercase();
    let input_name = input_name.to_lowercase();
    let argument_name = argument_name.to_lowercase();
    let detail = detail.to_lowercase();

    input_name.contains(&filter) || argument_name.contains(&filter) || detail.contains(&filter)
}

fn format_json_value(value: &JsonValue) -> String {
    match value {
        JsonValue::String(text) => text.clone(),
        JsonValue::Number(number) => number.to_string(),
        JsonValue::Bool(boolean) => boolean.to_string(),
        JsonValue::Null => String::new(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| value.to_string()),
    }
}

fn apply_value_to_state(state: &mut WorkflowRunState, item: &UnresolvedItem, value: JsonValue) {
    let value_for_state = value.clone();

    match &item.source {
        Some(BindingSource::Input { input_name }) => {
            state.set_input_value(input_name, value_for_state.clone());
        }
        Some(BindingSource::Step { step_id }) => {
            state.run_context_mut().steps.insert(step_id.clone(), value_for_state.clone());
        }
        Some(BindingSource::Multiple { step_id, input_name }) => {
            state.set_input_value(input_name, value_for_state.clone());
            state.run_context_mut().steps.insert(step_id.clone(), value_for_state.clone());
        }
        None => {}
    }

    state.set_input_value(&item.input, value_for_state.clone());
    state.persist_provider_outcome(&item.input, &item.argument, ProviderBindingOutcome::Resolved(value_for_state));
}

fn collect_candidates(state: &WorkflowRunState, item: &UnresolvedItem) -> Vec<CandidateItem> {
    let mut candidates = Vec::new();
    let mut seen_values = HashSet::new();

    if let Some(existing) = state.run_context.inputs.get(&item.input) {
        push_candidate(
            format!("inputs.{} (current)", item.input),
            existing,
            &mut candidates,
            &mut seen_values,
        );
    }

    match &item.source {
        Some(BindingSource::Input { input_name }) => {
            if let Some(value) = state.run_context.inputs.get(input_name) {
                gather_json_candidates(&format!("inputs.{input_name}"), value, &mut candidates, &mut seen_values, 0);
            }
        }
        Some(BindingSource::Step { step_id }) => {
            if let Some(value) = state.run_context.steps.get(step_id) {
                gather_json_candidates(&format!("steps.{step_id}"), value, &mut candidates, &mut seen_values, 0);
            }
        }
        Some(BindingSource::Multiple { step_id, input_name }) => {
            if let Some(value) = state.run_context.inputs.get(input_name) {
                gather_json_candidates(&format!("inputs.{input_name}"), value, &mut candidates, &mut seen_values, 0);
            }
            if let Some(value) = state.run_context.steps.get(step_id) {
                gather_json_candidates(&format!("steps.{step_id}"), value, &mut candidates, &mut seen_values, 0);
            }
        }
        None => {}
    }

    if candidates.len() < MAX_CANDIDATES {
        for (input_name, value) in state.run_context.inputs.iter() {
            gather_json_candidates(&format!("inputs.{input_name}"), value, &mut candidates, &mut seen_values, 0);
            if candidates.len() >= MAX_CANDIDATES {
                break;
            }
        }
    }

    if candidates.len() < MAX_CANDIDATES {
        for (step_id, value) in state.run_context.steps.iter() {
            gather_json_candidates(&format!("steps.{step_id}"), value, &mut candidates, &mut seen_values, 0);
            if candidates.len() >= MAX_CANDIDATES {
                break;
            }
        }
    }

    candidates.truncate(MAX_CANDIDATES);
    candidates
}

fn gather_json_candidates(
    base_label: &str,
    value: &JsonValue,
    candidates: &mut Vec<CandidateItem>,
    seen: &mut HashSet<String>,
    depth: usize,
) {
    if candidates.len() >= MAX_CANDIDATES {
        return;
    }

    if depth > 3 {
        return;
    }

    match value {
        JsonValue::String(_) | JsonValue::Number(_) | JsonValue::Bool(_) => {
            push_candidate(base_label.to_string(), value, candidates, seen);
        }
        JsonValue::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                let label = format!("{base_label}[{index}]");
                gather_json_candidates(&label, item, candidates, seen, depth + 1);
                if candidates.len() >= MAX_CANDIDATES {
                    break;
                }
            }
        }
        JsonValue::Object(map) => {
            for (key, item_value) in map.iter() {
                let label = if base_label.is_empty() {
                    key.to_string()
                } else {
                    format!("{base_label}.{key}")
                };
                gather_json_candidates(&label, item_value, candidates, seen, depth + 1);
                if candidates.len() >= MAX_CANDIDATES {
                    break;
                }
            }
        }
        JsonValue::Null => {}
    }
}

fn push_candidate(label: String, value: &JsonValue, candidates: &mut Vec<CandidateItem>, seen: &mut HashSet<String>) {
    if candidates.len() >= MAX_CANDIDATES {
        return;
    }

    let fingerprint = serde_json::to_string(value).unwrap_or_default();
    if !seen.insert(format!("{label}|{fingerprint}")) {
        return;
    }

    let preview = format_preview(value);
    let display_label = if preview.is_empty() {
        label.clone()
    } else {
        format!("{label} → {preview}")
    };

    candidates.push(CandidateItem {
        label: display_label,
        value: value.clone(),
    });
}
