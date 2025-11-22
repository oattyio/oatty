use crate::ui::theme::catalog::{self, ThemeDefinition};

/// UI state for the theme picker modal.
#[derive(Debug, Clone)]
pub struct ThemePickerState {
    options: Vec<&'static ThemeDefinition>,
    pub selected_index: usize,
}

impl Default for ThemePickerState {
    fn default() -> Self {
        Self::new()
    }
}

impl ThemePickerState {
    /// Build a state instance seeded with all available theme definitions.
    pub fn new() -> Self {
        Self {
            options: catalog::all().iter().collect(),
            selected_index: 0,
        }
    }

    /// Returns the currently selected definition, if any.
    pub fn selected_option(&self) -> Option<&'static ThemeDefinition> {
        self.options.get(self.selected_index).copied()
    }

    /// All available theme definitions.
    pub fn options(&self) -> &[&'static ThemeDefinition] {
        &self.options
    }

    /// Move selection to the next option, wrapping at the end.
    pub fn select_next(&mut self) {
        if self.options.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1) % self.options.len();
    }

    /// Move selection to the previous option, wrapping to the end.
    pub fn select_previous(&mut self) {
        if self.options.is_empty() {
            return;
        }
        if self.selected_index == 0 {
            self.selected_index = self.options.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }

    /// Position the selection on the active theme so the picker reflects current state.
    pub fn set_active_theme(&mut self, theme_id: &str) {
        if let Some(idx) = self
            .options
            .iter()
            .position(|definition| definition.id.eq_ignore_ascii_case(theme_id))
        {
            self.selected_index = idx;
        }
    }
}
