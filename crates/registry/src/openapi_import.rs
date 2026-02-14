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
    /// Whether imported catalog should be enabled.
    pub enabled: bool,
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

    let mut normalized_catalog = apply_catalog_overrides(
        generated_catalog,
        request.catalog_title_override.as_deref(),
        request.vendor_override.as_deref(),
        request.base_url_override.as_deref(),
        request.source.as_deref(),
        request.source_type.as_deref(),
    );
    normalized_catalog.is_enabled = request.enabled;

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
