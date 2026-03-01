use std::{
    borrow::Cow,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
    vec,
};

use crate::{
    app::App,
    ui::{
        components::{
            Component,
            common::{
                ConfirmationModalButton, ConfirmationModalOpts, TextInputState,
                key_value_editor::KeyValueEditorView,
                manual_entry_modal::state::{ManualEntryKind, ManualEntryState, ManualEntryValueState},
            },
            library::{CatalogProjection, state::LibraryEditorField, types::CatalogValidationError},
        },
        theme::{
            Theme,
            theme_helpers::{
                self, ButtonRenderOptions, ButtonType, block, button_primary_style, button_secondary_style, create_checkbox,
                create_list_with_highlight, create_radio_button, render_button,
            },
        },
    },
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use oatty_registry::CommandRegistry;
use oatty_types::{Effect, ExecOutcome, MessageType, Modal, Msg, TransientMessage};
use oatty_util::truncate_with_ellipsis;
use rat_focus::{FocusFlag, HasFocus};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect, Spacing},
    style::Modifier,
    symbols::merge::MergeStrategy,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph},
};
use unicode_width::UnicodeWidthStr;
use url::Url;

const DESCRIPTION_LABEL: &str = "Description: ";
const PAGE_SCROLL_STEP: usize = 10;

#[derive(Debug, Default)]
pub struct LibraryLayout {
    pub import_catalog_button: Rect,
    pub remove_catalog_button: Rect,
    pub api_list: Rect,
    pub message: Rect,
    pub details: Rect,
    pub description: Rect,
    pub add_base_url_button: Rect,
    pub remove_base_url_button: Rect,
    pub base_url_radio_group: Rect,
    pub kv_editor: Rect,
    pub truncated_description: usize,
}

impl From<Vec<Rect>> for LibraryLayout {
    fn from(rects: Vec<Rect>) -> Self {
        Self {
            import_catalog_button: rects[0],
            remove_catalog_button: rects[1],
            api_list: rects[2],
            details: rects[3],
            message: rects[4],
            // Dynamically calculated
            description: Rect::default(),
            base_url_radio_group: Rect::default(),
            kv_editor: Rect::default(),
            add_base_url_button: Rect::default(),
            remove_base_url_button: Rect::default(),
            truncated_description: 0,
        }
    }
}

#[derive(Debug, Default)]
pub struct LibraryComponent {
    layout: LibraryLayout,
    kv_view: KeyValueEditorView,
    staged_catalog_contents: Option<String>,
}

impl LibraryComponent {
    fn create_projections(command_registry: Arc<Mutex<CommandRegistry>>) -> Vec<CatalogProjection> {
        let mut catalogs = Vec::with_capacity(2);
        let Ok(lock) = command_registry.try_lock() else {
            return catalogs;
        };
        if let Some(catalogs_config) = lock.config.catalogs.as_ref() {
            for catalog_config in catalogs_config {
                catalogs.push(CatalogProjection::from(catalog_config));
            }
        }

        catalogs
    }
    /// Renders the buttons for the library component.
    fn render_buttons(&self, frame: &mut Frame, app: &App) {
        let theme = &*app.ctx.theme;
        let import_button_opts = ButtonRenderOptions {
            selected: false,
            focused: app.library.f_import_button.get(),
            borders: Borders::ALL,
            enabled: true,
            button_type: ButtonType::Primary,
        };

        render_button(frame, self.layout.import_catalog_button, "Import", theme, import_button_opts);
        let maybe_selected_index = app.library.api_selected_index();
        let remove_button_opts = ButtonRenderOptions {
            selected: false,
            focused: app.library.f_remove_button.get(),
            borders: Borders::ALL,
            enabled: maybe_selected_index.is_some(),
            button_type: ButtonType::Destructive,
        };
        render_button(frame, self.layout.remove_catalog_button, "Remove", theme, remove_button_opts);
    }

    fn render_api_list(&mut self, frame: &mut Frame, app: &mut App) {
        let is_list_focused = app.library.f_api_list.get();
        let list_block = block(&*app.ctx.theme, Some("Catalogs"), is_list_focused).merge_borders(MergeStrategy::Exact);
        let list_inner = list_block.inner(self.layout.api_list);
        frame.render_widget(list_block, self.layout.api_list);

        let list_items = self
            .build_api_list_items(app, list_inner.width as usize)
            .unwrap_or(vec![ListItem::new("Use Import to add new items")]);

        if app.library.api_selected_index().is_none() && !list_items.is_empty() {
            app.library.set_api_selected_index(Some(0));
        }

        let list = create_list_with_highlight(list_items, &*app.ctx.theme, is_list_focused, None);
        frame.render_stateful_widget(list, list_inner, app.library.api_list_state_mut());
    }

    fn render_message(&self, frame: &mut Frame, app: &mut App) {
        let theme = &*app.ctx.theme;
        if let Some(message) = app.library.message_ref()
            && let Some(message_paragraph) = theme_helpers::create_status_paragraph(theme, message, self.layout.message.width, false)
        {
            frame.render_widget(message_paragraph, self.layout.message);
        }
    }

    fn render_details(&mut self, frame: &mut Frame, app: &mut App) {
        let area = self.layout.details;
        let theme = &*app.ctx.theme;
        let details_block = block(theme, Some("Catalog Details"), false).merge_borders(MergeStrategy::Exact);
        let details_inner = details_block.inner(area);
        frame.render_widget(details_block, area);

        let Some(projection) = app.library.selected_projection() else {
            frame.render_widget(
                Paragraph::new("Select an item to configure").block(Block::new().padding(Padding::new(1, 1, 1, 1))),
                details_inner,
            );
            return;
        };
        let (title_style, enabled_text) = if projection.is_enabled {
            (theme.status_success(), "enabled")
        } else {
            (theme.text_muted_style(), "disabled")
        };
        let block = Block::new().padding(Padding::new(1, 1, 0, 1));
        let inner = block.inner(details_inner);
        let truncated_description = truncate_with_ellipsis(
            &projection.description,
            (inner.width as usize).saturating_sub(DESCRIPTION_LABEL.width()),
        );
        self.layout.truncated_description = truncated_description.width();

        let summary_lines = vec![
            Line::from(vec![
                Span::styled(projection.title.clone(), title_style.add_modifier(Modifier::BOLD)),
                Span::styled(format!(" ({})", enabled_text), theme.text_muted_style()),
            ]),
            Line::from(vec![
                Span::styled("Command Prefix: ", theme.text_muted_style()),
                Span::styled(projection.vendor.clone(), theme.syntax_type_style()),
            ]),
            Line::from(vec![
                Span::styled("Endpoints: ", theme.text_muted_style()),
                Span::styled(projection.command_count.to_string(), theme.syntax_number_style()),
            ]),
            Line::from(vec![
                Span::styled("Workflows: ", theme.text_muted_style()),
                Span::styled(projection.workflow_count.to_string(), theme.syntax_number_style()),
            ]),
            Line::from(vec![
                Span::styled("Value providers: ", theme.text_muted_style()),
                Span::styled(projection.provider_contract_count.to_string(), theme.syntax_number_style()),
            ]),
            Line::from(vec![
                Span::styled(DESCRIPTION_LABEL, theme.text_muted_style()),
                Span::styled(truncated_description, theme.syntax_string_style()),
            ]),
        ];

        let summary = Paragraph::new(summary_lines).block(block);
        let line_ct = summary.line_count(inner.width) as u16;
        frame.render_widget(summary, details_inner);
        self.layout.description = Rect::new(inner.x + DESCRIPTION_LABEL.width() as u16, inner.y + 5, inner.width, 1);
        let mut remaining_area = details_inner;
        remaining_area.y = details_inner.y + line_ct;
        remaining_area.height = details_inner.height.saturating_sub(line_ct);

        let remaining_area_layout = Layout::vertical([
            Constraint::Length(projection.base_urls.len() as u16 + 5), // base url radio group
            Constraint::Percentage(100),                               // kv editor
        ])
        .split(remaining_area);

        self.render_base_url_radios(frame, remaining_area_layout[0], app);

        self.layout.kv_editor = remaining_area_layout[1];
        let kv_state = app.library.kv_state_mut();
        self.kv_view
            .render_with_state(frame, self.layout.kv_editor, &*app.ctx.theme, kv_state);
    }

    fn render_base_url_radios(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let Some(projection) = app.library.selected_projection() else {
            return;
        };
        let theme = &*app.ctx.theme;
        let mut radio_group = Vec::with_capacity(projection.base_urls.len());
        let maybe_selected_idx = app.library.url_selected_index();
        for (idx, url) in projection.base_urls.iter().enumerate() {
            let is_focused = maybe_selected_idx.is_some_and(|i| i == idx);
            let is_checked = idx == projection.base_url_index;
            let mut radio = create_radio_button(None, is_checked, is_focused, theme);
            let label_theme = if is_checked {
                theme.status_success()
            } else if Url::parse(url).is_err() {
                theme.status_error()
            } else {
                theme.text_muted_style()
            };
            radio.push_span(Span::styled(format!(" {}", url), label_theme));
            radio_group.push(ListItem::from(radio));
        }
        let block = block(theme, Some("Active base URL"), app.library.f_url_list_container.get());
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let layout = Layout::vertical([
            Constraint::Length(1),       // Error message
            Constraint::Length(1),       // Buttons
            Constraint::Percentage(100), // List
        ])
        .split(inner);

        let buttons_layout = Layout::horizontal([
            Constraint::Min(0),     // Left spacer
            Constraint::Length(7),  // Add
            Constraint::Length(1),  // spacer
            Constraint::Length(10), // Remove
        ])
        .split(layout[1]);
        if let Some(err) = app.library.base_url_err() {
            let error_message = Line::from(vec![Span::styled(format!("✘ {}", err), theme.status_error())]);
            frame.render_widget(error_message, layout[0]);
        }

        let add = Line::from(vec![
            Span::styled(" + ", theme.status_success()),
            Span::styled("Add ", theme.text_primary_style()),
        ])
        .style(button_primary_style(theme, true, app.library.f_add_url_button.get()));
        frame.render_widget(add, buttons_layout[1]);
        self.layout.add_base_url_button = buttons_layout[1];

        let remove = Line::from(vec![
            Span::styled(" − ", theme.status_error()),
            Span::styled("Remove ", theme.text_secondary_style()),
        ])
        .style(button_secondary_style(theme, true, app.library.f_remove_url_button.get()));
        frame.render_widget(remove, buttons_layout[3]);
        self.layout.remove_base_url_button = buttons_layout[3];

        let list = List::new(radio_group).highlight_style(app.ctx.theme.selection_style().add_modifier(Modifier::BOLD));

        frame.render_stateful_widget(list, layout[2], app.library.url_list_state_mut());
        self.layout.base_url_radio_group = layout[2];
    }

    /// Renders the list items for the library component.
    fn build_api_list_items(&mut self, app: &mut App, list_inner: usize) -> Option<Vec<ListItem<'static>>> {
        let mouse_over_index = app.library.api_mouse_over_index();
        let selected_index = app.library.api_selected_index();
        let mut list_items = Vec::new();
        if app.library.projections().is_empty() {
            app.library
                .set_projections(Self::create_projections(Arc::clone(&app.ctx.command_registry)));
        }

        for (index, catalog) in app.library.projections().iter().enumerate() {
            let is_checkbox_focused = selected_index == Some(index);
            let mut list_item = self.build_api_list_item(catalog, is_checkbox_focused, list_inner, &*app.ctx.theme);
            if mouse_over_index.is_some_and(|hover| hover == index) {
                list_item = list_item.style(app.ctx.theme.selection_style().add_modifier(Modifier::BOLD));
            }
            list_items.push(list_item);
        }

        Some(list_items)
    }

    fn refresh_projections_from_registry_if_safe(&mut self, app: &mut App) {
        if app.library.is_dirty() || app.library.kv_state().is_dirty() || app.library.active_input_field().is_some() {
            return;
        }

        let refreshed_projections = Self::create_projections(Arc::clone(&app.ctx.command_registry));
        if self.projections_match_registry(app.library.projections(), &refreshed_projections) {
            return;
        }

        let selected_title = app.library.selected_projection().map(|projection| projection.title.to_string());
        app.library.set_projections(refreshed_projections);
        if let Some(title) = selected_title
            && let Some(index) = app.library.projections().iter().position(|projection| projection.title == title)
        {
            app.library.set_api_selected_index(Some(index));
        }
    }

    fn projections_match_registry(&self, current: &[CatalogProjection], refreshed: &[CatalogProjection]) -> bool {
        current.len() == refreshed.len()
            && current.iter().zip(refreshed.iter()).all(|(left, right)| {
                left.title == right.title
                    && left.description == right.description
                    && left.headers == right.headers
                    && left.base_urls == right.base_urls
                    && left.base_url_index == right.base_url_index
                    && left.vendor == right.vendor
                    && left.command_count == right.command_count
                    && left.workflow_count == right.workflow_count
                    && left.provider_contract_count == right.provider_contract_count
                    && left.is_enabled == right.is_enabled
            })
    }

    /// Renders a single list item for the library component.
    fn build_api_list_item(
        &self,
        catalog: &CatalogProjection,
        is_checkbox_focused: bool,
        list_inner: usize,
        theme: &dyn Theme,
    ) -> ListItem<'static> {
        let style = if catalog.is_enabled {
            theme.status_success()
        } else {
            theme.text_muted_style()
        };
        let mut check_box_line = create_checkbox(None, catalog.is_enabled, is_checkbox_focused, theme);
        let title = catalog.title.clone();
        let remaining_width = list_inner.saturating_sub(title.width() + 8); // +8 for the enabled/disabled status
        check_box_line.push_span(Span::styled(format!(" {}", title), style));

        let status = if catalog.is_enabled { "enabled" } else { "disabled" };
        check_box_line.push_span(Span::styled(format!("{:>remaining_width$}", status), theme.text_muted_style()));
        ListItem::new(check_box_line).style(theme.text_primary_style())
    }

    fn prompt_remove_catalog(&mut self, app: &mut App, idx: usize) -> Vec<Effect> {
        let Some(projection) = app.library.projections().get(idx) else {
            return Vec::new();
        };

        let message = format!(
            "Are you sure you want to remove '{}'? \nThis action cannot be undone.",
            projection.title
        );
        let buttons = vec![
            ConfirmationModalButton::new("Cancel", FocusFlag::default(), ButtonType::Secondary),
            ConfirmationModalButton::new("Confirm", app.library.f_modal_confirmation_button.clone(), ButtonType::Destructive),
        ];
        app.confirmation_modal_state.update_opts(ConfirmationModalOpts {
            title: Some("Destructive Action".to_string()),
            message: Some(message),
            r#type: Some(MessageType::Warning),
            buttons,
        });
        vec![Effect::ShowModal(Modal::Confirmation)]
    }

    fn handle_modal_button_click(&mut self, button_id: usize, app: &mut App) -> Vec<Effect> {
        if button_id == app.library.f_modal_confirmation_button.widget_id()
            && let Some(projection) = app.library.selected_projection()
        {
            return vec![Effect::RemoveCatalog(projection.title.clone())];
        }

        Vec::new()
    }

    fn handle_exec_completed(&mut self, outcome: ExecOutcome, app: &mut App) -> Vec<Effect> {
        match outcome {
            ExecOutcome::FileContents(contents, _) | ExecOutcome::RemoteFileContents(contents, _) => {
                self.staged_catalog_contents = Some(contents);
                app.library.set_message(None);
                app.manual_entry_state = Some(ManualEntryState {
                    value: ManualEntryValueState::Text(TextInputState::new()),
                    kind: ManualEntryKind::Text,
                    title: "Command prefix".to_string(),
                    label: Some("Specify a command prefix or press Enter to default to the vendor name".to_string()),
                    placeholder: Some("e.g. 'oatty-'".to_string()),
                    ..Default::default()
                });
                return vec![Effect::ShowModal(Modal::ManualEntry)];
            }
            ExecOutcome::RegistryCatalogGenerated(catalog) => {
                app.library.set_message(Some(TransientMessage::new(
                    Cow::from("import successful"),
                    MessageType::Success,
                    Duration::from_millis(5000),
                )));
                app.library.push_projection(CatalogProjection::from(&catalog));
            }

            ExecOutcome::RegistryCatalogGenerationError(message) | ExecOutcome::RegistryConfigSaveError(message) => {
                app.library
                    .set_message(Some(TransientMessage::new(Cow::from(message), MessageType::Error, Duration::MAX)));
                app.library.clear_projections();
            }

            ExecOutcome::RegistryConfigSaved => {
                app.library.set_message(Some(TransientMessage::new(
                    Cow::from("Registry configuration saved successfully"),
                    MessageType::Success,
                    Duration::from_millis(5000),
                )));
                app.library.clear_projections();
                app.library.kv_state_mut().reset_dirty();
            }

            _ => {}
        }
        Vec::new()
    }

    fn handle_manual_entry_modal_closed(&mut self, app: &mut App) -> Vec<Effect> {
        if let Some((contents, state)) = self.staged_catalog_contents.take().zip(app.manual_entry_state.take()) {
            let maybe_command_prefix = state
                .value
                .text_buffer()
                .and_then(|t| if t.is_empty() { None } else { Some(t.input().to_string()) });
            return vec![Effect::ImportRegistryCatalog(contents, maybe_command_prefix)];
        }
        Vec::new()
    }

    fn toggle_enabled(idx: usize, app: &mut App) -> Vec<Effect> {
        let projection = app.library.get_projection_mut(idx).unwrap();
        projection.is_enabled = !projection.is_enabled;

        vec![Effect::UpdateCatalogEnabledState {
            is_enabled: projection.is_enabled,
            title: projection.title.clone(),
        }]
    }

    fn update_base_url(idx: usize, app: &mut App) -> Vec<Effect> {
        let Some(projection) = app.library.selected_projection().filter(|p| p.base_urls.len() > idx) else {
            return Vec::new();
        };
        vec![Effect::UpdateCatalogBaseUrlIndex {
            base_url_index: idx,
            title: projection.title.clone(),
        }]
    }

    fn handle_api_list_mouse_down(&mut self, pos: Position, maybe_list_idx: Option<usize>, app: &mut App) -> Vec<Effect> {
        app.focus.focus(&app.library.f_api_list);
        // The click is within the area of where the checkboxes are
        if let Some(index) = maybe_list_idx
            && (3..=5).contains(&pos.x.saturating_sub(self.layout.api_list.x))
            && index < app.library.projections().len()
        {
            return Self::toggle_enabled(index, app);
        } else {
            app.library.set_api_selected_index(maybe_list_idx);
        }
        Vec::new()
    }

    fn handle_url_radio_mouse_down(&mut self, pos: Position, app: &mut App) -> Vec<Effect> {
        app.focus.focus(&app.library.f_url_list);
        let idx = pos.y.saturating_sub(self.layout.base_url_radio_group.y) as usize + app.library.url_list_offset();
        // if we're over a radio button, click it without changing list selection
        if (1..=3).contains(&pos.x.saturating_sub(self.layout.base_url_radio_group.x)) {
            return Self::update_base_url(idx, app);
        } else {
            app.library.set_url_selected_index(Some(idx));
            app.focus.focus(&app.library.f_base_url_input);
        }
        Vec::new()
    }

    fn handle_import(&self) -> Vec<Effect> {
        vec![Effect::ShowModal(Modal::FilePicker(vec!["json", "yml", "yaml"]))]
    }

    fn add_new_url_row(&mut self, app: &mut App) {
        if !app.library.url_add_row() {
            return;
        }
        app.focus.focus(&app.library.f_base_url_input);
    }

    fn position_cursor_for_focused_input(&self, frame: &mut Frame, app: &App) {
        match app.library.active_input_field() {
            Some(LibraryEditorField::Description) => {
                let Rect { x, y, .. } = self.layout.description;
                let cols = self
                    .layout
                    .truncated_description
                    .min(app.library.description_text_input().cursor_columns());
                let pos = Position::new(x + cols as u16, y);
                frame.set_cursor_position(pos);
            }
            Some(LibraryEditorField::BaseUrl) => {
                let Rect { x, y, .. } = self.layout.base_url_radio_group;
                let prefix_width = 4;
                let selected_index = app.library.url_selected_index().unwrap_or(0);
                let input_y = selected_index + y as usize + app.library.api_list_offset();
                let cols = app.library.url_text_input().cursor_columns();
                let pos = Position::new(x + prefix_width + cols as u16, input_y as u16);
                frame.set_cursor_position(pos);
            }
            None => {}
        }
    }

    fn track_lost_kv_focus(&self, app: &App) -> Vec<Effect> {
        // Losing focus will trigger an auto-save for headers if dirty
        if app.library.kv_state().focus().lost()
            && app.library.kv_state().is_dirty()
            && let Some(projection) = app.library.selected_projection()
        {
            return vec![Effect::UpdateCatalogHeaders {
                title: projection.title.clone(),
                headers: app.library.kv_state().valid_rows(),
            }];
        }
        Vec::new()
    }

    fn track_lost_input_focus(&self, app: &mut App) -> Vec<Effect> {
        let (description_lost, base_url_input_lost) = (app.library.f_description_input.lost(), app.library.f_base_url_input.lost());
        if (description_lost || base_url_input_lost) && app.library.is_dirty() {
            app.library.set_base_url_err(None);
            let Some(projection) = app.library.selected_projection() else {
                return Vec::new();
            };
            match projection.validate() {
                Ok(()) => {
                    return if description_lost {
                        vec![Effect::UpdateCatalogDescription {
                            title: projection.title.clone(),
                            description: projection.description.to_string(),
                        }]
                    } else {
                        vec![Effect::UpdateCatalogBaseUrls {
                            title: projection.title.clone(),
                            base_urls: projection.base_urls.clone(),
                        }]
                    };
                }

                Err(CatalogValidationError::BaseUrlIndex(err)) | Err(CatalogValidationError::BaseUrls(err)) => {
                    app.library.set_base_url_err(Some(Cow::from(err)));
                }

                Err(err) => {
                    app.library.set_message(Some(TransientMessage::new(
                        Cow::from(format!("{}", err)),
                        MessageType::Error,
                        Duration::MAX,
                    )));
                }
            }
        }

        Vec::new()
    }

    fn move_api_selection_to_first(&self, app: &mut App) {
        if app.library.projections().is_empty() {
            app.library.set_api_selected_index(None);
            return;
        }
        app.library.set_api_selected_index(Some(0));
    }

    fn move_api_selection_to_last(&self, app: &mut App) {
        let projection_count = app.library.projections().len();
        if projection_count == 0 {
            app.library.set_api_selected_index(None);
            return;
        }
        app.library.set_api_selected_index(Some(projection_count.saturating_sub(1)));
    }

    fn move_api_selection_by_page(&self, app: &mut App, forward: bool) {
        let projection_count = app.library.projections().len();
        if projection_count == 0 {
            app.library.set_api_selected_index(None);
            return;
        }
        let current_index = app
            .library
            .api_selected_index()
            .unwrap_or(0)
            .min(projection_count.saturating_sub(1));
        let next_index = if forward {
            current_index
                .saturating_add(PAGE_SCROLL_STEP)
                .min(projection_count.saturating_sub(1))
        } else {
            current_index.saturating_sub(PAGE_SCROLL_STEP)
        };
        app.library.set_api_selected_index(Some(next_index));
    }

    fn move_url_selection_to_first(&self, app: &mut App) {
        let Some(base_url_count) = app.library.selected_projection().map(|projection| projection.base_urls.len()) else {
            app.library.set_url_selected_index(None);
            return;
        };
        if base_url_count == 0 {
            app.library.set_url_selected_index(None);
            return;
        }
        app.library.set_url_selected_index(Some(0));
    }

    fn move_url_selection_to_last(&self, app: &mut App) {
        let Some(base_url_count) = app.library.selected_projection().map(|projection| projection.base_urls.len()) else {
            app.library.set_url_selected_index(None);
            return;
        };
        if base_url_count == 0 {
            app.library.set_url_selected_index(None);
            return;
        }
        app.library.set_url_selected_index(Some(base_url_count.saturating_sub(1)));
    }

    fn move_url_selection_by_page(&self, app: &mut App, forward: bool) {
        let Some(base_url_count) = app.library.selected_projection().map(|projection| projection.base_urls.len()) else {
            app.library.set_url_selected_index(None);
            return;
        };
        if base_url_count == 0 {
            app.library.set_url_selected_index(None);
            return;
        }
        let current_index = app.library.url_selected_index().unwrap_or(0).min(base_url_count.saturating_sub(1));
        let next_index = if forward {
            current_index.saturating_add(PAGE_SCROLL_STEP).min(base_url_count.saturating_sub(1))
        } else {
            current_index.saturating_sub(PAGE_SCROLL_STEP)
        };
        app.library.set_url_selected_index(Some(next_index));
    }
}

impl Component for LibraryComponent {
    fn handle_message(&mut self, app: &mut App, msg: Msg) -> Vec<Effect> {
        match msg {
            Msg::Tick => {
                self.refresh_projections_from_registry_if_safe(app);
                Vec::new()
            }
            Msg::ConfirmationModalButtonClicked(button_id) => self.handle_modal_button_click(button_id, app),
            Msg::ManualEntryModalClosed => self.handle_manual_entry_modal_closed(app),
            Msg::ExecCompleted(outcome) => self.handle_exec_completed(*outcome, app),
            _ => Vec::new(),
        }
    }

    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        if app.library.kv_state().is_focused() {
            self.kv_view
                .handle_key_event(app.library.kv_state_mut(), key, Rc::clone(&app.focus));
            return self.track_lost_kv_focus(app);
        }

        match key.code {
            KeyCode::Tab => {
                app.focus.next();
                return self.track_lost_input_focus(app);
            }
            KeyCode::BackTab => {
                app.focus.prev();
                return self.track_lost_input_focus(app);
            }
            KeyCode::Enter | KeyCode::Char(' ') if app.library.active_input_field().is_none() => {
                // Import button
                if app.library.f_import_button.get() {
                    return self.handle_import();
                }
                if app.library.f_add_url_button.get() {
                    self.add_new_url_row(app);
                    return Vec::new();
                }
                if app.library.f_remove_url_button.get() {
                    app.library.remove_url_row();
                    return Vec::new();
                }
                if let Some(idx) = app.library.api_selected_index() {
                    // Remove button
                    if app.library.f_remove_button.get() {
                        return self.prompt_remove_catalog(app, idx);
                    }
                    // Enabled checkbox
                    if app.library.f_api_list.get() {
                        return Self::toggle_enabled(idx, app);
                    }

                    if let Some(idx) = app.library.url_selected_index()
                        && app.library.f_url_list.get()
                    {
                        return Self::update_base_url(idx, app);
                    }
                }
            }
            KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return self.handle_import();
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(idx) = app.library.api_selected_index() {
                    return self.prompt_remove_catalog(app, idx);
                }
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) && app.library.f_url_list_container.get() => {
                app.library.url_add_row();
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) && app.library.f_url_list_container.get() => {
                app.library.remove_url_row();
            }
            KeyCode::Char(c) => {
                app.library.insert_character_for_active_field(c);
            }
            KeyCode::Up => {
                if app.library.f_api_list.get() {
                    app.library.api_select_previous();
                } else if app.library.f_url_list.get() {
                    app.library.url_select_previous();
                }
            }
            KeyCode::Down => {
                if app.library.f_api_list.get() {
                    app.library.api_select_next();
                } else if app.library.f_url_list.get() {
                    app.library.url_select_next();
                }
            }
            KeyCode::PageUp => {
                if app.library.f_api_list.get() {
                    self.move_api_selection_by_page(app, false);
                } else if app.library.f_url_list.get() {
                    self.move_url_selection_by_page(app, false);
                }
            }
            KeyCode::PageDown => {
                if app.library.f_api_list.get() {
                    self.move_api_selection_by_page(app, true);
                } else if app.library.f_url_list.get() {
                    self.move_url_selection_by_page(app, true);
                }
            }
            KeyCode::Home => {
                if app.library.f_api_list.get() {
                    self.move_api_selection_to_first(app);
                } else if app.library.f_url_list.get() {
                    self.move_url_selection_to_first(app);
                }
            }
            KeyCode::End => {
                if app.library.f_api_list.get() {
                    self.move_api_selection_to_last(app);
                } else if app.library.f_url_list.get() {
                    self.move_url_selection_to_last(app);
                }
            }
            KeyCode::Right => {
                app.library.move_cursor_right();
            }
            KeyCode::Left => {
                app.library.move_cursor_left();
            }
            KeyCode::Backspace => {
                app.library.delete_previous_character();
            }
            KeyCode::Delete => {
                app.library.delete_next_character();
            }
            _ => {}
        }
        Vec::new()
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let pos = Position {
            x: mouse.column,
            y: mouse.row,
        };

        if self.layout.kv_editor.contains(pos) {
            if let MouseEventKind::Down(MouseButton::Left) = mouse.kind
                && !app.library.kv_state().is_focused()
            {
                app.focus.focus(app.library.kv_state());
            }
            self.kv_view
                .handle_mouse_event(app.library.kv_state_mut(), mouse, Rc::clone(&app.focus));
            return Vec::new();
        }
        let hit_test_api_list = self.layout.api_list.contains(pos);
        let maybe_api_list_idx = if hit_test_api_list {
            Some(pos.y.saturating_sub(self.layout.api_list.y + 1) as usize + app.library.api_list_offset())
        } else {
            None
        };

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                match () {
                    _ if hit_test_api_list => {
                        return self.handle_api_list_mouse_down(pos, maybe_api_list_idx, app);
                    }
                    _ if self.layout.base_url_radio_group.contains(pos) => {
                        return self.handle_url_radio_mouse_down(pos, app);
                    }
                    _ if self.layout.description.contains(pos) => {
                        app.focus.focus(&app.library.f_description_input);
                    }
                    _ if self.layout.add_base_url_button.contains(pos) => {
                        app.focus.focus(&app.library.f_add_url_button);
                        self.add_new_url_row(app);
                    }
                    _ if self.layout.import_catalog_button.contains(pos) => {
                        app.focus.focus(&app.library.f_import_button);
                        return self.handle_import();
                    }
                    _ if self.layout.remove_catalog_button.contains(pos) => {
                        app.focus.focus(&app.library.f_remove_button);
                        let Some(idx) = app.library.api_selected_index() else {
                            return Vec::new();
                        };

                        return self.prompt_remove_catalog(app, idx);
                    }
                    _ if self.layout.remove_base_url_button.contains(pos) => {
                        app.library.remove_url_row();
                    }
                    // Autosave when focus is lost
                    _ if app.library.kv_state().is_focused() => {
                        return self.track_lost_kv_focus(app);
                    }
                    () => {}
                }
            }
            MouseEventKind::Moved | MouseEventKind::Up(MouseButton::Left) => {
                app.library.api_set_mouse_over_index(maybe_api_list_idx);
            }
            MouseEventKind::ScrollDown if self.layout.api_list.contains(pos) => {
                app.library.api_list_state_mut().scroll_down_by(1);
            }
            MouseEventKind::ScrollUp if self.layout.api_list.contains(pos) => {
                app.library.api_list_state_mut().scroll_up_by(1);
            }
            MouseEventKind::ScrollDown if self.layout.base_url_radio_group.contains(pos) => {
                app.library.url_list_state_mut().scroll_down_by(1);
            }
            MouseEventKind::ScrollUp if self.layout.base_url_radio_group.contains(pos) => {
                app.library.url_list_state_mut().scroll_up_by(1);
            }
            _ => {}
        }

        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        self.layout = LibraryLayout::from(self.get_preferred_layout(app, rect));

        self.render_buttons(frame, app);
        self.render_api_list(frame, app);
        self.render_message(frame, app);
        self.render_details(frame, app);
        self.position_cursor_for_focused_input(frame, app);
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        if app.library.kv_state().is_focused() {
            let mut spans = Vec::new();
            self.kv_view.add_table_hints(&mut spans, &*app.ctx.theme);
            return spans;
        }

        let mut hints = Vec::new();
        if app.library.f_api_list.get() {
            hints.push(("↑/↓", " Navigate catalogs "));
            hints.push(("PgUp/PgDn", " Page catalogs "));
            hints.push(("Home/End", " Jump catalogs "));
        }
        if app.library.f_url_list_container.get() {
            hints.push(("↑/↓", " Navigate base URLs "));
            hints.push(("PgUp/PgDn", " Page base URLs "));
            hints.push(("Home/End", " Jump base URLs "));
            hints.push(("Ctrl+N", " Add base URL "));
            hints.push(("Ctrl+D", " Delete base URL "));
        }
        if app.library.f_add_url_button.get() {
            hints.push(("Enter/Space", " Add base URL "));
        }
        if app.library.f_remove_url_button.get() && app.library.url_selected_index().is_some() {
            hints.push(("Enter/Space", " Remove base URL "));
        }
        if app.library.f_import_button.get() {
            hints.push(("Enter/Space", " Import catalog "));
        }

        hints.push(("Ctrl+O", " Import catalog "));
        if app.library.api_selected_index().is_some() {
            hints.push(("Ctrl+R", " Remove catalog "));

            if app.library.f_api_list.get() {
                hints.push(("Enter/Space", " Toggle enabled "));
            }
            if app.library.f_remove_button.get() {
                hints.push(("Enter/Space", " Remove catalog "));
            }
        }

        if app.library.f_url_list.get()
            && let Some((projection, idx)) = app.library.selected_projection().zip(app.library.url_selected_index())
            && idx != projection.base_url_index
        {
            hints.push(("Enter/Space", " Set active base URL "));
        }

        theme_helpers::build_hint_spans(&*app.ctx.theme, &hints)
    }

    fn get_preferred_layout(&self, _app: &App, area: Rect) -> Vec<Rect> {
        let outter = Layout::vertical([
            Constraint::Percentage(100), // Content
            Constraint::Length(2),       // status/error
        ])
        .split(area);

        let inner = Layout::horizontal([
            Constraint::Percentage(30), // Left pane
            Constraint::Percentage(70), // Right pane
        ])
        .spacing(Spacing::Overlap(1))
        .split(outter[0]);

        let left_pane = Layout::vertical([
            Constraint::Length(3), // import/remove buttons
            Constraint::Min(1),    // api list
        ])
        .split(inner[0]);

        let buttons = Layout::horizontal([
            Constraint::Min(0),     // Spacer
            Constraint::Length(12), // Import button
            Constraint::Length(1),  // spacer
            Constraint::Length(12), // Remove button
            Constraint::Length(1),  // spacer
        ])
        .split(left_pane[0]);

        vec![
            buttons[1],   // Import button
            buttons[3],   // Remove button
            left_pane[1], // List
            inner[1],     // Details area (info + url radio group)
            outter[1],    // Error
        ]
    }
}
