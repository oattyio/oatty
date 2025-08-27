//! Help modal component for displaying command documentation.
//!
//! This module provides a component for rendering the help modal, which displays
//! comprehensive documentation for Heroku commands including usage syntax,
//! arguments, options, and examples.
use heroku_types::CommandSpec;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    symbols::line,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::{
    app, theme,
    ui::{components::component::Component, utils::centered_rect},
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

    /// Builds comprehensive help text for a command specification.
    ///
    /// This function generates detailed help documentation for a command,
    /// including usage syntax, description, arguments, options, and examples.
    /// The help text is formatted for display in the help modal.
    ///
    /// # Arguments
    ///
    /// * `spec` - The command specification to generate help for
    ///
    /// # Returns
    ///
    /// A formatted string containing the complete help documentation.
    ///
    /// # Help Sections
    ///
    /// The generated help includes:
    /// - **USAGE**: Command syntax with positional arguments
    /// - **DESCRIPTION**: Command summary from spec
    /// - **ARGUMENTS**: Positional argument details with help text
    /// - **OPTIONS**: Flag descriptions, types, and defaults
    /// - **EXAMPLE**: Sample command with current field values
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use heroku_registry::CommandSpec;
    ///
    /// // Create a minimal CommandSpec for testing
    /// let spec = CommandSpec {
    ///     name: "apps:info".to_string(),
    ///     group: "apps".to_string(),
    ///     summary: "Show app info".to_string(),
    ///     method: "GET".to_string(),
    ///     path: "/apps/{app}".to_string(),
    ///     flags: vec![],
    ///     positional_args: vec!["app".to_string()],
    ///     positional_help: std::collections::HashMap::new(),
    /// };
    /// let help_text = build_command_help(&spec);
    /// println!("{}", help_text);
    /// ```
    fn build_command_help(spec: &CommandSpec) -> Text<'_> {
        let mut split = spec.name.splitn(2, ':');
        let group = split.next().unwrap_or("");
        let rest = split.next().unwrap_or("");
        let cmd = if rest.is_empty() {
            group.to_string()
        } else {
            format!("{} {}", group, rest)
        };
        let mut lines: Vec<Line<'_>> = vec![Line::from("")];
        lines.push(Line::styled(" USAGE:", theme::title_style()));
        // Command Usage
        let mut command: Line<'_> = Line::from(format!("  heroku {}", cmd));

        for arg in &spec.positional_args {
            command.push_span(Span::raw(format!(" <{}>", arg)));
        }

        let flags = {
            let mut flags: Vec<_> = spec.flags.iter().collect();
            flags.sort_by_key(|flag| !flag.required);
            flags.into_iter().filter_map(|flag| {
                let format_str = if flag.required {
                    format!(" --{} <value>", flag.name)
                } else {
                    format!(" [--{} <value>]", flag.name)
                };
                Some(Span::styled(format_str, theme::text_style()))
            })
        };
        flags.for_each(|f| command.push_span(f));

        lines.push(command);
        lines.push(Line::from(""));
        // Description
        lines.push(Line::styled(" DESCRIPTION:", theme::title_style()));
        lines.push(Line::from(format!("  {}", spec.summary)));

        // Arguments
        if !spec.positional_args.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::styled(" ARGUMENTS:", theme::title_style()));
            for p in &spec.positional_args {
                if let Some(desc) = spec.positional_help.get(p) {
                    let mut arg_line = Line::styled(format!("  {} ", p.to_uppercase()), theme::list_highlight_style());
                    arg_line.push_span(Span::styled(format!("{}", desc), theme::text_muted()));
                    lines.push(arg_line);
                } else {
                    lines.push(Line::raw(format!(
                        "  {}: Path parameter derived from the endpoint URL.\n",
                        p
                    )));
                }
            }
        }
        // Options
        if !spec.flags.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::styled(" OPTIONS:", theme::title_style()));
            for f in &spec.flags {
                let mut flag_line = if f.short_name.is_some() {
                    Line::styled(
                        format!("  -{},  --{}", f.short_name.as_ref().unwrap(), f.name),
                        theme::list_highlight_style(),
                    )
                } else {
                    Line::styled(format!("  --{}", f.name), theme::list_highlight_style())
                };

                if f.r#type != "boolean" {
                    flag_line.push_span(Span::raw(" <VALUE>"));
                }
                if f.required {
                    flag_line.push_span(Span::styled("  (required)", theme::text_muted()));
                }
                if !f.enum_values.is_empty() {
                    flag_line.push_span(Span::styled(
                        format!("  [enum: {}]", f.enum_values.join("|")),
                        theme::text_muted(),
                    ));
                }
                if let Some(def) = &f.default_value {
                    flag_line.push_span(Span::styled(format!("  [default: {}]", def), theme::text_muted()));
                }
                if let Some(desc) = &f.description {
                    flag_line.push_span(Span::styled(format!(" — {}", desc), theme::text_muted()));
                }
                lines.push(flag_line);
            }
        }

        Text::from(lines)
    }
}

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
    /// ```rust
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
            theme::title_style(),
        )]);
        if let Some(spec) = app.help.spec().or(app.builder.selected_command()) {
            let mut split = spec.name.splitn(2, ':');
            let group = split.next().unwrap_or("");
            let rest = split.next().unwrap_or("");
            let cmd = if rest.is_empty() {
                group.to_string()
            } else {
                format!("{} {}", group, rest)
            };
            title = format!("Help — {}", cmd);
            text = HelpComponent::build_command_help(spec);
        }

        title.push_str("  [Esc] Close");
        let block = Block::default()
            .title(Span::styled(title, theme::title_style().fg(theme::ACCENT)))
            .borders(Borders::ALL)
            .border_style(theme::border_style(true));

        // Clear background, draw block, then split inner area for content/footer
        frame.render_widget(Clear, area);
        frame.render_widget(block.clone(), area);
        let inner = block.inner(area);
        let splits = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);

        // Content paragraph inside inner content rect
        let p = Paragraph::new(text)
            .style(theme::text_style())
            .wrap(Wrap { trim: false });
        frame.render_widget(p, splits[0]);

        // Footer hint pinned to baseline (styled)
        let footer = Paragraph::new(Line::from(vec![
            Span::styled("Hint: ", theme::text_muted()),
            Span::styled("Ctrl+H", theme::title_style().fg(theme::ACCENT)),
            Span::styled(" close  ", theme::text_muted()),
            Span::styled("Ctrl+Y", theme::title_style().fg(theme::ACCENT)),
            Span::styled(" copy", theme::text_muted()),
        ]))
        .style(theme::text_muted());
        frame.render_widget(footer, splits[1]);
    }
}
