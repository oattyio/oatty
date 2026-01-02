use std::sync::{Arc, Mutex};

use crate::{
    app::App,
    ui::{
        components::{Component, common::ConfirmationModalOpts, library::CatalogProjection},
        theme::{
            Theme,
            theme_helpers::{ButtonRenderOptions, block, create_checkbox, create_list_with_highlight, render_button},
        },
    },
};
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use oatty_registry::CommandRegistry;
use oatty_types::{Effect, ExecOutcome, Modal, Msg, Severity, manifest::RegistryCatalog};
use rat_focus::FocusFlag;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Borders, ListItem, Paragraph},
};
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Default)]
pub struct LibraryLayout {
    pub header: Rect,
    pub import_button: Rect,
    pub remove_button: Rect,
    pub list_area: Rect,
    pub details_area: Rect,
}

impl From<Vec<Rect>> for LibraryLayout {
    fn from(rects: Vec<Rect>) -> Self {
        Self {
            header: rects[0],
            import_button: rects[1],
            remove_button: rects[2],
            list_area: rects[3],
            details_area: rects[4],
        }
    }
}

#[derive(Debug, Default)]
pub struct LibraryComponent {
    staged_catalogs: Vec<RegistryCatalog>,
    projections: Vec<CatalogProjection>,

    layout: LibraryLayout,
}

impl LibraryComponent {
    pub fn new(command_registry: Arc<Mutex<CommandRegistry>>) -> Self {
        Self {
            projections: Self::create_projections(command_registry),
            ..Default::default()
        }
    }

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
        let maybe_selected_index = app.library.selected_index();
        let remove_button_opts = ButtonRenderOptions {
            selected: false,
            focused: app.library.f_remove_button.get(),
            borders: Borders::ALL,
            enabled: maybe_selected_index.is_some(),
            is_primary: false,
        };
        render_button(frame, layout.1, "Remove", theme, remove_button_opts);
    }

    fn render_list(&self, frame: &mut Frame, area: Rect, app: &mut App) {
        let theme = &*app.ctx.theme;
        let is_list_focused = app.library.f_list_view.get() || app.library.f_selection_checkbox.get();
        let list_block = block::<String>(theme, None, is_list_focused);
        let list_inner = list_block.inner(area);
        let list_items = self
            .build_list_items(app, list_inner.width as usize)
            .unwrap_or(vec![ListItem::new("Use Import to add new items")]);

        let list = create_list_with_highlight(list_items, theme, is_list_focused, Some(list_block));
        frame.render_stateful_widget(list, area, app.library.list_state_mut());
    }

    fn render_details(&self, frame: &mut Frame, area: Rect, app: &mut App) {
        let theme = &*app.ctx.theme;
    }

    /// Renders the list items for the library component.
    fn build_list_items(&self, app: &App, list_inner: usize) -> Option<Vec<ListItem<'static>>> {
        let mouse_over_index = app.library.mouse_over_index();
        let selected_index = app.library.selected_index();
        let mut list_items = Vec::new();

        for (index, catalog) in self.projections.iter().enumerate() {
            let is_checkbox_focused = selected_index == Some(index) && app.library.f_selection_checkbox.get();
            let mut list_item = self.build_list_item(catalog, is_checkbox_focused, list_inner, &*app.ctx.theme);
            if mouse_over_index.is_some_and(|hover| hover == index) {
                list_item = list_item.style(app.ctx.theme.selection_style().add_modifier(Modifier::BOLD));
            }
            list_items.push(list_item);
        }

        Some(list_items)
    }

    /// Renders a single list item for the library component.
    fn build_list_item(
        &self,
        catalog: &CatalogProjection,
        is_checkbox_focused: bool,
        list_inner: usize,
        theme: &dyn Theme,
    ) -> ListItem<'static> {
        let style = if catalog.is_staged {
            theme.status_warning()
        } else {
            theme.status_success()
        };
        let mut check_box_line = create_checkbox(None, catalog.is_enabled, is_checkbox_focused, theme);
        let title = catalog.title.clone();
        let remaining_width = list_inner.saturating_sub(title.width() + 8); // +8 for the enabled/disabled status
        check_box_line.push_span(Span::styled(format!(" {}", title), style));

        let status = if catalog.is_enabled { "enabled" } else { "disabled" };
        check_box_line.push_span(Span::styled(format!("{:>remaining_width$}", status), theme.text_muted_style()));
        ListItem::new(check_box_line).style(theme.text_primary_style())
    }

    fn remove_projection(&mut self, app: &mut App, idx: usize) -> Vec<Effect> {
        let projection = self.projections.remove(idx);
        if projection.is_staged
            && self
                .staged_catalogs
                .get(idx)
                .is_some_and(|catalog| catalog.title == projection.title)
        {
            self.staged_catalogs.remove(idx);
        } else if !projection.is_staged {
            let message = format!("Are you sure you want to remove '{}'?", projection.title);
            let buttons = vec![
                ("Yes".to_string(), FocusFlag::default()),
                ("Cancel".to_string(), FocusFlag::default()),
            ];
            app.confirmation_modal_state.update_opts(ConfirmationModalOpts {
                title: Some("Confirm Destructive Action".to_string()),
                message: Some(message),
                severity: Some(Severity::Warning),
                buttons,
            });
            return vec![Effect::ShowModal(Modal::Confirmation)];
        }
        vec![]
    }
}

impl Component for LibraryComponent {
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let layout = LibraryLayout::from(self.get_preferred_layout(app, rect));
        frame.render_widget(Paragraph::new(Line::from("Library")), layout.header);

        self.render_buttons(frame, (layout.import_button, layout.remove_button), app);
        self.render_list(frame, layout.list_area, app);
        self.render_details(frame, layout.details_area, app);

        self.layout = layout;
    }

    fn handle_message(&mut self, app: &mut App, msg: Msg) -> Vec<Effect> {
        if let Msg::ExecCompleted(outcome) = msg {
            match *outcome {
                ExecOutcome::FileContents(contents, _) | ExecOutcome::RemoteFileContents(contents, _) => {
                    return vec![Effect::ImportRegistryCatalog(contents)];
                }
                ExecOutcome::RegistryCatalog(catalog) => {
                    let mut projection = CatalogProjection::from(&catalog);
                    projection.is_staged = true;
                    self.projections.push(projection);
                    self.staged_catalogs.push(catalog);
                    app.library.set_selected_index(Some(self.staged_catalogs.len().saturating_sub(1)));
                }
                _ => {}
            }
        }
        Vec::new()
    }

    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
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
                    return vec![Effect::ShowModal(Modal::FilePicker(vec!["json", "yml", "yaml"]))];
                }
                // Remove button
                if let Some(idx) = app.library.selected_index()
                    && app.library.f_remove_button.get()
                    && idx < self.projections.len()
                {
                    return self.remove_projection(app, idx);
                }
                // Selection checkbox
                if app.library.f_selection_checkbox.get() {
                    if let Some(idx) = app.library.selected_index()
                        && idx < self.projections.len()
                    {
                        let projection = self.projections.get_mut(idx).unwrap();
                        projection.is_enabled = !projection.is_enabled;
                    }
                }
            }
            KeyCode::Up => {
                app.library.list_state_mut().select_previous();
            }
            KeyCode::Down => {
                app.library.list_state_mut().select_next();
            }
            _ => {}
        }
        Vec::new()
    }

    fn get_preferred_layout(&self, _app: &App, area: Rect) -> Vec<Rect> {
        let outter = Layout::vertical([
            Constraint::Length(1), // Header
            Constraint::Min(1),    // Bottom pane
        ])
        .split(area);

        let inner = Layout::horizontal([
            Constraint::Fill(2), // Left pane
            Constraint::Fill(5), // Right pane
        ])
        .split(outter[1]);

        let left_pane = Layout::vertical([
            Constraint::Length(3), // Buttons
            Constraint::Min(1),    // List
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
            outter[0],    // Header
            buttons[1],   // Import button
            buttons[3],   // Remove button
            left_pane[1], // List
            inner[1],     // Details area
        ]
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let pos = Position {
            x: mouse.column,
            y: mouse.row,
        };
        let hit_test_list = self.layout.list_area.contains(pos);
        let list_offset = app.library.offset();

        let idx = if hit_test_list {
            Some(pos.y.saturating_sub(self.layout.list_area.y + 1) as usize + list_offset)
        } else {
            None
        };

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if hit_test_list {
                    app.focus.focus(&app.library.f_list_view);
                    // Check if the click is within the range of the checkbox
                    if let Some(index) = idx
                        && (3..=5).contains(&pos.x.saturating_sub(self.layout.list_area.x))
                        && index < self.projections.len()
                    {
                        let projection = self.projections.get_mut(index).unwrap();
                        projection.is_enabled = !projection.is_enabled;
                    } else {
                        app.library.set_selected_index(idx);
                    }
                }
            }
            MouseEventKind::Moved | MouseEventKind::Up(MouseButton::Left) => {
                app.library.set_mouse_over_index(idx);
            }
            _ => {}
        }

        Vec::new()
    }
}
