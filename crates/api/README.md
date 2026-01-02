oatty-api — Minimal HTTP Client

Overview
- Thin wrapper around `reqwest::Client` with Oatty defaults and auth precedence.
- Used by the CLI and TUI for live API execution.

Defaults
- Base URL: selected from the catalog's OpenAPI `servers` metadata (fallback to `HttpCommandSpec::base_url`).
- Headers:
  - `Accept: application/json`
  - `User-Agent: oatty-tui/0.1; <os>`
- Timeout: 30 seconds.

Auth
- `OATTY_API_TOKEN` (Bearer)

API
- `OattyClient::new_with_base_url(base_url: impl Into<String>) -> Result<OattyClient>`
  - Applies default headers and optional Authorization.
- `OattyClient::request(Method, path: &str) -> RequestBuilder`
  - `path` is appended to the base URL and returned `RequestBuilder` can be customized (e.g., `.json(&value)`).

Examples
```rust
let client = oatty_api::OattyClient::new_with_base_url("https://api.example.com")?;
let resp = client
    .request(reqwest::Method::GET, "/apps")
    .send()
    .await?;
println!("{}\n{}", resp.status(), resp.text().await?);
```

Testing & Troubleshooting
- 401 Unauthorized → set `OATTY_API_TOKEN`.
- Network errors → check connectivity and proxy settings. With CLI/TUI, set `RUST_LOG=info` for more details.
