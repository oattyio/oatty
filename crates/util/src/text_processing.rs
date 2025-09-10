//! # Text Processing Utilities
//!
//! This module provides utilities for text processing, including sensitive data redaction
//! and fuzzy string matching with sophisticated scoring algorithms.

use once_cell::sync::Lazy;
use regex::Regex;

/// Redacts values that look like secrets in a string.
///
/// This function scans input text for patterns that commonly indicate
/// sensitive information like API keys, tokens, passwords, and database URLs.
/// When found, these values are replaced with `[REDACTED]` while preserving
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
/// use heroku_util::text_processing::redact_sensitive;
///
/// let input = "API_KEY=abc123 TOKEN=xyz789";
/// let redacted = redact_sensitive(input);
/// assert_eq!(redacted, "API_KEY=[REDACTED] TOKEN=[REDACTED]");
///
/// let input = "Authorization: Bearer secret123";
/// let redacted = redact_sensitive(input);
/// assert_eq!(redacted, "Authorization: [REDACTED]");
/// ```
pub fn redact_sensitive(input: &str) -> String {
    redact_sensitive_with(input, "[REDACTED]")
}

/// Redacts sensitive-looking values, using a custom replacement token.
pub fn redact_sensitive_with(input: &str, replacement: &str) -> String {
    let mut redacted = input.to_string();

    for pattern in get_redact_patterns().iter() {
        redacted = pattern
            .replace_all(&redacted, |captures: &regex::Captures| {
                let prefix = captures.get(1).map(|m| m.as_str()).unwrap_or("");
                format!("{}{}", prefix, replacement)
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
            // Authorization headers (keep prefix, redact value)
            Regex::new(r"(?i)(authorization:\s+)([^\s]+(?:\s+[^\s]+)*)").unwrap(),
            // Bearer tokens in free text
            Regex::new(r"(?i)((?:^|\b)Bearer\s+)([A-Za-z0-9\-._~+/]+=*)").unwrap(),
            // Basic auth in free text
            Regex::new(r"(?i)((?:^|\b)Basic\s+)([A-Za-z0-9+/]+=*)").unwrap(),
            // Common key/token env or labels (keep prefix including delimiter)
            Regex::new(r"(?i)((?:api[\s_-]?key|auth[\s_-]?token|token|secret|password)\s*[:=]\s*)([^\s,;]+)").unwrap(),
            // Env-like KEY=VALUE patterns for KEY/TOKEN/SECRET/PASSWORD
            Regex::new(r"(?i)((?:[A-Z0-9_]*?(?:KEY|TOKEN|SECRET|PASSWORD))=)([^\s]+)").unwrap(),
            // Database URLs
            Regex::new(r"(?i)(DATABASE_URL=)([^\s]+)").unwrap(),
            // JWT-like tokens (replace entirely)
            Regex::new(r"(eyJ[A-Za-z0-9\-._~+/]+=*)").unwrap(),
            // UUIDs (replace entirely)
            Regex::new(r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b").unwrap(),
            // Credit card numbers (replace entirely)
            Regex::new(r"\b\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}\b").unwrap(),
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
/// use heroku_util::text_processing::fuzzy_score;
///
/// // Basic matching
/// assert!(fuzzy_score("applications", "app").unwrap() > 0);
/// assert!(fuzzy_score("string1 string2", "string1 string2").unwrap() > 0);
/// assert!(fuzzy_score("string1:string2", "string1 string2").unwrap() > 0);
///
/// // No matches
/// assert!(fuzzy_score("applications", "qqqqqq").is_none());
/// assert!(fuzzy_score("", "app").is_none());
///
/// // Edge cases
/// assert_eq!(fuzzy_score("", ""), Some(0));
/// assert_eq!(fuzzy_score("hello", ""), Some(0));
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
    let mut hay_index = 0;

    for token in needle_tokens {
        let token_score = score_single_token(hay_data, token, &mut hay_index)?;
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
/// * `hay_index` - The current position in the haystack (updated during matching)
///
/// # Returns
/// Some score if the token matches, None if it doesn't match
fn score_single_token(hay_data: &HaystackData, token: &[char], hay_index: &mut usize) -> Option<i64> {
    let mut token_score = 0;
    let mut consecutive = 0;
    let mut first_match_index = None;
    let mut previous_index = None;

    for &needle_character in token {
        let found = hay_data.chars[*hay_index..]
            .iter()
            .enumerate()
            .find(|(_, character)| **character == needle_character);

        if let Some((relative_index, _)) = found {
            let absolute_index = *hay_index + relative_index;

            if first_match_index.is_none() {
                first_match_index = Some(absolute_index);
            }

            token_score += score_character_match(absolute_index, previous_index, &mut consecutive, hay_data);

            *hay_index = absolute_index + 1;
            previous_index = Some(absolute_index);
        } else {
            return None; // No match for this character
        }
    }

    // Add bonuses for this token
    token_score += calculate_token_bonuses(hay_data, token, first_match_index);

    Some(token_score)
}

/// Scores a single character match within a token.
///
/// Calculates the score contribution for matching a character, including
/// consecutive match bonuses, gap penalties, and word boundary bonuses.
///
/// # Arguments
/// * `absolute_index` - The absolute index of the current match
/// * `previous_index` - The index of the previous match (if any)
/// * `consecutive` - The consecutive match counter (updated)
/// * `hay_data` - The prepared haystack data
///
/// # Returns
/// The score contribution for this character match
fn score_character_match(
    absolute_index: usize,
    previous_index: Option<usize>,
    consecutive: &mut i64,
    hay_data: &HaystackData,
) -> i64 {
    let mut score = 0;

    // Handle consecutive matches
    if let Some(previous) = previous_index {
        if absolute_index == previous + 1 {
            *consecutive += 1;
        } else {
            *consecutive = 1; // Reset on non-consecutive match
        }

        // Penalize gaps
        let gap = (absolute_index - previous - 1) as i64;
        score -= gap / 2;
    }

    score += 6 * *consecutive;

    // Bonus for word boundary (start of hay or after space/punctuation)
    if is_word_boundary(absolute_index, hay_data) {
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
/// * `index` - The index to check
/// * `hay_data` - The prepared haystack data
///
/// # Returns
/// True if the position is a word boundary
fn is_word_boundary(index: usize, hay_data: &HaystackData) -> bool {
    index == 0
        || hay_data
            .chars
            .get(index - 1)
            .is_some_and(|character| character.is_whitespace() || character.is_ascii_punctuation())
}

/// Calculates bonus scores for a token match.
///
/// Applies bonuses for prefix matches and early positioning within
/// the haystack.
///
/// # Arguments
/// * `hay_data` - The prepared haystack data
/// * `token` - The token that was matched
/// * `first_match_index` - The index of the first character match
///
/// # Returns
/// The total bonus score for this token
fn calculate_token_bonuses(hay_data: &HaystackData, token: &[char], first_match_index: Option<usize>) -> i64 {
    let mut bonus = 0;

    // Prefix match bonus for this token
    let token_string: String = token.iter().copied().collect();
    if hay_data.lower.starts_with(&token_string) {
        bonus += 30;
    }

    // Early match bonus
    if let Some(start) = first_match_index {
        bonus += i64::max(0, 20 - start as i64);
    }

    bonus
}
