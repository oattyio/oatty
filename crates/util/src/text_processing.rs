//! # Text Processing Utilities
//!
//! This module provides utilities for text processing, including sensitive data redaction
//! and fuzzy string matching with sophisticated scoring algorithms.

use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;

const MIN_SECRET_LENGTH: usize = 16;
const HIGH_ENTROPY_THRESHOLD: f64 = 3.5;
const MIN_UNIQUE_CHARACTERS: usize = 6;
const LONG_HEX_LENGTH: usize = 64;
const MIN_BASE64_LENGTH: usize = 24;

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
                let value = captures.get(2);
                let suffix = captures.get(3).map(|m| m.as_str()).unwrap_or("");
                if value.is_some() {
                    if prefix.is_empty() && suffix.is_empty() {
                        replacement.to_string()
                    } else {
                        format!("{}{}{}", prefix, replacement, suffix)
                    }
                } else {
                    replacement.to_string()
                }
            })
            .to_string();
    }

    redacted
}

/// Determines whether a value should be treated as a secret.
///
/// The detection combines pattern-based recognition for well-known credential
/// formats, reuse of the redaction patterns, and heuristic checks for
/// high-entropy tokens. The function assumes the input has already been
/// extracted as a value (for example, the right-hand side of an environment
/// variable) and therefore trims surrounding quotes or delimiters before
/// evaluation.
///
/// # Arguments
/// * `value` - The raw value to evaluate
///
/// # Returns
/// `true` when the value resembles a credential and should be stored in secure
/// storage, otherwise `false`.
pub fn is_secret(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }

    if contains_secret_block(trimmed) {
        return true;
    }

    let normalized = normalize_secret_candidate(trimmed);
    if normalized.is_empty() {
        return false;
    }

    if get_value_only_patterns().iter().any(|pattern| pattern.is_match(normalized)) {
        return true;
    }

    if get_redact_patterns().iter().any(|pattern| pattern.is_match(normalized)) {
        return true;
    }

    looks_like_high_entropy_secret(normalized)
}

/// Returns compiled regex patterns for detecting sensitive information.
///
/// The patterns intentionally emphasize environment variables and structured
/// configuration keys that should be persisted in a secure keychain instead of
/// plain text storage. They retain earlier coverage for authorization headers
/// and common inline secrets so that existing redaction behaviour remains
/// intact while broadening coverage for credential-like values.
///
/// # Returns
/// A vector of compiled regex patterns ordered from most specific to most
/// general. Patterns later in the list can assume that more precise matches
/// already ran.
pub fn get_redact_patterns() -> &'static Vec<Regex> {
    static REDACT_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(build_redact_patterns);

    &REDACT_PATTERNS
}

/// Builds the ordered list of redaction patterns used across the crate.
fn build_redact_patterns() -> Vec<Regex> {
    let mut patterns = Vec::new();

    patterns.extend(get_value_only_patterns().iter().cloned());
    patterns.extend(build_authorization_patterns());
    patterns.extend(build_sensitive_env_patterns());
    patterns.extend(build_structured_secret_patterns());

    patterns
}

/// Captures authorization headers and inline bearer/basic credentials.
fn build_authorization_patterns() -> Vec<Regex> {
    vec![
        Regex::new(r"(?i)(authorization:\s+)([^\s]+(?:\s+[^\s]+)*)").unwrap(),
        Regex::new(r"(?i)((?:^|\b)Bearer\s+)([A-Za-z0-9\-._~+/]+=*)").unwrap(),
        Regex::new(r"(?i)((?:^|\b)Basic\s+)([A-Za-z0-9+/]+=*)").unwrap(),
    ]
}

/// Detects environment variable assignments and configuration entries whose
/// values are considered secret-worthy and should be stored in secure storage.
fn build_sensitive_env_patterns() -> Vec<Regex> {
    const KEYCHAIN_ENV_KEYWORDS: &[&str] = &[
        "ACCESS_KEY",
        "ACCESS_KEY_ID",
        "API_KEY",
        "API_TOKEN",
        "APP_KEY",
        "AUTH_TOKEN",
        "BEARER_TOKEN",
        "CERTIFICATE",
        "CLIENT_ID",
        "CLIENT_SECRET",
        "CONNECTION_STRING",
        "DATABASE_PASSWORD",
        "DATABASE_URI",
        "DATABASE_URL",
        "DB_PASSWORD",
        "DB_URI",
        "DB_URL",
        "ENCRYPTION_KEY",
        "HEROKU_API_KEY",
        "HEROKU_API_TOKEN",
        "HEROKU_OAUTH_ACCESS_TOKEN",
        "HEROKU_OAUTH_ID",
        "HEROKU_OAUTH_REFRESH_TOKEN",
        "HEROKU_OAUTH_SECRET",
        "HEROKU_OAUTH_TOKEN",
        "HEROKU_POSTGRESQL",
        "HEROKU_REDIS",
        "JWT",
        "KAFKA_API_SECRET",
        "KAFKA_PASSWORD",
        "LICENSE_KEY",
        "MASTER_KEY",
        "MONGO_URI",
        "MONGODB_URI",
        "PASSPHRASE",
        "PASSCODE",
        "PASSWORD",
        "PRIVATE_KEY",
        "PRIVATE_TOKEN",
        "REDIS_URI",
        "REDIS_URL",
        "REFRESH_TOKEN",
        "SAS_KEY",
        "SAS_TOKEN",
        "SECRET",
        "SECRET_ACCESS_KEY",
        "SECRET_KEY",
        "SECRET_TOKEN",
        "SERVICE_ACCOUNT_KEY",
        "SESSION_TOKEN",
        "SIGNING_KEY",
        "SLACK_TOKEN",
        "SSH_KEY",
        "SSH_PRIVATE_KEY",
        "TOKEN",
        "WEBHOOK_SECRET",
    ];

    let keyword_fragment = build_keyword_fragment(KEYCHAIN_ENV_KEYWORDS);

    let shell_assignment = format!(
        r"(?xim)((?:export\s+)?[A-Za-z0-9_]*?(?:{keywords})[A-Za-z0-9_]*\s*=\s*)([^\s]+)",
        keywords = keyword_fragment.clone()
    );

    let json_assignment = format!(
        "(?xim)((?:(?:\"|')?[A-Za-z0-9_.-]*?(?:{keywords})[A-Za-z0-9_.-]*[\"']?\\s*:\\s*)(?:\"|'))([^\"']+)((?:\"|'))",
        keywords = keyword_fragment.clone()
    );

    let yaml_assignment = format!(
        r#"(?xim)((?:^|[\s,{{\[(])['"]?[A-Za-z0-9_.-]*?(?:{keywords})[A-Za-z0-9_.-]*['"]?\s*:\s+)([^"'\s,}}\]]+)"#,
        keywords = keyword_fragment.clone()
    );

    vec![
        Regex::new(&shell_assignment).unwrap(),
        Regex::new(&json_assignment).unwrap(),
        Regex::new(&yaml_assignment).unwrap(),
    ]
}

fn build_structured_secret_patterns() -> Vec<Regex> {
    vec![
        Regex::new(r"(?i)((?:api[\s_-]?key|auth[\s_-]?token|token|secret|password)\s*[:=]\s*)([^\s,;]+)").unwrap(),
        Regex::new(r"(?i)(DATABASE_URL=)([^\s]+)").unwrap(),
        Regex::new(r"(eyJ[A-Za-z0-9\-._~+/]+=*)").unwrap(),
        Regex::new(r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b").unwrap(),
        Regex::new(r"\b\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}\b").unwrap(),
    ]
}

fn get_value_only_patterns() -> &'static Vec<Regex> {
    static VALUE_ONLY_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(build_value_only_patterns);

    &VALUE_ONLY_PATTERNS
}

fn build_value_only_patterns() -> Vec<Regex> {
    vec![
        Regex::new(r"(?i)(HRKU-[A-Za-z0-9_-]{60})").unwrap(),
        Regex::new(r"(?i)(sk_(?:live|test)_[A-Za-z0-9]{16,})").unwrap(),
        Regex::new(r"(?i)(rk_(?:live|test)_[A-Za-z0-9]{16,})").unwrap(),
        Regex::new(r"(?i)(pk_(?:live|test)_[A-Za-z0-9]{16,})").unwrap(),
        Regex::new(r"(?i)((?:gh[oprsu]|github_pat)_[A-Za-z0-9_]{22,40})").unwrap(),
        Regex::new(r"(?i)(glpat-[A-Za-z0-9_=-]{20,26})").unwrap(),
        Regex::new(r"(?i)([rs]k_live_[A-Za-z0-9]{24,247})").unwrap(),
        Regex::new(r"(?i)(sq0i[a-z]{2}-[A-Za-z0-9_-]{22,43})").unwrap(),
        Regex::new(r"(?i)(sq0c[a-z]{2}-[A-Za-z0-9_-]{40,50})").unwrap(),
        Regex::new(r"(?i)(EAAA[A-Za-z0-9+=-]{60})").unwrap(),
        Regex::new(r"(?i)(AccountKey=[A-Za-z0-9+/=]{88})").unwrap(),
        Regex::new(r"(?i)(npm_[A-Za-z0-9]{36})").unwrap(),
        Regex::new(r"(?i)(//[^\s]+/:_authToken=[A-Za-z0-9_-]+)").unwrap(),
        Regex::new(r"(?i)(xox[aboprs]-(?:\d+-)+[\da-z]+)").unwrap(),
        Regex::new(r"(?i)(https://hooks\.slack\.com/services/T[A-Za-z0-9_]+/B[A-Za-z0-9_]+/[A-Za-z0-9_]+)").unwrap(),
        Regex::new(r"(?i)(SG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43})").unwrap(),
        Regex::new(r"(?i)((?:AC|SK)[0-9a-z]{32})").unwrap(),
        Regex::new(r"(?i)([0-9a-f]{32}-us[0-9]{1,2})").unwrap(),
        Regex::new(r"(?i)(s-s4t2(?:af|ud)-[0-9a-f]{64})").unwrap(),
        Regex::new(r"(?i)(ya29\.[0-9A-Za-z\-_]{20,})").unwrap(),
        Regex::new(r"(?i)(AIzaSy[A-Za-z0-9_-]{33})").unwrap(),
        Regex::new(r"(?i)(shp(?:at|ca|ea|mp|pc|se)_[0-9a-f]{32})").unwrap(),
        Regex::new(r"(?i)(heroku_[0-9a-f]{32,})").unwrap(),
        Regex::new(r"(?i)(postgres(?:ql)?://[^\s]+)").unwrap(),
        Regex::new(r"(?i)(mysql://[^\s]+)").unwrap(),
        Regex::new(r"(?i)(rediss?://[^\s]+)").unwrap(),
        Regex::new(r"(?i)(amqps?://[^\s]+)").unwrap(),
        Regex::new(r"(PuTTY-User-Key-File-2)").unwrap(),
        Regex::new(r"(AGE-SECRET-KEY-1[0-9A-Z]{58})").unwrap(),
        Regex::new(r"(?s)(-{5}BEGIN (?:DSA|EC|OPENSSH|PGP PRIVATE|PRIVATE|RSA|SSH2 ENCRYPTED) KEY-{5}(?:$|[^-]{63,}-{5}END))").unwrap(),
        Regex::new(r"(?s)(-----BEGIN [^-]+-----[\s\S]+?-----END [^-]+-----)").unwrap(),
    ]
}

fn build_keyword_fragment(keywords: &[&str]) -> String {
    keywords
        .iter()
        .map(|keyword| keyword.split('_').map(regex::escape).collect::<Vec<_>>().join("[_\\-]?"))
        .collect::<Vec<_>>()
        .join("|")
}

fn contains_secret_block(value: &str) -> bool {
    let uppercased = value.to_ascii_uppercase();
    uppercased.contains("-----BEGIN ") || uppercased.contains("PRIVATE KEY") || uppercased.contains("BEGIN CERTIFICATE")
}

fn normalize_secret_candidate(value: &str) -> &str {
    let mut normalized = value.trim().trim_end_matches([';', ',']);

    if let Some(stripped) = strip_matching_wrapper(normalized, "\"") {
        normalized = stripped.trim();
    }
    if let Some(stripped) = strip_matching_wrapper(normalized, "'") {
        normalized = stripped.trim();
    }
    if let Some(stripped) = strip_matching_wrapper(normalized, "`") {
        normalized = stripped.trim();
    }

    normalized
}

fn strip_matching_wrapper<'a>(value: &'a str, wrapper: &str) -> Option<&'a str> {
    value.strip_prefix(wrapper).and_then(|inner| inner.strip_suffix(wrapper))
}

fn looks_like_high_entropy_secret(value: &str) -> bool {
    if value.len() < MIN_SECRET_LENGTH {
        return false;
    }

    if value.chars().any(char::is_whitespace) {
        return false;
    }

    let analysis = analyze_characters(value);

    if is_long_hex_secret(value, analysis.length) {
        return true;
    }

    let entropy = calculate_shannon_entropy(&analysis.frequencies, analysis.length);

    if is_base64_like(value, &analysis) && entropy >= HIGH_ENTROPY_THRESHOLD {
        return true;
    }

    if analysis.has_lowercase
        && analysis.has_uppercase
        && analysis.has_digit
        && entropy >= HIGH_ENTROPY_THRESHOLD
        && (analysis.has_symbol || analysis.unique_characters >= MIN_UNIQUE_CHARACTERS)
    {
        return true;
    }

    false
}

struct CharacterAnalysis {
    frequencies: HashMap<char, usize>,
    length: usize,
    unique_characters: usize,
    has_lowercase: bool,
    has_uppercase: bool,
    has_digit: bool,
    has_symbol: bool,
}

fn analyze_characters(value: &str) -> CharacterAnalysis {
    let mut frequencies: HashMap<char, usize> = HashMap::new();
    let mut length = 0usize;
    let mut has_lowercase = false;
    let mut has_uppercase = false;
    let mut has_digit = false;
    let mut has_symbol = false;

    for character in value.chars() {
        length += 1;
        *frequencies.entry(character).or_insert(0) += 1;

        if character.is_ascii_lowercase() {
            has_lowercase = true;
        } else if character.is_ascii_uppercase() {
            has_uppercase = true;
        } else if character.is_ascii_digit() {
            has_digit = true;
        } else {
            has_symbol = true;
        }
    }

    let unique_characters = frequencies.len();

    CharacterAnalysis {
        frequencies,
        length,
        unique_characters,
        has_lowercase,
        has_uppercase,
        has_digit,
        has_symbol,
    }
}

fn calculate_shannon_entropy(frequencies: &HashMap<char, usize>, length: usize) -> f64 {
    if length == 0 {
        return 0.0;
    }

    let length_f64 = length as f64;
    frequencies.values().fold(0.0, |entropy, count| {
        let probability = *count as f64 / length_f64;
        entropy - probability * probability.log2()
    })
}

fn is_long_hex_secret(value: &str, length: usize) -> bool {
    length >= LONG_HEX_LENGTH && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn is_base64_like(value: &str, analysis: &CharacterAnalysis) -> bool {
    if value.len() < MIN_BASE64_LENGTH || !value.len().is_multiple_of(4) {
        return false;
    }

    if !value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '+' | '/' | '=' | '-' | '_'))
    {
        return false;
    }

    let has_base64_symbol = value.chars().any(|character| matches!(character, '+' | '/' | '=' | '-' | '_'));

    (analysis.has_lowercase && analysis.has_uppercase && analysis.has_digit && has_base64_symbol)
        || (analysis.unique_characters >= MIN_UNIQUE_CHARACTERS && has_base64_symbol)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_shell_style_sensitive_env_vars() {
        let input = "export AWS_SECRET_ACCESS_KEY=supersecret";
        let expected = "export AWS_SECRET_ACCESS_KEY=[REDACTED]";

        assert_eq!(redact_sensitive(input), expected);
    }

    #[test]
    fn redacts_json_style_sensitive_entries() {
        let input = r#"{"clientSecret": "top-secret"}"#;
        let expected = r#"{"clientSecret": "[REDACTED]"}"#;

        assert_eq!(redact_sensitive(input), expected);
    }

    #[test]
    fn ignores_non_sensitive_environment_variables() {
        let input = "PORT=8080";
        assert_eq!(redact_sensitive(input), input);
    }

    #[test]
    fn redacts_bare_value_tokens() {
        let input = "sk_live_1234567890abcdef123456";
        assert_eq!(redact_sensitive(input), "[REDACTED]");
    }

    #[test]
    fn redacts_bare_prefixed_heroku_token() {
        let input = "HRKU-AALJCYR7SRzPkj9_BGqhi1jAI1J5P4WfD6ITENvdVydAPCnNcAlrMMahHrTo";
        assert_eq!(redact_sensitive(input), "[REDACTED]");
    }

    #[test]
    fn redacts_slack_webhook_url() {
        let input = "https://hooks.slack.com/services/T123ABC45/B678DEF90/abcdefGhijk";
        assert_eq!(redact_sensitive(input), "[REDACTED]");
    }

    #[test]
    fn redacts_npm_legacy_auth_token() {
        let input = "//registry.npmjs.org/:_authToken=abcdef1234567890";
        assert_eq!(redact_sensitive(input), "[REDACTED]");
    }

    #[test]
    fn redacts_heroku_api_key_assignment() {
        let input = "HEROKU_API_KEY=01234567-89ab-cdef-0123-456789abcdef";
        let expected = "HEROKU_API_KEY=[REDACTED]";

        assert_eq!(redact_sensitive(input), expected);
    }

    #[test]
    fn redacts_heroku_postgres_url_value() {
        let input = "postgres://user:superSecretPass@ec2-34-201-12-34.compute-1.amazonaws.com:5432/dbname";

        assert_eq!(redact_sensitive(input), "[REDACTED]");
    }

    #[test]
    fn identifies_known_secret_formats() {
        let sendgrid_token = format!("SG.{}.{}", "a".repeat(22), "b".repeat(43));
        let azure_account = format!("AccountKey={}", "A".repeat(88));
        let npm_legacy = "//registry.npmjs.org/:_authToken=abcdef1234567890";

        assert!(is_secret("sk_live_1234567890abcdef123456"));
        assert!(is_secret("ghp_abcdEFGHijklMNOPqrstUVWXyz0123456789"));
        assert!(is_secret("github_pat_11ABCDxyz0123456789ABCDEFGH"));
        assert!(is_secret("ya29.A0AVA9y1ExampleTokenValue1234567890ABCDEFGHI"));
        assert!(is_secret("HRKU-AALJCYR7SRzPkj9_BGqhi1jAI1J5P4WfD6ITENvdVydAPCnNcAlrMMahHrTo"));
        assert!(is_secret("heroku_abcdefabcdefabcdefabcdefabcdefab"));
        assert!(is_secret(
            "postgres://user:superSecretPass@ec2-34-201-12-34.compute-1.amazonaws.com:5432/dbname"
        ));
        assert!(is_secret(&sendgrid_token));
        assert!(is_secret(&azure_account));
        assert!(is_secret(npm_legacy));
        assert!(is_secret(
            "s-s4t2af-0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        ));
    }

    #[test]
    fn identifies_pem_blocks_as_secret() {
        let pem = "-----BEGIN PRIVATE KEY-----\nABCDEF\n-----END PRIVATE KEY-----";
        assert!(is_secret(pem));
    }

    #[test]
    fn does_not_flag_non_secret_values() {
        assert!(!is_secret("http://localhost:3000"));
        assert!(!is_secret("plain-text-value"));
        assert!(!is_secret("12345678901234567890"));
    }
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
fn score_character_match(absolute_index: usize, previous_index: Option<usize>, consecutive: &mut i64, hay_data: &HaystackData) -> i64 {
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

/// Recursively redacts sensitive data from JSON values.
///
/// This method traverses the JSON structure and applies redaction to all
/// string values while preserving the overall structure. Used for security
/// when displaying JSON data in the UI.
///
/// # Arguments
///
/// * `v` - The JSON value to redact
///
/// # Returns
///
/// A new JSON value with all string content redacted
pub fn redact_json(v: &Value) -> Value {
    match v {
        Value::String(s) => Value::String(redact_sensitive(s)),
        Value::Array(arr) => Value::Array(arr.iter().map(redact_json).collect()),
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, val) in map.iter() {
                out.insert(k.clone(), redact_json(val));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}
