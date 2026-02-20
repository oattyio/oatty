use crate::app::App;
use crate::cmd::parse_workflow_definition;
use crate::ui::components::common::{ConfirmationModalButton, ConfirmationModalOpts};
use crate::ui::components::component::Component;
use crate::ui::components::workflows::list::WorkflowListEntry;
use crate::ui::theme::theme_helpers as th;
use crate::ui::theme::theme_helpers::ButtonType;
use crate::ui::theme::theme_helpers::create_spans_with_match;
use anyhow::{Result, anyhow};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use oatty_engine::WorkflowRunState;
use oatty_types::workflow::{WorkflowCatalogRequirement, WorkflowCatalogRequirementSourceType, collect_missing_catalog_requirements};
use oatty_types::{Effect, ExecOutcome, MessageType, Modal, Msg, Route, validate_candidate_value};
use oatty_util::{HistoryKey, expand_tilde, value_contains_secret, workflow_input_uses_history};
use ratatui::layout::Position;
use ratatui::widgets::ListItem;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};
use tracing::warn;
use url::Url;

#[derive(Debug, Default, Clone, Copy)]
struct WorkflowsLayout {
    search_area: Rect,
    search_inner_area: Rect,
    import_button_area: Rect,
    remove_button_area: Rect,
    list_area: Rect,
}

#[derive(Debug, Clone)]
enum WorkflowImportConfirmationAction {
    RemoveWorkflow {
        workflow_id: String,
    },
    InstallRequirementsAndImport {
        workflow_content: String,
        catalog_installs: Vec<PendingCatalogInstall>,
        unresolved_requirements: Vec<String>,
    },
}

#[derive(Debug, Clone)]
struct PendingCatalogInstall {
    requirement: WorkflowCatalogRequirement,
    source: String,
    source_type: WorkflowCatalogRequirementSourceType,
}

/// Renders the workflow picker view, including search, filtered listing, and footer hints.
#[derive(Debug, Default)]
pub struct WorkflowsComponent {
    layout: WorkflowsLayout,
    mouse_over_idx: Option<usize>,
    pending_confirmation_action: Option<WorkflowImportConfirmationAction>,
    pending_catalog_install_queue: Vec<PendingCatalogInstall>,
    active_catalog_install: Option<PendingCatalogInstall>,
    pending_workflow_import_content: Option<String>,
}

impl WorkflowsComponent {
    fn cancel_pending_catalog_install_sequence(&mut self) -> Option<String> {
        let active_source = self.active_catalog_install.take().map(|install| install.source);
        self.pending_catalog_install_queue.clear();
        self.pending_workflow_import_content = None;
        active_source
    }

    fn handle_search_key(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        // Only handle here when the search field is active, mirroring browser behavior
        if !app.workflows.f_search.get() {
            return Vec::new();
        }

        match key.code {
            // Esc clears the current search query (do not exit search)
            KeyCode::Esc => {
                app.workflows.clear_search();
            }
            KeyCode::Backspace => {
                app.workflows.pop_search_char();
            }
            KeyCode::Left => {
                app.workflows.move_search_left();
            }
            KeyCode::Right => {
                app.workflows.move_search_right();
            }
            KeyCode::Char(character) if !character.is_control() => {
                app.workflows.append_search_char(character);
            }
            _ => {}
        }

        Vec::new()
    }

    fn render_panel(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let workflows_count = app.workflows.total_count();
        let filtered_count = app.workflows.filtered_count();
        let title = if filtered_count == workflows_count {
            format!("Workflows ({workflows_count})")
        } else {
            format!("Workflows ({filtered_count}/{workflows_count})")
        };

        // Match the BrowserComponent layout: dedicated search panel (with its own block)
        // and a list panel (with its own block and title)
        let layout = Layout::vertical([
            Constraint::Length(3), // Search panel area (title and borders)
            Constraint::Length(3), // Action buttons
            Constraint::Min(1),    // List area
        ])
        .split(area);

        let search_inner_area = self.render_search_bar(frame, layout[0], app);
        let (import_button_area, remove_button_area) = self.render_action_buttons(frame, layout[1], app);
        let list_area = self.render_workflow_list(frame, layout[2], app, &title);
        self.layout = WorkflowsLayout {
            search_area: layout[0],
            search_inner_area,
            import_button_area,
            remove_button_area,
            list_area,
        };
    }

    fn render_action_buttons(&self, frame: &mut Frame, area: Rect, app: &mut App) -> (Rect, Rect) {
        let buttons = Layout::horizontal([
            Constraint::Min(0),
            Constraint::Length(12),
            Constraint::Length(1),
            Constraint::Length(12),
        ])
        .split(area);
        let theme = &*app.ctx.theme;

        let import_options = th::ButtonRenderOptions::new(
            true,
            app.workflows.f_import_button.get(),
            false,
            ratatui::widgets::Borders::ALL,
            ButtonType::Primary,
        );
        th::render_button(frame, buttons[1], "Import", theme, import_options);

        let remove_enabled = app.workflows.selected_workflow_removal_target().is_some();
        let remove_options = th::ButtonRenderOptions::new(
            remove_enabled,
            app.workflows.f_remove_button.get(),
            false,
            ratatui::widgets::Borders::ALL,
            ButtonType::Destructive,
        );
        th::render_button(frame, buttons[3], "Remove", theme, remove_options);

        (buttons[1], buttons[3])
    }

    fn render_search_bar(&mut self, frame: &mut Frame, area: Rect, app: &App) -> Rect {
        let search_query = app.workflows.search_query();
        let theme = &*app.ctx.theme;
        let is_focused = app.workflows.f_search.get();

        // Create a block similar to the browser search panel
        let search_title = Line::from(Span::styled(
            "Search Workflows",
            theme.text_secondary_style().add_modifier(Modifier::BOLD),
        ));
        let mut search_block = th::block::<String>(theme, None, is_focused);
        search_block = search_block.title(search_title);
        let inner_area = search_block.inner(area);

        // Show only the query text (or a muted placeholder) inside the block
        let content_line = if is_focused || !search_query.is_empty() {
            Line::from(Span::styled(search_query.to_string(), theme.text_primary_style()))
        } else {
            Line::from(Span::from(""))
        };

        let paragraph = Paragraph::new(content_line).block(search_block).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);

        if is_focused {
            let cursor_cols = app.workflows.search_cursor_columns() as u16;
            let cursor_x = inner_area.x.saturating_add(cursor_cols);
            let cursor_y = inner_area.y;
            frame.set_cursor_position((cursor_x, cursor_y));
        }
        inner_area
    }

    fn render_workflow_list(&mut self, frame: &mut Frame, area: Rect, app: &mut App, title: &str) -> Rect {
        let theme = &*app.ctx.theme;
        let is_focused = app.workflows.list.f_list.get();
        let block = th::block(theme, Some(title), is_focused);
        let list_area = block.inner(area);
        frame.render_widget(block, area);

        let filter_input = app.workflows.search_query();
        let identifier_style = theme.syntax_type_style();
        let summary_style = theme.syntax_string_style();
        let highlight_style = theme.search_highlight_style();
        let (items, filtered_count) = {
            let state = &app.workflows;
            let title_width = state.filtered_title_width().clamp(12, 40);
            let available_summary_width = area.width.saturating_sub(title_width as u16 + 1).saturating_sub(4) as usize;

            let items: Vec<ListItem> = state
                .filtered_indices()
                .iter()
                .enumerate()
                .filter_map(|(idx, workflow_index)| {
                    state.workflow_entry_by_index(*workflow_index).map(|entry| {
                        let identifier_prefix = if entry.is_invalid() { "⚠ " } else { "" };
                        let identifier_cell = format!(
                            "{:<width$}",
                            format!(
                                "{}{}",
                                identifier_prefix,
                                entry.display_title().unwrap_or(entry.display_identifier())
                            ),
                            width = title_width
                        );
                        let summary = Self::summarize_workflow_entry(entry, available_summary_width);
                        let needle = filter_input.to_string();
                        let mut spans = create_spans_with_match(needle.clone(), identifier_cell, identifier_style, highlight_style);
                        spans.extend(create_spans_with_match(needle, summary, summary_style, highlight_style));
                        let mut list_item = ListItem::from(Line::from(spans));
                        if self.mouse_over_idx.is_some_and(|mouse_idx| mouse_idx == idx) {
                            list_item = list_item.style(theme.selection_style().add_modifier(Modifier::BOLD));
                        }
                        list_item
                    })
                })
                .collect();
            (items, state.filtered_count())
        };

        if filtered_count == 0 {
            let message = if app.workflows.total_count() == 0 {
                "No workflows are available yet."
            } else {
                "No workflows match the current search."
            };
            let message_paragraph = Paragraph::new(message).style(theme.text_muted_style()).wrap(Wrap { trim: true });
            frame.render_widget(message_paragraph, list_area);
            return list_area;
        }
        let is_list_focused = app.workflows.list.f_list.get();
        let list_state = app.workflows.list_state();
        if !is_list_focused {
            list_state.select(None);
        }
        let list = th::create_list_with_highlight(items, theme, is_list_focused, None);

        frame.render_stateful_widget(list, list_area, list_state);
        list_area
    }

    fn summarize_workflow_entry(entry: &WorkflowListEntry, max_width: usize) -> String {
        let summary_source = entry
            .display_description()
            .filter(|value| !value.is_empty())
            .or_else(|| entry.display_title().filter(|value| !value.is_empty()))
            .unwrap_or("No description provided.");

        if max_width == 0 {
            return summary_source.to_string();
        }

        let mut summary = summary_source.to_string();
        if summary.chars().count() > max_width {
            summary = summary.chars().take(max_width.saturating_sub(3)).collect::<String>();
            summary.push_str("...");
        }
        summary
    }

    fn emit_workflow_load_messages(&self, app: &mut App) {
        for message in app.workflows.take_load_messages() {
            app.append_log_message(message);
        }
    }

    /// Populate run state inputs with history-backed defaults when available.
    fn seed_history_defaults(&mut self, app: &mut App, run_state: &mut WorkflowRunState) -> Vec<Effect> {
        let mut effects = Vec::new();
        for (input_name, definition) in &run_state.workflow.inputs {
            if !workflow_input_uses_history(definition) {
                continue;
            }

            let key = HistoryKey::workflow_input(
                app.ctx.history_profile_id.clone(),
                run_state.workflow.identifier.clone(),
                input_name.clone(),
            );

            match app.ctx.history_store.get_latest_value(&key) {
                Ok(Some(stored)) => {
                    if stored.value.is_null() || value_contains_secret(&stored.value) {
                        continue;
                    }
                    if let Some(validation) = &definition.validate
                        && let Err(error) = validate_candidate_value(&stored.value, validation)
                    {
                        let message = format!("History default for '{}' failed validation: {}", input_name, error);
                        warn!(
                            input = %input_name,
                            workflow = %run_state.workflow.identifier,
                            "{}",
                            message
                        );
                        effects.push(Effect::Log(message));
                        continue;
                    }

                    run_state.run_context.inputs.insert(input_name.clone(), stored.value);
                }
                Ok(None) => {}
                Err(error) => {
                    let message = format!("Failed to load history default for '{}': {}", input_name, error);
                    warn!(
                        input = %input_name,
                        workflow = %run_state.workflow.identifier,
                        "{}",
                        message
                    );
                    effects.push(Effect::Log(message));
                }
            }
        }
        effects
    }

    fn handle_import_workflow(&self) -> Vec<Effect> {
        vec![Effect::ShowModal(Modal::FilePicker(vec!["yaml", "yml", "json"]))]
    }

    fn prompt_remove_workflow(&mut self, app: &mut App) -> Vec<Effect> {
        let Some((workflow_id, workflow_label)) = app.workflows.selected_workflow_removal_target() else {
            return Vec::new();
        };

        let message = format!(
            "Are you sure you want to remove '{}'? \nThis action cannot be undone.",
            workflow_label
        );
        app.confirmation_modal_state.update_opts(ConfirmationModalOpts {
            title: Some("Destructive Action".to_string()),
            message: Some(message),
            r#type: Some(MessageType::Warning),
            buttons: vec![
                ConfirmationModalButton::new("Cancel", rat_focus::FocusFlag::default(), ButtonType::Secondary),
                ConfirmationModalButton::new(
                    "Confirm",
                    app.workflows.f_modal_confirmation_button.clone(),
                    ButtonType::Destructive,
                ),
            ],
        });
        self.set_pending_confirmation_action(app, WorkflowImportConfirmationAction::RemoveWorkflow { workflow_id });
        vec![Effect::ShowModal(Modal::Confirmation)]
    }

    fn handle_exec_completed(&mut self, outcome: ExecOutcome, app: &mut App) -> Vec<Effect> {
        match outcome {
            ExecOutcome::FileContents(contents, _) | ExecOutcome::RemoteFileContents(contents, _) => {
                if let Some(active_catalog_install) = self.active_catalog_install.take() {
                    let maybe_prefix = active_catalog_install.requirement.vendor.trim();
                    let maybe_prefix = (!maybe_prefix.is_empty()).then(|| maybe_prefix.to_string());
                    return vec![Effect::ImportRegistryCatalog(contents, maybe_prefix)];
                }

                return self.prepare_workflow_import(contents, app);
            }
            ExecOutcome::RegistryCatalogGenerated(_) => {
                if self.pending_workflow_import_content.is_some() || !self.pending_catalog_install_queue.is_empty() {
                    return self.start_next_catalog_install_or_import_workflow(app);
                }
            }
            ExecOutcome::RegistryCatalogGenerationError(error_message) => {
                if self.pending_workflow_import_content.is_some() || self.active_catalog_install.is_some() {
                    let active_source = self
                        .cancel_pending_catalog_install_sequence()
                        .unwrap_or_else(|| "<unknown source>".to_string());
                    app.append_log_message(format!(
                        "Catalog install from '{}' failed while importing workflow: {}. Workflow import was cancelled.",
                        active_source, error_message
                    ));
                    return Vec::new();
                }
            }
            ExecOutcome::Log(log_message) => {
                if self.active_catalog_install.is_some() {
                    let active_source = self
                        .cancel_pending_catalog_install_sequence()
                        .unwrap_or_else(|| "<unknown source>".to_string());
                    app.append_log_message(format!(
                        "Catalog source read failed for '{}' while importing workflow: {}. Workflow import was cancelled.",
                        active_source, log_message
                    ));
                    return Vec::new();
                }
            }
            ExecOutcome::WorkflowImported { .. } => {
                let _ = app.workflows.ensure_loaded(&app.ctx.command_registry);
                self.emit_workflow_load_messages(app);
            }
            ExecOutcome::WorkflowRemoved { .. } => {
                let _ = app.workflows.ensure_loaded(&app.ctx.command_registry);
                self.emit_workflow_load_messages(app);
            }
            ExecOutcome::WorkflowOperationError(_) => {}
            _ => {}
        }
        Vec::new()
    }

    fn set_pending_confirmation_action(&mut self, app: &mut App, action: WorkflowImportConfirmationAction) {
        self.pending_confirmation_action = Some(action);
        app.focus.focus(&app.workflows.f_modal_confirmation_button);
    }

    fn clear_pending_confirmation_action(&mut self) {
        self.pending_confirmation_action = None;
    }

    fn build_missing_catalog_prompt(
        &self,
        workflow_id: &str,
        catalog_installs: &[PendingCatalogInstall],
        unresolved_requirements: &[String],
    ) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "Workflow '{}' requires catalogs that are missing in this environment.",
            workflow_id
        ));
        if !catalog_installs.is_empty() {
            lines.push(String::new());
            lines.push("Installable requirements:".to_string());
            for install in catalog_installs {
                let title = install.requirement.title.as_deref().unwrap_or("<untitled>");
                lines.push(format!("- {} ({}, source: {})", title, install.requirement.vendor, install.source));
            }
        }
        if !unresolved_requirements.is_empty() {
            lines.push(String::new());
            lines.push("Requirements without installable sources:".to_string());
            for unresolved in unresolved_requirements {
                lines.push(format!("- {}", unresolved));
            }
        }
        lines.push(String::new());
        lines.push("Install available catalogs now and continue with workflow import?".to_string());
        lines.join("\n")
    }

    fn infer_catalog_source_type(requirement: &WorkflowCatalogRequirement) -> Option<WorkflowCatalogRequirementSourceType> {
        if let Some(source_type) = requirement.source_type {
            return Some(source_type);
        }
        let source = requirement.source.as_deref()?.trim();
        if source.starts_with("http://") || source.starts_with("https://") {
            return Some(WorkflowCatalogRequirementSourceType::Url);
        }
        Some(WorkflowCatalogRequirementSourceType::Path)
    }

    fn prepare_workflow_import(&mut self, content: String, app: &mut App) -> Vec<Effect> {
        let definition = match parse_workflow_definition(&content) {
            Ok(definition) => definition,
            Err(error) => {
                app.append_log_message(format!("Failed to parse workflow import content: {error}"));
                return vec![Effect::ImportWorkflowManifest(content)];
            }
        };
        let missing_requirements_result: Result<_, String> = (|| {
            let registry_guard = app
                .ctx
                .command_registry
                .lock()
                .map_err(|error| format!("Failed to inspect catalog requirements during workflow import: {error}"))?;
            Ok(collect_missing_catalog_requirements(
                definition.requires.as_ref(),
                registry_guard.config.catalogs.as_deref().unwrap_or(&[]),
            ))
        })();
        let missing_requirements = match missing_requirements_result {
            Ok(missing_requirements) => missing_requirements,
            Err(message) => {
                app.append_log_message(message);
                return vec![Effect::ImportWorkflowManifest(content)];
            }
        };

        if missing_requirements.is_empty() {
            return vec![Effect::ImportWorkflowManifest(content)];
        }

        let mut catalog_installs = Vec::new();
        let mut unresolved_requirements = Vec::new();
        for missing_requirement in missing_requirements {
            let requirement = missing_requirement.requirement;
            let source = requirement.source.as_deref().map(str::trim).map(str::to_string).unwrap_or_default();
            if source.is_empty() {
                unresolved_requirements.push(missing_requirement.reason);
                continue;
            }
            let Some(source_type) = Self::infer_catalog_source_type(&requirement) else {
                unresolved_requirements.push(missing_requirement.reason);
                continue;
            };
            catalog_installs.push(PendingCatalogInstall {
                requirement,
                source,
                source_type,
            });
        }

        let workflow_identifier = definition.workflow;
        let prompt_message = self.build_missing_catalog_prompt(&workflow_identifier, &catalog_installs, &unresolved_requirements);

        app.confirmation_modal_state.update_opts(ConfirmationModalOpts {
            title: Some("Missing Catalog Requirements".to_string()),
            message: Some(prompt_message),
            r#type: Some(MessageType::Warning),
            buttons: vec![
                ConfirmationModalButton::new("Cancel", rat_focus::FocusFlag::default(), ButtonType::Secondary),
                ConfirmationModalButton::new("Confirm", app.workflows.f_modal_confirmation_button.clone(), ButtonType::Primary),
            ],
        });
        self.set_pending_confirmation_action(
            app,
            WorkflowImportConfirmationAction::InstallRequirementsAndImport {
                workflow_content: content,
                catalog_installs,
                unresolved_requirements,
            },
        );
        vec![Effect::ShowModal(Modal::Confirmation)]
    }

    fn start_next_catalog_install_or_import_workflow(&mut self, app: &mut App) -> Vec<Effect> {
        while let Some(next_install) = self.pending_catalog_install_queue.first().cloned() {
            self.pending_catalog_install_queue.remove(0);
            return match next_install.source_type {
                WorkflowCatalogRequirementSourceType::Path => {
                    self.active_catalog_install = Some(next_install.clone());
                    vec![Effect::ReadFileContents(expand_tilde(next_install.source.as_str()))]
                }
                WorkflowCatalogRequirementSourceType::Url => {
                    let parsed_url = match Url::parse(next_install.source.as_str()) {
                        Ok(parsed_url) => parsed_url,
                        Err(error) => {
                            app.append_log_message(format!(
                                "Skipping catalog install from invalid URL '{}': {}",
                                next_install.source, error
                            ));
                            continue;
                        }
                    };
                    self.active_catalog_install = Some(next_install);
                    vec![Effect::ReadRemoteFileContents(parsed_url)]
                }
            };
        }

        if let Some(workflow_content) = self.pending_workflow_import_content.take() {
            return vec![Effect::ImportWorkflowManifest(workflow_content)];
        }
        Vec::new()
    }

    fn handle_modal_button_click(&mut self, button_id: usize, app: &mut App) -> Vec<Effect> {
        if button_id != app.workflows.f_modal_confirmation_button.widget_id() {
            return Vec::new();
        }

        let Some(action) = self.pending_confirmation_action.take() else {
            return Vec::new();
        };

        match action {
            WorkflowImportConfirmationAction::RemoveWorkflow { workflow_id } => vec![Effect::RemoveWorkflow(workflow_id.into())],
            WorkflowImportConfirmationAction::InstallRequirementsAndImport {
                workflow_content,
                catalog_installs,
                unresolved_requirements,
            } => {
                if !unresolved_requirements.is_empty() {
                    for unresolved_requirement in unresolved_requirements {
                        app.append_log_message(format!(
                            "Catalog requirement still unresolved; import may fail until installed manually: {}",
                            unresolved_requirement
                        ));
                    }
                }
                self.pending_workflow_import_content = Some(workflow_content);
                self.pending_catalog_install_queue = catalog_installs;
                self.start_next_catalog_install_or_import_workflow(app)
            }
        }
    }

    fn hit_test_list(&mut self, app: &mut App, position: Position) -> Option<usize> {
        if !self.layout.list_area.contains(position) {
            return None;
        }
        let offset = app.workflows.list.list_state().offset();
        let idx = (position.y as usize).saturating_sub(self.layout.list_area.y as usize) + offset;
        if app.workflows.filtered_indices().get(idx).is_some() {
            Some(idx)
        } else {
            None
        }
    }

    /// Open the interactive input view for the selected workflow.
    pub fn open_workflow_inputs(&mut self, app: &mut App) -> Result<()> {
        let Some(workflow) = app.workflows.selected_openable_workflow() else {
            return Err(anyhow!(
                "{}",
                app.workflows
                    .selected_workflow_block_reason()
                    .unwrap_or_else(|| "No workflow selected".to_string())
            ));
        };
        let mut run_state = WorkflowRunState::new(workflow.clone());
        self.seed_history_defaults(app, &mut run_state);
        run_state.apply_input_defaults();
        run_state.evaluate_input_providers()?;
        app.workflows.begin_inputs_session(run_state);
        Ok(())
    }
}

impl Component for WorkflowsComponent {
    fn handle_message(&mut self, app: &mut App, msg: Msg) -> Vec<Effect> {
        match msg {
            Msg::ConfirmationModalButtonClicked(button_id) => self.handle_modal_button_click(button_id, app),
            Msg::ConfirmationModalClosed => {
                self.clear_pending_confirmation_action();
                Vec::new()
            }
            Msg::ExecCompleted(outcome) => self.handle_exec_completed(*outcome, app),
            _ => Vec::new(),
        }
    }

    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('o') => return self.handle_import_workflow(),
                KeyCode::Char('r') => return self.prompt_remove_workflow(app),
                _ => {}
            }
        }

        // Handle tab/backtab to switch focus between focusable fields.
        match key.code {
            KeyCode::Tab => {
                app.focus.next();
            }
            KeyCode::BackTab => {
                app.focus.prev();
            }
            _ => {}
        }
        let mut effects = Vec::new();
        if let Err(error) = app.workflows.ensure_loaded(&app.ctx.command_registry) {
            app.append_log_message(format!("Failed to load workflows: {error}"));
            return effects;
        }
        self.emit_workflow_load_messages(app);
        // Defer to the search field if it's focused.
        if app.workflows.f_search.get() {
            return self.handle_search_key(app, key);
        }

        if app.workflows.f_import_button.get() {
            if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                return self.handle_import_workflow();
            }
            return Vec::new();
        }

        if app.workflows.f_remove_button.get() {
            if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                return self.prompt_remove_workflow(app);
            }
            return Vec::new();
        }

        // Handle key events for the list.
        match key.code {
            KeyCode::Esc => {
                if !app.workflows.search_query().is_empty() {
                    app.workflows.clear_search();
                    app.focus.focus(&app.workflows.f_search);
                }
            }
            KeyCode::Down => app.workflows.select_next(),
            KeyCode::Up => app.workflows.select_prev(),
            KeyCode::PageUp => {
                app.workflows.list_state().scroll_up_by(10);
            }
            KeyCode::PageDown => {
                app.workflows.list_state().scroll_down_by(10);
            }
            KeyCode::Home => {
                app.workflows.list_state().scroll_up_by(u16::MAX);
            }
            KeyCode::End => {
                app.workflows.list_state().scroll_down_by(u16::MAX);
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                if let Err(error) = self.open_workflow_inputs(app) {
                    effects.push(Effect::Log(format!("Failed to open workflow inputs: {error}")));
                } else {
                    effects.push(Effect::SwitchTo(Route::WorkflowInputs));
                }
            }
            _ => {}
        }

        effects
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let position = Position {
            x: mouse.column,
            y: mouse.row,
        };

        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            if self.layout.search_area.contains(position) {
                app.focus.focus(&app.workflows.f_search);
                let relative_column = mouse.column.saturating_sub(self.layout.search_inner_area.x);
                app.workflows.set_search_cursor_from_column(relative_column);
            }

            if self.layout.import_button_area.contains(position) {
                app.focus.focus(&app.workflows.f_import_button);
                return self.handle_import_workflow();
            }

            if self.layout.remove_button_area.contains(position) {
                app.focus.focus(&app.workflows.f_remove_button);
                return self.prompt_remove_workflow(app);
            }

            if self.layout.list_area.contains(position)
                && let Some(idx) = self.hit_test_list(app, position)
            {
                app.focus.focus(&app.workflows.list.f_list);
                let currently_selected = app.workflows.list.selected_filtered_index();
                if currently_selected == Some(idx) {
                    return self.handle_key_events(app, KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
                }
                app.workflows.list.set_selected_workflow(idx);
                return Vec::new();
            }
        }

        if mouse.kind == MouseEventKind::Moved || mouse.kind == MouseEventKind::Up(MouseButton::Left) {
            self.mouse_over_idx = self.hit_test_list(app, position);
        }

        if mouse.kind == MouseEventKind::ScrollDown && self.layout.list_area.contains(position) {
            app.workflows.list_state().scroll_down_by(1);
        }

        if mouse.kind == MouseEventKind::ScrollUp && self.layout.list_area.contains(position) {
            app.workflows.list_state().scroll_up_by(1);
        }

        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        if let Err(error) = app.workflows.ensure_loaded(&app.ctx.command_registry) {
            let block = Paragraph::new(format!("Workflows failed to load: {error}"))
                .style(app.ctx.theme.status_error())
                .wrap(Wrap { trim: true });
            frame.render_widget(block, area);
            app.append_log_message(format!("Workflow load error: {error}"));
            return;
        }
        self.emit_workflow_load_messages(app);

        self.render_panel(frame, area, app);
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        let mut hints: Vec<(&str, &str)> = Vec::new();

        let search_focused = app.workflows.f_search.get();
        if search_focused {
            hints.push(("Esc", " Clear search  "));
        } else {
            hints.push(("Shift+Tab", " Focus search  "));
            hints.push(("Esc", " Clear filter  "));
        }

        if app.workflows.f_import_button.get() {
            hints.push(("Enter/Space", " Import workflow  "));
        }
        if app.workflows.f_remove_button.get() && app.workflows.selected_workflow_removal_target().is_some() {
            hints.push(("Enter/Space", " Remove workflow  "));
        }
        if app.workflows.list.f_list.get() {
            hints.push(("↑/↓", " Select  "));
            hints.push(("PgUp/PgDn", " Page  "));
            hints.push(("Home/End", " Jump  "));
            hints.push(("Enter", " Open inputs"));
        }
        hints.push(("Ctrl+O", " Import workflow  "));
        if app.workflows.selected_workflow_removal_target().is_some() {
            hints.push(("Ctrl+R", " Remove workflow"));
        }

        th::build_hint_spans(theme, &hints)
    }
}
