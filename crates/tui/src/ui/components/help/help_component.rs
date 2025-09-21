//! Help modal component for displaying command documentation.
//!
//! This module provides a component for rendering the help modal, which
//! displays comprehensive documentation for Heroku commands including usage
//! syntax, arguments, options, and examples.
use std::vec;

use crossterm::event::{KeyCode, KeyEvent};
use heroku_types::Effect;
use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Text},
    widgets::{Clear, Paragraph, Wrap},
};

use crate::{
    app::{self, App},
    ui::{
        components::{component::Component, help::content::build_command_help_text},
        theme::theme_helpers as th,
        utils::centered_rect,
    },
};

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
/// command palette or command browser. It displays help for the currently
/// selected command or the command being typed.
///
/// # Examples
///
/// ```rust,ignore
/// use heroku_tui::ui::components::HelpComponent;
///
/// let mut help = HelpComponent::new();
/// help.init()?;
/// ```
#[derive(Debug, Default)]
pub struct HelpComponent;

impl Component for HelpComponent {
    /// Renders the help modal overlay with detailed command documentation.
    ///
    /// This function displays a modal dialog containing comprehensive help
    /// information for the selected command. The help includes usage syntax,
    /// description, arguments, options, and examples.
    ///
    /// # Arguments
    ///
    /// * `f` - The frame to render to
    /// * `app` - The application state containing help data
    /// * `area` - The full screen area (modal will be centered within this)
    ///
    /// # Features
    ///
    /// - Centers modal at 80% width and 70% height
    /// - Shows command name in title with close hint
    /// - Displays comprehensive help text with sections:
    ///   - USAGE: Command syntax with arguments
    ///   - DESCRIPTION: Command summary
    ///   - ARGUMENTS: Positional argument details
    ///   - OPTIONS: Flag descriptions and types
    ///   - EXAMPLE: Sample command with current values
    /// - Includes footer with keyboard shortcuts
    /// - Uses themed styling for borders and text
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use ratatui::Frame;
    /// use crate::app::App;
    ///
    /// let app = App::new();
    /// let area = Rect::new(0, 0, 100, 50);
    /// draw_help_modal(&mut frame, &app, area);
    /// ```
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut app::App) {
        let area = centered_rect(80, 70, rect);
        // Prefer help_spec when set, otherwise picked
        let mut title = "Help".to_string();
        let mut text = Text::from(vec![Line::styled(
            "Select a command to view detailed help.".to_string(),
            app.ctx.theme.text_secondary_style().add_modifier(Modifier::BOLD),
        )]);
        if let Some(spec) = app.help.spec().or(app.browser.selected_command()) {
            let mut split = spec.name.splitn(2, ':');
            let group = split.next().unwrap_or("");
            let rest = split.next().unwrap_or("");
            let cmd = if rest.is_empty() {
                group.to_string()
            } else {
                format!("{} {}", group, rest)
            };
            title = format!("Help — {}", cmd);
            text = build_command_help_text(&*app.ctx.theme, spec);
        }

        title.push_str("  [Esc] Close");
        let block = th::block(&*app.ctx.theme, Some(&title), true);

        // Clear background, draw block, then split inner area for content/footer
        frame.render_widget(Clear, area);
        frame.render_widget(block.clone(), area);
        let inner = block.inner(area);

        // Content paragraph inside inner content rect
        let paragraph = Paragraph::new(text)
            .style(app.ctx.theme.text_primary_style())
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, inner);
    }

    fn handle_key_events(&mut self, _app: &mut App, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Esc => vec![Effect::CloseModal],

            _ => vec![],
        }
    }
}
