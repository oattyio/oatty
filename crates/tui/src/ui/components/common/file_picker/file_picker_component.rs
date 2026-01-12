//! Interactive modal that lets the user browse and preview files before importing them.

use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use oatty_types::{Effect, ExecOutcome, Msg};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, List, Paragraph},
};
use url::Url;

use crate::{
    app::App,
    ui::{
        components::{Component, common::file_picker::state::Shortcut, find_target_index_by_mouse_position},
        theme::theme_helpers::{ButtonRenderOptions, block, build_hint_spans, render_button},
    },
};

/// Layout helper that stores the resolved rectangles for each render region.
#[derive(Debug, Clone, Default)]
pub struct FilePickerLayout {
    shortcut_bar_area: Rect,
    header_area: Rect,
    header_inner_area: Rect,
    file_list_area: Rect,
    preview_area: Rect,
    error_message_area: Rect,
    cancel_button_area: Rect,
    open_button_area: Rect,
}

impl From<&[Rect]> for FilePickerLayout {
    fn from(layout: &[Rect]) -> Self {
        FilePickerLayout {
            shortcut_bar_area: layout[0],
            header_area: layout[1],
            header_inner_area: Rect::default(),
            file_list_area: layout[2],
            preview_area: layout[3],
            error_message_area: layout[4],
            cancel_button_area: layout[5],
            open_button_area: layout[6],
        }
    }
}

/// Controller + renderer for the shared file picker modal.
#[derive(Debug, Clone, Default)]
pub struct FilePickerModal {
    layout: FilePickerLayout,
    shortcut_rects: Vec<Rect>,
}
/// Component responsible for rendering the file picker and handling user input.
impl FilePickerModal {
    fn render_shortcuts(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) -> Option<()> {
        let theme = &*app.ctx.theme;
        let block = Block::bordered().style(theme.border_style(false));
        let inner = block.inner(rect);

        frame.render_widget(block, rect);
        self.shortcut_rects.clear();
        let file_picker = app.file_picker.as_ref()?;
        let shortcuts = file_picker.shortcuts().iter().enumerate();
        let shortcut_focus = &file_picker.shortcuts_focus;
        let selected_idx = file_picker.selected_shortcut_idx();
        for (index, item) in shortcuts {
            let Shortcut { name, .. } = item;
            let is_focused = shortcut_focus.get(index).is_some_and(|f| f.get());
            let is_selected = index == selected_idx;
            let area = Rect {
                x: inner.x,
                y: inner.y + (3 * index as u16),
                width: inner.width,
                height: 3,
            };
            let borders = if is_focused { Borders::ALL } else { Borders::NONE };
            let options = ButtonRenderOptions::new(true, is_focused, is_selected, borders, false);
            if area.y + area.height > inner.y + inner.height {
                continue;
            }
            render_button(frame, area, name, &*app.ctx.theme, options);
            self.shortcut_rects.push(area);
        }

        Some(())
    }

    fn render_header(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) -> Option<Rect> {
        let file_picker = app.file_picker.as_mut()?;
        let is_focused = file_picker.f_path_input.get();
        let theme = &*app.ctx.theme;
        let title = Line::from(Span::styled("Path/URL", theme.text_secondary_style().add_modifier(Modifier::BOLD)));
        let mut input_block = block::<String>(theme, None, is_focused);
        input_block = input_block.title(title);
        let inner_area = input_block.inner(rect);

        let path_input_state = file_picker.path_input_state_mut();
        let path_query = path_input_state.input();
        let content_line = if is_focused || !path_query.is_empty() {
            Line::from(Span::styled(path_query.to_string(), theme.text_primary_style()))
        } else {
            Line::from(Span::from(""))
        };
        let search_paragraph = Paragraph::new(content_line).style(theme.text_primary_style()).block(input_block);
        frame.render_widget(search_paragraph, rect);
        if is_focused {
            let cursor_columns = path_input_state.cursor_columns() as u16;
            let cursor_x = inner_area.x.saturating_add(cursor_columns);
            let cursor_y = inner_area.y;
            frame.set_cursor_position((cursor_x, cursor_y));
        }

        Some(inner_area)
    }

    fn render_list(&self, frame: &mut Frame, rect: Rect, app: &mut App) -> Option<()> {
        let file_picker = app.file_picker.as_mut()?;

        let mut list_items = file_picker.list_items().to_vec();
        let is_focused = file_picker.f_list.get();
        if let Some(idx) = file_picker.mouse_over_idx()
            && list_items.len() > idx
        {
            let list_item = list_items[idx]
                .clone()
                .style(app.ctx.theme.selection_style().add_modifier(Modifier::BOLD));
            list_items[idx] = list_item;
        }

        let list_block = Block::new()
            .borders(Borders::LEFT)
            .border_style(app.ctx.theme.border_style(is_focused));
        let list = List::new(list_items)
            .block(list_block)
            .highlight_style(app.ctx.theme.selection_style());
        frame.render_stateful_widget(list, rect, file_picker.list_state_mut());
        Some(())
    }

    fn render_preview(&self, frame: &mut Frame, area: Rect, app: &mut App) -> Option<()> {
        let file_picker = app.file_picker.as_ref()?;
        let is_focused = file_picker.f_preview.get();
        let p_block = Block::new()
            .borders(Borders::LEFT)
            .border_style(app.ctx.theme.border_style(is_focused));
        let Some(file_contents) = file_picker.file_contents() else {
            frame.render_widget(
                Paragraph::new("No preview available")
                    .style(app.ctx.theme.status_info())
                    .block(p_block),
                area,
            );
            return None;
        };
        let line_indices = file_picker.line_indices();
        let offset = file_picker.preview_scroll_offset() as usize;
        let len = line_indices.len().min(offset + area.height as usize);

        let begin = line_indices[offset].0;
        let end = line_indices[len - 1].1;
        let slice = file_contents.get(begin..end)?;

        let preview = Paragraph::new(slice).block(p_block).style(app.ctx.theme.text_primary_style());
        frame.render_widget(preview, area);
        Some(())
    }

    fn render_error_message(&self, frame: &mut Frame, area: Rect, app: &mut App) -> Option<()> {
        let error_message = app.file_picker.as_ref()?.user_input_error();
        if let Some(error_message) = error_message {
            let error_message = Paragraph::new(error_message).style(app.ctx.theme.status_error());
            frame.render_widget(error_message, area);
        }
        Some(())
    }

    fn render_buttons(&self, frame: &mut Frame, layout: &FilePickerLayout, app: &mut App) -> Option<()> {
        let file_picker = app.file_picker.as_ref()?;
        let options = ButtonRenderOptions::new(true, file_picker.f_cancel.get(), false, Borders::ALL, false);
        render_button(frame, layout.cancel_button_area, "Cancel", &*app.ctx.theme, options);

        let selected = file_picker.selected_file().is_some() || file_picker.is_path_input_valid();
        let options = ButtonRenderOptions::new(selected, file_picker.f_confirm.get(), false, Borders::ALL, false);
        render_button(frame, layout.open_button_area, "Open", &*app.ctx.theme, options);

        Some(())
    }

    fn handle_maybe_button_click(&mut self, pos: Position, app: &mut App) -> Option<Vec<Effect>> {
        let file_picker = &mut app.file_picker.as_mut()?;
        let shortcut_area = &self.layout.shortcut_bar_area;
        if let Some(shortcut_idx) = find_target_index_by_mouse_position(shortcut_area, &self.shortcut_rects, pos.x, pos.y) {
            let path = file_picker.shortcut_pressed(shortcut_idx)?;
            app.focus.focus(file_picker.shortcuts_focus().get(shortcut_idx)?);
            return Some(vec![Effect::ListDirectoryContents(path)]);
        }

        if self.layout.cancel_button_area.contains(pos) {
            return Some(vec![Effect::CloseModal]);
        }

        if self.layout.open_button_area.contains(pos) {
            return self.maybe_commit_selection(app);
        }
        None
    }

    fn maybe_commit_selection(&mut self, app: &mut App) -> Option<Vec<Effect>> {
        let file_picker = app.file_picker.as_mut()?;
        if let Some(selected_file) = file_picker.selected_file().cloned() {
            if selected_file.is_directory {
                file_picker.set_cur_dir(Some(selected_file.path.clone()));
                return Some(vec![Effect::ListDirectoryContents(selected_file.path)]);
            } else {
                return Some(vec![Effect::CloseModal, Effect::ReadFileContents(selected_file.path)]);
            }
        }

        if file_picker.is_path_input_valid() {
            let input = file_picker.user_input()?;
            if let Ok(url) = Url::parse(input) {
                return Some(vec![Effect::CloseModal, Effect::ReadRemoteFileContents(url)]);
            }
            if Path::new(input).try_exists().is_ok() {
                let path_buf = PathBuf::from(input);
                if path_buf.is_dir() {
                    file_picker.set_cur_dir(Some(path_buf.clone()));
                    return Some(vec![Effect::ListDirectoryContents(path_buf)]);
                }
                return Some(vec![Effect::CloseModal, Effect::ReadFileContents(path_buf)]);
            }
        } else {
            file_picker.set_user_input_error(Some("Invalid path or url".to_string()));
        }
        None
    }
}

impl Component for FilePickerModal {
    fn handle_message(&mut self, app: &mut App, msg: Msg) -> Vec<Effect> {
        let (Msg::ExecCompleted(outcome), Some(file_picker)) = (msg, app.file_picker.as_mut()) else {
            return Vec::new();
        };

        match *outcome {
            ExecOutcome::FileContents(contents, path) if file_picker.selected_file().is_some_and(|f| f.path == *path) => {
                file_picker.set_file_contents(contents);
            }
            ExecOutcome::DirectoryContents { entries, root_path } if file_picker.cur_dir().is_some_and(|f| *f == root_path) => {
                file_picker.set_dir_contents(Some(entries));
                file_picker.rebuild_list_items(&*app.ctx.theme);
            }
            _ => {}
        }

        Vec::new()
    }

    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        let Some(file_picker) = app.file_picker.as_mut() else {
            return Vec::new();
        };

        match key.code {
            KeyCode::Char(' ') | KeyCode::Enter if file_picker.f_list.get() || file_picker.f_confirm.get() => {
                if let Some(effects) = self.maybe_commit_selection(app) {
                    return effects;
                }
            }

            KeyCode::Char(character)
                if file_picker.f_path_input.get()
                    && !character.is_control()
                    && (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT) =>
            {
                file_picker.insert_path_char(character);
                file_picker.set_selected_index(None);
            }

            KeyCode::Backspace if file_picker.f_path_input.get() => {
                file_picker.backspace_path_char();
            }

            KeyCode::Delete if file_picker.f_path_input.get() => {
                file_picker.delete_path_char();
            }

            KeyCode::Right if file_picker.f_path_input.get() => {
                file_picker.path_input_state_mut().move_right();
            }

            KeyCode::Left if file_picker.f_path_input.get() => {
                file_picker.path_input_state_mut().move_left();
            }

            KeyCode::Down if file_picker.f_list.get() => {
                if let Some(entry) = file_picker.select_next()
                    && !entry.is_directory
                {
                    return vec![Effect::ReadFileContents(entry.path.clone())];
                }
            }

            KeyCode::Down if file_picker.f_preview.get() => {
                file_picker.scroll_preview_down_by(1, self.layout.preview_area.height);
            }

            KeyCode::Up if file_picker.f_list.get() => {
                if let Some(entry) = file_picker.select_previous()
                    && !entry.is_directory
                {
                    return vec![Effect::ReadFileContents(entry.path.clone())];
                }
            }

            KeyCode::Up if file_picker.f_preview.get() => {
                file_picker.scroll_preview_up_by(1);
            }

            KeyCode::Tab => {
                app.focus.next();
            }

            KeyCode::BackTab => {
                app.focus.prev();
            }

            KeyCode::Esc => {
                return vec![Effect::CloseModal];
            }

            _ => {}
        }

        Vec::new()
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let Some(file_picker) = app.file_picker.as_mut() else {
            return Vec::new();
        };
        let pos = Position::new(mouse.column, mouse.row);
        let list_area = &self.layout.file_list_area;
        let hit_test_list = list_area.contains(pos);
        let hit_test_preview = self.layout.preview_area.contains(pos);
        let hit_test_path_input = self.layout.header_area.contains(pos);
        let list_offset = file_picker.list_state_offset();
        let idx = if hit_test_list {
            Some(pos.y as usize - list_area.y as usize + list_offset)
        } else {
            None
        };

        match mouse.kind {
            MouseEventKind::Moved | MouseEventKind::Up(MouseButton::Left) => {
                file_picker.set_mouse_over_idx(idx);
            }
            MouseEventKind::ScrollDown => {
                if hit_test_list {
                    file_picker.list_state_mut().scroll_down_by(1);
                } else if hit_test_preview {
                    file_picker.scroll_preview_down_by(1, self.layout.preview_area.height);
                }
            }
            MouseEventKind::ScrollUp => {
                if hit_test_list {
                    file_picker.list_state_mut().scroll_up_by(1);
                } else if hit_test_preview {
                    file_picker.scroll_preview_up_by(1);
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                match () {
                    _ if hit_test_list => {
                        if !file_picker.f_list.get() {
                            app.focus.focus(&file_picker.f_list);
                        }
                        // set_selected_index also updates the cur_dir
                        if let Some(entry) = file_picker.set_selected_index(idx) {
                            return if entry.is_directory {
                                vec![Effect::ListDirectoryContents(entry.path.clone())]
                            } else {
                                vec![Effect::ReadFileContents(entry.path.clone())]
                            };
                        }
                    }
                    _ if hit_test_path_input => {
                        let relative_column = mouse.column.saturating_sub(self.layout.header_inner_area.x);
                        let path_input_state = file_picker.path_input_state_mut();
                        let cursor_index = path_input_state.cursor_index_for_column(relative_column);
                        path_input_state.set_cursor(cursor_index);
                        app.focus.focus(&file_picker.f_path_input);
                    }
                    _ => return self.handle_maybe_button_click(pos, app).unwrap_or_default(),
                }
            }

            _ => {}
        }
        Vec::new()
    }

    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let block = block(&*app.ctx.theme, Some("File Picker"), true);

        let mut layout = FilePickerLayout::from(self.get_preferred_layout(app, block.inner(rect)).as_slice());
        frame.render_widget(block, rect);
        self.render_shortcuts(frame, layout.shortcut_bar_area, app);
        if let Some(inner_area) = self.render_header(frame, layout.header_area, app) {
            layout.header_inner_area = inner_area;
        }
        self.render_list(frame, layout.file_list_area, app);
        self.render_preview(frame, layout.preview_area, app);
        self.render_error_message(frame, layout.error_message_area, app);
        self.render_buttons(frame, &layout, app);

        self.layout = layout;
    }

    /// Builds the footer hint line describing the file picker key bindings.
    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let Some(file_picker) = app.file_picker.as_ref() else {
            return vec![];
        };
        let mut hints = vec![(" Esc", " Cancel")];
        if file_picker.f_list.get() {
            hints.push((" ↑/↓", " Navigate"));
        }
        if file_picker.f_list.get() || file_picker.f_confirm.get() {
            hints.push((" Enter/Space", " Select"));
        }
        build_hint_spans(&*app.ctx.theme, &hints).to_vec()
    }

    /// Calculates the preferred layout for the modal's regions.
    fn get_preferred_layout(&self, _app: &App, area: Rect) -> Vec<Rect> {
        let outer_areas = Layout::horizontal([
            Constraint::Length(13), // Left pane
            Constraint::Length(1),  // Spacer
            Constraint::Min(1),     // Right pane
        ])
        .split(area);

        let inner_areas = Layout::vertical(vec![
            Constraint::Length(3), // Path input
            Constraint::Min(5),    // File viewer
            Constraint::Length(3), // Buttons
        ])
        .split(outer_areas[2]);

        let file_viewer_areas = Layout::horizontal([
            Constraint::Percentage(50), // File viewer
            Constraint::Percentage(50), // File info
        ])
        .split(inner_areas[1]);

        let button_areas = Layout::horizontal(vec![
            Constraint::Min(10),    // Error message
            Constraint::Length(10), // Cancel button
            Constraint::Length(1),  // Spacer
            Constraint::Length(10), // Open button
        ])
        .split(inner_areas[2]);

        let error_message = Layout::vertical(vec![
            Constraint::Length(2), // spacer to pin error message to baseline
            Constraint::Length(1), // error message
        ])
        .split(button_areas[0]);

        vec![
            outer_areas[0],       // Shortcut bar area
            inner_areas[0],       // Header area
            file_viewer_areas[0], // File viewer area
            file_viewer_areas[1], // File info area
            error_message[1],     // Error message area
            button_areas[1],      // Cancel button area
            button_areas[3],      // Open button area
        ]
    }

    /// Requests the initial directory listing when the modal becomes visible.
    fn on_route_enter(&mut self, app: &mut App) -> Vec<Effect> {
        if let Some(file_picker) = app.file_picker.as_ref()
            && let Some(shortcut) = file_picker.shortcuts().first()
        {
            vec![Effect::ListDirectoryContents(shortcut.path.clone())]
        } else {
            vec![]
        }
    }
}
