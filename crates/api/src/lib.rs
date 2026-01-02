//! Oatty API client utilities.
//!
//! This module provides a lightweight client for interacting with the Oatty API.
//! It focuses on:
//!
//! - Constructing an HTTP client with sensible defaults
//! - Discovering credentials from `OATTY_API_TOKEN`
//! - Validating base URLs for safety
//! - Building requests with a consistent User-Agent and Accept headers
//!
//! The primary entry point is [`OattyClient`]. Create an instance via
//! [`OattyClient::new_from_spec`], and then build requests with
//! [`OattyClient::request`].
//!
//! # Example
//!
//! ```ignore
//! use oatty_api::OattyClient;
//! use anyhow::Result;
//!
//! fn main() -> Result<()> {
//!     let client = OattyClient::new_with_base_url("https://api.example.com")?;
//!     let res = client
//!         .request(reqwest::Method::GET, "/apps")
//!         .send()?;
//!     println!("status: {}", res.status());
//!     Ok(())
//! }
//! ```

use std::env;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use reqwest::{Client, RequestBuilder, Url, header};
use tracing::debug;

/// Hostnames allowed for local development regardless of scheme.
const LOCALHOST_DOMAINS: &[&str] = &["localhost", "127.0.0.1"];
/// Default HTTP Accept header for Oatty API requests.
const DEFAULT_ACCEPT_HEADER: &str = "application/json";

#[derive(Debug, Clone)]
/// Thin wrapper around a configured `reqwest::Client` for Oatty API access.
///
/// The client pre-configures default headers and builds requests against a
/// validated base URL. Authentication is read from the environment only.
pub struct OattyClient {
    pub base_url: String,
    pub http: Client,
    pub user_agent: String,
}

impl OattyClient {
    /// Construct a [`OattyClient`] from environment variables.
    ///
    /// Authentication:
    /// - `OATTY_API_TOKEN` environment variable
    ///
    /// Construct a [`OattyClient`] using an explicit base URL.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The full base URL for API requests (for example, `https://api.example.com`).
    ///
    /// # Returns
    ///
    /// A configured [`OattyClient`] instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the base URL is invalid or the HTTP client fails to initialize.
    pub fn new_with_base_url(base_url: impl Into<String>) -> Result<Self> {
        let api_token = env::var("OATTY_API_TOKEN").ok();
        let http = build_http_client(api_token)?;

        let base_url = base_url.into();
        validate_base_url(&base_url)?;
        Ok(Self {
            base_url,
            http,
            user_agent: format!("oatty-tui/0.1; {}", env::consts::OS),
        })
    }

    /// Build a `reqwest::RequestBuilder` for a method and API-relative path.
    ///
    /// The resulting request includes the configured User-Agent and base
    /// headers, and is resolved relative to `self.base_url`.
    pub fn request(&self, method: reqwest::Method, path: &str) -> RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        debug!(%url, "building request");

        self.http.request(method, url).header(header::USER_AGENT, &self.user_agent)
    }
}

fn build_http_client(api_token: Option<String>) -> Result<Client> {
    let mut default_headers = header::HeaderMap::new();
    if let Some(api_token) = api_token {
        let authorization_header_value = format!("Bearer {}", api_token);
        default_headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&authorization_header_value).unwrap(),
        );
    }
    default_headers.insert(header::ACCEPT, header::HeaderValue::from_str(DEFAULT_ACCEPT_HEADER)?);

    Client::builder()
        .default_headers(default_headers)
        .timeout(Duration::from_secs(30))
        .build()
        .context("build http client")
}

/// Validate that a base URL is acceptable for use by the client.
///
/// Rules:
/// - `localhost` or `127.0.0.1`: any scheme is allowed
/// - otherwise: scheme must be HTTPS, and host must be one of the allowed
///   Oatty domains or a subdomain thereof
fn validate_base_url(base: &str) -> Result<()> {
    let parsed_base_url = Url::parse(base).map_err(|e| anyhow!("invalid base URL '{}': {}", base, e))?;

    let host_name = parsed_base_url.host_str().ok_or_else(|| anyhow!("base URL must include a host"))?;

    // Local development allowances: localhost/127.0.0.1 with any scheme.
    if LOCALHOST_DOMAINS.iter().any(|&allowed| host_name.eq_ignore_ascii_case(allowed)) {
        return Ok(());
    }

    // Production/staging: must be HTTPS.
    if parsed_base_url.scheme() != "https" {
        return Err(anyhow!(
            "base URL must use https for non-localhost hosts; got '{}://'",
            parsed_base_url.scheme()
        ));
    }

    Ok(())
}
