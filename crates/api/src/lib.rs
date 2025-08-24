use anyhow::{Context, Result};
use reqwest::{Client, RequestBuilder};
use std::time::Duration;
use tracing::debug;

const DEFAULT_BASE_URL: &str = "https://api.heroku.com";

#[derive(Debug, Clone)]
pub struct HerokuClient {
    pub base_url: String,
    pub http: Client,
    pub user_agent: String,
}

impl HerokuClient {
    pub fn new_from_env() -> Result<Self> {
        let token = std::env::var("HEROKU_API_KEY")
            .ok()
            .or_else(get_netrc_token);
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(t) = token {
            let val = format!("Bearer {}", t);
            headers.insert(
                reqwest::header::AUTHORIZATION,
                reqwest::header::HeaderValue::from_str(&val).unwrap(),
            );
        }
        headers.insert(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("application/vnd.heroku+json; version=3"),
        );
        let http = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()
            .context("build http client")?;
        Ok(Self {
            base_url: std::env::var("HEROKU_API_BASE").unwrap_or_else(|_| DEFAULT_BASE_URL.into()),
            http,
            user_agent: format!(
                "heroku-cli/0.1 (+https://example.com); {}",
                std::env::consts::OS
            ),
        })
    }

    pub fn request(&self, method: reqwest::Method, path: &str) -> RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        debug!(%url, "building request");
        self.http
            .request(method, url)
            .header(reqwest::header::USER_AGENT, &self.user_agent)
    }
}

fn get_netrc_token() -> Option<String> {
    let home = dirs_next::home_dir()?;
    let netrc_path = home.join(".netrc");
    let content = std::fs::read_to_string(netrc_path).ok()?;
    parse_netrc_for_heroku(&content)
}

fn parse_netrc_for_heroku(content: &str) -> Option<String> {
    // Very naive parser adequate for placeholder
    let mut machine_is_heroku = false;
    let mut login_is_api = false;
    for token in content.split_whitespace() {
        match token {
            "machine" => {
                machine_is_heroku = false;
                login_is_api = false;
            }
            "api.heroku.com" => machine_is_heroku = true,
            "login" if machine_is_heroku => login_is_api = true,
            val if login_is_api => {
                if val == "api" { /* ignore */
                } else {
                    login_is_api = false
                }
            }
            "password" if machine_is_heroku => {
                // Next token should be the token
                // This is simplistic: in real code we should iterate properly
            }
            other if machine_is_heroku => {
                // Capture token after "password"
                // For placeholder, accept any long token-looking value
                if other.len() > 20 {
                    return Some(other.to_string());
                }
            }
            _ => {}
        }
    }
    None
}
