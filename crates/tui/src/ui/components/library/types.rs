use std::borrow::Cow;

use anyhow::Result;
use indexmap::IndexSet;
use oatty_mcp::EnvVar;
use oatty_types::manifest::RegistryCatalog;
use thiserror::Error;
use url::Url;

#[derive(Debug, Default)]
pub struct CatalogProjection {
    /// Title of the registry.
    pub title: Cow<'static, str>,
    /// Description of the registry. May be copied from the schema description.
    pub description: Cow<'static, str>,
    /// Headers to include when making requests to the API endpoints.
    pub headers: IndexSet<EnvVar>,
    /// Base URLs for the API endpoints.
    pub base_urls: Vec<String>,
    /// Index of the currently selected base URL.
    pub base_url_index: usize,
    /// Vendor information for the registry.
    pub vendor: Cow<'static, str>,
    /// Command count for the registry.
    pub command_count: usize,
    /// Workflow count for the registry.
    pub workflow_count: usize,
    /// Provider contract count for the registry.
    pub provider_contract_count: usize,
    /// Whether the catalog is active.
    pub is_enabled: bool,
}

impl CatalogProjection {
    pub fn validate(&self) -> Result<(), CatalogValidationError> {
        for url in &self.base_urls {
            Url::parse(url).map_err(|e| CatalogValidationError::BaseUrls(format!("{}", e)))?;
        }
        match () {
            _ if self.headers.iter().any(|h| h.key.is_empty()) => {
                Err(CatalogValidationError::Headers("Header key cannot be empty".to_string()))
            }
            _ if self.base_urls.is_empty() => Err(CatalogValidationError::BaseUrls("Base URLs cannot be empty".to_string())),
            _ if self.base_url_index >= self.base_urls.len() => {
                Err(CatalogValidationError::BaseUrlIndex("Invalid base URL index".to_string()))
            }
            _ if self.vendor.is_empty() => Err(CatalogValidationError::CommandPrefix("Command prefix cannot be empty".to_string())),

            _ => Ok(()),
        }
    }
}

impl From<&RegistryCatalog> for CatalogProjection {
    fn from(value: &RegistryCatalog) -> Self {
        let mut projection = value
            .manifest
            .as_ref()
            .map(|m| CatalogProjection {
                vendor: Cow::Owned(m.vendor.clone()),
                command_count: m.commands.len(),
                workflow_count: 0,
                provider_contract_count: m.provider_contracts.len(),
                ..Default::default()
            })
            .unwrap_or_default();

        projection.title = Cow::Owned(value.title.clone());
        projection.description = Cow::Owned(value.description.clone());
        projection.headers = value.headers.clone();
        projection.base_urls = value.base_urls.clone();
        projection.base_url_index = value.base_url_index;
        projection.is_enabled = value.is_enabled;

        projection
    }
}

#[derive(Debug, Error)]
pub enum CatalogValidationError {
    #[error("Headers error: {0}")]
    Headers(String),
    #[error("Base url error: {0}")]
    BaseUrls(String),
    #[error("Base url index error: {0}")]
    BaseUrlIndex(String),
    #[error("Command prefix error: {0}")]
    CommandPrefix(String),
}
