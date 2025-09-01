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
/// Simple subsequence fuzzy matcher with a naive scoring heuristic.
///
/// Returns `Some(score)` if all characters in `needle` appear in order within
/// `hay`, otherwise returns `None`. Higher scores indicate better matches. The
/// scoring favors consecutive matches, prefix matches, and shorter candidates.
///
/// Arguments:
/// - `hay`: The candidate string to search within.
/// - `needle`: The query to match as a subsequence.
///
/// Returns: `Option<i64>` where `Some(score)` indicates a match.
///
/// Example:
///
/// ```rust
/// use heroku_util::fuzzy_score;
/// assert!(fuzzy_score("applications", "app").unwrap() > 0);
/// assert!(fuzzy_score("applications", "axp").is_none());
/// ```
pub fn fuzzy_score(hay: &str, needle: &str) -> Option<i64> {
    if needle.is_empty() {
        return Some(0);
    }
    let mut h_lower = String::with_capacity(hay.len());
    for ch in hay.chars() {
        h_lower.extend(ch.to_lowercase());
    }
    let mut n_lower = String::with_capacity(needle.len());
    for ch in needle.chars() {
        n_lower.extend(ch.to_lowercase());
    }

    let h = h_lower.as_str();
    let n = n_lower.as_str();

    let mut hi = 0usize;
    let mut score: i64 = 0;
    let mut consec = 0i64;
    let mut first_match_idx: Option<usize> = None;
    for ch in n.chars() {
        if let Some(pos) = h[hi..].find(ch) {
            let abs = hi + pos;
            if first_match_idx.is_none() {
                first_match_idx = Some(abs);
            }
            hi = abs + ch.len_utf8();
            consec += 1;
            score += 6 * consec; // stronger reward for consecutive matches
        } else {
            return None;
        }
    }
    // Boost for prefix match
    if h.starts_with(n) {
        score += 30;
    }
    // Earlier start is better
    if let Some(start) = first_match_idx {
        score += i64::max(0, 20 - start as i64);
    }
    // Prefer shorter candidates when all else equal
    score -= hay.len() as i64 / 8;
    Some(score)
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
