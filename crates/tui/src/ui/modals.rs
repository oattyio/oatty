use super::utils::centered_rect;
use crate::app::App;
use crate::theme;
use ratatui::{prelude::*, widgets::*};

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
pub fn draw_help_modal(f: &mut Frame, app: &App, area: Rect) {
    let area = centered_rect(80, 70, area);
    // Prefer help_spec when set, otherwise picked
    let spec_for_help = app.help.spec.as_ref().or(app.builder.selected_command());
    let mut title = if let Some(spec) = spec_for_help {
        let mut split = spec.name.splitn(2, ':');
        let group = split.next().unwrap_or("");
        let rest = split.next().unwrap_or("");
        let cmd = if rest.is_empty() {
            group.to_string()
        } else {
            format!("{} {}", group, rest)
        };
        format!("Help — {}", cmd)
    } else {
        "Help".to_string()
    };
    title.push_str("  [Esc] Close");
    let block = Block::default()
        .title(Span::styled(title, theme::title_style().fg(theme::ACCENT)))
        .borders(Borders::ALL)
        .border_style(theme::border_style(true));

    // Prepare content text (without footer)
    let text = if let Some(spec) = spec_for_help {
        build_command_help(spec)
    } else {
        "Select a command to view detailed help.".to_string()
    };

    // Clear background, draw block, then split inner area for content/footer
    f.render_widget(Clear, area);
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);
    let splits = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    // Content paragraph inside inner content rect
    let p = Paragraph::new(text)
        .style(theme::text_style())
        .wrap(Wrap { trim: false });
    f.render_widget(p, splits[0]);

    // Footer hint pinned to baseline (styled)
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("Hint: ", theme::text_muted()),
        Span::styled("Ctrl+H", theme::title_style().fg(theme::ACCENT)),
        Span::styled(" close  ", theme::text_muted()),
        Span::styled("Ctrl+Y", theme::title_style().fg(theme::ACCENT)),
        Span::styled(" copy", theme::text_muted()),
    ]))
    .style(theme::text_muted());
    f.render_widget(footer, splits[1]);
}

/// Renders the table modal for displaying JSON results.
///
/// This function displays a large modal dialog optimized for viewing
/// tabular data from JSON results. It automatically chooses between
/// table view for arrays and key-value view for objects.
///
/// # Arguments
///
/// * `f` - The frame to render to
/// * `app` - The application state containing result data
/// * `area` - The full screen area (modal will be centered within this)
///
/// # Features
///
/// - Centers modal at 96% width and 90% height for maximum space
/// - Automatically detects JSON arrays and shows table view
/// - Falls back to key-value display for objects
/// - Includes scroll controls in title
/// - Footer with navigation shortcuts
/// - Uses themed styling for borders and text
///
/// # Display Logic
///
/// 1. If JSON contains arrays → Show scrollable table with offset
/// 2. If JSON is object → Show key-value pairs
/// 3. If no JSON → Show "No results to display"
///
/// # Examples
///
/// ```rust,no_run
/// use ratatui::prelude::*;
/// use heroku_tui::app::App;
/// use heroku_registry::Registry;
///
/// let registry = Registry::from_embedded_schema().unwrap();
/// let app = App::new(registry);
/// let area = Rect::new(0, 0, 100, 50);
/// // draw_table_modal(&mut frame, &app, area);
/// ```
// This function has been migrated to TableComponent::render()
// and is no longer needed as part of the component architecture migration.

/// Renders the command builder modal with full interface.
///
/// This function displays a comprehensive modal dialog that provides the
/// complete command building interface. It includes search, command list,
/// input fields, and preview areas in a single modal.
///
/// # Arguments
///
/// * `f` - The frame to render to
/// * `app` - The application state containing builder data
/// * `area` - The full screen area (modal will be centered within this)
///
/// # Features
///
/// - Centers modal at 96% width and 90% height for maximum space
/// - Three-panel layout: commands, inputs, preview
/// - Search functionality at the top
/// - Footer with keyboard shortcuts
/// - Uses themed styling for borders and text
///
/// # Layout Structure
///
/// ```
/// +- Search Bar ----------------------------------------------+
/// +- Commands ---+- Inputs ---+- Preview ---------------------+
/// |              |            |                               |
/// |              |            |                               |
/// |              |            |                               |
/// +- Footer --------------------------------------------------+
/// ```
///
/// # Examples
///
/// ```rust,no_run
/// use ratatui::prelude::*;
/// use heroku_tui::app::App;
/// use heroku_registry::Registry;
///
/// let registry = Registry::from_embedded_schema().unwrap();
/// let mut app = App::new(registry);
/// let area = Rect::new(0, 0, 100, 50);
/// // draw_builder_modal(&mut frame, &mut app, area);
/// ```
// This function has been migrated to BuilderComponent::render()
// and is no longer needed as part of the component architecture migration.

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
fn build_command_help(spec: &heroku_registry::CommandSpec) -> String {
    let mut split = spec.name.splitn(2, ':');
    let group = split.next().unwrap_or("");
    let rest = split.next().unwrap_or("");
    let cmd = if rest.is_empty() {
        group.to_string()
    } else {
        format!("{} {}", group, rest)
    };

    // Usage
    let mut usage = format!("USAGE:\n  heroku {}", cmd);
    for p in &spec.positional_args {
        usage.push_str(&format!(" <{}>", p));
    }
    // Compact options indicator
    if !spec.flags.is_empty() {
        usage.push_str(" [OPTIONS]");
    }

    // Description
    let mut out = String::new();
    out.push_str(&usage);
    out.push_str("\n\nDESCRIPTION:\n  ");
    out.push_str(&spec.summary);

    // Arguments
    if !spec.positional_args.is_empty() {
        out.push_str("\n\nARGUMENTS:\n");
        for p in &spec.positional_args {
            if let Some(desc) = spec.positional_help.get(p) {
                out.push_str(&format!("  {} {}\n", p.to_uppercase(), desc));
            } else {
                out.push_str(&format!(
                    "  {}: Path parameter derived from the endpoint URL.\n",
                    p
                ));
            }
        }
    }

    // Options
    if !spec.flags.is_empty() {
        out.push_str("\nOPTIONS:\n");
        for f in &spec.flags {
            let mut line = format!("  --{}", f.name);
            if f.r#type != "boolean" {
                line.push_str(" <VALUE>");
            }
            if f.required {
                line.push_str("  (required)");
            }
            if !f.enum_values.is_empty() {
                line.push_str(&format!("  [enum: {}]", f.enum_values.join("|")));
            }
            if let Some(def) = &f.default_value {
                line.push_str(&format!("  [default: {}]", def));
            }
            if let Some(desc) = &f.description {
                line.push_str(&format!(" — {}", desc));
            }
            out.push_str(&line);
            out.push('\n');
        }
    }

    out
}
