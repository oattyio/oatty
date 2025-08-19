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
