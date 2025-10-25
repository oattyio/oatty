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

    let mut lines: Vec<Line<'_>> = vec![Line::from("")];
    lines.push(Line::styled(" USAGE:", theme.text_primary_style().add_modifier(Modifier::BOLD)));

    let mut usage_spans: Vec<Span<'_>> = vec![
        Span::styled("  ", theme.text_primary_style()),
        Span::styled("heroku", theme.syntax_keyword_style()),
        Span::raw(" "),
        Span::styled(group.to_string(), theme.syntax_type_style()),
        Span::raw(" "),
        Span::styled(name.to_string(), theme.syntax_function_style()),
    ];

    for arg in &spec.positional_args {
        usage_spans.push(Span::raw(" "));
        usage_spans.push(Span::styled(format!("<{}>", arg.name), theme.syntax_type_style()));
    }

    let mut flags_sorted: Vec<_> = spec.flags.iter().collect();
    flags_sorted.sort_by_key(|flag| !flag.required);
    for flag in flags_sorted.into_iter() {
        if flag.required {
            usage_spans.push(Span::raw(" "));
            usage_spans.push(Span::styled(format!("--{}", flag.name), theme.syntax_keyword_style()));
            if flag.r#type != "boolean" {
                usage_spans.push(Span::raw(" "));
                usage_spans.push(Span::styled("<value>", theme.syntax_string_style()));
            }
        } else {
            usage_spans.push(Span::styled(" [", theme.text_muted_style()));
            usage_spans.push(Span::styled(format!("--{}", flag.name), theme.syntax_keyword_style()));
            if flag.r#type != "boolean" {
                usage_spans.push(Span::raw(" "));
                usage_spans.push(Span::styled("<value>", theme.syntax_string_style()));
            }
            usage_spans.push(Span::styled("]", theme.text_muted_style()));
        }
    }

    let command: Line<'_> = Line::from(usage_spans);
    lines.push(command);
    lines.push(Line::from(""));

    lines.push(Line::styled(
        " DESCRIPTION:",
        theme.text_primary_style().add_modifier(Modifier::BOLD),
    ));
    let summary_lines = spec.summary.trim().split('\n').map(|line| Line::from(format!("  {}", line.trim())));
    lines.extend(summary_lines);

    match spec.execution() {
        CommandExecution::Http(http) => {
            lines.push(Line::from(""));
            lines.push(Line::styled(" BACKEND:", theme.text_primary_style().add_modifier(Modifier::BOLD)));
            let mut backend_spans = vec![
                Span::styled("  HTTP ", theme.syntax_keyword_style()),
                Span::styled(http.method.clone(), theme.syntax_keyword_style()),
                Span::raw(" "),
                Span::styled(http.path.clone(), theme.syntax_string_style()),
            ];
            backend_spans.push(Span::styled(format!(" (service: {:?})", http.service_id), theme.text_muted_style()));
            lines.push(Line::from(backend_spans));
        }
        CommandExecution::Mcp(mcp) => {
            lines.push(Line::from(""));
            lines.push(Line::styled(" BACKEND:", theme.text_secondary_style().add_modifier(Modifier::BOLD)));
            let mut backend_spans = vec![
                Span::styled("  MCP plugin ", theme.syntax_keyword_style()),
                Span::styled(format!("'{}'", mcp.plugin_name), theme.syntax_type_style()),
                Span::styled(" tool ", theme.text_secondary_style()),
                Span::styled(format!("'{}'", mcp.tool_name), theme.syntax_function_style()),
            ];
            if let Some(auth) = &mcp.auth_summary {
                backend_spans.push(Span::raw(" "));
                backend_spans.push(Span::styled(format!("Auth: {}", auth), theme.text_muted_style()));
            }
            lines.push(Line::from(backend_spans));
        }
    }
    let mut show_providers_note = false;
    if !spec.positional_args.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::styled(" ARGUMENTS:", theme.text_primary_style().add_modifier(Modifier::BOLD)));
        for pa in &spec.positional_args {
            let mut spans: Vec<Span<'_>> = vec![
                Span::styled("  ", theme.text_primary_style()),
                Span::styled(pa.name.to_uppercase(), theme.syntax_type_style().add_modifier(Modifier::BOLD)),
            ];

            let has_provider = pa.provider.is_some();
            show_providers_note |= has_provider;

            if let Some(desc) = &pa.help {
                spans.push(Span::raw(" "));
                if has_provider {
                    spans.push(Span::styled("(*) ", theme.syntax_keyword_style()));
                }
                spans.push(Span::styled(desc.to_string(), theme.text_muted_style()));
                lines.push(Line::from(spans));
            } else {
                spans.push(Span::styled(
                    ": Path parameter derived from the endpoint URL.",
                    theme.text_muted_style(),
                ));
                lines.push(Line::from(spans));
            }
        }
    }

    if !spec.flags.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::styled(" OPTIONS:", theme.text_primary_style().add_modifier(Modifier::BOLD)));
        for flag in &spec.flags {
            show_providers_note |= flag.provider.is_some();

            let mut spans: Vec<Span<'_>> = vec![Span::styled("  ", theme.text_primary_style())];
            if let Some(short) = &flag.short_name {
                spans.push(Span::styled(format!("-{}", short), theme.syntax_keyword_style()));
                spans.push(Span::styled(", ", theme.text_primary_style()));
            }
            spans.push(Span::styled(format!("--{}", flag.name), theme.syntax_keyword_style()));

            if flag.r#type != "boolean" {
                spans.push(Span::raw(" "));
                spans.push(Span::styled("<value>", theme.syntax_string_style()));
            }
            if flag.required {
                spans.push(Span::raw(" "));
                spans.push(Span::styled("(required)", theme.syntax_keyword_style()));
            }
            if !flag.enum_values.is_empty() {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    format!("[enum: {}]", flag.enum_values.join("|")),
                    theme.syntax_string_style(),
                ));
            }
            if let Some(def) = &flag.default_value {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(format!("[default: {}]", def), theme.syntax_number_style()));
            }
            if let Some(desc) = &flag.description {
                spans.push(Span::raw(" — "));
                if flag.provider.is_some() {
                    spans.push(Span::styled("(*) ", theme.syntax_keyword_style()));
                }
                spans.push(Span::styled(desc.to_string(), theme.text_muted_style()));
            }
            lines.push(Line::from(spans));
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
