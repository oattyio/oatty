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
//! [`OattyClient::new`], and then build requests with
//! [`OattyClient::request`].
//!
//! # Example
//!
//! ```ignore
//! use indexmap::IndexSet;
//! use oatty_api::OattyClient;
//! use oatty_types::EnvVar;
//! use anyhow::Result;
//!
//! fn main() -> Result<()> {
//!     let headers = IndexSet::<EnvVar>::new();
//!     let client = OattyClient::new("https://api.example.com", &headers)?;
//!     let res = client
//!         .request(reqwest::Method::GET, "/apps")
//!         .send()?;
//!     println!("status: {}", res.status());
//!     Ok(())
//! }
//! ```

use std::time::Duration;
use std::{env, str::FromStr};

use anyhow::{Context, Result, anyhow};
use indexmap::IndexSet;
use oatty_types::EnvVar;
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
    /// Construct an [`OattyClient`] using an explicit base URL and optional custom headers.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The full base URL for API requests (for example, `https://api.example.com`).
    /// * `headers` - Additional headers (for example, `Authorization`) pulled from the current environment.
    ///
    /// # Returns
    ///
    /// A configured [`OattyClient`] instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the base URL is invalid or the HTTP client fails to initialize.
    pub fn new(base_url: impl Into<String>, headers: &IndexSet<EnvVar>) -> Result<Self> {
        let http = build_http_client(headers)?;

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

fn build_http_client(headers: &IndexSet<EnvVar>) -> Result<Client> {
    let mut default_headers = header::HeaderMap::new();
    default_headers.insert(header::ACCEPT, header::HeaderValue::from_str(DEFAULT_ACCEPT_HEADER)?);
    for h in headers {
        let header_name = header::HeaderName::from_str(&h.key)?;
        let header_value = header::HeaderValue::from_str(&h.value)?;
        default_headers.insert(header_name, header_value);
    }
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
