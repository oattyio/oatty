heroku-api — Minimal HTTP Client

Overview
- Thin wrapper around `reqwest::Client` with Oatty defaults and auth precedence.
- Used by the CLI and TUI for live API execution.

Defaults
- Base URL: `https://api.heroku.com` (override with `HEROKU_API_BASE`). Only HTTPS hosts under `heroku.com` are allowed; e.g., `https://api.heroku.com`, `https://eu.api.heroku.com`. Non-Oatty domains are rejected to prevent token exfiltration.
- Headers:
  - `Accept: application/vnd.heroku+json; version=3`
  - `User-Agent: heroku-cli/0.1 (+https://example.com); <os>`
- Timeout: 30 seconds.

Auth Precedence
1) `HEROKU_API_KEY`
2) `~/.netrc` (naive parser placeholder)

API
- `OattyClient::new_from_env() -> Result<OattyClient>`
  - Applies base URL overrides, headers, and optional Authorization.
- `OattyClient::request(Method, path: &str) -> RequestBuilder`
  - `path` is appended to the base URL and returned `RequestBuilder` can be customized (e.g., `.json(&value)`).

Examples
```rust
let client = oatty_api::OattyClient::new_from_env()?;
let resp = client
    .request(reqwest::Method::GET, "/apps")
    .send()
    .await?;
println!("{}\n{}", resp.status(), resp.text().await?);
```

Testing & Troubleshooting
- 401 Unauthorized → set `HEROKU_API_KEY` or configure `~/.netrc` for `api.heroku.com`.
- Network errors → check connectivity and proxy settings. With CLI/TUI, set `RUST_LOG=info` for more details.

