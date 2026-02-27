//! Catalog patch application service.
//!
//! This module provides deterministic command-level patching for an existing
//! catalog manifest. Patch operations replace full command specifications by a
//! strict match key and then persist the patched catalog through the normal
//! registry insertion path.

use crate::CommandRegistry;
use crate::catalog_persistence::{CatalogPersistErrorKind, replace_catalog_and_persist};
use oatty_registry_gen::io::build_provider_contracts_for_commands;
use oatty_types::{CommandSpec, manifest::RegistryCatalog};
use oatty_util::sort_and_dedup_commands;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Request to apply deterministic command replacements to an existing catalog.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CatalogPatchApplyRequest {
    /// Existing catalog title to patch.
    pub target_catalog_title: String,
    /// Ordered replacement operations applied from first to last.
    pub operations: Vec<CatalogPatchOperation>,
    /// Fail when an operation does not match an existing command.
    pub fail_on_missing: bool,
    /// Fail when an operation matches multiple commands.
    pub fail_on_ambiguous: bool,
    /// Persist the patched catalog by replacing the existing catalog entry.
    pub overwrite_existing_catalog: bool,
}

impl Default for CatalogPatchApplyRequest {
    fn default() -> Self {
        Self {
            target_catalog_title: String::new(),
            operations: Vec::new(),
            fail_on_missing: true,
            fail_on_ambiguous: true,
            overwrite_existing_catalog: true,
        }
    }
}

impl CatalogPatchApplyRequest {
    /// Creates a patch request with default policy values.
    pub fn new(target_catalog_title: String, operations: Vec<CatalogPatchOperation>) -> Self {
        Self {
            target_catalog_title,
            operations,
            ..Self::default()
        }
    }

    /// Applies optional policy overrides while preserving defaults when omitted.
    pub fn with_policy_overrides(
        mut self,
        fail_on_missing: Option<bool>,
        fail_on_ambiguous: Option<bool>,
        overwrite_existing_catalog: Option<bool>,
    ) -> Self {
        if let Some(fail_on_missing) = fail_on_missing {
            self.fail_on_missing = fail_on_missing;
        }
        if let Some(fail_on_ambiguous) = fail_on_ambiguous {
            self.fail_on_ambiguous = fail_on_ambiguous;
        }
        if let Some(overwrite_existing_catalog) = overwrite_existing_catalog {
            self.overwrite_existing_catalog = overwrite_existing_catalog;
        }
        self
    }
}

/// Successful patch application metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CatalogPatchApplyResult {
    /// Catalog identifier that was patched.
    pub catalog_id: String,
    /// Number of requested operations.
    pub requested_operation_count: usize,
    /// Number of operations that were applied.
    pub applied_operation_count: usize,
    /// Number of commands in the patched manifest.
    pub final_command_count: usize,
    /// Number of provider contracts in the patched manifest.
    pub final_provider_contract_count: usize,
    /// Per-operation outcomes.
    pub operation_results: Vec<CatalogPatchOperationResult>,
    /// Patched catalog record when persisted.
    pub catalog: Option<RegistryCatalog>,
}

/// Single patch operation for replacing one command.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CatalogPatchOperation {
    /// Optional operation identifier for diagnostics.
    pub operation_id: Option<String>,
    /// Strict matching key used to find the target command.
    pub match_command: CatalogCommandMatchKey,
    /// Full replacement command specification.
    pub replacement_command: CommandSpec,
}

/// Stable key for matching target commands in a catalog manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CatalogCommandMatchKey {
    /// Command group (for example, `projects`).
    pub group: String,
    /// Command name (for example, `projects:list`).
    pub name: String,
    /// HTTP method (for example, `GET`).
    pub http_method: String,
    /// HTTP path (for example, `/v1/projects`).
    pub http_path: String,
}

/// Per-operation patch result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CatalogPatchOperationResult {
    /// Optional operation identifier.
    pub operation_id: Option<String>,
    /// Operation status.
    pub status: CatalogPatchOperationStatus,
    /// Number of commands matched by the operation key.
    pub matched_count: usize,
    /// Canonical identifier of the replaced command after patching.
    pub replaced_canonical_id: Option<String>,
    /// Optional detail message.
    pub message: Option<String>,
}

/// Patch operation status classification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CatalogPatchOperationStatus {
    /// Operation replaced a command.
    Applied,
    /// Operation was skipped due to non-fatal policy.
    Skipped,
}

/// Errors emitted while applying a catalog patch.
#[derive(Debug, Error)]
pub enum CatalogPatchApplyError {
    /// Target catalog was not found.
    #[error("catalog '{0}' not found")]
    CatalogNotFound(String),
    /// Target catalog has no in-memory manifest payload.
    #[error("catalog '{0}' has no manifest content")]
    MissingManifest(String),
    /// An operation did not match any command.
    #[error("operation {operation_index} target not found")]
    TargetNotFound { operation_index: usize },
    /// An operation matched multiple commands.
    #[error("operation {operation_index} target is ambiguous (matched {matched_count})")]
    TargetAmbiguous { operation_index: usize, matched_count: usize },
    /// The patch request requires overwrite persistence.
    #[error("overwrite_existing_catalog must be true to persist a patch")]
    OverwriteRequired,
    /// Registry replacement failed.
    #[error("failed to replace catalog '{catalog_id}': {message}")]
    ReplaceFailed { catalog_id: String, message: String },
    /// Registry insertion failed.
    #[error("failed to insert patched catalog '{catalog_id}': {message}")]
    InsertFailed { catalog_id: String, message: String },
    /// Registry config persistence failed.
    #[error("failed to save patched catalog '{catalog_id}': {message}")]
    SaveFailed { catalog_id: String, message: String },
    /// Patched catalog record could not be reloaded after save.
    #[error("patched catalog '{0}' is unavailable after save")]
    PersistedCatalogUnavailable(String),
}

/// Applies deterministic patch operations to an existing catalog manifest.
pub fn apply_catalog_patch(
    registry: &mut CommandRegistry,
    request: CatalogPatchApplyRequest,
) -> Result<CatalogPatchApplyResult, CatalogPatchApplyError> {
    if !request.overwrite_existing_catalog {
        return Err(CatalogPatchApplyError::OverwriteRequired);
    }

    let existing_catalog = get_catalog_by_title(registry, &request.target_catalog_title)
        .cloned()
        .ok_or_else(|| CatalogPatchApplyError::CatalogNotFound(request.target_catalog_title.clone()))?;
    let mut patched_catalog = existing_catalog.clone();
    let manifest = patched_catalog
        .manifest
        .as_mut()
        .ok_or_else(|| CatalogPatchApplyError::MissingManifest(request.target_catalog_title.clone()))?;

    let mut operation_results = Vec::with_capacity(request.operations.len());
    let mut applied_operation_count = 0usize;
    for (operation_index, operation) in request.operations.iter().enumerate() {
        let matching_indexes = find_matching_command_indexes(&manifest.commands, &operation.match_command);
        if matching_indexes.is_empty() {
            if request.fail_on_missing {
                return Err(CatalogPatchApplyError::TargetNotFound { operation_index });
            }
            operation_results.push(build_skipped_result(
                operation.operation_id.clone(),
                0,
                "target command not found".to_string(),
            ));
            continue;
        }
        if matching_indexes.len() > 1 {
            if request.fail_on_ambiguous {
                return Err(CatalogPatchApplyError::TargetAmbiguous {
                    operation_index,
                    matched_count: matching_indexes.len(),
                });
            }
            operation_results.push(build_skipped_result(
                operation.operation_id.clone(),
                matching_indexes.len(),
                "target command match is ambiguous".to_string(),
            ));
            continue;
        }

        let target_index = matching_indexes[0];
        manifest.commands[target_index] = operation.replacement_command.clone();
        applied_operation_count += 1;
        operation_results.push(CatalogPatchOperationResult {
            operation_id: operation.operation_id.clone(),
            status: CatalogPatchOperationStatus::Applied,
            matched_count: 1,
            replaced_canonical_id: Some(manifest.commands[target_index].canonical_id()),
            message: None,
        });
    }

    sort_and_dedup_commands(&mut manifest.commands);
    manifest.provider_contracts = build_provider_contracts_for_commands(&manifest.commands);

    replace_catalog_and_persist(registry, &request.target_catalog_title, patched_catalog.clone())
        .map_err(|error| map_catalog_persist_error_to_patch_error(&request.target_catalog_title, error.kind, error.message))?;
    let persisted_catalog = get_catalog_by_title(registry, &request.target_catalog_title)
        .cloned()
        .ok_or_else(|| CatalogPatchApplyError::PersistedCatalogUnavailable(request.target_catalog_title.clone()))?;

    Ok(CatalogPatchApplyResult {
        catalog_id: request.target_catalog_title,
        requested_operation_count: request.operations.len(),
        applied_operation_count,
        final_command_count: persisted_catalog
            .manifest
            .as_ref()
            .map(|manifest| manifest.commands.len())
            .unwrap_or(0),
        final_provider_contract_count: persisted_catalog
            .manifest
            .as_ref()
            .map(|manifest| manifest.provider_contracts.len())
            .unwrap_or(0),
        operation_results,
        catalog: Some(persisted_catalog),
    })
}

fn build_skipped_result(operation_id: Option<String>, matched_count: usize, message: String) -> CatalogPatchOperationResult {
    CatalogPatchOperationResult {
        operation_id,
        status: CatalogPatchOperationStatus::Skipped,
        matched_count,
        replaced_canonical_id: None,
        message: Some(message),
    }
}

fn find_matching_command_indexes(commands: &[CommandSpec], match_key: &CatalogCommandMatchKey) -> Vec<usize> {
    commands
        .iter()
        .enumerate()
        .filter_map(|(index, command)| {
            let http = command.http()?;
            let method_matches = http.method.eq_ignore_ascii_case(match_key.http_method.trim());
            let path_matches = http.path.trim() == match_key.http_path.trim();
            if command.group == match_key.group && command.name == match_key.name && method_matches && path_matches {
                return Some(index);
            }
            None
        })
        .collect()
}

fn map_catalog_persist_error_to_patch_error(catalog_id: &str, kind: CatalogPersistErrorKind, message: String) -> CatalogPatchApplyError {
    match kind {
        CatalogPersistErrorKind::Replace => CatalogPatchApplyError::ReplaceFailed {
            catalog_id: catalog_id.to_string(),
            message,
        },
        CatalogPersistErrorKind::Insert => CatalogPatchApplyError::InsertFailed {
            catalog_id: catalog_id.to_string(),
            message,
        },
        CatalogPersistErrorKind::Save => CatalogPatchApplyError::SaveFailed {
            catalog_id: catalog_id.to_string(),
            message,
        },
    }
}

fn get_catalog_by_title<'catalog>(registry: &'catalog CommandRegistry, catalog_title: &str) -> Option<&'catalog RegistryCatalog> {
    registry
        .config
        .catalogs
        .as_ref()
        .and_then(|catalogs| catalogs.iter().find(|catalog| catalog.title == catalog_title))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RegistryConfig;
    use indexmap::IndexMap;
    use oatty_types::{command::HttpCommandSpec, manifest::RegistryManifest};
    use std::{fs, time};

    fn build_command(group: &str, name: &str, method: &str, path: &str) -> CommandSpec {
        CommandSpec::new_http(
            group.to_string(),
            name.to_string(),
            "summary".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new(method, path, None, None),
            0,
        )
    }

    fn sample_registry() -> CommandRegistry {
        let command = build_command("apps", "apps:list", "GET", "/apps");
        let manifest = RegistryManifest {
            commands: vec![command],
            provider_contracts: IndexMap::new(),
            vendor: "apps".to_string(),
        };
        let manifest_bytes: Vec<u8> = manifest.clone().try_into().expect("manifest serializes");
        let manifest_path = unique_manifest_path();
        fs::write(&manifest_path, manifest_bytes).expect("manifest file");
        let catalog = RegistryCatalog {
            title: "Apps".to_string(),
            description: String::new(),
            vendor: Some("apps".to_string()),
            manifest_path: manifest_path.to_string_lossy().to_string(),
            import_source: None,
            import_source_type: None,
            headers: Default::default(),
            base_urls: vec!["https://example.com".to_string()],
            base_url_index: 0,
            manifest: Some(manifest),
            is_enabled: true,
        };
        CommandRegistry::from_registry_config(RegistryConfig {
            catalogs: Some(vec![catalog]),
        })
        .expect("registry")
    }

    fn unique_manifest_path() -> std::path::PathBuf {
        let nanos = time::SystemTime::now().duration_since(time::UNIX_EPOCH).expect("time").as_nanos();
        std::env::temp_dir().join(format!("oatty-catalog-patch-{nanos}.bin"))
    }

    #[test]
    fn apply_catalog_patch_replaces_matching_command() {
        let mut registry = sample_registry();
        let replacement = build_command("apps", "apps:list", "GET", "/v2/apps");
        let result = apply_catalog_patch(
            &mut registry,
            CatalogPatchApplyRequest {
                target_catalog_title: "Apps".to_string(),
                operations: vec![CatalogPatchOperation {
                    operation_id: Some("replace-apps-list".to_string()),
                    match_command: CatalogCommandMatchKey {
                        group: "apps".to_string(),
                        name: "apps:list".to_string(),
                        http_method: "GET".to_string(),
                        http_path: "/apps".to_string(),
                    },
                    replacement_command: replacement,
                }],
                fail_on_missing: true,
                fail_on_ambiguous: true,
                overwrite_existing_catalog: true,
            },
        )
        .expect("patch succeeds");

        assert_eq!(result.applied_operation_count, 1);
        let command = registry.find_by_group_and_cmd_ref("apps", "apps:list").expect("patched command");
        assert_eq!(command.http().expect("http command").path, "/v2/apps");
    }

    #[test]
    fn apply_catalog_patch_fails_when_target_missing() {
        let mut registry = sample_registry();
        let error = apply_catalog_patch(
            &mut registry,
            CatalogPatchApplyRequest {
                target_catalog_title: "Apps".to_string(),
                operations: vec![CatalogPatchOperation {
                    operation_id: None,
                    match_command: CatalogCommandMatchKey {
                        group: "apps".to_string(),
                        name: "apps:get".to_string(),
                        http_method: "GET".to_string(),
                        http_path: "/apps/{id}".to_string(),
                    },
                    replacement_command: build_command("apps", "apps:get", "GET", "/apps/{id}"),
                }],
                fail_on_missing: true,
                fail_on_ambiguous: true,
                overwrite_existing_catalog: true,
            },
        )
        .expect_err("missing should fail");

        assert!(matches!(error, CatalogPatchApplyError::TargetNotFound { .. }));
    }
}
