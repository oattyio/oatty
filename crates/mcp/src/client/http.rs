//! HTTP helpers for rmcp-backed MCP clients using Streamable HTTP transport.

use crate::config::McpServer;
use anyhow::Result;
use oatty_types::EnvVar;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderName, HeaderValue};

/// Resolve the fully-qualified endpoint used for Streamable HTTP transport.
pub(crate) fn resolve_streamable_endpoint(server: &McpServer) -> Result<String> {
    server
        .base_url
        .as_ref()
        .map(|url| url.as_str().to_string())
        .ok_or_else(|| anyhow::anyhow!("base_url required for HTTP transport"))
}

/// Build a reqwest client injecting configured headers and OAuth bearer if available.
pub(crate) async fn build_http_client_with_auth(server: &McpServer) -> Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    for EnvVar { key, value, .. } in &server.headers {
        if let (Ok(name), Ok(value)) = (HeaderName::try_from(key.as_str()), HeaderValue::try_from(value.as_str())) {
            headers.insert(name, value);
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
    let service = "oatty-mcp-oauth";
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

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn resolve_endpoint_returns_string() {
        let url = Url::parse("https://example.com/mcp").unwrap();
        let server = McpServer {
            base_url: Some(url),
            ..Default::default()
        };
        let endpoint = resolve_streamable_endpoint(&server).expect("endpoint resolves");
        assert_eq!(endpoint, "https://example.com/mcp");
    }

    #[test]
    fn resolve_endpoint_errors_without_url() {
        let server = McpServer::default();
        let err = resolve_streamable_endpoint(&server).expect_err("missing endpoint");
        assert!(err.to_string().contains("base_url"));
    }
}
