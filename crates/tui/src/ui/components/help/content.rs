//! Reusable help content builder used by HelpComponent and the Command Browser.

use heroku_types::{CommandSpec, command::CommandExecution};
use ratatui::{
    style::Modifier,
    text::{Line, Span, Text},
};

/// Build comprehensive help text for a command specification.
///
/// Produces a themed `Text` with sections: USAGE, DESCRIPTION, BACKEND, ARGUMENTS, OPTIONS.
pub(crate) fn build_command_help_text<'a>(theme: &'a dyn crate::ui::theme::roles::Theme, spec: &'a CommandSpec) -> Text<'a> {
    let name = &spec.name;
    let group = &spec.group;
    let cmd = format!("{} {}", group, name);

    let mut lines: Vec<Line<'_>> = vec![Line::from("")];
    lines.push(Line::styled(" USAGE:", theme.text_secondary_style().add_modifier(Modifier::BOLD)));

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
                theme.text_secondary_style().add_modifier(Modifier::BOLD),
            ));
            if flag.r#type != "boolean" {
                usage_spans.push(Span::styled(" <value>", theme.text_muted_style()));
            }
        } else {
            usage_spans.push(Span::styled(" [", theme.text_muted_style()));
            usage_spans.push(Span::styled(
                format!("--{}", flag.name),
                theme.text_secondary_style().add_modifier(Modifier::BOLD),
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
        theme.text_secondary_style().add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::from(format!("  {}", spec.summary)));

    match spec.execution() {
        CommandExecution::Http(http) => {
            lines.push(Line::from(""));
            lines.push(Line::styled(" BACKEND:", theme.text_secondary_style().add_modifier(Modifier::BOLD)));
            lines.push(Line::from(format!(
                "  HTTP {} {} (service: {:?})",
                http.method, http.path, http.service_id
            )));
        }
        CommandExecution::Mcp(mcp) => {
            lines.push(Line::from(""));
            lines.push(Line::styled(" BACKEND:", theme.text_secondary_style().add_modifier(Modifier::BOLD)));
            lines.push(Line::from(format!("  MCP plugin '{}' tool '{}'", mcp.plugin_name, mcp.tool_name)));
            if let Some(auth) = &mcp.auth_summary {
                lines.push(Line::from(format!("  Auth: {}", auth)));
            }
        }
    }
    let mut show_providers_note = false;
    if !spec.positional_args.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::styled(
            " ARGUMENTS:",
            theme.text_secondary_style().add_modifier(Modifier::BOLD),
        ));
        for pa in &spec.positional_args {
            let has_provider = if pa.provider.is_some() { "(*) " } else { "" };
            show_providers_note |= pa.provider.is_some();

            if let Some(desc) = &pa.help {
                let mut arg_line = Line::styled(
                    format!("  {} ", pa.name.to_uppercase()),
                    theme.text_secondary_style().add_modifier(Modifier::BOLD),
                );
                arg_line.push_span(Span::styled(format!("{has_provider}{desc}"), theme.text_muted_style()));
                lines.push(arg_line);
            } else {
                lines.push(Line::from(format!("  {}: Path parameter derived from the endpoint URL.", pa.name)));
            }
        }
    }

    if !spec.flags.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::styled(" OPTIONS:", theme.text_secondary_style().add_modifier(Modifier::BOLD)));
        for flag in &spec.flags {
            let mut flag_line = if let Some(short) = &flag.short_name {
                Line::styled(
                    format!("  -{},  --{}", short, flag.name),
                    theme.text_secondary_style().add_modifier(Modifier::BOLD),
                )
            } else {
                Line::styled(
                    format!("  --{}", flag.name),
                    theme.text_secondary_style().add_modifier(Modifier::BOLD),
                )
            };

            if flag.r#type != "boolean" {
                flag_line.push_span(Span::styled(" <value>", theme.text_muted_style()));
            }
            if flag.required {
                flag_line.push_span(Span::styled("  (required)", theme.text_muted_style()));
            }
            if !flag.enum_values.is_empty() {
                flag_line.push_span(Span::styled(
                    format!("  [enum: {}]", flag.enum_values.join("|")),
                    theme.text_muted_style(),
                ));
            }
            if let Some(def) = &flag.default_value {
                flag_line.push_span(Span::styled(format!("  [default: {}]", def), theme.text_muted_style()));
            }
            if let Some(desc) = &flag.description {
                let has_provider = if flag.provider.is_some() { "(*) " } else { "" };
                show_providers_note |= flag.provider.is_some();
                flag_line.push_span(Span::styled(format!(" — {}{}", has_provider, desc), theme.text_muted_style()));
            }
            lines.push(flag_line);
        }
    }

    if show_providers_note {
        lines.push(Line::from(""));
        lines.push(Line::styled(
            "(*) Value options are provided to you from your account info (Tab to invoke)",
            theme.text_muted_style(),
        ));
    }

    Text::from(lines)
}
