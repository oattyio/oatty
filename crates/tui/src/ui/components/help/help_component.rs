//! Help modal component for displaying command documentation.
//!
//! This module provides a component for rendering the help modal, which
//! displays comprehensive documentation for Oatty commands, including usage
//! syntax, arguments, options, and examples.

use crate::ui::theme::Theme;
use crate::{
    app::App,
    ui::{
        components::{component::Component, help::content::build_command_help_text},
        theme::theme_helpers as th,
    },
};
use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use oatty_types::{CommandSpec, Effect};
use ratatui::layout::Position;
use ratatui::prelude::Span;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    symbols::merge::MergeStrategy,
    text::{Line, Text},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
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
/// The help component is typically activated by pressing F1 in the
/// command palette or command browser. It displays help for the currently
/// selected command or the command being typed.
///
/// # Examples
///
/// ```rust,ignore
/// use oatty_tui::ui::components::HelpComponent;
///
/// let mut help = HelpComponent::default();
/// help.init()?;
/// ```
#[derive(Debug, Default)]
pub struct HelpComponent {
    focused: bool,
    render_area: Rect,
    merge_borders: bool,
}

impl Component for HelpComponent {
    fn handle_key_events(&mut self, app: &mut App, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Esc => vec![Effect::CloseModal],
            other => {
                if Self::handle_scroll_key(app, other) {
                    return Vec::new();
                }
                Vec::new()
            }
        }
    }

    fn handle_mouse_events(&mut self, app: &mut App, mouse: MouseEvent) -> Vec<Effect> {
        let position = Position {
            x: mouse.column,
            y: mouse.row,
        };
        if !self.render_area.contains(position) {
            return Vec::new();
        }
        match mouse.kind {
            MouseEventKind::ScrollDown => app.help.scroll_lines(1),
            MouseEventKind::ScrollUp => app.help.scroll_lines(-1),
            _ => {}
        }
        Vec::new()
    }

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
    /// * `area` - The full-screen area (modal will be centered within this)
    ///
    /// # Features
    ///
    /// - Center modal at 80% width and 70% height
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
    fn render(&mut self, frame: &mut Frame, rect: Rect, app: &mut App) {
        let spec = app.help.spec().or(app.browser.selected_command());
        let theme = &*app.ctx.theme;
        let (title, text) = self.resolve_title_and_text(spec, theme, &app.ctx.product_name);
        let mut block = th::block(theme, Some(&title), self.focused);
        if self.merge_borders {
            block = block.merge_borders(MergeStrategy::Exact);
        }

        frame.render_widget(block.clone(), rect);
        let inner = block.inner(rect);

        let help = &mut app.help;
        help.update_viewport_height(inner.height);
        let mut paragraph = Paragraph::new(text).style(theme.text_primary_style()).wrap(Wrap { trim: false });

        let line_count = paragraph.line_count(inner.width);
        let capped_height = line_count.min(u16::MAX as usize) as u16;
        help.update_content_height(capped_height);

        paragraph = paragraph.scroll((help.scroll_offset(), 0));
        frame.render_widget(paragraph, inner);
        self.render_scrollbar(frame, inner, app);

        self.render_area = rect;
    }

    /// Renders the footer with keyboard shortcut hints.
    ///
    /// This method displays helpful keyboard shortcuts at the bottom of the
    /// browser modal to guide user interaction.
    ///
    /// # Arguments
    /// * `app` - The application state containing theme information
    fn get_hint_spans(&self, app: &App) -> Vec<Span<'_>> {
        let theme = &*app.ctx.theme;
        th::build_hint_spans(
            theme,
            &[
                ("↑/↓", " Scroll  "),
                ("PgUp/PgDn", " Page  "),
                ("Home/End", " Jump  "),
                ("Esc", " Close "),
            ],
        )
    }
}

impl HelpComponent {
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn set_merge_borders(&mut self, merge_borders: bool) {
        self.merge_borders = merge_borders;
    }

    fn handle_scroll_key(app: &mut App, code: KeyCode) -> bool {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                app.help.scroll_lines(-1);
                true
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.help.scroll_lines(1);
                true
            }
            KeyCode::PageUp => {
                app.help.scroll_pages(-1);
                true
            }
            KeyCode::PageDown => {
                app.help.scroll_pages(1);
                true
            }
            KeyCode::Home => {
                app.help.scroll_to_top();
                true
            }
            KeyCode::End => {
                app.help.scroll_to_bottom();
                true
            }
            _ => false,
        }
    }

    fn resolve_title_and_text<'a>(
        &self,
        command_spec: Option<&CommandSpec>,
        theme: &'a dyn Theme,
        product_name: &str,
    ) -> (String, Text<'a>) {
        if let Some(spec) = command_spec {
            let title = format!("Help — {}", spec.canonical_id());
            let text = build_command_help_text(theme, spec, product_name);
            return (title, text);
        }

        let fallback = Text::from(vec![Line::styled(
            "Select a command to view detailed help.",
            theme.text_secondary_style().add_modifier(Modifier::BOLD),
        )]);
        ("Help".to_string(), fallback)
    }

    fn render_scrollbar(&self, frame: &mut Frame, area: Rect, app: &App) {
        if !app.help.is_scrollable() {
            return;
        }

        let theme = &*app.ctx.theme;
        let viewport_height = usize::from(app.help.viewport_height().max(1));
        let max_scroll_offset = app.help.content_height().saturating_sub(app.help.viewport_height());
        let content_length = usize::from(max_scroll_offset.saturating_add(1));
        let mut scrollbar_state = ScrollbarState::new(content_length)
            .position(app.help.scroll_offset() as usize)
            .viewport_content_length(viewport_height);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .thumb_style(Style::default().fg(theme.roles().scrollbar_thumb))
            .track_style(Style::default().fg(theme.roles().scrollbar_track));
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}
