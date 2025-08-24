//! Command builder component for interactive command construction.
//!
//! This module provides a component for rendering the command builder modal,
//! which allows users to interactively build Heroku commands through a
//! multi-panel interface with search, command selection, and parameter input.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{layout::Rect, Frame};

use crate::{app, component::Component};

/// Command builder component for interactive command construction.
///
/// This component provides a comprehensive modal interface for building
/// Heroku commands interactively. It includes search functionality,
/// command selection, and parameter input with validation.
///
/// # Features
///
/// - **Search panel**: Filter and search for available commands
/// - **Command list**: Browse and select from available commands
/// - **Input panel**: Fill in command parameters with validation
/// - **Preview panel**: See the generated command in real-time
/// - **Focus management**: Navigate between panels with Tab/Shift+Tab
/// - **Keyboard shortcuts**: Quick access to help, tables, and copy
///
/// # Panel Layout
///
/// The builder modal is divided into three main panels:
///
/// 1. **Search Panel** (top) - Command search and filtering
/// 2. **Command List** (left) - Available commands selection
/// 3. **Input Panel** (center) - Parameter input and validation
/// 4. **Preview Panel** (right) - Generated command preview
///
/// # Key Bindings
///
/// ## Global Shortcuts
/// - **Ctrl+F**: Close builder modal
/// - **Ctrl+H**: Open help modal
/// - **Ctrl+T**: Open table modal
/// - **Ctrl+Y**: Copy current command
///
/// ## Navigation
/// - **Tab**: Move to next panel
/// - **Shift+Tab**: Move to previous panel
/// - **Escape**: Clear search or close modal
///
/// ## Search Panel
/// - **Character input**: Add to search query
/// - **Backspace**: Remove character
/// - **Arrow keys**: Navigate suggestions
/// - **Enter**: Select command
///
/// ## Command List
/// - **Arrow keys**: Navigate commands
/// - **Enter**: Select command
///
/// ## Input Panel
/// - **Arrow keys**: Navigate fields
/// - **Character input**: Edit field values
/// - **Space**: Toggle boolean fields
/// - **Left/Right**: Cycle enum values
/// - **Enter**: Execute command
///
/// # Examples
///
/// ```rust
/// use heroku_tui::ui::components::BuilderComponent;
///
/// let mut builder = BuilderComponent::new();
/// builder.init()?;
/// ```
#[derive(Default)]
pub struct BuilderComponent;

impl BuilderComponent {
    /// Creates a new builder component instance.
    ///
    /// # Returns
    ///
    /// A new BuilderComponent with default state
    pub fn new() -> Self {
        Self
    }

    /// Handle key events for the command builder modal.
    ///
    /// This method processes keyboard input for the builder, handling
    /// navigation between panels, input editing, and special commands.
    /// The behavior varies based on which panel currently has focus.
    ///
    /// # Arguments
    ///
    /// * `app` - The application state to update
    /// * `key` - The key event to process
    ///
    /// # Returns
    ///
    /// `Result<Vec<Effect>>` containing any effects that should be processed
    pub fn handle_key(&mut self, app: &mut app::App, key: KeyEvent) -> Result<Vec<app::Effect>> {
        let mut effects: Vec<app::Effect> = Vec::new();
        match app.focus {
            app::Focus::Search => match key.code {
                KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::ToggleTable))
                }
                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::ToggleBuilder))
                }
                KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::ToggleHelp))
                }
                KeyCode::Char(c)
                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
                {
                    effects.extend(app.update(app::Msg::SearchChar(c)))
                }
                KeyCode::Backspace => effects.extend(app.update(app::Msg::SearchBackspace)),
                KeyCode::Esc => effects.extend(app.update(app::Msg::SearchClear)),
                KeyCode::Tab => effects.extend(app.update(app::Msg::FocusNext)),
                KeyCode::BackTab => effects.extend(app.update(app::Msg::FocusPrev)),
                KeyCode::Down => effects.extend(app.update(app::Msg::MoveSelection(1))),
                KeyCode::Up => effects.extend(app.update(app::Msg::MoveSelection(-1))),
                KeyCode::Enter => effects.extend(app.update(app::Msg::Enter)),
                KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::CopyCommand))
                }
                _ => {}
            },
            app::Focus::Commands => match key.code {
                KeyCode::Char('t') => effects.extend(app.update(app::Msg::ToggleTable)),
                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::ToggleBuilder))
                }
                KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::ToggleHelp))
                }
                KeyCode::Down => effects.extend(app.update(app::Msg::MoveSelection(1))),
                KeyCode::Up => effects.extend(app.update(app::Msg::MoveSelection(-1))),
                KeyCode::Enter => effects.extend(app.update(app::Msg::Enter)),
                KeyCode::Tab => effects.extend(app.update(app::Msg::FocusNext)),
                KeyCode::BackTab => effects.extend(app.update(app::Msg::FocusPrev)),
                KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::CopyCommand))
                }
                _ => {}
            },
            app::Focus::Inputs => match key.code {
                KeyCode::Char('t') => effects.extend(app.update(app::Msg::ToggleTable)),
                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::ToggleBuilder))
                }
                KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::ToggleHelp))
                }
                KeyCode::Tab => effects.extend(app.update(app::Msg::FocusNext)),
                KeyCode::BackTab => effects.extend(app.update(app::Msg::FocusPrev)),
                KeyCode::Up => effects.extend(app.update(app::Msg::InputsUp)),
                KeyCode::Down => effects.extend(app.update(app::Msg::InputsDown)),
                KeyCode::Enter => effects.extend(app.update(app::Msg::Run)),
                KeyCode::Left => effects.extend(app.update(app::Msg::InputsCycleLeft)),
                KeyCode::Right => effects.extend(app.update(app::Msg::InputsCycleRight)),
                KeyCode::Backspace => effects.extend(app.update(app::Msg::InputsBackspace)),
                KeyCode::Char(' ') => effects.extend(app.update(app::Msg::InputsToggleSpace)),
                KeyCode::Char(c)
                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
                {
                    effects.extend(app.update(app::Msg::InputsChar(c)))
                }
                KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    effects.extend(app.update(app::Msg::CopyCommand))
                }
                _ => {}
            },
        }
        Ok(effects)
    }
}

impl Component for BuilderComponent {
    /// Renders the command builder modal with all panels.
    ///
    /// This method delegates rendering to the modal system, which handles
    /// the layout, styling, and content generation for the builder interface.
    ///
    /// # Arguments
    ///
    /// * `f` - The frame to render to
    /// * `rect` - The rectangular area to render in
    /// * `app` - The application state containing builder data
    fn render(&mut self, f: &mut Frame, rect: Rect, app: &mut app::App) {
        crate::ui::modals::draw_builder_modal(f, app, rect);
    }
}
