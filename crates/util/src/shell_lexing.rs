//! # Shell-like Lexing Utilities
//!
//! This module provides utilities for tokenizing shell-like input, supporting
//! single and double quotes, backslash escapes, and position tracking.

/// Tokenize input using a simple, shell-like lexer.
///
/// Supports single and double quotes and backslash escapes. Used by the
/// suggestion engine to derive tokens and assess completeness of flag values.
/// This function returns owned strings, discarding position information.
///
/// # Arguments
/// * `input` - The raw input line to tokenize
///
/// # Returns
/// A vector of tokens preserving quoted segments
///
/// # Example
/// ```rust
/// use heroku_util::shell_lexing::lex_shell_like;
///
/// let tokens = lex_shell_like("cmd --flag 'some value'");
/// assert_eq!(
///     tokens,
///     vec!["cmd", "--flag", "'some value'"]
/// );
///
/// let tokens = lex_shell_like("echo \"hello world\"");
/// assert_eq!(
///     tokens,
///     vec!["echo", "\"hello world\""]
/// );
///
/// let tokens = lex_shell_like("path\\ with\\ spaces");
/// assert_eq!(
///     tokens,
///     vec!["path\\ with\\ spaces"]
/// );
/// ```
pub fn lex_shell_like(input: &str) -> Vec<String> {
    lex_shell_like_ranged(input)
        .into_iter()
        .map(|token| token.text.to_string())
        .collect()
}

/// Token with original byte positions.
///
/// Represents a single token extracted from shell-like input,
/// preserving both the text content and its position in the original string.
/// This is useful for applications that need to know where tokens
/// appear in the original text for error reporting or highlighting.
///
/// # Fields
/// * `text` - The text content of the token
/// * `start` - The starting byte position in the original string
/// * `end` - The ending byte position in the original string
#[derive(Debug, Clone)]
pub struct LexToken<'a> {
    /// The text content of the token
    pub text: &'a str,
    /// The starting byte position in the original string
    pub start: usize,
    /// The ending byte position in the original string
    pub end: usize,
}

/// Tokenize input returning borrowed slices and byte ranges.
///
/// This function provides more detailed tokenization than `lex_shell_like`,
/// returning tokens with their original positions in the input string.
/// This is useful for applications that need to know where tokens
/// appear in the original text for error reporting, syntax highlighting,
/// or cursor positioning.
///
/// # Arguments
/// * `input` - The input string to tokenize
///
/// # Returns
/// A vector of tokens with position information
///
/// # Example
/// ```rust
/// use heroku_util::shell_lexing::lex_shell_like_ranged;
///
/// let tokens = lex_shell_like_ranged("hello world");
/// assert_eq!(tokens.len(), 2);
/// assert_eq!(tokens[0].text, "hello");
/// assert_eq!(tokens[0].start, 0);
/// assert_eq!(tokens[0].end, 5);
///
/// assert_eq!(tokens[1].text, "world");
/// assert_eq!(tokens[1].start, 6);
/// assert_eq!(tokens[1].end, 11);
///
/// // Quoted strings preserve their quotes
/// let tokens = lex_shell_like_ranged("cmd 'arg with spaces'");
/// assert_eq!(tokens[1].text, "'arg with spaces'");
/// ```
pub fn lex_shell_like_ranged(input: &str) -> Vec<LexToken<'_>> {
    let mut tokens = Vec::new();
    let mut current_index = 0usize;
    let bytes = input.as_bytes();

    while current_index < bytes.len() {
        // Skip leading whitespace
        current_index = skip_whitespace(bytes, current_index);

        if current_index >= bytes.len() {
            break;
        }

        let start = current_index;
        current_index = parse_token(bytes, current_index);

        tokens.push(LexToken {
            text: &input[start..current_index],
            start,
            end: current_index,
        });
    }

    tokens
}

/// Skips whitespace characters starting from the given index.
///
/// This function advances the index past any consecutive whitespace
/// characters, including spaces, tabs, and other ASCII whitespace.
///
/// # Arguments
/// * `bytes` - The byte array to scan
/// * `start_index` - The starting index to begin scanning from
///
/// # Returns
/// The index after the last whitespace character
fn skip_whitespace(bytes: &[u8], start_index: usize) -> usize {
    let mut index = start_index;
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

/// Parses a single token from the input bytes.
///
/// Handles quoted strings, escaped characters, and regular text.
/// Stops at whitespace or the end of input. This function properly
/// handles nested quotes and escaped characters within quoted strings.
///
/// # Arguments
/// * `bytes` - The byte array to parse
/// * `start_index` - The starting index for this token
///
/// # Returns
/// The index after the end of the token
fn parse_token(bytes: &[u8], start_index: usize) -> usize {
    let mut index = start_index;
    let mut in_single_quotes = false;
    let mut in_double_quotes = false;

    while index < bytes.len() {
        let byte = bytes[index];

        // Handle escaped characters
        if byte == b'\\' && index + 1 < bytes.len() {
            index += 2;
            continue;
        }

        // Handle single quotes
        if byte == b'\'' && !in_double_quotes {
            in_single_quotes = !in_single_quotes;
            index += 1;
            continue;
        }

        // Handle double quotes
        if byte == b'"' && !in_single_quotes {
            in_double_quotes = !in_double_quotes;
            index += 1;
            continue;
        }

        // Stop at whitespace if not in quotes
        if !in_single_quotes && !in_double_quotes && byte.is_ascii_whitespace() {
            break;
        }

        index += 1;
    }

    index
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokenization() {
        let tokens = lex_shell_like("hello world");
        assert_eq!(tokens, vec!["hello", "world"]);
    }

    #[test]
    fn test_quoted_strings() {
        let tokens = lex_shell_like("cmd 'arg with spaces'");
        assert_eq!(tokens, vec!["cmd", "'arg with spaces'"]);

        let tokens = lex_shell_like("echo \"hello world\"");
        assert_eq!(tokens, vec!["echo", "\"hello world\""]);
    }

    #[test]
    fn test_escaped_characters() {
        let tokens = lex_shell_like("path\\ with\\ spaces");
        assert_eq!(tokens, vec!["path\\ with\\ spaces"]);
    }

    #[test]
    fn test_mixed_quotes() {
        let tokens = lex_shell_like("cmd 'single' \"double\"");
        assert_eq!(tokens, vec!["cmd", "'single'", "\"double\""]);
    }

    #[test]
    fn test_empty_input() {
        let tokens = lex_shell_like("");
        assert_eq!(tokens, Vec::<String>::new());
    }

    #[test]
    fn test_whitespace_only() {
        let tokens = lex_shell_like("   \t  \n  ");
        assert_eq!(tokens, Vec::<String>::new());
    }

    #[test]
    fn test_ranged_tokenization() {
        let tokens = lex_shell_like_ranged("hello world");
        assert_eq!(tokens.len(), 2);

        assert_eq!(tokens[0].text, "hello");
        assert_eq!(tokens[0].start, 0);
        assert_eq!(tokens[0].end, 5);

        assert_eq!(tokens[1].text, "world");
        assert_eq!(tokens[1].start, 6);
        assert_eq!(tokens[1].end, 11);
    }

    #[test]
    fn test_quoted_ranged_tokenization() {
        let input = "cmd 'arg with spaces'";
        let tokens = lex_shell_like_ranged(input);
        assert_eq!(tokens.len(), 2);

        assert_eq!(tokens[0].text, "cmd");
        assert_eq!(tokens[0].start, 0);
        assert_eq!(tokens[0].end, 3);

        assert_eq!(tokens[1].text, "'arg with spaces'");
        assert_eq!(tokens[1].start, 4);
        assert_eq!(tokens[1].end, input.len());
    }
}
