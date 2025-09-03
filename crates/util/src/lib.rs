use chrono::{DateTime, Datelike, NaiveDate};
use once_cell::sync::Lazy;
use regex::Regex;

/// Redacts values that look like secrets in a string.
///
/// This function scans input text for patterns that commonly indicate
/// sensitive information like API keys, tokens, passwords, and database URLs.
/// When found, these values are replaced with `<redacted>` while preserving
/// the key names for debugging purposes.
///
/// # Arguments
/// * `input` - The input string that may contain sensitive information
///
/// # Returns
/// A new string with sensitive values redacted
///
/// # Example
/// ```rust
/// use heroku_util::redact_sensitive;
///
/// let input = "API_KEY=abc123 TOKEN=xyz789";
/// let redacted = redact_sensitive(input);
/// assert_eq!(redacted, "API_KEY=<redacted> TOKEN=<redacted>");
/// ```
pub fn redact_sensitive(input: &str) -> String {
    let mut redacted = input.to_string();

    for pattern in get_redact_patterns().iter() {
        redacted = pattern
            .replace_all(&redacted, |caps: &regex::Captures| {
                let prefix = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                format!("{}<redacted>", prefix)
            })
            .to_string();
    }

    redacted
}

/// Returns compiled regex patterns for detecting sensitive information.
///
/// These patterns are compiled once and reused for performance.
/// They detect:
/// - Authorization headers
/// - Environment variables ending in KEY, TOKEN, SECRET, or PASSWORD
/// - Database URLs
///
/// # Returns
/// A vector of compiled regex patterns
fn get_redact_patterns() -> &'static Vec<Regex> {
    static REDACT_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
        vec![
            Regex::new(r"(?i)(authorization: )([\w\-\.=:/+]+)").unwrap(),
            Regex::new(r"(?i)([A-Z0-9_]*?(KEY|TOKEN|SECRET|PASSWORD)=)([^\s]+)").unwrap(),
            Regex::new(r"(?i)(DATABASE_URL)=([^\s]+)").unwrap(),
        ]
    });

    &REDACT_PATTERNS
}

/// Fuzzy matcher for subsequence scoring, handling space-separated tokens.
///
/// Returns `Some(score)` if all characters in each space-separated token of `needle`
/// appear in order within `hay`, otherwise returns `None`. Higher scores indicate better
/// matches. The scoring favors consecutive matches, word boundary matches, prefix matches,
/// and shorter candidates. Spaces in `needle` are treated as token separators, not literal
/// characters, unless the entire `needle` is a single space.
///
/// # Arguments
/// * `hay` - The candidate string to search within
/// * `needle` - The query to match, with space-separated tokens
///
/// # Returns
/// `Option<i64>` where `Some(score)` indicates a match
///
/// # Example
/// ```rust
/// use heroku_util::fuzzy_score;
/// assert!(fuzzy_score("applications", "app").unwrap() > 0);
/// assert!(fuzzy_score("string1 string2", "string1 string2").unwrap() > 0);
/// assert!(fuzzy_score("string1:string2", "string1 string2").unwrap() > 0);
/// assert!(fuzzy_score("applications", "qqqqqq").is_none());
/// assert!(fuzzy_score("", "app").is_none());
/// ```
pub fn fuzzy_score(hay: &str, needle: &str) -> Option<i64> {
    if hay.is_empty() && !needle.is_empty() {
        return None;
    }
    if needle.is_empty() {
        return Some(0);
    }

    // Handle space-only needle
    if needle.trim().is_empty() {
        return Some(0);
    }

    let hay_data = prepare_haystack(hay);
    let needle_tokens = prepare_needle_tokens(needle);

    if needle_tokens.is_empty() {
        return Some(0);
    }

    let total_score = score_all_tokens(&hay_data, &needle_tokens)?;

    // Penalty for hay length (shorter candidates preferred)
    let final_score = total_score - hay_data.chars.len() as i64 / 8;

    Some(final_score)
}

/// Prepares the haystack string for fuzzy matching.
///
/// Converts the input to lowercase and collects characters for efficient
/// indexing during the matching process.
///
/// # Arguments
/// * `hay` - The original haystack string
///
/// # Returns
/// A struct containing the prepared haystack data
struct HaystackData {
    lower: String,
    chars: Vec<char>,
}

fn prepare_haystack(hay: &str) -> HaystackData {
    let lower: String = hay.chars().flat_map(|c| c.to_lowercase()).collect();
    let chars: Vec<char> = lower.chars().collect();

    HaystackData { lower, chars }
}

/// Prepares needle tokens for fuzzy matching.
///
/// Splits the needle into space-separated tokens, converts each to lowercase,
/// and filters out empty tokens.
///
/// # Arguments
/// * `needle` - The needle string to tokenize
///
/// # Returns
/// A vector of character vectors representing the tokens
fn prepare_needle_tokens(needle: &str) -> Vec<Vec<char>> {
    needle
        .split_whitespace()
        .map(|token| token.chars().flat_map(|c| c.to_lowercase()).collect())
        .filter(|token: &Vec<char>| !token.is_empty())
        .collect()
}

/// Scores all tokens in the needle against the haystack.
///
/// Iterates through each token, finding the best match and accumulating
/// the total score across all tokens.
///
/// # Arguments
/// * `hay_data` - The prepared haystack data
/// * `needle_tokens` - The prepared needle tokens
///
/// # Returns
/// Some total score if all tokens match, None if any token fails to match
fn score_all_tokens(hay_data: &HaystackData, needle_tokens: &[Vec<char>]) -> Option<i64> {
    let mut total_score = 0;
    let mut hay_idx = 0;

    for token in needle_tokens {
        let token_score = score_single_token(hay_data, token, &mut hay_idx)?;
        total_score += token_score;
    }

    Some(total_score)
}

/// Scores a single token against the haystack.
///
/// Finds the best match for a token within the haystack, starting from
/// the current haystack index. Updates the haystack index for the next token.
///
/// # Arguments
/// * `hay_data` - The prepared haystack data
/// * `token` - The token to score
/// * `hay_idx` - The current position in the haystack (updated during matching)
///
/// # Returns
/// Some score if the token matches, None if it doesn't match
fn score_single_token(hay_data: &HaystackData, token: &[char], hay_idx: &mut usize) -> Option<i64> {
    let mut token_score = 0;
    let mut consec = 0;
    let mut first_match_idx = None;
    let mut prev_idx = None;

    for &n_char in token {
        let found = hay_data.chars[*hay_idx..]
            .iter()
            .enumerate()
            .find(|(_, c)| **c == n_char);

        if let Some((rel_idx, _)) = found {
            let abs_idx = *hay_idx + rel_idx;

            if first_match_idx.is_none() {
                first_match_idx = Some(abs_idx);
            }

            token_score += score_character_match(abs_idx, prev_idx, &mut consec, hay_data);

            *hay_idx = abs_idx + 1;
            prev_idx = Some(abs_idx);
        } else {
            return None; // No match for this character
        }
    }

    // Add bonuses for this token
    token_score += calculate_token_bonuses(hay_data, token, first_match_idx);

    Some(token_score)
}

/// Scores a single character match within a token.
///
/// Calculates the score contribution for matching a character, including
/// consecutive match bonuses, gap penalties, and word boundary bonuses.
///
/// # Arguments
/// * `abs_idx` - The absolute index of the current match
/// * `prev_idx` - The index of the previous match (if any)
/// * `consec` - The consecutive match counter (updated)
/// * `hay_data` - The prepared haystack data
///
/// # Returns
/// The score contribution for this character match
fn score_character_match(abs_idx: usize, prev_idx: Option<usize>, consec: &mut i64, hay_data: &HaystackData) -> i64 {
    let mut score = 0;

    // Handle consecutive matches
    if let Some(prev) = prev_idx {
        if abs_idx == prev + 1 {
            *consec += 1;
        } else {
            *consec = 1; // Reset on non-consecutive match
        }

        // Penalize gaps
        let gap = (abs_idx - prev - 1) as i64;
        score -= gap / 2;
    }

    score += 6 * *consec;

    // Bonus for word boundary (start of hay or after space/punctuation)
    if is_word_boundary(abs_idx, hay_data) {
        score += 10;
    }

    score
}

/// Checks if a position in the haystack represents a word boundary.
///
/// A word boundary occurs at the start of the string or after whitespace
/// or punctuation characters.
///
/// # Arguments
/// * `idx` - The index to check
/// * `hay_data` - The prepared haystack data
///
/// # Returns
/// True if the position is a word boundary
fn is_word_boundary(idx: usize, hay_data: &HaystackData) -> bool {
    idx == 0
        || hay_data
            .chars
            .get(idx - 1)
            .is_some_and(|c| c.is_whitespace() || c.is_ascii_punctuation())
}

/// Calculates bonus scores for a token match.
///
/// Applies bonuses for prefix matches and early positioning within
/// the haystack.
///
/// # Arguments
/// * `hay_data` - The prepared haystack data
/// * `token` - The token that was matched
/// * `first_match_idx` - The index of the first character match
///
/// # Returns
/// The total bonus score for this token
fn calculate_token_bonuses(hay_data: &HaystackData, token: &[char], first_match_idx: Option<usize>) -> i64 {
    let mut bonus = 0;

    // Prefix match bonus for this token
    let token_str: String = token.iter().copied().collect();
    if hay_data.lower.starts_with(&token_str) {
        bonus += 30;
    }

    // Early match bonus
    if let Some(start) = first_match_idx {
        bonus += i64::max(0, 20 - start as i64);
    }

    bonus
}

/// Tokenize input using a simple, shell-like lexer.
///
/// Supports single and double quotes and backslash escapes. Used by the
/// suggestion engine to derive tokens and assess completeness of flag values.
///
/// # Arguments
/// * `input` - The raw input line
///
/// # Returns
/// A vector of tokens preserving quoted segments
///
/// # Example
/// ```rust
/// use heroku_util::lex_shell_like;
/// let toks = lex_shell_like("cmd --flag 'some value'");
/// assert_eq!(
///     toks,
///     vec!["cmd", "--flag", "'some value'"]
///         .iter()
///         .map(|s| s.to_string())
///         .collect::<Vec<_>>()
/// );
/// ```
pub fn lex_shell_like(input: &str) -> Vec<String> {
    lex_shell_like_ranged(input)
        .into_iter()
        .map(|t| t.text.to_string())
        .collect()
}

/// Token with original byte positions.
///
/// Represents a single token extracted from shell-like input,
/// preserving both the text content and its position in the original string.
#[derive(Debug, Clone)]
pub struct LexTok<'a> {
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
/// appear in the original text.
///
/// # Arguments
/// * `input` - The input string to tokenize
///
/// # Returns
/// A vector of tokens with position information
///
/// # Example
/// ```rust
/// use heroku_util::lex_shell_like_ranged;
/// let tokens = lex_shell_like_ranged("hello world");
/// assert_eq!(tokens.len(), 2);
/// assert_eq!(tokens[0].text, "hello");
/// assert_eq!(tokens[0].start, 0);
/// assert_eq!(tokens[0].end, 5);
/// ```
pub fn lex_shell_like_ranged(input: &str) -> Vec<LexTok<'_>> {
    let mut tokens = Vec::new();
    let mut i = 0usize;
    let bytes = input.as_bytes();

    while i < bytes.len() {
        // Skip leading whitespace
        i = skip_whitespace(bytes, i);

        if i >= bytes.len() {
            break;
        }

        let start = i;
        i = parse_token(bytes, i);

        tokens.push(LexTok {
            text: &input[start..i],
            start,
            end: i,
        });
    }

    tokens
}

/// Skips whitespace characters starting from the given index.
///
/// # Arguments
/// * `bytes` - The byte array to scan
/// * `start_idx` - The starting index
///
/// # Returns
/// The index after the last whitespace character
fn skip_whitespace(bytes: &[u8], start_idx: usize) -> usize {
    let mut i = start_idx;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// Parses a single token from the input bytes.
///
/// Handles quoted strings, escaped characters, and regular text.
/// Stops at whitespace or the end of input.
///
/// # Arguments
/// * `bytes` - The byte array to parse
/// * `start_idx` - The starting index for this token
///
/// # Returns
/// The index after the end of the token
fn parse_token(bytes: &[u8], start_idx: usize) -> usize {
    let mut i = start_idx;
    let mut in_single_quotes = false;
    let mut in_double_quotes = false;

    while i < bytes.len() {
        let b = bytes[i];

        // Handle escaped characters
        if b == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }

        // Handle single quotes
        if b == b'\'' && !in_double_quotes {
            in_single_quotes = !in_single_quotes;
            i += 1;
            continue;
        }

        // Handle double quotes
        if b == b'"' && !in_single_quotes {
            in_double_quotes = !in_double_quotes;
            i += 1;
            continue;
        }

        // Stop at whitespace if not in quotes
        if !in_single_quotes && !in_double_quotes && b.is_ascii_whitespace() {
            break;
        }

        i += 1;
    }

    i
}

// Generated at build-time from schemas/heroku-schema.json
pub mod generated_date_fields {
    include!(concat!(env!("OUT_DIR"), "/date_fields.rs"));
}

/// Returns true if a JSON key looks like a date field.
///
/// Uses generated schema-derived keys with fallback heuristics.
/// Checks for common date field patterns like suffixes (_at, _on, _date)
/// and specific field names (created, updated, released).
///
/// # Arguments
/// * `key` - The JSON key to check
///
/// # Returns
/// True if the key appears to represent a date field
///
/// # Example
/// ```rust
/// use heroku_util::is_date_like_key;
/// assert!(is_date_like_key("created_at"));
/// assert!(is_date_like_key("updated_on"));
/// assert!(is_date_like_key("release_date"));
/// assert!(is_date_like_key("created"));
/// assert!(!is_date_like_key("name"));
/// ```
pub fn is_date_like_key(key: &str) -> bool {
    let normalized_key = normalize_date_key(key);

    // Check against generated schema keys first
    if generated_date_fields::DATE_FIELD_KEYS.contains(&normalized_key.as_str()) {
        return true;
    }

    // Fallback to heuristic patterns
    is_heuristic_date_key(&normalized_key)
}

/// Normalizes a key for date field detection.
///
/// Converts to lowercase and replaces spaces and hyphens with underscores
/// to standardize the format for comparison.
///
/// # Arguments
/// * `key` - The original key string
///
/// # Returns
/// The normalized key string
fn normalize_date_key(key: &str) -> String {
    key.to_ascii_lowercase().replace([' ', '-'], "_")
}

/// Applies heuristic rules to determine if a normalized key is date-like.
///
/// Checks for common date field patterns that aren't covered by the
/// generated schema.
///
/// # Arguments
/// * `normalized_key` - The normalized key string
///
/// # Returns
/// True if the key matches date field heuristics
fn is_heuristic_date_key(normalized_key: &str) -> bool {
    normalized_key.ends_with("_at")
        || normalized_key.ends_with("_on")
        || normalized_key.ends_with("_date")
        || normalized_key == "created"
        || normalized_key == "updated"
        || normalized_key == "released"
}

/// Formats common date strings into MM/DD/YYYY if parsable.
///
/// Attempts to parse the input string using common date formats
/// and returns a formatted string if successful. Supports RFC3339
/// timestamps and ISO date formats.
///
/// # Arguments
/// * `s` - The date string to format
///
/// # Returns
/// Some formatted date string if parsing succeeds, None otherwise
///
/// # Example
/// ```rust
/// use heroku_util::format_date_mmddyyyy;
/// assert_eq!(format_date_mmddyyyy("2023-12-25"), Some("12/25/2023".to_string()));
/// assert_eq!(format_date_mmddyyyy("2023/12/25"), Some("12/25/2023".to_string()));
/// assert_eq!(format_date_mmddyyyy("2023-12-25T10:30:00Z"), Some("12/25/2023".to_string()));
/// assert_eq!(format_date_mmddyyyy("invalid"), None);
/// ```
pub fn format_date_mmddyyyy(s: &str) -> Option<String> {
    // Try RFC3339 timestamp format first
    if let Some(formatted) = parse_rfc3339_date(s) {
        return Some(formatted);
    }

    // Try ISO date formats
    if let Some(formatted) = parse_iso_date(s) {
        return Some(formatted);
    }

    None
}

/// Parses an RFC3339 timestamp and formats it as MM/DD/YYYY.
///
/// # Arguments
/// * `s` - The RFC3339 timestamp string
///
/// # Returns
/// Some formatted date string if parsing succeeds, None otherwise
fn parse_rfc3339_date(s: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(s).ok().map(|dt| {
        let d = dt.date_naive();
        format!("{:02}/{:02}/{}", d.month(), d.day(), d.year())
    })
}

/// Parses ISO date formats and formats them as MM/DD/YYYY.
///
/// Supports both YYYY-MM-DD and YYYY/MM/DD formats.
///
/// # Arguments
/// * `s` - The ISO date string
///
/// # Returns
/// Some formatted date string if parsing succeeds, None otherwise
fn parse_iso_date(s: &str) -> Option<String> {
    let formats = ["%Y-%m-%d", "%Y/%m/%d"];

    for format in formats.iter() {
        if let Ok(d) = NaiveDate::parse_from_str(s, format) {
            return Some(format!("{:02}/{:02}/{}", d.month(), d.day(), d.year()));
        }
    }

    None
}
