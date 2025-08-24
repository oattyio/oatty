//! Command palette component for input and suggestions.
//!
//! This module provides a component for rendering the command palette, which
//! handles text input, command suggestions, and user interactions for
//! building Heroku CLI commands.

use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};

use crate::{app, component::Component};

/// Command palette component for input and suggestions.
///
/// This component encapsulates the command palette experience including the
/// input line, suggestions popup, and help integration. It provides a
/// comprehensive interface for building and executing Heroku commands.
///
/// # Features
///
/// - Text input with cursor navigation
/// - Real-time command suggestions
/// - Suggestion acceptance and completion
/// - Help integration (Ctrl+H)
/// - Error display and validation
/// - Ghost text for completion hints
///
/// # Key Bindings
///
/// - **Character input**: Add characters to the input
/// - **Backspace**: Remove character before cursor
/// - **Arrow keys**: Navigate suggestions (Up/Down) or move cursor (Left/Right)
/// - **Tab**: Accept current suggestion
/// - **Ctrl+H**: Open help for current command
/// - **Ctrl+F**: Open command builder modal
/// - **Enter**: Execute command
/// - **Escape**: Clear input and close suggestions
///
/// # Examples
///
/// ```rust
/// use heroku_tui::ui::components::PaletteComponent;
///
/// let mut palette = PaletteComponent::new();
/// palette.init()?;
/// ```
#[derive(Default)]
pub struct PaletteComponent;

impl PaletteComponent {
    /// Creates a new palette component instance.
    ///
    /// # Returns
    ///
    /// A new PaletteComponent with default state
    pub fn new() -> Self {
        Self
    }

    /// Handle a key event for the command palette.
    ///
    /// This method processes keyboard input for the palette, handling
    /// text input, navigation, suggestion acceptance, and special commands.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state to update
    /// * `key` - The key event to process
    ///
    /// # Returns
    ///
    /// `Result<bool>` where `true` indicates the key was handled
    pub fn handle_key(&mut self, app: &mut app::App, key: KeyEvent) -> Result<bool> {
        crate::palette_comp::handle_key(app, key)
    }
}

impl Component for PaletteComponent {
    /// Renders the command palette with input and suggestions.
    ///
    /// This method delegates rendering to the palette system, which handles
    /// the input display, suggestion popup, and cursor positioning.
    ///
    /// # Arguments
    ///
    /// * `f` - The frame to render to
    /// * `rect` - The rectangular area to render in
    /// * `app` - The application state containing palette data
    fn render(&mut self, f: &mut Frame, rect: Rect, app: &mut app::App) {
        crate::palette::render_palette(f, rect, app);
    }
}
