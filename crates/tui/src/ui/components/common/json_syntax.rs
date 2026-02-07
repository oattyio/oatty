//! JSON pretty-print and syntax-highlighting helpers for TUI components.

use crate::ui::theme::roles::Theme;
use ratatui::text::{Line, Span};
/// Builds syntax-highlighted lines from an already pretty-printed JSON string.
pub fn highlight_pretty_json_lines<'value>(formatted_json: &'value str, theme: &dyn Theme) -> Vec<Line<'value>> {
    formatted_json
        .lines()
        .map(|line| Line::from(highlight_json_line(line, theme)))
        .collect()
}

fn highlight_json_line<'line>(line: &'line str, theme: &dyn Theme) -> Vec<Span<'line>> {
    let mut spans = Vec::new();
    let mut index = 0usize;
    while index < line.len() {
        let remaining = &line[index..];
        let Some(character) = remaining.chars().next() else {
            break;
        };
        let character_length = character.len_utf8();
        if character.is_whitespace() {
            let whitespace_end = remaining
                .find(|candidate: char| !candidate.is_whitespace())
                .unwrap_or(remaining.len());
            spans.push(Span::styled(&remaining[..whitespace_end], theme.text_primary_style()));
            index += whitespace_end;
            continue;
        }
        if character == '"' {
            let (token, consumed_length) = parse_json_string_token(remaining);
            spans.push(Span::styled(token, theme.syntax_string_style()));
            index += consumed_length;
            continue;
        }
        if let Some(token) = punctuation_token(character) {
            spans.push(Span::styled(token, theme.syntax_type_style()));
            index += character_length;
            continue;
        }
        if starts_with_json_keyword(remaining, "true") || starts_with_json_keyword(remaining, "false") {
            let token = if starts_with_json_keyword(remaining, "true") {
                "true"
            } else {
                "false"
            };
            spans.push(Span::styled(token, theme.syntax_keyword_style()));
            index += token.len();
            continue;
        }
        if starts_with_json_keyword(remaining, "null") {
            spans.push(Span::styled("null", theme.text_muted_style()));
            index += 4;
            continue;
        }
        if character == '-' || character.is_ascii_digit() {
            let number_length = parse_json_number_length(remaining);
            if number_length > 0 {
                spans.push(Span::styled(&remaining[..number_length], theme.syntax_number_style()));
                index += number_length;
                continue;
            }
        }

        spans.push(Span::styled(&remaining[..character_length], theme.text_primary_style()));
        index += character_length;
    }
    spans
}

fn punctuation_token(character: char) -> Option<&'static str> {
    match character {
        '{' => Some("{"),
        '}' => Some("}"),
        '[' => Some("["),
        ']' => Some("]"),
        ':' => Some(":"),
        ',' => Some(","),
        _ => None,
    }
}

fn parse_json_string_token(input: &str) -> (&str, usize) {
    let bytes = input.as_bytes();
    let mut index = 1usize;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if escaped {
            escaped = false;
            index += 1;
            continue;
        }
        if byte == b'\\' {
            escaped = true;
            index += 1;
            continue;
        }
        if byte == b'"' {
            return (&input[..=index], index + 1);
        }
        index += 1;
    }
    (input, input.len())
}

fn parse_json_number_length(input: &str) -> usize {
    let mut index = 0usize;
    for character in input.chars() {
        if character.is_ascii_digit() || matches!(character, '-' | '+' | '.' | 'e' | 'E') {
            index += character.len_utf8();
        } else {
            break;
        }
    }
    index
}

fn starts_with_json_keyword(input: &str, keyword: &str) -> bool {
    input.strip_prefix(keyword).is_some_and(|rest| {
        rest.chars()
            .next()
            .is_none_or(|character| !character.is_ascii_alphanumeric() && character != '_')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_string_token_handles_escaped_quotes() {
        let input = "\"value with \\\"escaped\\\" quote\",";
        let (token, consumed) = parse_json_string_token(input);
        assert_eq!(token, "\"value with \\\"escaped\\\" quote\"");
        assert_eq!(consumed, token.len());
    }

    #[test]
    fn starts_with_json_keyword_rejects_identifier_prefixes() {
        assert!(starts_with_json_keyword("true,", "true"));
        assert!(!starts_with_json_keyword("trueValue", "true"));
    }

    #[test]
    fn parse_json_number_length_reads_numeric_sequences() {
        assert_eq!(parse_json_number_length("-12.45e+3,"), 9);
        assert_eq!(parse_json_number_length("abc"), 0);
    }

    #[test]
    fn punctuation_token_maps_json_punctuation() {
        assert_eq!(punctuation_token('{'), Some("{"));
        assert_eq!(punctuation_token(':'), Some(":"));
        assert_eq!(punctuation_token('x'), None);
    }
}
