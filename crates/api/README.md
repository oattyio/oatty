oatty-api — Minimal HTTP Client

Overview
- Thin wrapper around `reqwest::Client` with Oatty defaults; authentication headers are supplied by the caller.
- Used by the CLI and TUI for live API execution.

Defaults
- Base URL: selected from the catalog's OpenAPI `servers` metadata (fallback to `HttpCommandSpec::base_url`).
- Headers:
  - `Accept: application/json`
  - `User-Agent: oatty-tui/0.1; <os>`
- Timeout: 30 seconds.

Auth
- Headers are configured on the catalog (for example, an `Authorization` header
  with a value such as `${env:OATTY_API_TOKEN}` or `${secret:NAME}`), then
  interpolated by the registry at execution time before being passed to this
  client. No environment variable is read automatically by this crate.

API
- `OattyClient::new(base_url: impl Into<String>, headers: &IndexSet<EnvVar>) -> Result<OattyClient>`
  - Applies default headers plus any caller-supplied headers (for example, an
    `Authorization` header interpolated from `${env:...}` or `${secret:...}`).
- `OattyClient::request(Method, path: &str) -> RequestBuilder`
  - `path` is appended to the base URL and returned `RequestBuilder` can be customized (e.g., `.json(&value)`).

Examples
```rust
use indexmap::IndexSet;
use oatty_types::{EnvSource, EnvVar};

let mut headers = IndexSet::new();
// Values are already resolved by the caller (the registry interpolates
// `${env:...}` / `${secret:...}` before constructing this client).
headers.insert(EnvVar::new(
    "Authorization".into(),
    "Bearer token".into(),
    EnvSource::Raw,
));
let client = oatty_api::OattyClient::new("https://api.example.com", &headers)?;
let resp = client
    .request(reqwest::Method::GET, "/apps")
    .send()
    .await?;
println!("{}\n{}", resp.status(), resp.text().await?);
```

Testing & Troubleshooting
- 401 Unauthorized → verify the catalog header interpolation (for example,
  `${env:OATTY_API_TOKEN}` or `${secret:NAME}`) resolves to a valid
  credential.
- Network errors → check connectivity and proxy settings. With CLI/TUI, set `RUST_LOG=info` for more details.
