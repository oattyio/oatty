//! State container for the shared file picker modal.

use dirs_next::{desktop_dir, document_dir, download_dir, home_dir};
use oatty_types::DirectoryEntry;
use rat_focus::{FocusFlag, HasFocus};
use ratatui::{
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{ListItem, ListState},
};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};
use url::Url;

use crate::ui::{components::common::TextInputState, theme::Theme};

/// Quick access shortcut displayed in the file picker sidebar.
#[derive(Debug, Clone)]
pub struct Shortcut {
    /// Human-friendly label rendered in the shortcut button.
    pub name: String,
    /// Target directory navigated to when the shortcut is selected.
    pub path: PathBuf,
}

/// UI state backing the file picker modal.
///
/// The state tracks the active directory, currently selected entry, preview contents,
/// and the focus graph that allows keyboard navigation between controls.
/// It also owns the `TextInputState` used to accept direct paths or URLs and performs
/// lightweight validation on that user input before a selection is committed.
#[derive(Debug, Clone, Default)]
pub struct FilePickerState {
    cur_dir: Option<PathBuf>,
    dir_contents: Option<Vec<DirectoryEntry>>,
    file_contents: Option<String>,
    line_indices: Vec<(usize, usize)>,

    allowed_extensions: Vec<&'static str>,
    shortcuts: Vec<Shortcut>,
    list_state: ListState,
    list_items: Vec<ListItem<'static>>,

    path_input_state: TextInputState,
    is_path_input_valid: bool,

    mouse_over_idx: Option<usize>,
    selected_shortcut_idx: usize,
    selected_file_idx: Option<usize>,
    preview_scroll_offset: u16,
    user_input_error: Option<String>,
    // Focus
    container_focus: FocusFlag,
    // public for ergonomics
    pub f_path_input: FocusFlag,
    pub f_list: FocusFlag,
    pub f_preview: FocusFlag,
    pub f_cancel: FocusFlag,
    pub f_confirm: FocusFlag,
    pub shortcuts_focus: Vec<FocusFlag>,
}

impl FilePickerState {
    /// Builds a new state instance that accepts the provided file extensions.
    pub fn new(allowed_extensions: Vec<&'static str>) -> Self {
        let shortcuts: Vec<Shortcut> = [home_dir(), desktop_dir(), document_dir(), download_dir()]
            .iter()
            .flatten()
            .map(|path| Shortcut {
                name: path.file_name().unwrap().to_string_lossy().into_owned(),
                path: path.to_path_buf(),
            })
            .collect();

        let shortcuts_focus = shortcuts
            .iter()
            .map(|s| FocusFlag::new().with_name(&format!("filepicker.shortcut.{}", &s.name)))
            .collect();

        Self {
            cur_dir: home_dir(),
            allowed_extensions,
            shortcuts,
            shortcuts_focus,
            mouse_over_idx: None,
            container_focus: FocusFlag::new().with_name("filepicker.container"),
            f_list: FocusFlag::new().with_name("filepicker.list"),
            f_preview: FocusFlag::new().with_name("filepicker.preview"),
            f_cancel: FocusFlag::new().with_name("filepicker.cancel"),
            f_confirm: FocusFlag::new().with_name("filepicker.confirm"),
            selected_shortcut_idx: 0,
            ..Default::default()
        }
    }

    /// Returns a mutable reference to the underlying path input widget.
    pub fn path_input_state_mut(&mut self) -> &mut TextInputState {
        &mut self.path_input_state
    }

    /// Inserts a character at the current cursor position inside the path input.
    pub fn insert_path_char(&mut self, c: char) {
        self.path_input_state.insert_char(c);
        self.validate_path_input();
    }

    /// Removes the previous character from the path input and re-validates the value.
    pub fn backspace_path_char(&mut self) {
        self.path_input_state.backspace();
        self.validate_path_input();
    }

    /// Deletes the character under the cursor from the path input and re-validates the value.
    pub fn delete_path_char(&mut self) {
        self.path_input_state.delete();
        self.validate_path_input();
    }

    fn validate_path_input(&mut self) {
        let input = self.path_input_state.input();
        self.is_path_input_valid = !input.is_empty() && (Url::parse(input).is_ok() || Path::new(input).try_exists().is_ok());
    }

    /// Shows or clears the inline error rendered beneath the path input.
    pub fn set_user_input_error(&mut self, error: Option<String>) {
        self.user_input_error = error;
    }

    /// Returns the active error message associated with the path input.
    pub fn user_input_error(&self) -> Option<&str> {
        self.user_input_error.as_deref()
    }

    /// Returns the raw user input when the field is non-empty.
    pub fn user_input(&self) -> Option<&str> {
        let input = self.path_input_state.input();
        if input.is_empty() { None } else { Some(input) }
    }

    /// Stores the provided preview contents and resets the scroll offset.
    pub fn set_file_contents(&mut self, contents: String) {
        self.calc_line_indices(&contents);
        self.file_contents = Some(contents);
        self.preview_scroll_offset = 0;
    }

    /// Returns the cached file contents currently displayed in the preview pane.
    pub fn file_contents(&self) -> Option<&str> {
        self.file_contents.as_deref()
    }

    /// Returns byte index pairs for each preview line to support constant-time slicing.
    pub fn line_indices(&self) -> &Vec<(usize, usize)> {
        &self.line_indices
    }

    /// Sets the active directory, defaulting to the user's home directory.
    pub fn set_cur_dir(&mut self, maybe_dir: Option<PathBuf>) {
        self.cur_dir = maybe_dir.or(home_dir());
        self.set_dir_contents(None);
    }

    /// Returns the directory currently being inspected.
    pub fn cur_dir(&self) -> Option<&PathBuf> {
        self.cur_dir.as_ref()
    }

    /// Replaces the displayed directory entries and clears the current selection.
    pub fn set_dir_contents(&mut self, maybe_contents: Option<Vec<DirectoryEntry>>) {
        self.dir_contents = maybe_contents;
        self.set_selected_index(None);
    }

    /// Returns the configured shortcut buttons.
    pub fn shortcuts(&self) -> &Vec<Shortcut> {
        &self.shortcuts
    }

    /// Provides mutable access to the file list widget state for selection changes.
    pub fn list_state_mut(&mut self) -> &mut ListState {
        &mut self.list_state
    }

    /// Returns the current scroll offset for the list widget.
    pub fn list_state_offset(&self) -> usize {
        self.list_state.offset()
    }

    /// Returns the directory entry currently highlighted in the list.
    pub fn selected_file(&self) -> Option<&DirectoryEntry> {
        if let (Some(idx), Some(contents)) = (self.selected_file_idx, self.dir_contents.as_ref()) {
            contents.get(idx)
        } else {
            None
        }
    }

    /// Indicates whether the typed path currently points to a valid file, directory, or URL.
    pub fn is_path_input_valid(&self) -> bool {
        self.is_path_input_valid
    }

    /// Updates the selected index and returns the newly selected entry when present.
    ///
    /// Selecting a directory updates `cur_dir` while selecting a file clears any existing preview
    /// so new contents can be loaded.
    pub fn set_selected_index(&mut self, maybe_idx: Option<usize>) -> Option<&DirectoryEntry> {
        self.file_contents = None;
        self.user_input_error = None;
        self.line_indices.clear();
        if maybe_idx.is_none() {
            self.list_state.select(None);
            self.selected_file_idx = None;
            return None;
        }
        let idx = maybe_idx?;
        let paths = self.dir_contents.as_ref()?;

        if self.can_select_idx(idx) {
            self.list_state.select(Some(idx));
            self.selected_file_idx = Some(idx);
            let proposed = paths.get(idx)?;
            if proposed.is_directory {
                self.cur_dir = Some(proposed.path.clone());
            }
            self.path_input_state.clear();
            return Some(proposed);
        }

        None
    }

    /// Returns `true` when the provided extension is part of the allow-list.
    pub fn is_allowed_extension(&self, extension: Option<&OsStr>) -> bool {
        if let Some(ext) = extension
            && let Some(s) = ext.to_str()
        {
            self.allowed_extensions.contains(&s)
        } else {
            false
        }
    }

    /// Stores the index the mouse is currently hovering.
    pub fn set_mouse_over_idx(&mut self, maybe_idx: Option<usize>) {
        self.mouse_over_idx = maybe_idx;
    }

    /// Returns the hovered index, if any.
    pub fn mouse_over_idx(&self) -> Option<usize> {
        self.mouse_over_idx
    }

    /// Scrolls the preview content upward by a fixed number of rows.
    pub fn scroll_preview_up_by(&mut self, amount: u16) {
        self.preview_scroll_offset = self.preview_scroll_offset.saturating_sub(amount);
    }

    /// Scrolls the preview content downward, clamping at the total number of lines.
    pub fn scroll_preview_down_by(&mut self, amount: u16, viewport_size: u16) {
        let max_scroll = self
            .file_contents()
            .map_or(0, |contents| contents.lines().count() as u16)
            .saturating_sub(viewport_size);
        self.preview_scroll_offset = self.preview_scroll_offset.saturating_add(amount).min(max_scroll);
    }

    /// Returns the preview scroll offset expressed as a line count.
    pub fn preview_scroll_offset(&self) -> u16 {
        self.preview_scroll_offset
    }

    /// Returns the memoized list items used for rendering each directory entry.
    pub fn list_items(&self) -> &[ListItem<'static>] {
        &self.list_items
    }

    /// Returns the focus handles associated with the shortcut buttons.
    pub fn shortcuts_focus(&self) -> &Vec<FocusFlag> {
        &self.shortcuts_focus
    }

    /// Handles activation of a shortcut button and returns the directory to open.
    pub fn shortcut_pressed(&mut self, idx: usize) -> Option<PathBuf> {
        let Shortcut { path, .. } = self.shortcuts.get(idx)?;
        let payload = Some(path.clone());
        self.set_cur_dir(payload.clone());
        self.selected_shortcut_idx = idx;
        payload
    }

    /// Returns the index of the currently selected shortcut.
    pub fn selected_shortcut_idx(&self) -> usize {
        self.selected_shortcut_idx
    }

    /// Advances selection to the next valid entry, wrapping as needed.
    pub fn select_next(&mut self) -> Option<&DirectoryEntry> {
        let idx = self.list_state.selected().map(|i| i + 1).unwrap_or(0);
        let len = self.list_items.len();
        let end = idx + len;
        for i in idx..end {
            let proposed = i % len;
            if self.can_select_idx(proposed) {
                return self.set_selected_index(Some(proposed));
            }
        }
        None
    }

    /// Moves selection to the previous valid entry, wrapping when necessary.
    pub fn select_previous(&mut self) -> Option<&DirectoryEntry> {
        let len = self.list_items.len();
        let mut idx = self.list_state.selected().unwrap_or(len);
        idx += len;
        for i in (0..idx).rev() {
            let proposed = i % len;
            if self.can_select_idx(proposed) {
                return self.set_selected_index(Some(proposed));
            }
        }
        None
    }

    fn can_select_idx(&self, idx: usize) -> bool {
        if let Some(paths) = self.dir_contents.as_ref()
            && let Some(proposed) = paths.get(idx)
        {
            return proposed.is_directory || self.is_allowed_extension(proposed.path.extension());
        }
        false
    }

    /// Recomputes the directory list backing the UI using the provided theme styles.
    pub fn rebuild_list_items(&mut self, theme: &dyn Theme) -> &Vec<ListItem<'_>> {
        self.list_items = match &self.dir_contents.as_ref() {
            None => {
                vec![ListItem::new(Line::from(vec![Span::styled(
                    "Directory contents unavailable",
                    theme.status_error(),
                )]))]
            }

            Some(contents) => self.build_list_items_from_paths(contents.iter().map(|c| &c.path).collect(), theme),
        };

        // Reset selection and move to first valid item
        self.list_state.select(None);
        self.select_next();

        &self.list_items
    }

    fn build_list_items_from_paths(&self, paths: Vec<&PathBuf>, theme: &dyn Theme) -> Vec<ListItem<'static>> {
        let mut list_items = Vec::with_capacity(paths.len());
        // Add ".." item if not root directory
        if let Some(cur_dir) = self.cur_dir.as_ref()
            && cur_dir.parent().is_some()
        {
            list_items.push(ListItem::new(Line::from(vec![Span::styled("/..", theme.syntax_keyword_style())])));
        }
        // If the cur_dir has a parent, it's the first item in the list
        // which has already been added.
        for path in paths.into_iter().skip(list_items.len()) {
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };

            let spans = if path.is_dir() {
                vec![Span::styled(format!("/{}", name), theme.syntax_keyword_style())]
            } else {
                let style = if self.is_allowed_extension(path.extension()) {
                    theme.syntax_string_style()
                } else {
                    theme.text_secondary_style().add_modifier(Modifier::DIM)
                };
                vec![Span::styled(name.to_owned(), style)]
            };

            list_items.push(ListItem::new(Line::from(spans)));
        }
        list_items
    }

    /// Calculates byte offsets for each newline so preview slicing stays constant time.
    fn calc_line_indices(&mut self, contents: &str) {
        self.line_indices.clear();
        let bytes = contents.as_bytes();
        let mut pos = 0;
        for i in 0..bytes.len() {
            if bytes[i] == b'\n' {
                self.line_indices.push((pos, i));
            }
            pos += 1;
        }
    }
}

impl HasFocus for FilePickerState {
    fn build(&self, builder: &mut rat_focus::FocusBuilder) {
        let tag = builder.start(self);
        builder.leaf_widget(&self.f_path_input);
        builder.leaf_widget(&self.f_list);
        if self.file_contents.is_some() {
            builder.leaf_widget(&self.f_preview);
        }
        builder.leaf_widget(&self.f_cancel);
        builder.leaf_widget(&self.f_confirm);

        for shortcut in &self.shortcuts_focus {
            builder.leaf_widget(shortcut);
        }
        builder.end(tag);
    }

    fn focus(&self) -> FocusFlag {
        self.container_focus.clone()
    }

    fn area(&self) -> Rect {
        Rect::default()
    }
}
