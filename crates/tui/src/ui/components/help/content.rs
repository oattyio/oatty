//! Reusable help content builder used by HelpComponent and the Command Browser.

use heroku_types::CommandSpec;
use ratatui::text::{Line, Span, Text};

/// Build comprehensive help text for a command specification.
///
/// Produces a themed `Text` with sections: USAGE, DESCRIPTION, ARGUMENTS, OPTIONS.
pub(crate) fn build_command_help_text<'a>(
    theme: &'a dyn crate::ui::theme::roles::Theme,
    spec: &'a CommandSpec,
) -> Text<'a> {
    let mut split = spec.name.splitn(2, ':');
    let group = split.next().unwrap_or("");
    let rest = split.next().unwrap_or("");
    let cmd = if rest.is_empty() { group.to_string() } else { format!("{} {}", group, rest) };

    let mut lines: Vec<Line<'_>> = vec![Line::from("")];
    lines.push(Line::styled(
        " USAGE:",
        theme
            .text_secondary_style()
            .add_modifier(ratatui::style::Modifier::BOLD),
    ));

    let mut usage_spans: Vec<Span<'_>> = vec![Span::styled(format!("  heroku {}", cmd), theme.text_primary_style())];

    for arg in &spec.positional_args {
        usage_spans.push(Span::styled(format!(" <{}>", arg.name), theme.text_muted_style()));
    }

    let mut flags_sorted: Vec<_> = spec.flags.iter().collect();
    flags_sorted.sort_by_key(|flag| !flag.required);
    for flag in flags_sorted.into_iter() {
        if flag.required {
            usage_spans.push(Span::styled(
                format!(" --{}", flag.name),
                theme
                    .text_secondary_style()
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ));
            if flag.r#type != "boolean" {
                usage_spans.push(Span::styled(" <value>", theme.text_muted_style()));
            }
        } else {
            usage_spans.push(Span::styled(" [", theme.text_muted_style()));
            usage_spans.push(Span::styled(
                format!("--{}", flag.name),
                theme
                    .text_secondary_style()
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ));
            if flag.r#type != "boolean" {
                usage_spans.push(Span::styled(" <value>", theme.text_muted_style()));
            }
            usage_spans.push(Span::styled("]", theme.text_muted_style()));
        }
    }

    let command: Line<'_> = Line::from(usage_spans);
    lines.push(command);
    lines.push(Line::from(""));

    lines.push(Line::styled(
        " DESCRIPTION:",
        theme
            .text_secondary_style()
            .add_modifier(ratatui::style::Modifier::BOLD),
    ));
    lines.push(Line::from(format!("  {}", spec.summary)));

    if !spec.positional_args.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::styled(
            " ARGUMENTS:",
            theme
                .text_secondary_style()
                .add_modifier(ratatui::style::Modifier::BOLD),
        ));
        for pa in &spec.positional_args {
            if let Some(desc) = &pa.help {
                let mut arg_line = Line::styled(
                    format!("  {} ", pa.name.to_uppercase()),
                    theme
                        .text_secondary_style()
                        .add_modifier(ratatui::style::Modifier::BOLD),
                );
                arg_line.push_span(Span::styled(desc.to_string(), theme.text_muted_style()));
                lines.push(arg_line);
            } else {
                lines.push(Line::from(format!("  {}: Path parameter derived from the endpoint URL.", pa.name)));
            }
        }
    }

    if !spec.flags.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::styled(
            " OPTIONS:",
            theme
                .text_secondary_style()
                .add_modifier(ratatui::style::Modifier::BOLD),
        ));
        for f in &spec.flags {
            let mut flag_line = if let Some(short) = &f.short_name {
                Line::styled(
                    format!("  -{},  --{}", short, f.name),
                    theme
                        .text_secondary_style()
                        .add_modifier(ratatui::style::Modifier::BOLD),
                )
            } else {
                Line::styled(
                    format!("  --{}", f.name),
                    theme
                        .text_secondary_style()
                        .add_modifier(ratatui::style::Modifier::BOLD),
                )
            };

            if f.r#type != "boolean" {
                flag_line.push_span(Span::styled(" <value>", theme.text_muted_style()));
            }
            if f.required {
                flag_line.push_span(Span::styled("  (required)", theme.text_muted_style()));
            }
            if !f.enum_values.is_empty() {
                flag_line.push_span(Span::styled(
                    format!("  [enum: {}]", f.enum_values.join("|")),
                    theme.text_muted_style(),
                ));
            }
            if let Some(def) = &f.default_value {
                flag_line.push_span(Span::styled(format!("  [default: {}]", def), theme.text_muted_style()));
            }
            if let Some(desc) = &f.description {
                flag_line.push_span(Span::styled(format!(" — {}", desc), theme.text_muted_style()));
            }
            lines.push(flag_line);
        }
    }

    Text::from(lines)
}

