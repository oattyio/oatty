use ratatui::{prelude::*, widgets::*};

use crate::app::{App, Focus};
use crate::theme;

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();

    // Default view: command palette (input), hints, logs
    let constraints = [
        Constraint::Length(3), // input line area
        Constraint::Length(1), // hints area
        Constraint::Min(1),    // spacer / future content
        Constraint::Length(6), // logs
    ];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(size);

    crate::palette::render_palette(f, chunks[0], app);
    // Hints outside the input block, shown when no error present and either no popup or no suggestions
    if app.palette.error.is_none() && (!app.palette.popup_open || app.palette.suggestions.is_empty()) {
        let hints = Paragraph::new(Line::from(vec![
            Span::styled("Hints: ", theme::text_muted()),
            Span::styled("↑/↓", theme::title_style().fg(theme::ACCENT)),
            Span::styled(" cycle  ", theme::text_muted()),
            Span::styled("Tab", theme::title_style().fg(theme::ACCENT)),
            Span::styled(" accept  ", theme::text_muted()),
            Span::styled("Ctrl-R", theme::title_style().fg(theme::ACCENT)),
            Span::styled(" history  ", theme::text_muted()),
            Span::styled("Ctrl-F", theme::title_style().fg(theme::ACCENT)),
            Span::styled(" builder  ", theme::text_muted()),
            Span::styled("Esc", theme::title_style().fg(theme::ACCENT)),
            Span::styled(" cancel", theme::text_muted()),
        ]))
        .style(theme::text_muted());
        f.render_widget(hints, chunks[1]);
    }

    draw_logs(f, app, chunks[3]);

    if app.show_help {
        draw_help_modal(f, app, f.area());
    }
    if app.show_table {
        draw_table_modal(f, app, f.area());
    }
    if app.show_builder {
        draw_builder_modal(f, app, f.area());
    }
}

fn draw_search(f: &mut Frame, app: &App, area: Rect) {
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

fn draw_commands(f: &mut Frame, app: &mut App, area: Rect) {
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
            let name = &all_commands[*idx].name;
            let mut split = name.splitn(2, ':');
            let group = split.next().unwrap_or("");
            let rest = split.next().unwrap_or("");
            let display = if rest.is_empty() {
                group.to_string()
            } else {
                format!("{} {}", group, rest)
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

fn draw_inputs(f: &mut Frame, app: &App, area: Rect) {
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

trait IfEmptyStr {
    fn if_empty_then(self, alt: String) -> String;
}

impl IfEmptyStr for String {
    fn if_empty_then(self, alt: String) -> String {
        if self.is_empty() {
            alt
        } else {
            self
        }
    }
}

fn draw_preview(f: &mut Frame, app: &App, area: Rect) {
    // If we have a JSON result, prefer a table when an array is present; else fallback to key/values
    if let Some(json) = &app.result_json {
        let has_array = match json {
            serde_json::Value::Array(a) => !a.is_empty(),
            serde_json::Value::Object(m) => {
                m.values().any(|v| matches!(v, serde_json::Value::Array(_)))
            }
            _ => false,
        };
        if has_array {
            crate::tables::draw_json_table(f, area, json);
        } else {
            crate::tables::draw_kv_or_text(f, area, json);
        }
        return;
    }

    let block = Block::default()
        .title(Span::styled("Command  [Ctrl+Y] Copy", theme::title_style()))
        .borders(Borders::ALL)
        .border_style(theme::border_style(false));
    let mut text = String::new();
    if let Some(spec) = &app.picked {
        let cli = crate::preview::cli_preview(spec, &app.fields);
        text = cli;
    } else {
        text.push_str("Select a command to see preview.");
    }
    let p = Paragraph::new(text)
        .style(theme::text_style())
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn draw_logs(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(Span::styled(
            format!("Logs ({})", app.logs.len()),
            theme::title_style(),
        ))
        .borders(Borders::ALL)
        .border_style(theme::border_style(false));
    let items: Vec<ListItem> = app
        .logs
        .iter()
        .map(|l| ListItem::new(l.as_str()).style(theme::text_style()))
        .collect();
    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    let area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1]);
    area[1]
}

fn draw_help_modal(f: &mut Frame, app: &App, area: Rect) {
    let area = centered_rect(80, 70, area);
    // Prefer help_spec when set, otherwise picked
    let spec_for_help = app.help_spec.as_ref().or(app.picked.as_ref());
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

fn draw_builder_modal(f: &mut Frame, app: &mut App, area: Rect) {
    let area = centered_rect(96, 90, area);
    let block = Block::default()
        .title(Span::styled(
            "Command Builder  [Esc] Close",
            theme::title_style().fg(theme::ACCENT),
        ))
        .borders(Borders::ALL)
        .border_style(theme::border_style(true));
    f.render_widget(Clear, area);
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(1),
        ])
        .split(inner);

    draw_search(f, app, chunks[0]);

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(35),
            Constraint::Percentage(35),
        ])
        .split(chunks[1]);
    draw_commands(f, app, main[0]);
    draw_inputs(f, app, main[1]);
    draw_preview(f, app, main[2]);

    // Footer hint for builder modal
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("Hint: ", theme::text_muted()),
        Span::styled("Ctrl+F", theme::title_style().fg(theme::ACCENT)),
        Span::styled(" close  ", theme::text_muted()),
        Span::styled("Enter", theme::title_style().fg(theme::ACCENT)),
        Span::styled(" apply  ", theme::text_muted()),
        Span::styled("Esc", theme::title_style().fg(theme::ACCENT)),
        Span::styled(" cancel", theme::text_muted()),
    ]))
    .style(theme::text_muted());
    f.render_widget(footer, chunks[2]);
}

fn draw_table_modal(f: &mut Frame, app: &App, area: Rect) {
    // Large modal to maximize space for tables
    let area = centered_rect(96, 90, area);
    let title = "Results  [Esc] Close  ↑/↓ Scroll";
    let block = Block::default()
        .title(Span::styled(title, theme::title_style().fg(theme::ACCENT)))
        .borders(Borders::ALL)
        .border_style(theme::border_style(true));

    f.render_widget(Clear, area);
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);
    // Split for content + footer
    let splits = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    if let Some(json) = &app.result_json {
        // Prefer table if array is present, else KV fallback even in modal
        let has_array = match json {
            serde_json::Value::Array(a) => !a.is_empty(),
            serde_json::Value::Object(m) => {
                m.values().any(|v| matches!(v, serde_json::Value::Array(_)))
            }
            _ => false,
        };
        if has_array {
            crate::tables::draw_json_table_with_offset(f, splits[0], json, app.table_offset);
        } else {
            crate::tables::draw_kv_or_text(f, splits[0], json);
        }
    } else {
        let p = Paragraph::new("No results to display").style(theme::text_muted());
        f.render_widget(p, splits[0]);
    }

    // Footer hint for table modal
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("Hint: ", theme::text_muted()),
        Span::styled("Esc", theme::title_style().fg(theme::ACCENT)),
        Span::styled(" close  ", theme::text_muted()),
        Span::styled("↑/↓", theme::title_style().fg(theme::ACCENT)),
        Span::styled(" scroll  ", theme::text_muted()),
        Span::styled("PgUp/PgDn", theme::title_style().fg(theme::ACCENT)),
        Span::styled(" faster  ", theme::text_muted()),
        Span::styled("Home/End", theme::title_style().fg(theme::ACCENT)),
        Span::styled(" jump", theme::text_muted()),
    ]))
    .style(theme::text_muted());
    f.render_widget(footer, splits[1]);
}

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

    // Example
    out.push_str("\nEXAMPLE:\n  ");
    out.push_str(&{
        let fields: Vec<crate::app::Field> = spec
            .positional_args
            .iter()
            .map(|p| crate::app::Field {
                name: p.clone(),
                required: true,
                is_bool: false,
                value: String::new(),
                enum_values: vec![],
                enum_idx: None,
            })
            .collect();
        crate::preview::cli_preview(spec, &fields)
    });

    out
}
