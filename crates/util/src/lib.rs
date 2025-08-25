use regex::Regex;

/// Redacts values that look like secrets in a string.
pub fn redact_sensitive(input: &str) -> String {
    let patterns = [
        r"(?i)(authorization: )([\w\-\.=:/+]+)",
        r"(?i)([A-Z0-9_]*?(KEY|TOKEN|SECRET|PASSWORD))=([^\s]+)",
        r"(?i)(DATABASE_URL)=([^\s]+)",
    ];
    let mut redacted = input.to_string();
    for pat in patterns {
        let re = Regex::new(pat).unwrap();
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
