//! Help modal component for displaying command documentation.
//!
//! This module provides a component for rendering the help modal, which displays
//! comprehensive documentation for Heroku commands including usage syntax,
//! arguments, options, and examples.

use ratatui::{Frame, layout::Rect};

use crate::{app, component::Component};

/// Help modal component for displaying command documentation.
///
/// This component renders a modal dialog containing detailed help information
/// for the selected command. The help includes usage syntax, description,
/// arguments, options, and examples.
///
/// # Features
///
/// - Displays comprehensive command documentation
/// - Shows usage syntax with arguments
/// - Lists all available flags and options
/// - Provides examples with current field values
/// - Includes keyboard shortcuts for navigation
///
/// # Usage
///
/// The help component is typically activated by pressing Ctrl+H in the
/// command palette or builder modal. It displays help for the currently
/// selected command or the command being typed.
///
/// # Examples
///
/// ```rust
/// use heroku_tui::ui::components::HelpComponent;
///
/// let mut help = HelpComponent::new();
/// help.init()?;
/// ```
#[derive(Default)]
pub struct HelpComponent;

impl HelpComponent {
    /// Creates a new help component instance.
    ///
    /// # Returns
    ///
    /// A new HelpComponent with default state
    pub fn new() -> Self {
        Self
    }
}

impl Component for HelpComponent {
    /// Renders the help modal with command documentation.
    ///
    /// This method delegates rendering to the modal system, which handles
    /// the layout, styling, and content generation for the help display.
    ///
    /// # Arguments
    ///
    /// * `f` - The frame to render to
    /// * `rect` - The rectangular area to render in
    /// * `app` - The application state containing help data
    fn render(&mut self, f: &mut Frame, rect: Rect, app: &mut app::App) {
        crate::ui::modals::draw_help_modal(f, app, rect);
    }
}
