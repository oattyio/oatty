use std::borrow::Cow;

use indexmap::IndexMap;
use oatty_types::manifest::RegistryCatalog;

#[derive(Debug, Default)]
pub struct CatalogProjection {
    /// Title of the registry.
    pub title: Cow<'static, str>,
    /// Description of the registry. May be copied from the schema description.
    pub description: Cow<'static, str>,
    /// Headers to include when making requests to the API endpoints.
    pub headers: IndexMap<String, String>,
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

impl From<&RegistryCatalog> for CatalogProjection {
    fn from(value: &RegistryCatalog) -> Self {
        let mut projection = value
            .manifest
            .as_ref()
            .map(|m| CatalogProjection {
                vendor: Cow::Owned(m.vendor.clone()),
                command_count: m.commands.len(),
                workflow_count: m.workflows.len(),
                provider_contract_count: m.provider_contracts.len(),
                ..Default::default()
            })
            .unwrap_or(CatalogProjection::default());

        projection.title = Cow::Owned(value.title.clone());
        projection.description = Cow::Owned(value.description.clone());
        projection.headers = value.headers.clone();
        projection.base_urls = value.base_urls.clone();
        projection.base_url_index = value.base_url_index;
        projection.is_enabled = value.is_enabled;

        projection
    }
}
