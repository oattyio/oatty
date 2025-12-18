heroku-util — Utilities & Redaction

Overview
- Small helpers shared across crates; notably, redaction of sensitive values for safe logging.

Redaction
- `redact_sensitive(&str) -> String` (referenced in CLI paths) masks sensitive headers and token-like substrings.
- Intended to prevent leaking credentials in logs, previews, or errors.

Examples
```rust
let header_line = "Authorization: Bearer xxxxxx";
let safe = oatty_util::redact_sensitive(header_line);
assert!(safe.contains("Authorization: "));
```

Testing
- Recommended golden tests exercising common sensitive fields:
  - `Authorization` headers, `api_key`, `password`, `token`, `secret` keys.
  - Ensure redaction is consistent across different contexts.

