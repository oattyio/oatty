//! Oatty API client utilities.
//!
//! This module provides a lightweight client for interacting with the Oatty API.
//! It focuses on:
//!
//! - Constructing an HTTP client with sensible defaults
//! - Discovering credentials from `HEROKU_API_KEY`
//! - Validating `HEROKU_API_BASE` for safety
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
//! use oatty_types::ServiceId;
//! use anyhow::Result;
//!
//! fn main() -> Result<()> {
//!     let client = OattyClient::new_from_service_id(ServiceId::CoreApi)?;
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
use oatty_types::{ServiceId, ToServiceIdInfo};
use reqwest::{Client, RequestBuilder, Url, header};
use tracing::debug;

/// Allowed hostnames or base domains for non-local configurations of
/// `HEROKU_API_BASE`. Subdomains of these domains are also allowed.
const ALLOWED_HEROKU_DOMAINS: &[&str] = &[
    "heroku.com",
    "herokai.com",
    "herokuspace.com",
    "herokudev.com",
    "heroku-data-api-staging.herokuapp.com",
];
/// Hostnames allowed for local development regardless of scheme.
const LOCALHOST_DOMAINS: &[&str] = &["localhost", "127.0.0.1"];

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
    /// - `HEROKU_API_KEY` environment variable
    ///
    /// The base URL is taken from `HEROKU_API_BASE` (if set) or falls back to
    /// the default public API. Non-localhost hosts must use HTTPS and be within
    /// an allowed Oatty domain.
    pub fn new_from_service_id(spec: ServiceId) -> Result<Self> {
        let api_token = env::var("HEROKU_API_KEY").ok();

        let mut default_headers = header::HeaderMap::new();
        if let Some(api_token) = api_token {
            let authorization_header_value = format!("Bearer {}", api_token);
            default_headers.insert(
                header::AUTHORIZATION,
                header::HeaderValue::from_str(&authorization_header_value).unwrap(),
            );
        }
        let accept_header = spec.accept_headers();
        default_headers.insert(header::ACCEPT, header::HeaderValue::from_str(accept_header)?);

        let http = Client::builder()
            .default_headers(default_headers)
            .timeout(Duration::from_secs(30))
            .build()
            .context("build http client")?;

        let base_url = env::var(spec.env_var()).unwrap_or_else(|_| spec.default_base_url().into());

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

/// Validate that a base URL is acceptable for use by the client.
///
/// Rules:
/// - `localhost` or `127.0.0.1`: any scheme is allowed
/// - otherwise: scheme must be HTTPS, and host must be one of the allowed
///   Oatty domains or a subdomain thereof
fn validate_base_url(base: &str) -> Result<()> {
    let parsed_base_url = Url::parse(base).map_err(|e| anyhow!("Invalid HEROKU_API_BASE URL '{}': {}", base, e))?;

    let host_name = parsed_base_url
        .host_str()
        .ok_or_else(|| anyhow!("HEROKU_API_BASE must include a host"))?;

    // Local development allowances: localhost/127.0.0.1 with any scheme.
    if LOCALHOST_DOMAINS.iter().any(|&allowed| host_name.eq_ignore_ascii_case(allowed)) {
        return Ok(());
    }

    // Production/staging: must be HTTPS and end with one of the allowed domains.
    if parsed_base_url.scheme() != "https" {
        return Err(anyhow!(
            "HEROKU_API_BASE must use https for non-localhost hosts; got '{}://'",
            parsed_base_url.scheme()
        ));
    }

    let is_allowed_domain = ALLOWED_HEROKU_DOMAINS
        .iter()
        .any(|&allowed_domain| host_name.eq_ignore_ascii_case(allowed_domain) || host_name.ends_with(&format!(".{}", allowed_domain)));
    if !is_allowed_domain {
        return Err(anyhow!(
            "HEROKU_API_BASE host '{}' is not allowed; must be one of {:?} or a subdomain, or localhost",
            host_name,
            ALLOWED_HEROKU_DOMAINS
        ));
    }

    Ok(())
}
