use chrono::{DateTime, Datelike, NaiveDate};
use once_cell::sync::Lazy;
use regex::Regex;

/// Redacts values that look like secrets in a string.
pub fn redact_sensitive(input: &str) -> String {
    static REDACT_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
        vec![
            Regex::new(r"(?i)(authorization: )([\w\-\.=:/+]+)").unwrap(),
            Regex::new(r"(?i)([A-Z0-9_]*?(KEY|TOKEN|SECRET|PASSWORD))=([^\s]+)").unwrap(),
            Regex::new(r"(?i)(DATABASE_URL)=([^\s]+)").unwrap(),
        ]
    });
    let mut redacted = input.to_string();
    for re in REDACT_PATTERNS.iter() {
        redacted = re
            .replace_all(&redacted, |caps: &regex::Captures| {
                let prefix = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                format!("{}<redacted>", prefix)
            })
            .to_string();
    }
    redacted
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
///
/// * `hay` - The candidate string to search within.
/// * `needle` - The query to match, with space-separated tokens.
///
/// # Returns
///
/// `Option<i64>` where `Some(score)` indicates a match.
///
/// # Example
///
/// ```rust
/// use heroku_util::fuzzy_score;
/// assert!(fuzzy_score("applications", "app").unwrap() > 0);
/// assert!(fuzzy_score("string1 string2", "string1 string2").unwrap() > 0);
/// assert!(fuzzy_score("string1:string2", "string1 string2").unwrap() > 0);
/// assert!(fuzzy_score("applications", "axp").is_none());
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

    // Convert hay to lowercase once, avoiding repeated allocations
    let h_lower: String = hay.chars().flat_map(|c| c.to_lowercase()).collect();
    let hay_chars: Vec<char> = h_lower.chars().collect();

    // Split needle into tokens and convert to lowercase
    let needle_tokens: Vec<Vec<char>> = needle
        .split_whitespace()
        .map(|token| token.chars().flat_map(|c| c.to_lowercase()).collect())
        .filter(|token: &Vec<char>| !token.is_empty())
        .collect();

    if needle_tokens.is_empty() {
        return Some(0);
    }

    let mut total_score = 0;
    let mut hay_idx = 0;

    // Match each token independently
    for token in needle_tokens {
        let mut token_score = 0;
        let mut consec = 0;
        let mut first_match_idx = None;
        let mut prev_idx = None;

        for &n_char in &token {
            // Find next matching character in hay starting from hay_idx
            let found = hay_chars[hay_idx..]
                .iter()
                .enumerate()
                .find(|(_, c)| **c == n_char);

            if let Some((rel_idx, _)) = found {
                let abs_idx = hay_idx + rel_idx;
                if first_match_idx.is_none() {
                    first_match_idx = Some(abs_idx);
                }

                // Reward consecutive matches
                if let Some(prev) = prev_idx {
                    if abs_idx == prev + 1 {
                        consec += 1;
                    } else {
                        consec = 1; // Reset on non-consecutive match
                    }
                    // Penalize gaps
                    let gap = (abs_idx - prev - 1) as i64;
                    token_score -= gap / 2;
                }
                token_score += 6 * consec;

                // Bonus for word boundary (start of hay or after space/punctuation)
                if abs_idx == 0 || hay_chars.get(abs_idx - 1).map_or(false, |c| c.is_whitespace() || c.is_ascii_punctuation()) {
                    token_score += 10;
                }

                hay_idx = abs_idx + 1;
                prev_idx = Some(abs_idx);
            } else {
                return None; // No match for this character
            }
        }

        // Prefix match bonus for this token
        let token_str: String = token.iter().copied().collect();
        if h_lower.starts_with(&token_str) {
            token_score += 30;
        }

        // Early match bonus
        if let Some(start) = first_match_idx {
            token_score += i64::max(0, 20 - start as i64);
        }

        total_score += token_score;
    }

    // Penalty for hay length (shorter candidates preferred)
    total_score -= hay_chars.len() as i64 / 8;

    Some(total_score)
}

/// Tokenize input using a simple, shell-like lexer.
///
/// Supports single and double quotes and backslash escapes. Used by the
/// suggestion engine to derive tokens and assess completeness of flag values.
///
/// Arguments:
/// - `input`: The raw input line.
///
/// Returns: A vector of tokens preserving quoted segments.
///
/// Example:
///
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
pub struct LexTok<'a> {
    pub text: &'a str,
    pub start: usize,
    pub end: usize,
}

/// Tokenize input returning borrowed slices and byte ranges.
pub fn lex_shell_like_ranged(input: &str) -> Vec<LexTok<'_>> {
    let mut out: Vec<LexTok<'_>> = Vec::new();
    let mut i = 0usize;
    let bytes = input.as_bytes();
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let start = i;
        let mut in_sq = false;
        let mut in_dq = false;
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if b == b'\'' && !in_dq {
                in_sq = !in_sq;
                i += 1;
                continue;
            }
            if b == b'"' && !in_sq {
                in_dq = !in_dq;
                i += 1;
                continue;
            }
            if !in_sq && !in_dq && b.is_ascii_whitespace() {
                break;
            }
            i += 1;
        }
        out.push(LexTok {
            text: &input[start..i],
            start,
            end: i,
        });
    }
    out
}

// Generated at build-time from schemas/heroku-schema.json
pub mod generated_date_fields {
    include!(concat!(env!("OUT_DIR"), "/date_fields.rs"));
}

/// Returns true if a JSON key looks like a date field.
/// Uses generated schema-derived keys with fallback heuristics.
pub fn is_date_like_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase().replace([' ', '-'], "_");
    if generated_date_fields::DATE_FIELD_KEYS.contains(&k.as_str()) {
        return true;
    }
    k.ends_with("_at")
        || k.ends_with("_on")
        || k.ends_with("_date")
        || k == "created"
        || k == "updated"
        || k == "released"
}

/// Formats common date strings into MM/DD/YYYY if parsable.
pub fn format_date_mmddyyyy(s: &str) -> Option<String> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        let d = dt.date_naive();
        return Some(format!("{:02}/{:02}/{}", d.month(), d.day(), d.year()));
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(format!("{:02}/{:02}/{}", d.month(), d.day(), d.year()));
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y/%m/%d") {
        return Some(format!("{:02}/{:02}/{}", d.month(), d.day(), d.year()));
    }
    None
}
