use crossterm::event::{KeyCode, KeyEvent};
use heroku_types::Effect;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::App;
use crate::ui::components::component::Component;
use crate::ui::components::table::{SelectableTableConfig, SelectableTableRow, SelectionMode, render_selectable_table};
use crate::ui::components::workflows::{
    FieldPickerPane, ProviderCacheSummary, WorkflowBindingTarget, format_cache_summary, format_preview, human_duration, summarize_values,
};
use crate::ui::theme::{roles::Theme, theme_helpers as th};
use crate::ui::utils::centered_rect;
use heroku_engine::{BindingSource, ProviderBindingOutcome, WorkflowRunState};
use heroku_types::{
    provider::{ProviderArgumentContract, ProviderFieldContract},
    workflow::{
        WorkflowInputDefinition, WorkflowInputMode, WorkflowMissingBehavior, WorkflowProviderArgumentValue, WorkflowProviderErrorPolicy,
        WorkflowValueProvider,
    },
};
use serde_json::Value as JsonValue;
use std::{collections::HashSet, time::Duration};

const MAX_CANDIDATES: usize = 24;

#[derive(Clone, Debug)]
struct UnresolvedItem {
    target: WorkflowBindingTarget,
    detail: String,
    path: Option<String>,
    outcome: ProviderBindingOutcome,
}

#[derive(Clone, Debug)]
struct CandidateItem {
    source_path: String,
    preview: String,
    value: JsonValue,
    metadata_badges: Vec<String>,
    is_selected: bool,
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
    /// Indicates whether the detail pane currently shows the inline field picker.
    field_picker_active: bool,
    /// Maintains navigation state for the inline field picker tree.
    field_picker_pane: FieldPickerPane,
    /// When true, typed characters update the picker filter instead of moving selection.
    field_picker_filter_active: bool,
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
            let effects = self.handle_filter_input(key);
            self.deactivate_field_picker(app);
            return effects;
        }

        if self.field_picker_active
            && matches!(self.focus, CollectorFocus::Candidates)
            && let Some(effects) = self.handle_field_picker_key(app, key)
        {
            return effects;
        }

        match key.code {
            KeyCode::Esc => {
                self.deactivate_field_picker(app);
                vec![Effect::CloseModal]
            }
            KeyCode::Tab => {
                self.focus = self.focus.next();
                if !matches!(self.focus, CollectorFocus::Candidates) {
                    self.deactivate_field_picker(app);
                }
                self.sync_manual_value(app);
                Vec::new()
            }
            KeyCode::BackTab => {
                self.focus = self.focus.previous();
                if !matches!(self.focus, CollectorFocus::Candidates) {
                    self.deactivate_field_picker(app);
                }
                self.sync_manual_value(app);
                Vec::new()
            }
            KeyCode::Char('/') if matches!(self.focus, CollectorFocus::UnresolvedList) => {
                self.search_active = true;
                self.filter_query.clear();
                self.deactivate_field_picker(app);
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
                    if self.field_picker_active {
                        let run_state = app.workflows.active_run_state();
                        self.field_picker_pane.sync_from_run_state(run_state);
                    }
                }
                Vec::new()
            }
            KeyCode::Char('f') | KeyCode::Char('F') if matches!(self.focus, CollectorFocus::Candidates) => {
                let view_model = CollectorViewModel::build(app, &self.filter_query, self.selected_index);
                if self.field_picker_active {
                    self.deactivate_field_picker(app);
                } else {
                    self.activate_field_picker(app, &view_model);
                }
                Vec::new()
            }
            KeyCode::F(2) => {
                self.focus = CollectorFocus::Manual;
                self.search_active = false;
                self.deactivate_field_picker(app);
                self.sync_manual_value(app);
                Vec::new()
            }
            KeyCode::Up => {
                let effects = self.handle_vertical_navigation(app, true);
                if matches!(self.focus, CollectorFocus::UnresolvedList) {
                    self.deactivate_field_picker(app);
                }
                effects
            }
            KeyCode::Down => {
                let effects = self.handle_vertical_navigation(app, false);
                if matches!(self.focus, CollectorFocus::UnresolvedList) {
                    self.deactivate_field_picker(app);
                }
                effects
            }
            KeyCode::Enter => self.handle_enter(app),
            KeyCode::Char(' ') if matches!(self.focus, CollectorFocus::Candidates) => {
                if self.field_picker_active {
                    return Vec::new();
                }
                let view_model = CollectorViewModel::build(app, &self.filter_query, self.selected_index);
                if matches!(view_model.selection_mode, WorkflowInputMode::Multiple)
                    && !view_model.candidate_items.is_empty()
                    && let (Some(item), Some(candidate)) = (
                        view_model.unresolved_items.get(self.selected_index).cloned(),
                        view_model.candidate_items.get(self.selected_candidate),
                    )
                {
                    let mut values = view_model.selected_values.clone();
                    if candidate.is_selected {
                        values.retain(|existing| existing != &candidate.value);
                    } else if !values.iter().any(|existing| existing == &candidate.value) {
                        values.push(candidate.value.clone());
                    }

                    let new_value = JsonValue::Array(values.clone());
                    if let Err(err) = app.workflows.apply_binding_value(&item.target, new_value) {
                        app.logs.entries.push(format!("Provider evaluation error: {err}"));
                    } else {
                        return self.finalize_binding_update(app);
                    }
                }
                Vec::new()
            }
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

        if view_model.candidate_items.is_empty() || self.selected_candidate >= view_model.candidate_items.len() {
            self.selected_candidate = 0;
        }
        let unresolved_count = view_model.unresolved_items.len();

        let header_title = format!(
            "Guided Input Collector — {} — Unresolved: {}",
            view_model.workflow_identifier, unresolved_count
        );
        let header_block = Block::default().title(header_title).borders(Borders::ALL);
        let inner = header_block.inner(modal_area);
        frame.render_widget(header_block, modal_area);

        let chunks = Layout::vertical([
            Constraint::Min(6),    // body
            Constraint::Length(5), // candidates + manual controls
            Constraint::Length(1), // footer
        ])
        .split(inner);

        let body_chunks = Layout::horizontal([Constraint::Percentage(42), Constraint::Percentage(58)]).split(chunks[0]);

        self.render_unresolved_list(frame, body_chunks[0], app, &view_model);
        self.render_details(frame, body_chunks[1], app, &view_model);
        self.render_manual_and_candidates(frame, chunks[1], app, &view_model);

        let footer = Paragraph::new(Line::from(vec![
            Span::styled("[Esc] Close  ", theme.text_secondary_style().add_modifier(Modifier::BOLD)),
            Span::raw("[Enter] Apply  [/] Filter  [r] Refresh  [f] Field picker  [Tab] Cycle focus  [F2] Manual"),
        ]))
        .block(Block::default().borders(Borders::ALL));
        frame.render_widget(footer, chunks[2]);
    }
}

impl WorkflowCollectorComponent {
    fn handle_field_picker_key(&mut self, app: &mut App, key: KeyEvent) -> Option<Vec<Effect>> {
        use KeyCode::*;

        if self.field_picker_filter_active {
            return match key.code {
                Esc => {
                    self.field_picker_filter_active = false;
                    Some(Vec::new())
                }
                Enter => {
                    self.field_picker_filter_active = false;
                    Some(self.apply_field_picker_selection(app))
                }
                Backspace => {
                    self.field_picker_pane.pop_filter_char();
                    Some(Vec::new())
                }
                Char('/') => {
                    self.field_picker_pane.clear_filter();
                    Some(Vec::new())
                }
                Char(character) if !character.is_control() => {
                    self.field_picker_pane.push_filter_char(character);
                    Some(Vec::new())
                }
                _ => Some(Vec::new()),
            };
        }

        match key.code {
            Esc => {
                self.deactivate_field_picker(app);
                Some(Vec::new())
            }
            Up => {
                self.field_picker_pane.select_prev();
                Some(Vec::new())
            }
            Down => {
                self.field_picker_pane.select_next();
                Some(Vec::new())
            }
            Left => {
                self.field_picker_pane.collapse_selected();
                Some(Vec::new())
            }
            Right => {
                self.field_picker_pane.expand_selected();
                Some(Vec::new())
            }
            Enter => Some(self.apply_field_picker_selection(app)),
            Char('/') => {
                self.field_picker_filter_active = true;
                self.field_picker_pane.clear_filter();
                Some(Vec::new())
            }
            Char(character) if !character.is_control() => {
                self.field_picker_filter_active = true;
                self.field_picker_pane.clear_filter();
                self.field_picker_pane.push_filter_char(character);
                Some(Vec::new())
            }
            Char('f') | Char('F') => {
                self.deactivate_field_picker(app);
                Some(Vec::new())
            }
            _ => None,
        }
    }

    fn activate_field_picker(&mut self, app: &mut App, view_model: &CollectorViewModel) {
        if let Some(item) = view_model.unresolved_items.get(self.selected_index) {
            self.field_picker_active = true;
            self.field_picker_filter_active = false;
            self.field_picker_pane.reset();
            app.workflows.set_field_picker_target(item.target.clone());
            let run_state = app.workflows.active_run_state();
            self.field_picker_pane.sync_from_run_state(run_state);
        }
    }

    fn deactivate_field_picker(&mut self, app: &mut App) {
        if !self.field_picker_active {
            return;
        }
        self.field_picker_active = false;
        self.field_picker_filter_active = false;
        self.field_picker_pane.reset();
        app.workflows.clear_field_picker_target();
    }

    fn apply_field_picker_selection(&mut self, app: &mut App) -> Vec<Effect> {
        let Some(value) = self.field_picker_pane.current_value() else {
            return Vec::new();
        };

        if let Err(err) = app.workflows.apply_field_picker_value(value) {
            app.logs.entries.push(format!("Failed to apply value from field picker: {err}"));
            return Vec::new();
        }

        self.deactivate_field_picker(app);
        self.finalize_binding_update(app)
    }

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
                if self.field_picker_active {
                    if moving_up {
                        self.field_picker_pane.select_prev();
                    } else {
                        self.field_picker_pane.select_next();
                    }
                    return Vec::new();
                }
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
                if self.field_picker_active {
                    return self.apply_field_picker_selection(app);
                }
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
                if let Some(value) = state.run_context.inputs.get(&item.target.input) {
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
        if let Err(err) = app.workflows.apply_binding_value(&item.target, value) {
            app.logs.entries.push(format!("Provider evaluation error: {err}"));
            return Vec::new();
        }
        self.finalize_binding_update(app)
    }

    fn render_unresolved_list(&mut self, frame: &mut Frame, area: Rect, app: &App, view_model: &CollectorViewModel) {
        let theme = &*app.ctx.theme;
        let mut inner_lines = Vec::new();
        let highlight_index = self.selected_index.min(view_model.unresolved_items.len().saturating_sub(1));

        for (index, item) in view_model.unresolved_items.iter().enumerate() {
            let prefix = if index == highlight_index { "▸" } else { " " };
            let required_badge = if item.target.required { " [required]" } else { "" };
            let detail = format!(
                "{prefix} {input}.{argument}{required} — {detail}",
                prefix = prefix,
                input = item.target.input,
                argument = item.target.argument,
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
        let block = th::block(theme, Some("Details"), false);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let Some(item) = view_model.unresolved_items.get(self.selected_index) else {
            let placeholder = Paragraph::new("Select an unresolved item or press Enter to run.")
                .style(theme.text_muted_style())
                .wrap(Wrap { trim: true });
            frame.render_widget(placeholder, inner);
            return;
        };

        let provider_label = view_model.provider_label.as_deref().unwrap_or("-");
        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from(vec![
            Span::styled("Input: ", theme.text_secondary_style()),
            Span::styled(&item.target.input, theme.text_primary_style().add_modifier(Modifier::BOLD)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Argument: ", theme.text_secondary_style()),
            Span::styled(&item.target.argument, theme.text_primary_style()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Provider: ", theme.text_secondary_style()),
            Span::styled(provider_label, theme.text_primary_style()),
        ]));

        if let Some(source) = binding_source_label(&item.target) {
            lines.push(Line::from(vec![
                Span::styled("Source: ", theme.text_secondary_style()),
                Span::styled(source, theme.text_primary_style()),
            ]));
        }

        lines.push(Line::from(vec![
            Span::styled("Required: ", theme.text_secondary_style()),
            Span::styled(if item.target.required { "yes" } else { "no" }, theme.text_primary_style()),
        ]));

        let (status_label, status_style) = describe_outcome(theme, &item.outcome, &item.detail);
        lines.push(Line::from(vec![
            Span::styled("Status: ", theme.text_secondary_style()),
            Span::styled(status_label, status_style),
        ]));

        if let Some(path) = &item.path {
            lines.push(Line::from(vec![
                Span::styled("Path: ", theme.text_secondary_style()),
                Span::styled(path, theme.text_primary_style()),
            ]));
        }

        if let Some(snapshot) = &view_model.provider_snapshot {
            lines.push(Line::from(vec![
                Span::styled("Cache: ", theme.text_secondary_style()),
                Span::styled(format_cache_summary(snapshot), theme.text_primary_style()),
            ]));
        }

        if !view_model.selected_value_labels.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Selected: ", theme.text_secondary_style()),
                Span::styled(view_model.selected_value_labels.join(", "), theme.text_primary_style()),
            ]));
        }

        if let Some(definition) = &view_model.selected_input_definition {
            lines.push(Line::from(Span::raw("")));

            if let Some(description) = definition.description.as_deref() {
                lines.push(Line::from(Span::styled(description, theme.text_muted_style())));
            }

            lines.push(Line::from(vec![
                Span::styled("Selection mode: ", theme.text_secondary_style()),
                Span::styled(selection_mode_label(view_model.selection_mode), theme.text_primary_style()),
            ]));

            if let Some(type_hint) = definition.r#type.as_deref() {
                lines.push(Line::from(vec![
                    Span::styled("Type: ", theme.text_secondary_style()),
                    Span::styled(type_hint, theme.text_primary_style()),
                ]));
            }

            if let Some(ttl) = definition.cache_ttl_sec {
                lines.push(Line::from(vec![
                    Span::styled("Cache TTL: ", theme.text_secondary_style()),
                    Span::styled(human_duration(Duration::from_secs(ttl)), theme.text_primary_style()),
                ]));
            }

            if let Some(placeholder) = definition.placeholder.as_deref() {
                lines.push(Line::from(vec![
                    Span::styled("Placeholder: ", theme.text_secondary_style()),
                    Span::styled(placeholder, theme.text_primary_style()),
                ]));
            }

            if let Some(select) = definition.select.as_ref() {
                let mut select_parts = Vec::new();
                if let Some(value) = select.value_field.as_deref() {
                    select_parts.push(format!("value={value}"));
                }
                if let Some(display) = select.display_field.as_deref() {
                    select_parts.push(format!("display={display}"));
                }
                if let Some(id_field) = select.id_field.as_deref() {
                    select_parts.push(format!("id={id_field}"));
                }
                if !select_parts.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("Select: ", theme.text_secondary_style()),
                        Span::styled(select_parts.join(" • "), theme.text_primary_style()),
                    ]));
                }
            }

            if let Some(validate) = definition.validate.as_ref() {
                lines.push(Line::from(vec![
                    Span::styled("Required by schema: ", theme.text_secondary_style()),
                    Span::styled(if validate.required { "yes" } else { "no" }, theme.text_primary_style()),
                ]));

                if !validate.allowed_values.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("Allowed: ", theme.text_secondary_style()),
                        Span::styled(summarize_values(&validate.allowed_values, 6), theme.text_primary_style()),
                    ]));
                }

                if let Some(pattern) = validate.pattern.as_deref() {
                    lines.push(Line::from(vec![
                        Span::styled("Pattern: ", theme.text_secondary_style()),
                        Span::styled(pattern, theme.text_primary_style()),
                    ]));
                }

                let mut length_parts = Vec::new();
                if let Some(min) = validate.min_length {
                    length_parts.push(format!("min {min}"));
                }
                if let Some(max) = validate.max_length {
                    length_parts.push(format!("max {max}"));
                }
                if !length_parts.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("Length: ", theme.text_secondary_style()),
                        Span::styled(length_parts.join(" • "), theme.text_primary_style()),
                    ]));
                }
            }

            if !definition.enumerated_values.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("Enumerated: ", theme.text_secondary_style()),
                    Span::styled(summarize_values(&definition.enumerated_values, 6), theme.text_primary_style()),
                ]));
            }

            if let Some(policy) = definition.on_error.as_ref() {
                lines.push(Line::from(vec![
                    Span::styled("on_error: ", theme.text_secondary_style()),
                    Span::styled(
                        match policy {
                            WorkflowProviderErrorPolicy::Manual => "manual",
                            WorkflowProviderErrorPolicy::Cached => "cached",
                            WorkflowProviderErrorPolicy::Fail => "fail",
                        },
                        theme.text_primary_style(),
                    ),
                ]));
            }

            if !definition.provider_args.is_empty() {
                let args = describe_provider_args(definition.provider_args.iter());
                if !args.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("Provider args: ", theme.text_secondary_style()),
                        Span::styled(args.join(" • "), theme.text_primary_style()),
                    ]));
                }
            }
        }

        if !view_model.provider_argument_contracts.is_empty() {
            lines.push(Line::from(Span::raw("")));
            lines.push(Line::from(vec![Span::styled(
                "Argument contract:",
                theme.text_secondary_style().add_modifier(Modifier::BOLD),
            )]));
            for contract in &view_model.provider_argument_contracts {
                let text = format_argument_contract(contract);
                lines.push(Line::from(Span::styled(text, theme.text_primary_style())));
            }
        }

        if !view_model.provider_return_fields.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                "Return fields:",
                theme.text_secondary_style().add_modifier(Modifier::BOLD),
            )]));
            for field in &view_model.provider_return_fields {
                let text = format_return_field(field);
                lines.push(Line::from(Span::styled(text, theme.text_primary_style())));
            }
        }

        if matches!(self.focus, CollectorFocus::Candidates) {
            lines.push(Line::from(Span::raw("")));
            let hint = if self.field_picker_active {
                "Field picker active — use ↑/↓ to browse, Enter to apply, Esc to return to candidates."
            } else {
                "Tip: press [f] to open the field picker for context browsing."
            };
            lines.push(Line::from(Span::styled(hint, theme.text_muted_style())));
        }

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, inner);
    }

    fn render_manual_and_candidates(&mut self, frame: &mut Frame, area: Rect, app: &App, view_model: &CollectorViewModel) {
        let theme = &*app.ctx.theme;
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(area);
        let candidate_area = split[0];
        let manual_area = split[1];

        if self.field_picker_active {
            let run_state = app.workflows.active_run_state();
            self.field_picker_pane.sync_from_run_state(run_state);
            let target = app.workflows.field_picker_target();
            render_field_picker_panel(
                frame,
                candidate_area,
                theme,
                &self.field_picker_pane,
                target,
                matches!(self.focus, CollectorFocus::Candidates),
                self.field_picker_filter_active,
            );
        } else {
            render_candidate_table(
                frame,
                candidate_area,
                theme,
                view_model,
                matches!(self.focus, CollectorFocus::Candidates),
                self.selected_candidate,
            );
        }

        render_manual_panel(
            frame,
            manual_area,
            theme,
            view_model,
            matches!(self.focus, CollectorFocus::Manual),
            &self.manual_value,
        );
    }

    fn finalize_binding_update(&mut self, app: &mut App) -> Vec<Effect> {
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
}

fn render_candidate_table(
    frame: &mut Frame,
    area: Rect,
    theme: &dyn Theme,
    view_model: &CollectorViewModel,
    focused: bool,
    selected_index: usize,
) {
    let title = view_model
        .provider_label
        .as_deref()
        .map(|label| format!("Candidates ({label})"))
        .unwrap_or_else(|| "Candidates".to_string());
    let mut config = SelectableTableConfig::new(
        title,
        vec!["Source".to_string(), "Preview".to_string()],
        selection_mode_for_input(view_model.selection_mode),
    );

    config.status_badge = view_model.provider_snapshot.as_ref().map(format_cache_summary);
    config.selected_labels = view_model.selected_value_labels.clone();
    config.metadata_title = Some("Why".to_string());
    config.focused = focused;
    let highlight = if view_model.candidate_items.is_empty() {
        None
    } else {
        Some(selected_index.min(view_model.candidate_items.len().saturating_sub(1)))
    };
    config.highlight_index = highlight;

    config.rows = view_model
        .candidate_items
        .iter()
        .map(|candidate| {
            SelectableTableRow::new(
                vec![candidate.source_path.clone(), candidate.preview.clone()],
                candidate.metadata_badges.clone(),
                candidate.is_selected,
            )
        })
        .collect();

    render_selectable_table(frame, area, &config, theme);
}

fn render_field_picker_panel(
    frame: &mut Frame,
    area: Rect,
    theme: &dyn Theme,
    pane: &FieldPickerPane,
    target: Option<&WorkflowBindingTarget>,
    focused: bool,
    filter_active: bool,
) {
    let title = target
        .map(|t| format!("Field Picker — {}.{}", t.input, t.argument))
        .unwrap_or_else(|| "Field Picker".to_string());
    let block = th::block(theme, Some(&title), focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    pane.render_inline(frame, inner, theme, target, filter_active);
}

fn render_manual_panel(
    frame: &mut Frame,
    area: Rect,
    theme: &dyn Theme,
    view_model: &CollectorViewModel,
    focused: bool,
    manual_value: &str,
) {
    let block = th::block(theme, Some("Manual Entry"), focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    if manual_value.is_empty() {
        lines.push(Line::from(Span::styled("<type value>", theme.text_muted_style())));
    } else {
        lines.push(Line::from(Span::styled(manual_value.to_string(), theme.text_primary_style())));
    }

    if let Some(policy) = &view_model.provider_error_policy {
        let policy_text = match policy {
            WorkflowProviderErrorPolicy::Manual => "on_error: manual (always allow entry)",
            WorkflowProviderErrorPolicy::Cached => "on_error: cached (reuse stored values)",
            WorkflowProviderErrorPolicy::Fail => "on_error: fail (halt on error)",
        };
        lines.push(Line::from(Span::styled(policy_text, theme.text_secondary_style())));
    }

    if let Some(summary) = &view_model.provider_snapshot {
        lines.push(Line::from(Span::styled(format_cache_summary(summary), theme.text_muted_style())));
    }

    lines.push(Line::from(Span::styled(
        "[F2] edit • [r] retry provider • [Esc] close",
        theme.text_muted_style(),
    )));

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn selection_mode_for_input(mode: WorkflowInputMode) -> SelectionMode {
    match mode {
        WorkflowInputMode::Single => SelectionMode::Single,
        WorkflowInputMode::Multiple => SelectionMode::Multiple,
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
    selection_mode: WorkflowInputMode,
    selected_values: Vec<JsonValue>,
    selected_value_labels: Vec<String>,
    provider_error_policy: Option<WorkflowProviderErrorPolicy>,
    provider_argument_contracts: Vec<ProviderArgumentContract>,
    provider_return_fields: Vec<ProviderFieldContract>,
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
                                target: WorkflowBindingTarget {
                                    input: input_name.clone(),
                                    argument: argument_name.clone(),
                                    source: Some(prompt.source.clone()),
                                    required: prompt.required,
                                },
                                detail: prompt.reason.message.clone(),
                                path: prompt.reason.path.clone(),
                                outcome: ProviderBindingOutcome::Prompt(prompt.clone()),
                            });
                        }
                        ProviderBindingOutcome::Error(error) => {
                            if !filter_matches(&filter_lower, input_name, argument_name, &error.message) {
                                continue;
                            }
                            unresolved.push(UnresolvedItem {
                                target: WorkflowBindingTarget {
                                    input: input_name.clone(),
                                    argument: argument_name.clone(),
                                    source: error.source.clone(),
                                    required: false,
                                },
                                detail: error.message.clone(),
                                path: None,
                                outcome: ProviderBindingOutcome::Error(error.clone()),
                            });
                        }
                        ProviderBindingOutcome::Skip(decision) => {
                            if !filter_matches(&filter_lower, input_name, argument_name, &decision.reason.message) {
                                continue;
                            }
                            unresolved.push(UnresolvedItem {
                                target: WorkflowBindingTarget {
                                    input: input_name.clone(),
                                    argument: argument_name.clone(),
                                    source: Some(decision.source.clone()),
                                    required: false,
                                },
                                detail: decision.reason.message.clone(),
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
            .and_then(|item| state.workflow.inputs.get(&item.target.input).cloned());
        let provider_label = selected_definition
            .as_ref()
            .and_then(|definition| definition.provider.as_ref())
            .map(|provider| match provider {
                WorkflowValueProvider::Id(id) => id.clone(),
                WorkflowValueProvider::Detailed(detail) => detail.id.clone(),
            });
        let provider_contract = if let Some(label) = provider_label.as_ref() {
            match app.ctx.registry.lock() {
                Ok(registry) => registry.provider_contracts.get(label).cloned(),
                Err(_) => None,
            }
        } else {
            None
        };
        let provider_cache_key = selected_item
            .as_ref()
            .and_then(|item| provider_label.as_ref().map(|label| format!("{}:{label}", item.target.input)));
        let provider_snapshot = provider_cache_key
            .as_deref()
            .and_then(|key| app.workflows.provider_snapshot(key))
            .map(ProviderCacheSummary::from_snapshot);
        let selection_mode = selected_definition
            .as_ref()
            .map(|definition| definition.mode)
            .unwrap_or(WorkflowInputMode::Single);
        let selected_values = selected_item
            .as_ref()
            .and_then(|item| state.run_context.inputs.get(&item.target.input))
            .map(|value| normalize_selected_values(value, selection_mode))
            .unwrap_or_default();
        let selected_value_labels = selected_values.iter().map(format_preview).collect::<Vec<_>>();
        let candidate_items = selected_item
            .as_ref()
            .map(|item| collect_candidates(state, item, &selected_values))
            .unwrap_or_default();
        let provider_error_policy = selected_definition.as_ref().and_then(|definition| definition.on_error.clone());

        model.unresolved_items = unresolved;
        model.selected_input_definition = selected_definition;
        model.provider_label = provider_label;
        model.candidate_items = candidate_items;
        model.provider_snapshot = provider_snapshot;
        model.selection_mode = selection_mode;
        model.selected_values = selected_values;
        model.selected_value_labels = selected_value_labels;
        model.provider_error_policy = provider_error_policy;
        if let Some(contract) = provider_contract {
            model.provider_argument_contracts = contract.arguments;
            model.provider_return_fields = contract.returns.fields;
        }
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

fn normalize_selected_values(value: &JsonValue, mode: WorkflowInputMode) -> Vec<JsonValue> {
    match mode {
        WorkflowInputMode::Single => {
            if value.is_null() {
                Vec::new()
            } else {
                vec![value.clone()]
            }
        }
        WorkflowInputMode::Multiple => match value {
            JsonValue::Array(items) => items.clone(),
            JsonValue::Null => Vec::new(),
            other => vec![other.clone()],
        },
    }
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

fn collect_candidates(state: &WorkflowRunState, item: &UnresolvedItem, selected_values: &[JsonValue]) -> Vec<CandidateItem> {
    let mut accumulator = CandidateAccumulator::new(selected_values);

    if let Some(existing) = state.run_context.inputs.get(&item.target.input) {
        accumulator.push_with_metadata(
            format!("inputs.{} (current)", item.target.input),
            existing,
            vec!["current".to_string(), format!("source: inputs.{}", item.target.input)],
        );
    }

    match &item.target.source {
        Some(BindingSource::Input { input_name }) => {
            if let Some(value) = state.run_context.inputs.get(input_name) {
                accumulator.gather_nested(&format!("inputs.{input_name}"), value, 0, &[format!("source: inputs.{input_name}")]);
            }
        }
        Some(BindingSource::Step { step_id }) => {
            if let Some(value) = state.run_context.steps.get(step_id) {
                accumulator.gather_nested(&format!("steps.{step_id}"), value, 0, &[format!("source: steps.{step_id}")]);
            }
        }
        Some(BindingSource::Multiple { step_id, input_name }) => {
            if let Some(value) = state.run_context.inputs.get(input_name) {
                accumulator.gather_nested(&format!("inputs.{input_name}"), value, 0, &[format!("source: inputs.{input_name}")]);
            }
            if let Some(value) = state.run_context.steps.get(step_id) {
                accumulator.gather_nested(&format!("steps.{step_id}"), value, 0, &[format!("source: steps.{step_id}")]);
            }
        }
        None => {}
    }

    if accumulator.len() < MAX_CANDIDATES {
        for (input_name, value) in state.run_context.inputs.iter() {
            accumulator.gather_nested(&format!("inputs.{input_name}"), value, 0, &[format!("source: inputs.{input_name}")]);
            if accumulator.len() >= MAX_CANDIDATES {
                break;
            }
        }
    }

    if accumulator.len() < MAX_CANDIDATES {
        for (step_id, value) in state.run_context.steps.iter() {
            accumulator.gather_nested(&format!("steps.{step_id}"), value, 0, &[format!("source: steps.{step_id}")]);
            if accumulator.len() >= MAX_CANDIDATES {
                break;
            }
        }
    }

    accumulator.into_items()
}

struct CandidateAccumulator<'a> {
    items: Vec<CandidateItem>,
    seen: HashSet<String>,
    selected_values: &'a [JsonValue],
}

impl<'a> CandidateAccumulator<'a> {
    fn new(selected_values: &'a [JsonValue]) -> Self {
        Self {
            items: Vec::new(),
            seen: HashSet::new(),
            selected_values,
        }
    }

    fn len(&self) -> usize {
        self.items.len()
    }

    fn into_items(mut self) -> Vec<CandidateItem> {
        self.items.truncate(MAX_CANDIDATES);
        self.items
    }

    fn gather_nested(&mut self, base_label: &str, value: &JsonValue, depth: usize, metadata: &[String]) {
        if self.items.len() >= MAX_CANDIDATES || depth > 3 {
            return;
        }

        match value {
            JsonValue::String(_) | JsonValue::Number(_) | JsonValue::Bool(_) => {
                self.push_with_metadata(base_label.to_string(), value, metadata.to_vec());
            }
            JsonValue::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    let label = format!("{base_label}[{index}]");
                    self.gather_nested(&label, item, depth + 1, metadata);
                    if self.items.len() >= MAX_CANDIDATES {
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
                    self.gather_nested(&label, item_value, depth + 1, metadata);
                    if self.items.len() >= MAX_CANDIDATES {
                        break;
                    }
                }
            }
            JsonValue::Null => {}
        }
    }

    fn push_with_metadata(&mut self, source_path: String, value: &JsonValue, mut metadata_badges: Vec<String>) {
        if self.items.len() >= MAX_CANDIDATES {
            return;
        }

        let fingerprint = serde_json::to_string(value).unwrap_or_default();
        if !self.seen.insert(format!("{source_path}|{fingerprint}")) {
            return;
        }

        if !metadata_badges.iter().any(|badge| badge.starts_with("type:")) {
            metadata_badges.push(value_type_label(value));
        }

        let preview = format_preview(value);
        let is_selected = self.selected_values.iter().any(|selected| selected == value);
        self.items.push(CandidateItem {
            source_path,
            preview,
            value: value.clone(),
            metadata_badges,
            is_selected,
        });
    }
}

fn value_type_label(value: &JsonValue) -> String {
    let label = match value {
        JsonValue::String(_) => "type: string",
        JsonValue::Number(_) => "type: number",
        JsonValue::Bool(_) => "type: bool",
        JsonValue::Array(_) => "type: array",
        JsonValue::Object(_) => "type: object",
        JsonValue::Null => "type: null",
    };
    label.to_string()
}

fn binding_source_label(target: &WorkflowBindingTarget) -> Option<String> {
    let source = target.source.as_ref()?;
    let label = match source {
        BindingSource::Input { input_name } => format!("inputs.{input_name}"),
        BindingSource::Step { step_id } => format!("steps.{step_id}"),
        BindingSource::Multiple { step_id, input_name } => format!("steps.{step_id} + inputs.{input_name}"),
    };
    Some(label)
}

fn describe_outcome(theme: &dyn Theme, outcome: &ProviderBindingOutcome, detail: &str) -> (String, Style) {
    let (label, style) = match outcome {
        ProviderBindingOutcome::Prompt(_) => ("Requires input", theme.status_warning()),
        ProviderBindingOutcome::Error(_) => ("Provider error", theme.status_error()),
        ProviderBindingOutcome::Skip(_) => ("Skipped", theme.status_warning()),
        ProviderBindingOutcome::Resolved(_) => ("Resolved", theme.status_success()),
    };
    let message = if detail.is_empty() {
        label.to_string()
    } else {
        format!("{label}: {detail}")
    };
    (message, style)
}

fn selection_mode_label(mode: WorkflowInputMode) -> &'static str {
    match mode {
        WorkflowInputMode::Single => "single value",
        WorkflowInputMode::Multiple => "multiple values",
    }
}

fn describe_provider_args<'a, I>(args: I) -> Vec<String>
where
    I: IntoIterator<Item = (&'a String, &'a WorkflowProviderArgumentValue)>,
{
    let mut result = Vec::new();
    for (name, value) in args {
        let description = match value {
            WorkflowProviderArgumentValue::Literal(literal) => {
                let trimmed = literal.trim();
                let display = if trimmed.chars().any(char::is_whitespace) {
                    format!("\"{trimmed}\"")
                } else {
                    trimmed.to_string()
                };
                format!("{name}={display}")
            }
            WorkflowProviderArgumentValue::Binding(binding) => {
                let mut target_parts = Vec::new();
                if let Some(step) = &binding.from_step {
                    target_parts.push(format!("steps.{step}"));
                }
                if let Some(input) = &binding.from_input {
                    target_parts.push(format!("inputs.{input}"));
                }
                if target_parts.is_empty() {
                    target_parts.push("context".to_string());
                }
                let mut target = target_parts.join(" | ");
                if let Some(path) = binding.path.as_deref().filter(|path| !path.is_empty()) {
                    if !target.ends_with('.') && !path.starts_with('[') {
                        target.push('.');
                    }
                    target.push_str(path);
                }

                let mut extras = Vec::new();
                if binding.required == Some(true) {
                    extras.push("required".to_string());
                }
                if let Some(on_missing) = binding.on_missing.as_ref() {
                    extras.push(format!("on_missing={}", describe_missing_behavior(on_missing)));
                }

                if extras.is_empty() {
                    format!("{name}⇢{target}")
                } else {
                    format!("{name}⇢{target} ({})", extras.join(", "))
                }
            }
        };
        result.push(description);
    }
    result
}

fn describe_missing_behavior(behavior: &WorkflowMissingBehavior) -> &'static str {
    match behavior {
        WorkflowMissingBehavior::Prompt => "prompt",
        WorkflowMissingBehavior::Skip => "skip",
        WorkflowMissingBehavior::Fail => "fail",
    }
}

fn format_argument_contract(contract: &ProviderArgumentContract) -> String {
    let mut text = format!("• {}", contract.name);
    if contract.required {
        text.push_str(" (required)");
    }
    if !contract.accepts.is_empty() {
        text.push_str(" • accepts ");
        text.push_str(&contract.accepts.join(", "));
    }
    if let Some(prefer) = &contract.prefer {
        text.push_str(" • prefer ");
        text.push_str(prefer);
    }
    text
}

fn format_return_field(field: &ProviderFieldContract) -> String {
    let mut text = format!("• {}", field.name);
    if let Some(ty) = &field.r#type {
        text.push_str(" (");
        text.push_str(ty);
        text.push(')');
    }
    if !field.tags.is_empty() {
        text.push_str(" [");
        text.push_str(&field.tags.join(", "));
        text.push(']');
    }
    text
}
