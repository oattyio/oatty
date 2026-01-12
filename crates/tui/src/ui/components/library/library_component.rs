use std::{
    rc::Rc,
    sync::{Arc, Mutex},
    vec,
};

use crate::{
    app::App,
    ui::{
        components::{
            Component,
            common::{ConfirmationModalOpts, key_value_editor::KeyValueEditorView},
            library::CatalogProjection,
        },
        theme::{
            Theme,
            theme_helpers::{
                self, ButtonRenderOptions, block, create_checkbox, create_list_with_highlight, create_radio_button, render_button,
            },
        },
    },
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use oatty_registry::CommandRegistry;
use oatty_types::{Effect, ExecOutcome, Modal, Msg, Severity};
use oatty_util::line_clamp;
use rat_focus::{FocusFlag, HasFocus};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Padding, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Default)]
pub struct LibraryLayout {
    pub import_button: Rect,
    pub remove_button: Rect,
    pub api_list_area: Rect,
    pub errors_area: Rect,
    pub details_area: Rect,
    pub base_url_area: Rect,
    pub kv_area: Rect,
}

impl From<Vec<Rect>> for LibraryLayout {
    fn from(rects: Vec<Rect>) -> Self {
        Self {
            import_button: rects[0],
            remove_button: rects[1],
            api_list_area: rects[2],
            details_area: rects[3],
            errors_area: rects[4],
            // Dynamically calculated
            base_url_area: Rect::default(),
            kv_area: Rect::default(),
        }
    }
}

#[derive(Debug, Default)]
pub struct LibraryComponent {
    layout: LibraryLayout,
    kv_view: KeyValueEditorView,
}

impl LibraryComponent {
    pub fn create_projections(command_registry: Arc<Mutex<CommandRegistry>>) -> Vec<CatalogProjection> {
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
    fn render_buttons(&mut self, frame: &mut Frame, layout: (Rect, Rect), app: &App) {
        let theme = &*app.ctx.theme;
        let import_button_opts = ButtonRenderOptions {
            selected: false,
            focused: app.library.f_import_button.get(),
            borders: Borders::ALL,
            enabled: true,
            is_primary: true,
        };

        render_button(frame, layout.0, "Import", theme, import_button_opts);
        let maybe_selected_index = app.library.api_selected_index();
        let remove_button_opts = ButtonRenderOptions {
            selected: false,
            focused: app.library.f_remove_button.get(),
            borders: Borders::ALL,
            enabled: maybe_selected_index.is_some(),
            is_primary: false,
        };
        render_button(frame, layout.1, "Remove", theme, remove_button_opts);
    }

    fn render_api_list(&mut self, frame: &mut Frame, area: Rect, app: &mut App) {
        let is_list_focused = app.library.f_api_list.get();
        let list_block = block::<String>(&*app.ctx.theme, None, is_list_focused);
        let list_inner = list_block.inner(area);
        let list_items = self
            .build_api_list_items(app, list_inner.width as usize)
            .unwrap_or(vec![ListItem::new("Use Import to add new items")]);

        if app.library.api_selected_index().is_none() && !list_items.is_empty() {
            app.library.set_api_selected_index(Some(0));
        }

        let list = create_list_with_highlight(list_items, &*app.ctx.theme, is_list_focused, Some(list_block));
        frame.render_stateful_widget(list, area, app.library.api_list_state_mut());
    }

    fn render_error(&self, frame: &mut Frame, area: Rect, app: &mut App) {
        let theme = &*app.ctx.theme;
        if let Some(error_message) = app.library.error_message() {
            let error_paragraph = Paragraph::new(error_message).style(theme.status_error());
            frame.render_widget(error_paragraph, area);
        }
    }

    fn render_details(&mut self, frame: &mut Frame, layout: &mut LibraryLayout, app: &mut App) {
        let area = layout.details_area;
        let Some(projection) = app.library.selected_projection() else {
            frame.render_widget(Paragraph::new("Select an item to configure"), area);
            return;
        };
        let description = line_clamp(projection.description.as_ref(), 3, area.width.saturating_sub(2) as usize);
        let theme = &*app.ctx.theme;
        let (title_style, enabled_text) = if projection.is_enabled {
            (theme.status_success(), "enabled")
        } else {
            (theme.text_muted_style(), "disabled")
        };
        let summary_lines = vec![
            Line::from(vec![
                Span::styled(projection.title.clone(), title_style.add_modifier(Modifier::BOLD)),
                Span::styled(format!(" ({})", enabled_text), theme.text_muted_style()),
            ]),
            Line::from(vec![
                Span::styled("Command Prefix: ", theme.text_primary_style()),
                Span::styled(projection.vendor.clone(), theme.syntax_type_style()),
            ]),
            Line::from(vec![
                Span::styled("Endpoints: ", theme.text_primary_style()),
                Span::styled(projection.command_count.to_string(), theme.syntax_number_style()),
            ]),
            Line::from(vec![
                Span::styled("Workflows: ", theme.text_primary_style()),
                Span::styled(projection.workflow_count.to_string(), theme.syntax_number_style()),
            ]),
            Line::from(vec![
                Span::styled("Value providers: ", theme.text_primary_style()),
                Span::styled(projection.provider_contract_count.to_string(), theme.syntax_number_style()),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(description, theme.syntax_type_style())]),
            Line::from(""),
        ];
        let block = Block::new().padding(Padding::horizontal(1));
        let inner = block.inner(area);
        let summary = Paragraph::new(summary_lines).wrap(Wrap { trim: true }).block(block);
        let line_ct = summary.line_count(inner.width) as u16;
        frame.render_widget(summary, area);

        // add / remove buttons
        let mut remaining_area = area;
        remaining_area.y = area.y + line_ct;
        remaining_area.height = area.height.saturating_sub(line_ct);

        let remaining_area_layout = Layout::vertical([
            Constraint::Length(projection.base_urls.len() as u16 + 2), // base url input
            Constraint::Percentage(100),                               // kv editor
        ])
        .split(remaining_area);

        layout.base_url_area = remaining_area_layout[0];
        self.render_base_url_radios(frame, remaining_area_layout[0], app);

        layout.kv_area = remaining_area_layout[1];
        let kv_state = app.library.kv_state_mut();
        self.kv_view
            .render_with_state(frame, remaining_area_layout[1], &*app.ctx.theme, kv_state);
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
            } else {
                theme.text_muted_style()
            };
            radio.push_span(Span::styled(format!(" {}", url), label_theme));
            radio_group.push(ListItem::from(radio));
        }
        let list = List::new(radio_group).highlight_style(theme.accent_primary_style()).block(block(
            theme,
            Some("Active base URL:"),
            app.library.f_url_list.get(),
        ));

        frame.render_stateful_widget(list, area, app.library.url_list_state_mut());
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

        let message = format!("Are you sure you want to remove '{}'?", projection.title);
        let buttons = vec![
            ("Yes".to_string(), app.library.f_modal_confirmation_button.clone()),
            ("Cancel".to_string(), FocusFlag::default()),
        ];
        app.confirmation_modal_state.update_opts(ConfirmationModalOpts {
            title: Some("Confirm Destructive Action".to_string()),
            message: Some(message),
            severity: Some(Severity::Warning),
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
                app.library.set_error_message(None);
                return vec![Effect::ImportRegistryCatalog(contents)];
            }
            ExecOutcome::RegistryCatalogGenerated(catalog) => {
                app.library.set_error_message(None);
                app.library.push_projection(CatalogProjection::from(&catalog));
            }

            ExecOutcome::RegistryCatalogGenerationError(message) => {
                app.library.set_error_message(Some(message));
            }

            ExecOutcome::RegistryConfigSaveError(message) => {
                app.library.set_error_message(Some(message));
                app.library.clear_projections();
            }

            ExecOutcome::RegistryConfigSaved => {
                app.library.set_error_message(None);
                app.library.clear_projections();
                app.library.kv_state_mut().reset_dirty();
            }

            _ => {}
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
            && (3..=5).contains(&pos.x.saturating_sub(self.layout.api_list_area.x))
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
        let idx = pos.y.saturating_sub(self.layout.base_url_area.y + 1) as usize + app.library.url_list_offset();
        // if we're over a radio button, click it without changing list selection
        if (1..=3).contains(&pos.x.saturating_sub(self.layout.base_url_area.x)) {
            return Self::update_base_url(idx, app);
        } else {
            app.library.set_url_selected_index(Some(idx));
        }
        Vec::new()
    }

    fn handle_import(&self) -> Vec<Effect> {
        vec![Effect::ShowModal(Modal::FilePicker(vec!["json", "yml", "yaml"]))]
    }
}

impl Component for LibraryComponent {
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let mut layout = LibraryLayout::from(self.get_preferred_layout(app, rect));

        self.render_buttons(frame, (layout.import_button, layout.remove_button), app);
        self.render_api_list(frame, layout.api_list_area, app);
        self.render_error(frame, layout.errors_area, app);
        self.render_details(frame, &mut layout, app);

        self.layout = layout;
    }

    fn handle_message(&mut self, app: &mut App, msg: Msg) -> Vec<Effect> {
        match msg {
            Msg::ConfirmationModalButtonClicked(button_id) => self.handle_modal_button_click(button_id, app),
            Msg::ExecCompleted(outcome) => self.handle_exec_completed(*outcome, app),
            _ => Vec::new(),
        }
    }

    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        if app.library.kv_state().is_focused() {
            self.kv_view
                .handle_key_event(app.library.kv_state_mut(), key, Rc::clone(&app.focus));
            // Losing focus will trigger an auto-save for headers if dirty
            if app.library.kv_state().focus().lost()
                && app.library.kv_state().is_dirty()
                && let Some(projection) = app.library.selected_projection()
            {
                return vec![Effect::UpdateCatalogHeaders {
                    title: projection.title.clone(),
                    headers: app.library.kv_state().rows().clone(),
                }];
            }
            return Vec::new();
        }

        match key.code {
            KeyCode::Tab => {
                app.focus.next();
            }
            KeyCode::BackTab => {
                app.focus.prev();
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                // Import button
                if app.library.f_import_button.get() {
                    return self.handle_import();
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
            KeyCode::Char('i') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return self.handle_import();
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(idx) = app.library.api_selected_index() {
                    return self.prompt_remove_catalog(app, idx);
                }
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
            _ => {}
        }
        Vec::new()
    }

    fn get_preferred_layout(&self, _app: &App, area: Rect) -> Vec<Rect> {
        let outter = Layout::vertical([
            Constraint::Percentage(100), // Content
            Constraint::Length(1),       // status/error
        ])
        .split(area);

        let inner = Layout::horizontal([
            Constraint::Percentage(30), // Left pane
            Constraint::Percentage(70), // Right pane
        ])
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

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let pos = Position {
            x: mouse.column,
            y: mouse.row,
        };

        if self.layout.kv_area.contains(pos) {
            if let MouseEventKind::Down(MouseButton::Left) = mouse.kind
                && !app.library.kv_state().is_focused()
            {
                app.focus.focus(app.library.kv_state());
            }
            self.kv_view
                .handle_mouse_event(app.library.kv_state_mut(), mouse, Rc::clone(&app.focus));
            return Vec::new();
        }
        let hit_test_api_list = self.layout.api_list_area.contains(pos);
        let hit_test_url_radios = self.layout.base_url_area.contains(pos);
        let maybe_api_list_idx = if hit_test_api_list {
            Some(pos.y.saturating_sub(self.layout.api_list_area.y + 1) as usize + app.library.api_list_offset())
        } else {
            None
        };

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                match () {
                    _ if hit_test_api_list => {
                        return self.handle_api_list_mouse_down(pos, maybe_api_list_idx, app);
                    }
                    _ if hit_test_url_radios => {
                        return self.handle_url_radio_mouse_down(pos, app);
                    }
                    _ if self.layout.import_button.contains(pos) => {
                        app.focus.focus(&app.library.f_import_button);
                        return self.handle_import();
                    }
                    () => {}
                }

                if let Some(idx) = app.library.api_selected_index()
                    && self.layout.remove_button.contains(pos)
                {
                    app.focus.focus(&app.library.f_remove_button);
                    return self.prompt_remove_catalog(app, idx);
                }

                if app.library.kv_state().is_focused() {
                    self.kv_view
                        .handle_mouse_event(app.library.kv_state_mut(), mouse, Rc::clone(&app.focus));
                }
            }
            MouseEventKind::Moved | MouseEventKind::Up(MouseButton::Left) => {
                app.library.api_set_mouse_over_index(maybe_api_list_idx);
            }
            _ => {}
        }

        Vec::new()
    }

    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let mut hints = Vec::with_capacity(4);

        if app.library.f_api_list.get() {
            hints.push(("↑/↓", " Navigate "))
        }

        hints.push(("Ctrl+I", " Import "));
        if app.library.api_selected_index().is_some() {
            hints.push(("Ctrl+R", " Remove "));

            if app.library.f_api_list.get() {
                hints.push(("Enter/Space", " Toggle enabled "))
            }
        }

        if let (Some(projection), Some(idx)) = (app.library.selected_projection(), app.library.url_selected_index())
            && idx != projection.base_url_index
        {
            hints.push(("Space/Enter", " select "));
        }

        let mut spans = theme_helpers::build_hint_spans(&*app.ctx.theme, &hints);

        if app.library.kv_state().is_focused() {
            self.kv_view.add_table_hints(&mut spans, &*app.ctx.theme);
        }

        spans
    }
}
