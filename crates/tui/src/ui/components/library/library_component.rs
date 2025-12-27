use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use oatty_types::{Effect, ExecOutcome, Modal, Msg};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Borders, ListItem, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    app::App,
    ui::{
        components::Component,
        theme::theme_helpers::{ButtonRenderOptions, block, create_list_with_highlight, render_button},
        utils::truncate_to_width,
    },
};

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
pub struct LibraryComponent;

impl LibraryComponent {
    fn build_list_items(&self, app: &App, width: usize, selected_index: Option<usize>) -> Option<Vec<ListItem<'static>>> {
        let lock = app.ctx.command_registry.try_lock().ok()?;
        let catalogs = lock.config.catalogs.as_ref()?;
        let list_items = catalogs
            .iter()
            .enumerate()
            .map(|(index, catalog)| {
                let mut w = width;
                let mut spans = Vec::with_capacity(2);
                let name = catalog.name.clone();
                w = w.saturating_sub(UnicodeWidthStr::width(name.as_str()));
                spans.push(Span::styled(name, app.ctx.theme.text_primary_style()));

                let mut base_url = format!(" ({})", catalog.base_url);
                let available = w.saturating_sub(UnicodeWidthStr::width(base_url.as_str()));
                if available == 0 {
                    base_url = truncate_to_width(base_url.as_str(), width as u16);
                }
                spans.push(Span::styled(base_url, app.ctx.theme.text_secondary_style()));

                ListItem::new(Line::from(spans)).style(app.ctx.theme.text_primary_style())
            })
            .collect();

        Some(list_items)
    }
}

impl Component for LibraryComponent {
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let layout = LibraryLayout::from(self.get_preferred_layout(app, rect));
        frame.render_widget(Paragraph::new(Line::from("Library")), layout.header);
        let theme = &*app.ctx.theme;
        let import_button_opts = ButtonRenderOptions {
            selected: false,
            focused: app.library.f_import_button.get(),
            borders: Borders::ALL,
            enabled: true,
            is_primary: true,
        };
        render_button(frame, layout.import_button, "Import", theme, import_button_opts);
        let maybe_selected_index = app.library.selected_index();
        let remove_button_opts = ButtonRenderOptions {
            selected: false,
            focused: app.library.f_remove_button.get(),
            borders: Borders::ALL,
            enabled: maybe_selected_index.is_some(),
            is_primary: false,
        };
        render_button(frame, layout.remove_button, "Remove", theme, remove_button_opts);

        let list_items = self
            .build_list_items(app, layout.list_area.width as usize, maybe_selected_index)
            .unwrap_or(vec![ListItem::new("Use Import to add new items")]);

        let is_list_focused = app.library.f_list_view.get();
        let list_block = block::<String>(theme, None, is_list_focused);
        let list = create_list_with_highlight(&list_items, theme, is_list_focused, Some(list_block));
        frame.render_stateful_widget(list, layout.list_area, app.library.list_state_mut());
    }

    fn handle_message(&mut self, app: &mut App, msg: &Msg) -> Vec<Effect> {
        if let Msg::ExecCompleted(outcome) = msg {
            match outcome.as_ref() {
                ExecOutcome::FileContents(file_path, contents) => {}
                ExecOutcome::RemoteFileContents(url, contents) => {}
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
            KeyCode::Enter => {
                if app.library.f_import_button.get() {
                    return vec![Effect::ShowModal(Modal::FilePicker(vec!["json", "yml", "yaml"]))];
                }
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
            Constraint::Fill(1), // Left pane
            Constraint::Fill(3), // Right pane
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
        Vec::new()
    }
}
