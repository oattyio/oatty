//! Catalog import and runtime management helpers for MCP tools.
//!
//! This module encapsulates OpenAPI source loading, import preview/validation,
//! and runtime catalog mutations so `server/core.rs` remains focused on routing.

use crate::server::schemas::{
    CatalogApplyPatchRequest, CatalogCommandMatchKeyInput, CatalogEditHeadersRequest, CatalogGetMaskedHeadersRequest,
    CatalogHeaderEditMode, CatalogHeaderEditRow, CatalogHeaderSource, CatalogImportOpenApiRequest, CatalogPreviewImportRequest,
    CatalogRemoveRequest, CatalogSetBaseUrlRequest, CatalogSetEnabledRequest, CatalogSourceType, CatalogValidateOpenApiRequest,
};
use crate::server::workflow::errors::{conflict_error, not_found_error};
use oatty_registry::{
    CatalogCommandMatchKey, CatalogHeaderEditMode as RegistryCatalogHeaderEditMode, CatalogHeaderEditRow as RegistryCatalogHeaderEditRow,
    CatalogMutationError, CatalogPatchApplyError, CatalogPatchApplyRequest, CatalogPatchOperation, CommandRegistry,
    OpenApiCatalogImportError, OpenApiCatalogImportRequest, apply_catalog_patch, import_openapi_catalog_into_registry,
};
use oatty_registry_gen::io::{ManifestInput, generate_catalog};
use oatty_types::{CommandSpec, EnvSource, EnvVar, manifest::RegistryCatalog};
use rmcp::model::ErrorData;
use serde_json::{Map, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const COMMAND_PREVIEW_MAX: usize = 50;

/// Validates an OpenAPI source without mutating runtime catalog state.
pub(crate) async fn validate_openapi_source(request: &CatalogValidateOpenApiRequest) -> Result<Value, ErrorData> {
    let source_content = load_catalog_source_content(&request.source, request.source_type).await?;
    let parsed_document = parse_openapi_document_value(&source_content)?;
    ensure_openapi_document_preflight(&parsed_document)?;
    Ok(preview_catalog_from_document(
        &parsed_document,
        &request.source,
        CatalogPreviewOptions::new(None, None, None, false),
    ))
}

/// Previews catalog import results without writing files or mutating registry state.
pub(crate) async fn preview_openapi_import(request: &CatalogPreviewImportRequest) -> Result<Value, ErrorData> {
    let source_content = load_catalog_source_content(&request.source, request.source_type).await?;
    let parsed_document = parse_openapi_document_value(&source_content)?;
    ensure_openapi_document_preflight(&parsed_document)?;
    Ok(preview_catalog_from_document(
        &parsed_document,
        &request.source,
        CatalogPreviewOptions::new(
            Some(request.catalog_title.as_str()),
            request.vendor.as_deref(),
            request.base_url.as_deref(),
            request.include_command_preview.unwrap_or(false),
        ),
    ))
}

/// Imports an OpenAPI source into runtime catalog configuration and refreshes registry state.
pub(crate) async fn import_openapi_catalog(
    registry: &Arc<Mutex<CommandRegistry>>,
    request: &CatalogImportOpenApiRequest,
) -> Result<Value, ErrorData> {
    let source_content = load_catalog_source_content(&request.source, request.source_type).await?;

    let mut registry_guard = registry.lock().map_err(|error| {
        internal_catalog_error(
            format!("registry lock failed: {error}"),
            serde_json::json!({
                "catalog_title": request.catalog_title,
                "source": request.source
            }),
            "Retry catalog import. If this persists, restart MCP server and retry.",
        )
    })?;
    let import_result = import_openapi_catalog_into_registry(
        &mut registry_guard,
        OpenApiCatalogImportRequest {
            source_content,
            catalog_title_override: Some(request.catalog_title.clone()),
            vendor_override: request.vendor.clone(),
            base_url_override: request.base_url.clone(),
            source: Some(request.source.clone()),
            source_type: request.source_type.map(|source_type| match source_type {
                CatalogSourceType::Path => "path".to_string(),
                CatalogSourceType::Url => "url".to_string(),
            }),
            enabled: request.enabled,
            overwrite: request.overwrite.unwrap_or(false),
        },
    )
    .map_err(|error| map_openapi_import_error_to_mcp(error, request))?;

    let catalog_metadata = masked_catalog_metadata(&import_result.catalog);

    Ok(serde_json::json!({
        "catalog_id": import_result.catalog_id,
        "catalog_path": oatty_registry::default_config_path().to_string_lossy(),
        "manifest_path": import_result.catalog.manifest_path,
        "command_count": import_result.command_count,
        "provider_contract_count": import_result.provider_contract_count,
        "enabled": import_result.catalog.is_enabled,
        "base_urls": catalog_metadata.get("base_urls").cloned().unwrap_or(Value::Array(Vec::new())),
        "selected_base_url": catalog_metadata.get("selected_base_url").cloned().unwrap_or(Value::Null),
        "masked_headers": catalog_metadata.get("masked_headers").cloned().unwrap_or(Value::Array(Vec::new())),
        "warnings": [],
    }))
}

/// Applies deterministic command-level patch operations to an existing catalog.
pub(crate) fn apply_catalog_patch_runtime(
    registry: &Arc<Mutex<CommandRegistry>>,
    request: &CatalogApplyPatchRequest,
) -> Result<Value, ErrorData> {
    let patch_request = map_patch_request(request)?;
    let mut registry_guard = registry.lock().map_err(|error| {
        internal_catalog_error(
            format!("registry lock failed: {error}"),
            serde_json::json!({ "catalog_id": request.catalog_id }),
            "Retry catalog patch apply. If this persists, restart MCP server and retry.",
        )
    })?;
    let patch_result =
        apply_catalog_patch(&mut registry_guard, patch_request).map_err(|error| map_catalog_patch_error_to_mcp(error, request))?;

    let catalog_metadata = patch_result
        .catalog
        .as_ref()
        .map(masked_catalog_metadata)
        .unwrap_or(Value::Object(Map::new()));

    Ok(serde_json::json!({
        "catalog_id": patch_result.catalog_id,
        "requested_operation_count": patch_result.requested_operation_count,
        "applied_operation_count": patch_result.applied_operation_count,
        "final_command_count": patch_result.final_command_count,
        "final_provider_contract_count": patch_result.final_provider_contract_count,
        "operation_results": patch_result.operation_results,
        "base_urls": catalog_metadata.get("base_urls").cloned().unwrap_or(Value::Array(Vec::new())),
        "selected_base_url": catalog_metadata.get("selected_base_url").cloned().unwrap_or(Value::Null),
        "masked_headers": catalog_metadata.get("masked_headers").cloned().unwrap_or(Value::Array(Vec::new())),
    }))
}

fn map_openapi_import_error_to_mcp(error: OpenApiCatalogImportError, request: &CatalogImportOpenApiRequest) -> ErrorData {
    match error {
        OpenApiCatalogImportError::SourceParse(message) => ErrorData::invalid_params(
            format!("source content is not valid JSON or YAML: {message}"),
            Some(serde_json::json!({
                "error_code": "OPENAPI_SOURCE_PARSE_FAILED",
                "violations": [
                    {
                        "path": "$",
                        "rule": "parse",
                        "message": message,
                    }
                ],
                "suggested_action": "Provide a valid OpenAPI JSON/YAML document."
            })),
        ),
        OpenApiCatalogImportError::PreflightValidation(violations) => ErrorData::invalid_params(
            "OpenAPI source failed preflight validation".to_string(),
            Some(serde_json::json!({
                "error_code": "OPENAPI_SOURCE_VALIDATION_FAILED",
                "violations": violations
                    .iter()
                    .map(oatty_util::OpenApiValidationViolation::to_json_value)
                    .collect::<Vec<Value>>(),
                "suggested_action": "Provide an OpenAPI 3.x document with a valid `paths` object and at least one HTTP operation."
            })),
        ),
        OpenApiCatalogImportError::CatalogConflict(catalog_id) => conflict_error(
            "CATALOG_CONFLICT",
            format!("catalog '{}' already exists", catalog_id),
            serde_json::json!({
                "catalog_id": catalog_id,
                "overwrite": request.overwrite.unwrap_or(false),
            }),
            "Set overwrite=true to replace the existing catalog.",
        ),
        OpenApiCatalogImportError::CatalogGeneration(message) => invalid_catalog_params_error(
            format!("failed to derive catalog from OpenAPI source: {message}"),
            serde_json::json!({
                "catalog_title": request.catalog_title,
                "vendor": request.vendor,
                "source": request.source
            }),
            "Review the schema for unsupported operations or malformed operation metadata, then retry import.",
        ),
        OpenApiCatalogImportError::ReplaceFailed { message, .. }
        | OpenApiCatalogImportError::InsertFailed(message)
        | OpenApiCatalogImportError::SaveFailed(message) => internal_catalog_error(
            format!("catalog import failed: {message}"),
            serde_json::json!({
                "catalog_title": request.catalog_title,
                "source": request.source
            }),
            "Retry import. If this persists, verify runtime config write permissions.",
        ),
        OpenApiCatalogImportError::PersistedCatalogUnavailable => internal_catalog_error(
            "catalog import succeeded but persisted catalog metadata is unavailable".to_string(),
            serde_json::json!({
                "catalog_title": request.catalog_title,
                "source": request.source
            }),
            "Run catalog list to verify persisted state and retry import if needed.",
        ),
    }
}

fn map_patch_request(request: &CatalogApplyPatchRequest) -> Result<CatalogPatchApplyRequest, ErrorData> {
    let mut operations = Vec::with_capacity(request.operations.len());
    for (operation_index, operation) in request.operations.iter().enumerate() {
        let replacement_command: CommandSpec = serde_json::from_value(operation.replacement_command.clone()).map_err(|error| {
            invalid_catalog_params_error(
                format!("invalid replacement_command at operation {operation_index}: {error}"),
                serde_json::json!({
                    "catalog_id": request.catalog_id,
                    "operation_id": operation.operation_id,
                    "operation_index": operation_index
                }),
                "Provide a valid CommandSpec payload for replacement_command.",
            )
        })?;

        operations.push(CatalogPatchOperation {
            operation_id: operation.operation_id.clone(),
            match_command: map_match_key(&operation.match_command),
            replacement_command,
        });
    }

    Ok(
        CatalogPatchApplyRequest::new(request.catalog_id.clone(), operations).with_policy_overrides(
            request.fail_on_missing,
            request.fail_on_ambiguous,
            request.overwrite,
        ),
    )
}

fn map_match_key(match_key: &CatalogCommandMatchKeyInput) -> CatalogCommandMatchKey {
    CatalogCommandMatchKey {
        group: match_key.group.clone(),
        name: match_key.name.clone(),
        http_method: match_key.http_method.clone(),
        http_path: match_key.http_path.clone(),
    }
}

fn map_catalog_patch_error_to_mcp(error: CatalogPatchApplyError, request: &CatalogApplyPatchRequest) -> ErrorData {
    match error {
        CatalogPatchApplyError::CatalogNotFound(catalog_id) => not_found_error(
            "CATALOG_NOT_FOUND",
            format!("catalog '{}' was not found", catalog_id),
            serde_json::json!({ "catalog_id": catalog_id }),
            "Use list_command_topics to inspect configured catalogs.",
        ),
        CatalogPatchApplyError::MissingManifest(catalog_id) => invalid_catalog_params_error(
            format!("catalog '{}' has no manifest content", catalog_id),
            serde_json::json!({ "catalog_id": catalog_id }),
            "Import or re-import the catalog so manifest content is available, then retry patch apply.",
        ),
        CatalogPatchApplyError::TargetNotFound { operation_index } => invalid_catalog_params_error(
            format!("operation {operation_index} target not found"),
            serde_json::json!({
                "catalog_id": request.catalog_id,
                "operation_index": operation_index
            }),
            "Run search_commands/get_command to confirm exact group/name/http method/path, then retry patch apply.",
        ),
        CatalogPatchApplyError::TargetAmbiguous {
            operation_index,
            matched_count,
        } => invalid_catalog_params_error(
            format!("operation {operation_index} target is ambiguous (matched {matched_count})"),
            serde_json::json!({
                "catalog_id": request.catalog_id,
                "operation_index": operation_index,
                "matched_count": matched_count
            }),
            "Tighten match_command fields so each operation resolves to exactly one command.",
        ),
        CatalogPatchApplyError::OverwriteRequired => invalid_catalog_params_error(
            "overwrite must be true for catalog patch apply",
            serde_json::json!({ "catalog_id": request.catalog_id }),
            "Set overwrite=true to persist the patched catalog.",
        ),
        CatalogPatchApplyError::ReplaceFailed { catalog_id, message }
        | CatalogPatchApplyError::InsertFailed { catalog_id, message }
        | CatalogPatchApplyError::SaveFailed { catalog_id, message } => internal_catalog_error(
            format!("failed to persist patched catalog '{}': {message}", catalog_id),
            serde_json::json!({ "catalog_id": catalog_id }),
            "Retry patch apply. If this persists, verify runtime config write permissions.",
        ),
        CatalogPatchApplyError::PersistedCatalogUnavailable(catalog_id) => internal_catalog_error(
            format!("patched catalog '{}' is unavailable after save", catalog_id),
            serde_json::json!({ "catalog_id": catalog_id }),
            "Run list_command_topics to verify persisted state and retry patch apply if needed.",
        ),
    }
}

/// Enables or disables an existing runtime catalog.
pub(crate) fn set_catalog_enabled_state(
    registry: &Arc<Mutex<CommandRegistry>>,
    request: &CatalogSetEnabledRequest,
) -> Result<Value, ErrorData> {
    let mut registry_guard = registry.lock().map_err(|error| {
        internal_catalog_error(
            format!("registry lock failed: {error}"),
            serde_json::json!({ "catalog_id": request.catalog_id }),
            "Retry catalog enable/disable. If this persists, restart MCP server and retry.",
        )
    })?;

    if request.enabled {
        registry_guard.enable_catalog(&request.catalog_id).map_err(|error| {
            invalid_catalog_params_error(
                format!("failed to enable catalog '{}': {error}", request.catalog_id),
                serde_json::json!({ "catalog_id": request.catalog_id }),
                "Use list_command_topics to verify the catalog id, then retry enable.",
            )
        })?;
    } else {
        registry_guard.disable_catalog(&request.catalog_id).map_err(|error| {
            invalid_catalog_params_error(
                format!("failed to disable catalog '{}': {error}", request.catalog_id),
                serde_json::json!({ "catalog_id": request.catalog_id }),
                "Use list_command_topics to verify the catalog id, then retry disable.",
            )
        })?;
    }

    registry_guard.config.save().map_err(|error| {
        internal_catalog_error(
            format!("failed to persist catalog config: {error}"),
            serde_json::json!({ "catalog_id": request.catalog_id }),
            "Verify runtime config write permissions, then retry.",
        )
    })?;

    let command_count_after_toggle = get_catalog_by_title(&registry_guard, &request.catalog_id)
        .and_then(|catalog| catalog.manifest.as_ref().map(|manifest| manifest.commands.len()))
        .unwrap_or(0);

    Ok(serde_json::json!({
        "catalog_id": request.catalog_id,
        "enabled": request.enabled,
        "command_count_after_toggle": command_count_after_toggle,
    }))
}

/// Sets the selected base URL for a catalog, adding the URL if it does not yet exist.
pub(crate) fn set_catalog_base_url(registry: &Arc<Mutex<CommandRegistry>>, request: &CatalogSetBaseUrlRequest) -> Result<Value, ErrorData> {
    let normalized_base_url = request.base_url.trim();
    if normalized_base_url.is_empty() {
        return Err(invalid_catalog_params_error(
            "base_url cannot be empty",
            serde_json::json!({ "catalog_id": request.catalog_id }),
            "Provide a non-empty absolute base URL.",
        ));
    }

    let updated_catalog = mutate_catalog_with_reload(
        registry,
        &request.catalog_id,
        "Retry catalog base URL update. If this persists, restart MCP server and retry.",
        "catalog update persisted but could not be reloaded",
        |registry_guard| {
            registry_guard
                .set_selected_base_url(&request.catalog_id, normalized_base_url)
                .map_err(|error| map_registry_catalog_mutation_error("base URL update", &request.catalog_id, error))
        },
    )?;

    Ok(serde_json::json!({
        "catalog_id": request.catalog_id,
        "selected_base_url": updated_catalog.selected_base_url(),
        "base_url_index": updated_catalog.base_url_index,
        "base_urls": updated_catalog.base_urls,
    }))
}

/// Mutates catalog headers according to the provided edit mode.
pub(crate) fn edit_catalog_headers(
    registry: &Arc<Mutex<CommandRegistry>>,
    request: &CatalogEditHeadersRequest,
) -> Result<Value, ErrorData> {
    let mode = request.mode.unwrap_or(CatalogHeaderEditMode::Upsert);
    let registry_mode = map_header_edit_mode(mode);
    let registry_rows = map_header_edit_rows(&request.headers);
    let updated_catalog = mutate_catalog_with_reload(
        registry,
        &request.catalog_id,
        "Retry header update. If this persists, restart MCP server and retry.",
        "header update persisted but catalog could not be reloaded",
        |registry_guard| {
            registry_guard
                .edit_catalog_headers(&request.catalog_id, registry_mode, &registry_rows)
                .map_err(|error| map_registry_catalog_mutation_error("header update", &request.catalog_id, error))
        },
    )?;
    Ok(serde_json::json!({
        "catalog_id": request.catalog_id,
        "mode": header_edit_mode_name(mode),
        "header_count": updated_catalog.headers.len(),
        "masked_headers": masked_headers(&updated_catalog.headers),
    }))
}

/// Returns masked catalog headers and selected base URL metadata.
pub(crate) fn get_catalog_masked_headers(
    registry: &Arc<Mutex<CommandRegistry>>,
    request: &CatalogGetMaskedHeadersRequest,
) -> Result<Value, ErrorData> {
    let registry_guard = registry.lock().map_err(|error| {
        internal_catalog_error(
            format!("registry lock failed: {error}"),
            serde_json::json!({ "catalog_id": request.catalog_id }),
            "Retry catalog header retrieval. If this persists, restart MCP server and retry.",
        )
    })?;
    let catalog = get_catalog_by_title(&registry_guard, &request.catalog_id).ok_or_else(|| {
        not_found_error(
            "CATALOG_NOT_FOUND",
            format!("catalog '{}' was not found", request.catalog_id),
            serde_json::json!({ "catalog_id": request.catalog_id }),
            "Use list_command_topics to inspect configured catalogs.",
        )
    })?;

    Ok(masked_catalog_metadata(catalog))
}

/// Removes an existing runtime catalog entry.
pub(crate) fn remove_catalog_runtime(registry: &Arc<Mutex<CommandRegistry>>, request: &CatalogRemoveRequest) -> Result<Value, ErrorData> {
    let remove_manifest = request.remove_manifest.unwrap_or(false);
    let mut registry_guard = registry.lock().map_err(|error| {
        internal_catalog_error(
            format!("registry lock failed: {error}"),
            serde_json::json!({ "catalog_id": request.catalog_id }),
            "Retry catalog removal. If this persists, restart MCP server and retry.",
        )
    })?;

    let removed = remove_catalog_internal(&mut registry_guard, &request.catalog_id, remove_manifest)?;
    let remaining_catalog_count = registry_guard.config.catalogs.as_ref().map(|catalogs| catalogs.len()).unwrap_or(0);
    registry_guard.config.save().map_err(|error| {
        internal_catalog_error(
            format!("failed to persist catalog config: {error}"),
            serde_json::json!({ "catalog_id": request.catalog_id }),
            "Verify runtime config write permissions, then retry.",
        )
    })?;

    Ok(serde_json::json!({
        "removed_catalog_id": removed.title,
        "manifest_removed": removed.manifest_removed,
        "remaining_catalog_count": remaining_catalog_count,
    }))
}

#[derive(Debug, Clone)]
struct CatalogPreviewOptions<'value> {
    catalog_title_override: Option<&'value str>,
    vendor_override: Option<&'value str>,
    base_url_override: Option<&'value str>,
    include_command_preview: bool,
}

impl<'value> CatalogPreviewOptions<'value> {
    fn new(
        catalog_title_override: Option<&'value str>,
        vendor_override: Option<&'value str>,
        base_url_override: Option<&'value str>,
        include_command_preview: bool,
    ) -> Self {
        Self {
            catalog_title_override,
            vendor_override,
            base_url_override,
            include_command_preview,
        }
    }
}

#[derive(Debug, Clone)]
struct RemovedCatalogMetadata {
    title: String,
    manifest_removed: bool,
}

async fn load_catalog_source_content(source: &str, source_type: Option<CatalogSourceType>) -> Result<String, ErrorData> {
    let resolved_source_type = resolve_catalog_source_type(source, source_type);
    match resolved_source_type {
        CatalogSourceType::Path => load_catalog_source_from_path(source),
        CatalogSourceType::Url => load_catalog_source_from_url(source).await,
    }
}

fn resolve_catalog_source_type(source: &str, source_type: Option<CatalogSourceType>) -> CatalogSourceType {
    if let Some(explicit_source_type) = source_type {
        return explicit_source_type;
    }
    if source.starts_with("http://") || source.starts_with("https://") {
        return CatalogSourceType::Url;
    }
    CatalogSourceType::Path
}

fn invalid_catalog_params_error(message: impl Into<String>, context: Value, suggested_action: &str) -> ErrorData {
    let message = message.into();
    ErrorData::invalid_params(
        message,
        Some(serde_json::json!({
            "context": context,
            "suggested_action": suggested_action
        })),
    )
}

fn internal_catalog_error(message: impl Into<String>, context: Value, suggested_action: &str) -> ErrorData {
    let message = message.into();
    ErrorData::internal_error(
        message,
        Some(serde_json::json!({
            "context": context,
            "suggested_action": suggested_action
        })),
    )
}

fn load_catalog_source_from_path(source: &str) -> Result<String, ErrorData> {
    let source_path = oatty_util::expand_tilde(source);
    std::fs::read_to_string(&source_path).map_err(|error| {
        ErrorData::invalid_params(
            format!("failed to read OpenAPI source path '{}': {error}", source_path.display()),
            Some(serde_json::json!({
                "source": source,
                "source_type": "path",
                "path": source_path.to_string_lossy(),
                "suggested_action": "Provide a readable local file path."
            })),
        )
    })
}

async fn load_catalog_source_from_url(source: &str) -> Result<String, ErrorData> {
    let url = reqwest::Url::parse(source).map_err(|error| {
        ErrorData::invalid_params(
            format!("invalid source URL '{source}': {error}"),
            Some(serde_json::json!({
                "source": source,
                "source_type": "url",
                "suggested_action": "Provide an absolute HTTP(S) URL."
            })),
        )
    })?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ErrorData::invalid_params(
            format!("unsupported URL scheme '{}'", url.scheme()),
            Some(serde_json::json!({
                "source": source,
                "allowed_schemes": ["http", "https"],
                "suggested_action": "Use an HTTP(S) URL or pass a local source path."
            })),
        ));
    }

    let response = reqwest::get(url).await.map_err(|error| {
        internal_catalog_error(
            format!("failed to fetch OpenAPI source URL: {error}"),
            serde_json::json!({ "source": source, "source_type": "url" }),
            "Verify network connectivity and URL reachability, then retry.",
        )
    })?;
    let status = response.status();
    if !status.is_success() {
        return Err(ErrorData::invalid_params(
            format!("OpenAPI source URL returned HTTP {status}"),
            Some(serde_json::json!({
                "source": source,
                "status": status.as_u16(),
                "suggested_action": "Verify the URL is reachable and serves an OpenAPI document."
            })),
        ));
    }
    response.text().await.map_err(|error| {
        internal_catalog_error(
            format!("failed to read OpenAPI URL response body: {error}"),
            serde_json::json!({ "source": source, "source_type": "url" }),
            "Retry fetch. If this persists, verify the source returns a readable text body.",
        )
    })
}

fn parse_openapi_document_value(source_content: &str) -> Result<Value, ErrorData> {
    serde_json::from_str::<Value>(source_content)
        .or_else(|_| serde_yaml::from_str::<Value>(source_content))
        .map_err(|error| {
            ErrorData::invalid_params(
                format!("source content is not valid JSON or YAML: {error}"),
                Some(serde_json::json!({
                    "error_code": "OPENAPI_SOURCE_PARSE_FAILED",
                    "violations": [
                        {
                            "path": "$",
                            "rule": "parse",
                            "message": error.to_string()
                        }
                    ],
                    "suggested_action": "Provide a valid OpenAPI JSON/YAML document."
                })),
            )
        })
}

fn ensure_openapi_document_preflight(document: &Value) -> Result<(), ErrorData> {
    let violations = oatty_util::collect_openapi_preflight_violations(document);
    if violations.is_empty() {
        return Ok(());
    }

    Err(ErrorData::invalid_params(
        "OpenAPI source failed preflight validation".to_string(),
        Some(serde_json::json!({
            "error_code": "OPENAPI_SOURCE_VALIDATION_FAILED",
            "violations": violations
                .iter()
                .map(oatty_util::OpenApiValidationViolation::to_json_value)
                .collect::<Vec<Value>>(),
            "suggested_action": "Provide an OpenAPI 3.x document with a valid `paths` object and at least one HTTP operation."
        })),
    ))
}

fn preview_catalog_from_document(document: &Value, source: &str, options: CatalogPreviewOptions<'_>) -> Value {
    let document_kind = detect_openapi_document_kind(document);
    let operation_count = count_openapi_operations(document);
    let warnings = build_openapi_warnings(document, &document_kind, operation_count);
    let preview_catalog_result = generate_catalog(ManifestInput::new(
        None,
        Some(document.to_string()),
        options.vendor_override.map(str::to_string),
    ));

    match preview_catalog_result {
        Ok(catalog) => {
            let normalized_catalog = apply_catalog_overrides(catalog, &options);
            let command_preview = if options.include_command_preview {
                Some(build_command_preview(
                    normalized_catalog
                        .manifest
                        .as_ref()
                        .map(|manifest| manifest.commands.as_slice())
                        .unwrap_or(&[]),
                ))
            } else {
                None
            };
            let provider_contract_count = normalized_catalog
                .manifest
                .as_ref()
                .map(|manifest| manifest.provider_contracts.len())
                .unwrap_or(0);
            let command_count = normalized_catalog
                .manifest
                .as_ref()
                .map(|manifest| manifest.commands.len())
                .unwrap_or(0);
            let projected_group_prefixes = normalized_catalog
                .manifest
                .as_ref()
                .map(|manifest| {
                    let mut groups = manifest
                        .commands
                        .iter()
                        .map(|command| command.group.clone())
                        .collect::<Vec<String>>();
                    groups.sort();
                    groups.dedup();
                    groups
                })
                .unwrap_or_default();
            let mut response = Map::new();
            response.insert("valid".to_string(), Value::Bool(true));
            response.insert("source".to_string(), Value::String(source.to_string()));
            response.insert("document_kind".to_string(), Value::String(document_kind));
            response.insert("operation_count".to_string(), serde_json::json!(operation_count));
            response.insert("warnings".to_string(), Value::Array(warnings));
            response.insert(
                "catalog".to_string(),
                serde_json::json!({
                    "title": normalized_catalog.title,
                    "vendor": normalized_catalog
                        .manifest
                        .as_ref()
                        .map(|manifest| manifest.vendor.clone())
                        .unwrap_or_default(),
                    "base_url": normalized_catalog.selected_base_url(),
                }),
            );
            response.insert("projected_command_count".to_string(), serde_json::json!(command_count));
            response.insert("provider_contract_count".to_string(), serde_json::json!(provider_contract_count));
            response.insert(
                "projected_group_prefixes".to_string(),
                Value::Array(projected_group_prefixes.into_iter().map(Value::String).collect::<Vec<Value>>()),
            );
            if let Some(preview) = command_preview
                && !preview.is_empty()
            {
                response.insert("command_preview".to_string(), Value::Array(preview));
            }
            Value::Object(response)
        }
        Err(error) => serde_json::json!({
            "valid": false,
            "source": source,
            "document_kind": document_kind,
            "operation_count": operation_count,
            "warnings": warnings,
            "violations": [
                {
                    "path": "$",
                    "rule": "openapi_generation",
                    "message": error.to_string(),
                }
            ]
        }),
    }
}

fn remove_catalog_internal(
    registry: &mut CommandRegistry,
    catalog_identifier: &str,
    remove_manifest: bool,
) -> Result<RemovedCatalogMetadata, ErrorData> {
    let Some(catalogs) = registry.config.catalogs.as_mut() else {
        return Err(not_found_error(
            "CATALOG_NOT_FOUND",
            format!("catalog '{}' was not found", catalog_identifier),
            serde_json::json!({ "catalog_id": catalog_identifier }),
            "Use list_command_topics to inspect configured catalogs.",
        ));
    };
    let Some(index) = catalogs.iter().position(|catalog| catalog.title == catalog_identifier) else {
        return Err(not_found_error(
            "CATALOG_NOT_FOUND",
            format!("catalog '{}' was not found", catalog_identifier),
            serde_json::json!({ "catalog_id": catalog_identifier }),
            "Use list_command_topics to inspect configured catalogs.",
        ));
    };

    let removed_catalog = catalogs.remove(index);
    let command_ids = removed_catalog
        .manifest
        .as_ref()
        .map(|manifest| {
            manifest
                .commands
                .iter()
                .map(|command| command.canonical_id())
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();
    registry.remove_commands(command_ids);

    let manifest_path = PathBuf::from(removed_catalog.manifest_path.clone());
    let manifest_removed = if remove_manifest && manifest_path.exists() {
        std::fs::remove_file(&manifest_path).map(|_| true).map_err(|error| {
            internal_catalog_error(
                format!("failed to remove catalog manifest '{}': {error}", manifest_path.display()),
                serde_json::json!({
                    "catalog_id": catalog_identifier,
                    "manifest_path": manifest_path.to_string_lossy().to_string()
                }),
                "Verify file permissions and retry catalog removal.",
            )
        })?
    } else {
        false
    };

    Ok(RemovedCatalogMetadata {
        title: removed_catalog.title,
        manifest_removed,
    })
}

fn get_catalog_by_title<'catalog>(registry: &'catalog CommandRegistry, catalog_title: &str) -> Option<&'catalog RegistryCatalog> {
    registry
        .config
        .catalogs
        .as_ref()
        .and_then(|catalogs| catalogs.iter().find(|catalog| catalog.title == catalog_title))
}

fn apply_catalog_overrides(mut catalog: RegistryCatalog, options: &CatalogPreviewOptions<'_>) -> RegistryCatalog {
    if let Some(catalog_title) = options.catalog_title_override {
        catalog.title = catalog_title.to_string();
    }
    if let Some(base_url_override) = options.base_url_override {
        catalog.base_urls = vec![base_url_override.to_string()];
        catalog.base_url_index = 0;
    }
    if let Some(vendor_override) = options.vendor_override
        && let Some(manifest) = catalog.manifest.as_mut()
    {
        manifest.vendor = vendor_override.to_string();
    }
    catalog
}

fn detect_openapi_document_kind(document: &Value) -> String {
    if document
        .get("openapi")
        .and_then(Value::as_str)
        .is_some_and(|version| version.starts_with("3."))
    {
        return "openapi_3".to_string();
    }
    if document.get("openapi").and_then(Value::as_str).is_some() {
        return "openapi_other".to_string();
    }
    if document.get("swagger").and_then(Value::as_str).is_some() {
        return "openapi_2".to_string();
    }
    "unknown".to_string()
}

fn count_openapi_operations(document: &Value) -> usize {
    let Some(paths) = document.get("paths").and_then(Value::as_object) else {
        return 0;
    };
    paths
        .values()
        .filter_map(Value::as_object)
        .map(|path_item| {
            path_item
                .keys()
                .filter(|key| matches!(key.as_str(), "get" | "post" | "put" | "patch" | "delete" | "options" | "head"))
                .count()
        })
        .sum()
}

fn build_openapi_warnings(document: &Value, document_kind: &str, operation_count: usize) -> Vec<Value> {
    let mut warnings = Vec::new();
    if document_kind != "openapi_3" {
        warnings.push(serde_json::json!("OpenAPI v3 is recommended; other versions may fail import."));
    }
    if operation_count == 0 {
        warnings.push(serde_json::json!("No operations were discovered under `paths`."));
    }
    if document.get("servers").and_then(Value::as_array).is_none() {
        warnings.push(serde_json::json!(
            "Document has no `servers` section; base_url may need manual override."
        ));
    }
    warnings
}

fn build_command_preview(commands: &[CommandSpec]) -> Vec<Value> {
    commands
        .iter()
        .take(COMMAND_PREVIEW_MAX)
        .map(|command| {
            serde_json::json!({
                "canonical_id": command.canonical_id(),
                "summary": command.summary,
                "execution_type": command_execution_type(command),
                "http_method": command.http().map(|http| http.method.clone()),
            })
        })
        .collect()
}

fn command_execution_type(command_spec: &CommandSpec) -> &'static str {
    if command_spec.http().is_some() {
        return "http";
    }
    if command_spec.mcp().is_some() {
        return "mcp";
    }
    "unknown"
}

fn masked_catalog_metadata(catalog: &RegistryCatalog) -> Value {
    serde_json::json!({
        "catalog_id": catalog.title,
        "selected_base_url": catalog.selected_base_url(),
        "base_url_index": catalog.base_url_index,
        "base_urls": catalog.base_urls,
        "masked_headers": masked_headers(&catalog.headers),
        "header_count": catalog.headers.len(),
    })
}

fn masked_headers(headers: &indexmap::IndexSet<EnvVar>) -> Vec<Value> {
    headers
        .iter()
        .map(|header| {
            let masked = header.masked();
            serde_json::json!({
                "key": masked.key,
                "source": format!("{}", masked.source),
                "effective": masked.effective,
                "masked_value_preview": masked.value,
            })
        })
        .collect()
}

fn header_edit_mode_name(mode: CatalogHeaderEditMode) -> &'static str {
    match mode {
        CatalogHeaderEditMode::Upsert => "upsert",
        CatalogHeaderEditMode::Remove => "remove",
        CatalogHeaderEditMode::ReplaceAll => "replace_all",
    }
}

fn map_header_edit_mode(mode: CatalogHeaderEditMode) -> RegistryCatalogHeaderEditMode {
    match mode {
        CatalogHeaderEditMode::Upsert => RegistryCatalogHeaderEditMode::Upsert,
        CatalogHeaderEditMode::Remove => RegistryCatalogHeaderEditMode::Remove,
        CatalogHeaderEditMode::ReplaceAll => RegistryCatalogHeaderEditMode::ReplaceAll,
    }
}

fn map_header_edit_rows(rows: &[CatalogHeaderEditRow]) -> Vec<RegistryCatalogHeaderEditRow> {
    rows.iter()
        .map(|row| RegistryCatalogHeaderEditRow {
            key: row.key.clone(),
            value: row.value.clone(),
            source: map_header_source(row.source),
            effective: row.effective.unwrap_or(true),
        })
        .collect()
}

fn map_header_source(source: Option<CatalogHeaderSource>) -> EnvSource {
    match source.unwrap_or(CatalogHeaderSource::Raw) {
        CatalogHeaderSource::File => EnvSource::File,
        CatalogHeaderSource::Secret => EnvSource::Secret,
        CatalogHeaderSource::Env => EnvSource::Env,
        CatalogHeaderSource::Raw => EnvSource::Raw,
    }
}

fn map_registry_catalog_mutation_error(action: &str, catalog_id: &str, error: CatalogMutationError) -> ErrorData {
    match error {
        CatalogMutationError::NoCatalogsConfigured | CatalogMutationError::CatalogNotFound { .. } => not_found_error(
            "CATALOG_NOT_FOUND",
            format!("catalog '{}' was not found", catalog_id),
            serde_json::json!({ "catalog_id": catalog_id }),
            "Use list_command_topics to inspect configured catalogs.",
        ),
        CatalogMutationError::EmptyBaseUrl | CatalogMutationError::EmptyHeaderKey | CatalogMutationError::MissingHeaderValue { .. } => {
            invalid_catalog_params_error(
                format!("failed {} for catalog '{}': {}", action, catalog_id, error),
                serde_json::json!({ "catalog_id": catalog_id }),
                "Verify provided values, then retry.",
            )
        }
    }
}

fn mutate_catalog_with_reload(
    registry: &Arc<Mutex<CommandRegistry>>,
    catalog_id: &str,
    lock_failure_suggested_action: &str,
    reload_failure_message: &str,
    mutate_catalog: impl FnOnce(&mut CommandRegistry) -> Result<(), ErrorData>,
) -> Result<RegistryCatalog, ErrorData> {
    let mut registry_guard = registry.lock().map_err(|error| {
        internal_catalog_error(
            format!("registry lock failed: {error}"),
            serde_json::json!({ "catalog_id": catalog_id }),
            lock_failure_suggested_action,
        )
    })?;
    mutate_catalog(&mut registry_guard)?;

    registry_guard.config.save().map_err(|error| {
        internal_catalog_error(
            format!("failed to persist catalog config: {error}"),
            serde_json::json!({ "catalog_id": catalog_id }),
            "Verify runtime config write permissions, then retry.",
        )
    })?;

    get_catalog_by_title(&registry_guard, catalog_id).cloned().ok_or_else(|| {
        internal_catalog_error(
            reload_failure_message.to_string(),
            serde_json::json!({ "catalog_id": catalog_id }),
            "Retry the operation. If this persists, restart MCP server.",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexSet;
    use oatty_types::{EnvSource, EnvVar, manifest::RegistryManifest};

    #[test]
    fn map_registry_catalog_mutation_error_returns_not_found_for_missing_catalog() {
        let error = map_registry_catalog_mutation_error(
            "header update",
            "missing",
            CatalogMutationError::CatalogNotFound {
                title: "missing".to_string(),
            },
        );

        assert_eq!(error.code, rmcp::model::ErrorCode::RESOURCE_NOT_FOUND);
    }

    #[test]
    fn map_registry_catalog_mutation_error_returns_invalid_params_for_missing_header_value() {
        let error = map_registry_catalog_mutation_error(
            "header update",
            "test",
            CatalogMutationError::MissingHeaderValue {
                key: "Authorization".to_string(),
            },
        );

        assert_eq!(error.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    }

    #[test]
    fn detect_openapi_document_kind_identifies_v3() {
        let document = serde_json::json!({
            "openapi": "3.0.3",
            "paths": {}
        });
        assert_eq!(detect_openapi_document_kind(&document), "openapi_3");
    }

    #[test]
    fn count_openapi_operations_counts_http_methods() {
        let document = serde_json::json!({
            "openapi": "3.0.3",
            "paths": {
                "/apps": {
                    "get": {},
                    "post": {},
                    "parameters": []
                },
                "/apps/{id}": {
                    "delete": {},
                    "patch": {}
                }
            }
        });
        assert_eq!(count_openapi_operations(&document), 4);
    }

    #[test]
    fn build_openapi_warnings_reports_missing_servers() {
        let document = serde_json::json!({
            "openapi": "3.0.3",
            "paths": {}
        });
        let warnings = build_openapi_warnings(&document, "openapi_3", 0);
        assert!(!warnings.is_empty());
    }

    #[test]
    fn preflight_reports_missing_openapi_version() {
        let document = serde_json::json!({
            "paths": {
                "/apps": {
                    "get": {}
                }
            }
        });

        let violations = oatty_util::collect_openapi_preflight_violations(&document);

        assert!(violations.iter().any(|violation| violation.path == "$.openapi"));
    }

    #[test]
    fn preflight_reports_swagger_v2_document() {
        let document = serde_json::json!({
            "swagger": "2.0",
            "paths": {
                "/apps": {
                    "get": {}
                }
            }
        });

        let violations = oatty_util::collect_openapi_preflight_violations(&document);

        assert!(violations.iter().any(|violation| violation.path == "$.swagger"));
    }

    #[test]
    fn preflight_reports_missing_operations() {
        let document = serde_json::json!({
            "openapi": "3.0.3",
            "paths": {}
        });

        let violations = oatty_util::collect_openapi_preflight_violations(&document);

        assert!(violations.iter().any(|violation| violation.rule == "operations_presence"));
    }

    #[test]
    fn preflight_accepts_minimal_valid_openapi3_document() {
        let document = serde_json::json!({
            "openapi": "3.0.3",
            "paths": {
                "/apps": {
                    "get": {}
                }
            }
        });

        assert!(ensure_openapi_document_preflight(&document).is_ok());
    }

    #[test]
    fn map_header_edit_rows_applies_defaults() {
        let rows = vec![CatalogHeaderEditRow {
            key: "Authorization".to_string(),
            value: Some("Bearer token".to_string()),
            source: None,
            effective: None,
        }];
        let mapped_rows = map_header_edit_rows(&rows);
        assert_eq!(mapped_rows.len(), 1);
        let mapped_row = &mapped_rows[0];
        assert_eq!(mapped_row.key, "Authorization");
        assert_eq!(mapped_row.source, EnvSource::Raw);
        assert!(mapped_row.effective);
    }

    #[test]
    fn map_header_edit_mode_preserves_variant_semantics() {
        assert_eq!(
            map_header_edit_mode(CatalogHeaderEditMode::Upsert),
            RegistryCatalogHeaderEditMode::Upsert
        );
        assert_eq!(
            map_header_edit_mode(CatalogHeaderEditMode::Remove),
            RegistryCatalogHeaderEditMode::Remove
        );
        assert_eq!(
            map_header_edit_mode(CatalogHeaderEditMode::ReplaceAll),
            RegistryCatalogHeaderEditMode::ReplaceAll
        );
    }

    #[test]
    fn masked_catalog_metadata_includes_selected_url_and_header_preview() {
        let mut catalog = RegistryCatalog {
            title: "Datadog".to_string(),
            description: String::new(),
            vendor: Some("datadog".to_string()),
            manifest_path: "/tmp/catalog.bin".to_string(),
            import_source: None,
            import_source_type: None,
            headers: IndexSet::new(),
            base_urls: vec!["https://api.us5.datadoghq.com".to_string()],
            base_url_index: 0,
            manifest: Some(RegistryManifest::default()),
            is_enabled: true,
        };
        catalog
            .headers
            .insert(EnvVar::new("DD-API-KEY".to_string(), "abc123".to_string(), EnvSource::Secret));
        let metadata = masked_catalog_metadata(&catalog);
        let selected_base_url = metadata.get("selected_base_url").and_then(Value::as_str).unwrap_or_default();
        assert_eq!(selected_base_url, "https://api.us5.datadoghq.com");
        let masked_headers = metadata
            .get("masked_headers")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert_eq!(masked_headers.len(), 1);
    }
}
