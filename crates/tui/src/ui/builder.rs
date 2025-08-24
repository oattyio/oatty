use super::utils::IfEmptyStr;
use crate::app::{App, Focus};
use crate::theme;
use ratatui::{prelude::*, widgets::*};

/// Renders the search input field for the command builder modal.
///
/// This function creates a search input widget with an optional DEBUG badge
/// when debug mode is enabled. The search field allows users to filter
/// available commands by typing.
///
/// # Arguments
///
/// * `f` - The frame to render to
/// * `app` - The application state containing search data
/// * `area` - The rectangular area to render the search field in
///
/// # Features
///
/// - Shows DEBUG badge when `app.debug_enabled` is true
/// - Handles cursor positioning when focused
/// - Uses themed styling for borders and text
///
/// # Examples
///
/// ```rust
/// use ratatui::Frame;
/// use crate::app::App;
///
/// let app = App::new();
/// let area = Rect::new(0, 0, 50, 3);
/// draw_search(&mut frame, &app, area);
/// ```
pub fn draw_search(f: &mut Frame, app: &App, area: Rect) {
    // Title with optional DEBUG badge
    let title = if app.debug_enabled {
        Line::from(vec![
            Span::styled("Search Commands", theme::title_style()),
            Span::raw("  "),
            Span::styled("[DEBUG]", theme::title_style().fg(theme::ACCENT)),
        ])
    } else {
        Line::from(Span::styled("Search Commands", theme::title_style()))
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme::border_style(app.focus == Focus::Search));
    let inner = block.inner(area);
    let p = Paragraph::new(app.search.as_str())
        .style(theme::text_style())
        .block(block);
    f.render_widget(p, area);
    if app.focus == Focus::Search {
        let x = inner.x.saturating_add(app.search.chars().count() as u16);
        let y = inner.y;
        f.set_cursor_position((x, y));
    }
}

/// Renders the list of available commands in the builder modal.
///
/// This function displays a scrollable list of commands filtered by the search
/// input. Commands are formatted as "group action" (e.g., "apps list") and
/// include a count of filtered results in the title.
///
/// # Arguments
///
/// * `f` - The frame to render to
/// * `app` - The application state containing command data
/// * `area` - The rectangular area to render the command list in
///
/// # Features
///
/// - Shows filtered command count in title
/// - Formats command names as "group action"
/// - Highlights selected command with "> " symbol
/// - Uses themed styling for list items and selection
///
/// # Examples
///
/// ```rust
/// use ratatui::Frame;
/// use crate::app::App;
///
/// let mut app = App::new();
/// let area = Rect::new(0, 0, 30, 10);
/// draw_commands(&mut frame, &mut app, area);
/// ```
pub fn draw_commands_list(f: &mut Frame, app: &mut App, area: Rect) {
    let title = format!("Commands ({})", app.filtered.len());
    let block = Block::default()
        .title(Span::styled(title, theme::title_style()))
        .borders(Borders::ALL)
        .border_style(theme::border_style(app.focus == Focus::Commands));

    let filtered = &app.filtered;
    let all_commands = &app.all_commands;
    let items: Vec<ListItem> = filtered
        .iter()
        .map(|idx| {
            let group = &all_commands[*idx].group;
            let name = &all_commands[*idx].name;
            let display = if name.is_empty() {
                group.to_string()
            } else {
                format!("{} {}", group, name)
            };
            ListItem::new(display).style(theme::text_style())
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(theme::list_highlight_style())
        .highlight_symbol("> ");
    let list_state = &mut app.list_state;
    f.render_stateful_widget(list, area, list_state);
}

/// Renders the input fields form for command parameters.
///
/// This function displays a form with all the input fields required for the
/// selected command. It handles different field types (boolean, enum, string)
/// and shows validation states including required field indicators.
///
/// # Arguments
///
/// * `f` - The frame to render to
/// * `app` - The application state containing field data
/// * `area` - The rectangular area to render the input form in
///
/// # Features
///
/// - Shows required fields with "*" and optional fields with "?"
/// - Displays enum values with current selection marked with "✓"
/// - Shows boolean fields as checkboxes "[ ]" or "[x]"
/// - Highlights the currently focused field
/// - Shows missing required fields in warning color
/// - Includes debug-only dry-run toggle when enabled
/// - Handles cursor positioning for text input fields
///
/// # Field Types
///
/// - **Boolean fields**: Displayed as checkboxes
/// - **Enum fields**: Show available options with current selection
/// - **String fields**: Display current value or placeholder
/// - **Required fields**: Marked with "*" and validated
///
/// # Examples
///
/// ```rust
/// use ratatui::Frame;
/// use crate::app::App;
///
/// let app = App::new();
/// let area = Rect::new(0, 0, 40, 15);
/// draw_inputs(&mut frame, &app, area);
/// ```
pub fn draw_inputs(f: &mut Frame, app: &App, area: Rect) {
    let title = match &app.picked {
        Some(s) => {
            let mut split = s.name.splitn(2, ':');
            let group = split.next().unwrap_or("");
            let rest = split.next().unwrap_or("");
            let disp = if rest.is_empty() {
                group.to_string()
            } else {
                format!("{} {}", group, rest)
            };
            format!("Inputs: {}", disp)
        }
        None => "Inputs".into(),
    };
    let block = Block::default()
        .title(Span::styled(title, theme::title_style()))
        .borders(Borders::ALL)
        .border_style(theme::border_style(app.focus == Focus::Inputs));

    // Draw the block first, then lay out inner area into content + footer rows
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);
    let splits = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    let content_rect = splits[0];
    let footer_rect = splits[1];

    let mut lines: Vec<Line> = Vec::new();
    let mut cursor_row: Option<u16> = None;
    let mut cursor_col: Option<u16> = None;
    for (i, field) in app.fields.iter().enumerate() {
        let marker = if field.required { "*" } else { "?" };
        let label = format!("{} {}", marker, field.name);
        let mut hint = String::new();
        if !field.enum_values.is_empty() {
            let enum_idx = field.enum_idx.unwrap_or(0);
            let opts = field
                .enum_values
                .iter()
                .enumerate()
                .map(|(i, v)| -> String {
                    if enum_idx == i {
                        format!("✓{}", v)
                    } else {
                        v.to_string()
                    }
                })
                .collect::<Vec<String>>()
                .join("|");
            hint = format!("enum: {}", opts);
        }
        let val = if field.is_bool {
            if field.value.is_empty() {
                "[ ]".to_string()
            } else {
                "[x]".to_string()
            }
        } else if !field.enum_values.is_empty() {
            field.value.clone().if_empty_then("<choose>".to_string())
        } else {
            field.value.clone()
        };

        if app.focus == Focus::Inputs && i == app.field_idx {
            let prefix = if hint.is_empty() {
                format!("{}: ", label)
            } else {
                format!("{} {}: ", label, hint)
            };
            let offset = if field.is_bool || !field.enum_values.is_empty() {
                0
            } else {
                field.value.chars().count()
            } as u16;
            cursor_col = Some(prefix.chars().count() as u16 + offset);
            cursor_row = Some(i as u16);
        }
        let mut line = Line::from(vec![Span::styled(
            label,
            if field.required {
                theme::text_style()
            } else {
                theme::text_muted()
            },
        )]);

        if !hint.is_empty() {
            line.push_span(Span::raw(" "));
            line.push_span(Span::styled(hint, theme::text_muted()));
        }
        line.push_span(Span::raw(": "));
        line.push_span(Span::styled(val, theme::text_style()));

        if app.focus == Focus::Inputs && i == app.field_idx {
            line = line.style(theme::highlight_style());
        }
        lines.push(line);
    }

    // Add Dry-run toggle as a selectable option when DEBUG is enabled
    if app.debug_enabled {
        let dry_label = "  Dry-run";
        let dry_val = if app.dry_run { "[x]" } else { "[ ]" };
        if app.focus == Focus::Inputs && app.field_idx == app.fields.len() {
            // Cursor at start of checkbox
            let prefix = format!("{} {}: ", dry_label, "optional");
            cursor_col = Some(prefix.chars().count() as u16);
            cursor_row = Some(lines.len() as u16);
        }
        let mut line = Line::from(vec![
            Span::styled(dry_label, theme::text_muted()),
            Span::raw(" "),
            Span::styled("optional", theme::text_muted()),
            Span::raw(": "),
            Span::styled(dry_val, theme::text_style()),
        ]);
        if app.focus == Focus::Inputs && app.field_idx == app.fields.len() {
            line = line.style(theme::highlight_style());
        }
        lines.push(line);
    }

    let missing: Vec<String> = app.missing_required();
    if app.focus == Focus::Inputs && !missing.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Missing required: ", Style::default().fg(theme::WARN)),
            Span::styled(missing.join(", "), theme::text_style()),
        ]));
    }

    // Render content lines inside the content_rect (no block so it uses inner area)
    let p = Paragraph::new(Text::from(lines as Vec<Line>)).style(theme::text_style());
    f.render_widget(p, content_rect);

    // Footer anchored at the base of the inputs pane
    let footer =
        Paragraph::new("Tab focus  Enter run  Ctrl+H help  Ctrl+C quit").style(theme::text_muted());
    f.render_widget(footer, footer_rect);

    if app.focus == Focus::Inputs {
        if let (Some(row), Some(col)) = (cursor_row, cursor_col) {
            let x = content_rect.x.saturating_add(col);
            let y = content_rect.y.saturating_add(row);
            f.set_cursor_position((x, y));
        }
    }
}
