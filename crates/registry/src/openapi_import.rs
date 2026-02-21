//! Shared OpenAPI catalog import service.
//!
//! This module centralizes OpenAPI parsing, preflight validation, catalog generation,
//! registry insertion, and configuration persistence so UI and MCP entry points
//! can share one implementation.

use crate::CommandRegistry;
use oatty_registry_gen::io::{ManifestInput, generate_catalog};
use oatty_types::manifest::RegistryCatalog;
use oatty_util::{OpenApiValidationViolation, collect_openapi_preflight_violations};
use serde_json::Value;
use thiserror::Error;

/// Request parameters for importing an OpenAPI source into the command registry.
#[derive(Debug, Clone)]
pub struct OpenApiCatalogImportRequest {
    /// OpenAPI source content (JSON or YAML text).
    pub source_content: String,
    /// Optional catalog title override.
    pub catalog_title_override: Option<String>,
    /// Optional vendor override used during generation and catalog manifest metadata.
    pub vendor_override: Option<String>,
    /// Optional base URL override applied to the imported catalog.
    pub base_url_override: Option<String>,
    /// Optional source string used for catalog provenance.
    pub source: Option<String>,
    /// Optional source type hint (`path` or `url`) used for catalog provenance.
    pub source_type: Option<String>,
    /// Optional enabled-state override for imported catalog.
    ///
    /// When omitted, overwrite operations preserve the existing enabled state.
    /// New imports default to enabled.
    pub enabled: Option<bool>,
    /// Whether existing catalog with same identifier should be replaced.
    pub overwrite: bool,
}

/// Successful OpenAPI catalog import result.
#[derive(Debug, Clone)]
pub struct OpenApiCatalogImportResult {
    /// Imported catalog identifier.
    pub catalog_id: String,
    /// Persisted catalog record after save.
    pub catalog: RegistryCatalog,
    /// Number of commands generated from the source.
    pub command_count: usize,
    /// Number of provider contracts generated from the source.
    pub provider_contract_count: usize,
}

/// Errors emitted by shared OpenAPI catalog import.
#[derive(Debug, Error)]
pub enum OpenApiCatalogImportError {
    /// Source was not valid JSON or YAML.
    #[error("source content is not valid JSON or YAML: {0}")]
    SourceParse(String),
    /// Source failed preflight validation.
    #[error("OpenAPI source failed preflight validation")]
    PreflightValidation(Vec<OpenApiValidationViolation>),
    /// Catalog generation failed.
    #[error("failed to derive catalog from OpenAPI source: {0}")]
    CatalogGeneration(String),
    /// Catalog already exists and overwrite was not enabled.
    #[error("catalog '{0}' already exists")]
    CatalogConflict(String),
    /// Existing catalog removal failed.
    #[error("failed to replace existing catalog '{catalog_id}': {message}")]
    ReplaceFailed { catalog_id: String, message: String },
    /// Registry insertion failed.
    #[error("failed to insert catalog: {0}")]
    InsertFailed(String),
    /// Registry configuration persistence failed.
    #[error("failed to persist catalog config: {0}")]
    SaveFailed(String),
    /// Persisted catalog metadata could not be retrieved after save.
    #[error("catalog import succeeded but persisted catalog metadata is unavailable")]
    PersistedCatalogUnavailable,
}

/// Imports an OpenAPI source into an existing command registry.
pub fn import_openapi_catalog_into_registry(
    registry: &mut CommandRegistry,
    request: OpenApiCatalogImportRequest,
) -> Result<OpenApiCatalogImportResult, OpenApiCatalogImportError> {
    let parsed_document = parse_openapi_document_value(&request.source_content)?;
    let preflight_violations = collect_openapi_preflight_violations(&parsed_document);
    if !preflight_violations.is_empty() {
        return Err(OpenApiCatalogImportError::PreflightValidation(preflight_violations));
    }

    let generated_catalog = generate_catalog(ManifestInput::new(
        None,
        Some(parsed_document.to_string()),
        request.vendor_override.clone(),
    ))
    .map_err(|error| OpenApiCatalogImportError::CatalogGeneration(error.to_string()))?;

    let prospective_catalog_id = request
        .catalog_title_override
        .clone()
        .unwrap_or_else(|| generated_catalog.title.clone());
    let existing_catalog = get_catalog_by_title(registry, &prospective_catalog_id).cloned();
    let mut normalized_catalog = apply_catalog_overrides(
        generated_catalog,
        request.catalog_title_override.as_deref(),
        request.vendor_override.as_deref(),
        request.base_url_override.as_deref(),
        request.source.as_deref(),
        request.source_type.as_deref(),
    );
    normalized_catalog = preserve_existing_runtime_configuration(
        normalized_catalog,
        existing_catalog.as_ref(),
        request.enabled,
        request.base_url_override.is_none(),
    );

    let command_count = normalized_catalog
        .manifest
        .as_ref()
        .map(|manifest| manifest.commands.len())
        .unwrap_or(0);
    let provider_contract_count = normalized_catalog
        .manifest
        .as_ref()
        .map(|manifest| manifest.provider_contracts.len())
        .unwrap_or(0);
    let catalog_id = normalized_catalog.title.clone();

    if registry_has_catalog(registry, &catalog_id) {
        if !request.overwrite {
            return Err(OpenApiCatalogImportError::CatalogConflict(catalog_id));
        }
        remove_catalog_for_overwrite(registry, &catalog_id)?;
    }

    registry
        .insert_catalog(normalized_catalog)
        .map_err(|error| OpenApiCatalogImportError::InsertFailed(error.to_string()))?;
    registry
        .config
        .save()
        .map_err(|error| OpenApiCatalogImportError::SaveFailed(error.to_string()))?;

    let persisted_catalog = get_catalog_by_title(registry, &catalog_id)
        .cloned()
        .ok_or(OpenApiCatalogImportError::PersistedCatalogUnavailable)?;

    Ok(OpenApiCatalogImportResult {
        catalog_id,
        catalog: persisted_catalog,
        command_count,
        provider_contract_count,
    })
}

fn parse_openapi_document_value(source_content: &str) -> Result<Value, OpenApiCatalogImportError> {
    serde_json::from_str::<Value>(source_content)
        .or_else(|_| serde_yaml::from_str::<Value>(source_content))
        .map_err(|error| OpenApiCatalogImportError::SourceParse(error.to_string()))
}

fn apply_catalog_overrides(
    mut catalog: RegistryCatalog,
    catalog_title_override: Option<&str>,
    vendor_override: Option<&str>,
    base_url_override: Option<&str>,
    source: Option<&str>,
    source_type: Option<&str>,
) -> RegistryCatalog {
    if let Some(catalog_title) = catalog_title_override {
        catalog.title = catalog_title.to_string();
    }
    if let Some(base_url) = base_url_override {
        catalog.base_urls = vec![base_url.to_string()];
        catalog.base_url_index = 0;
    }
    if let Some(vendor) = vendor_override
        && let Some(manifest) = catalog.manifest.as_mut()
    {
        manifest.vendor = vendor.to_string();
    }
    if let Some(manifest) = catalog.manifest.as_ref() {
        catalog.vendor = Some(manifest.vendor.clone());
    }
    if let Some(source) = source {
        catalog.import_source = Some(source.to_string());
    }
    if let Some(source_type) = source_type {
        catalog.import_source_type = Some(source_type.to_string());
    }
    catalog
}

fn preserve_existing_runtime_configuration(
    mut imported_catalog: RegistryCatalog,
    existing_catalog: Option<&RegistryCatalog>,
    enabled_override: Option<bool>,
    preserve_existing_base_urls: bool,
) -> RegistryCatalog {
    if let Some(existing_catalog) = existing_catalog {
        imported_catalog.headers = existing_catalog.headers.clone();
        if preserve_existing_base_urls && !existing_catalog.base_urls.is_empty() {
            imported_catalog.base_urls = existing_catalog.base_urls.clone();
            imported_catalog.base_url_index = existing_catalog
                .base_url_index
                .min(imported_catalog.base_urls.len().saturating_sub(1));
        }
        imported_catalog.is_enabled = enabled_override.unwrap_or(existing_catalog.is_enabled);
        return imported_catalog;
    }
    imported_catalog.is_enabled = enabled_override.unwrap_or(true);
    imported_catalog
}

fn registry_has_catalog(registry: &CommandRegistry, catalog_title: &str) -> bool {
    registry
        .config
        .catalogs
        .as_ref()
        .is_some_and(|catalogs| catalogs.iter().any(|catalog| catalog.title == catalog_title))
}

fn get_catalog_by_title<'catalog>(registry: &'catalog CommandRegistry, catalog_title: &str) -> Option<&'catalog RegistryCatalog> {
    registry
        .config
        .catalogs
        .as_ref()
        .and_then(|catalogs| catalogs.iter().find(|catalog| catalog.title == catalog_title))
}

fn remove_catalog_for_overwrite(registry: &mut CommandRegistry, catalog_id: &str) -> Result<(), OpenApiCatalogImportError> {
    registry
        .disable_catalog(catalog_id)
        .map_err(|error| OpenApiCatalogImportError::ReplaceFailed {
            catalog_id: catalog_id.to_string(),
            message: error.to_string(),
        })?;

    let Some(catalogs) = registry.config.catalogs.as_mut() else {
        return Err(OpenApiCatalogImportError::ReplaceFailed {
            catalog_id: catalog_id.to_string(),
            message: "no catalogs configured".to_string(),
        });
    };
    let Some(index) = catalogs.iter().position(|catalog| catalog.title == catalog_id) else {
        return Err(OpenApiCatalogImportError::ReplaceFailed {
            catalog_id: catalog_id.to_string(),
            message: "catalog not found".to_string(),
        });
    };

    let removed_catalog = catalogs.remove(index);
    let manifest_path = std::path::PathBuf::from(removed_catalog.manifest_path);
    if manifest_path.exists() {
        std::fs::remove_file(&manifest_path).map_err(|error| OpenApiCatalogImportError::ReplaceFailed {
            catalog_id: catalog_id.to_string(),
            message: format!("failed to remove old manifest '{}': {error}", manifest_path.display()),
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexSet;
    use oatty_types::{EnvSource, EnvVar, manifest::RegistryManifest};

    fn sample_catalog(title: &str) -> RegistryCatalog {
        RegistryCatalog {
            title: title.to_string(),
            description: "sample".to_string(),
            vendor: Some("sample".to_string()),
            manifest_path: "/tmp/sample.bin".to_string(),
            import_source: None,
            import_source_type: None,
            headers: IndexSet::new(),
            base_urls: vec!["https://api.example.com".to_string()],
            base_url_index: 0,
            manifest: Some(RegistryManifest::default()),
            is_enabled: true,
        }
    }

    #[test]
    fn preserve_existing_runtime_configuration_keeps_headers_base_url_and_enabled_by_default() {
        let mut existing = sample_catalog("Test");
        existing.headers.insert(EnvVar::new(
            "Authorization".to_string(),
            "Bearer existing".to_string(),
            EnvSource::Raw,
        ));
        existing.base_urls = vec!["https://api.us5.datadoghq.com".to_string(), "https://api.datadoghq.com".to_string()];
        existing.base_url_index = 1;
        existing.is_enabled = false;

        let mut imported = sample_catalog("Test");
        imported.headers.insert(EnvVar::new(
            "Authorization".to_string(),
            "Bearer imported".to_string(),
            EnvSource::Raw,
        ));
        imported.base_urls = vec!["https://api.imported.example.com".to_string()];
        imported.base_url_index = 0;
        imported.is_enabled = true;

        let merged = preserve_existing_runtime_configuration(imported, Some(&existing), None, true);

        assert_eq!(merged.headers, existing.headers);
        assert_eq!(merged.base_urls, existing.base_urls);
        assert_eq!(merged.base_url_index, existing.base_url_index);
        assert!(!merged.is_enabled);
    }

    #[test]
    fn preserve_existing_runtime_configuration_applies_enabled_override() {
        let existing = sample_catalog("Test");
        let imported = sample_catalog("Test");
        let merged = preserve_existing_runtime_configuration(imported, Some(&existing), Some(false), true);
        assert!(!merged.is_enabled);
    }

    #[test]
    fn preserve_existing_runtime_configuration_keeps_base_url_override_when_provided() {
        let mut existing = sample_catalog("Test");
        existing.base_urls = vec!["https://api.existing.example.com".to_string()];
        existing.base_url_index = 0;

        let mut imported = sample_catalog("Test");
        imported.base_urls = vec!["https://api.override.example.com".to_string()];
        imported.base_url_index = 0;

        let merged = preserve_existing_runtime_configuration(imported, Some(&existing), None, false);

        assert_eq!(merged.base_urls, vec!["https://api.override.example.com".to_string()]);
        assert_eq!(merged.base_url_index, 0);
    }
}
