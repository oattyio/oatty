//! HTTP/SSE helpers for rmcp-backed MCP clients.

use crate::config::McpServer;
use anyhow::Result;
use heroku_types::EnvVar;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderName, HeaderValue};
use url::Url;

/// Build the SSE URL using `baseUrl` and optional `ssePath` (defaults to "sse").
pub(crate) fn build_sse_url(server: &McpServer) -> Result<Url> {
    let base = server
        .base_url
        .clone()
        .ok_or_else(|| anyhow::anyhow!("base_url required for http"))?;
    let segment = server.sse_path.as_deref().unwrap_or("sse");
    let path = segment.strip_prefix('/').unwrap_or(segment);
    base.join(path)
        .map_err(|error| anyhow::anyhow!("failed to join path '{}': {}", path, error))
}

/// Build a reqwest client injecting configured headers and OAuth bearer if available.
pub(crate) async fn build_http_client_with_auth(server: &McpServer) -> Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    if let Some(map) = &server.headers {
        for EnvVar { key, value, .. } in map {
            if let (Ok(name), Ok(value)) = (HeaderName::try_from(key.as_str()), HeaderValue::try_from(value.as_str())) {
                headers.insert(name, value);
            }
        }
    }
    // OAuth bearer from keyring (or fallback to config token) if configured
    if let Some(auth) = &server.auth
        && (auth.scheme.eq_ignore_ascii_case("oauth") || auth.scheme.eq_ignore_ascii_case("oauth2"))
        && let Some(token) = load_oauth_token_from_keyring(server).await?.or_else(|| auth.token.clone())
        && let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", token))
    {
        headers.insert(AUTHORIZATION, value);
    }
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    Ok(client)
}

/// Retrieve a bearer token from the OS keyring for the given server base URL.
async fn load_oauth_token_from_keyring(server: &McpServer) -> Result<Option<String>> {
    // Compose a stable key based on base_url host + path
    let service = "heroku-mcp-oauth";
    let account = if let Some(url) = &server.base_url {
        format!("{}://{}{}", url.scheme(), url.host_str().unwrap_or(""), url.path())
    } else {
        "stdio".to_string()
    };
    let entry = keyring::Entry::new(service, &account)?;
    match entry.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => {
            tracing::warn!("keyring error: {}", e);
            Ok(None)
        }
    }
}
