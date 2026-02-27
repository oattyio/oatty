//! Shared catalog persistence helpers.
//!
//! This module centralizes catalog replace/insert + config persistence behavior
//! so import and patch flows reuse one implementation path.

use crate::CommandRegistry;
use anyhow::{Result, anyhow};
use heck::ToSnakeCase;
use oatty_types::manifest::RegistryCatalog;
use postcard::to_stdvec;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Error category emitted by catalog persistence operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CatalogPersistErrorKind {
    /// Existing catalog removal failed during overwrite.
    Replace,
    /// Inserting a catalog into registry state failed.
    Insert,
    /// Saving registry config to disk failed.
    Save,
}

/// Structured catalog persistence error.
#[derive(Debug)]
pub(crate) struct CatalogPersistError {
    /// Failure category.
    pub kind: CatalogPersistErrorKind,
    /// Human-readable failure message.
    pub message: String,
}

/// Inserts a catalog and persists registry config.
pub(crate) fn insert_catalog_and_persist(registry: &mut CommandRegistry, catalog: RegistryCatalog) -> Result<(), CatalogPersistError> {
    registry.insert_catalog(catalog).map_err(|error| CatalogPersistError {
        kind: CatalogPersistErrorKind::Insert,
        message: error.to_string(),
    })?;
    let inserted_catalog_title = registry
        .config
        .catalogs
        .as_ref()
        .and_then(|catalogs| catalogs.last())
        .map(|catalog| catalog.title.clone());
    persist_catalog_manifest_by_title(registry, inserted_catalog_title.as_deref()).map_err(|error| CatalogPersistError {
        kind: CatalogPersistErrorKind::Save,
        message: error.to_string(),
    })?;
    registry.config.save().map_err(|error| CatalogPersistError {
        kind: CatalogPersistErrorKind::Save,
        message: error.to_string(),
    })?;
    Ok(())
}

/// Replaces an existing catalog with a new catalog and persists registry config.
pub(crate) fn replace_catalog_and_persist(
    registry: &mut CommandRegistry,
    catalog_id: &str,
    replacement_catalog: RegistryCatalog,
) -> Result<(), CatalogPersistError> {
    let removed_catalog = remove_catalog_for_overwrite(registry, catalog_id).map_err(|error| CatalogPersistError {
        kind: CatalogPersistErrorKind::Replace,
        message: error.to_string(),
    })?;
    let manifest_backup = backup_manifest_bytes(&removed_catalog.manifest_path);

    if let Err(error) = registry.insert_catalog(replacement_catalog) {
        let _ = restore_catalog_after_failed_replace(registry, removed_catalog);
        return Err(CatalogPersistError {
            kind: CatalogPersistErrorKind::Insert,
            message: error.to_string(),
        });
    }
    if let Err(error) = persist_catalog_manifest_by_title(registry, Some(catalog_id)) {
        restore_manifest_backup(&removed_catalog.manifest_path, manifest_backup.as_deref());
        let _ = remove_catalog_entry_without_manifest_delete(registry, catalog_id);
        let _ = restore_catalog_after_failed_replace(registry, removed_catalog);
        return Err(CatalogPersistError {
            kind: CatalogPersistErrorKind::Save,
            message: error.to_string(),
        });
    }

    if let Err(error) = registry.config.save() {
        restore_manifest_backup(&removed_catalog.manifest_path, manifest_backup.as_deref());
        let _ = remove_catalog_entry_without_manifest_delete(registry, catalog_id);
        let _ = restore_catalog_after_failed_replace(registry, removed_catalog);
        return Err(CatalogPersistError {
            kind: CatalogPersistErrorKind::Save,
            message: error.to_string(),
        });
    }

    cleanup_replaced_manifest_file_if_orphan(registry, catalog_id, &removed_catalog.manifest_path);
    Ok(())
}

fn remove_catalog_for_overwrite(registry: &mut CommandRegistry, catalog_id: &str) -> Result<RegistryCatalog> {
    let removed_catalog_snapshot = registry
        .config
        .catalogs
        .as_ref()
        .and_then(|catalogs| catalogs.iter().find(|catalog| catalog.title == catalog_id))
        .cloned()
        .ok_or_else(|| anyhow!("catalog not found"))?;

    registry.disable_catalog(catalog_id).map_err(|error| anyhow!(error.to_string()))?;

    let Some(catalogs) = registry.config.catalogs.as_mut() else {
        return Err(anyhow!("no catalogs configured"));
    };
    let Some(index) = catalogs.iter().position(|catalog| catalog.title == catalog_id) else {
        return Err(anyhow!("catalog not found"));
    };
    catalogs.remove(index);
    reindex_catalog_identifiers(registry);
    Ok(removed_catalog_snapshot)
}

fn restore_catalog_after_failed_replace(registry: &mut CommandRegistry, removed_catalog: RegistryCatalog) -> Result<()> {
    registry
        .insert_catalog(removed_catalog)
        .map_err(|error| anyhow!("failed to restore replaced catalog: {error}"))
}

fn remove_catalog_entry_without_manifest_delete(registry: &mut CommandRegistry, catalog_id: &str) -> Result<()> {
    registry.disable_catalog(catalog_id).map_err(|error| anyhow!(error.to_string()))?;
    let Some(catalogs) = registry.config.catalogs.as_mut() else {
        return Err(anyhow!("no catalogs configured"));
    };
    let Some(index) = catalogs.iter().position(|catalog| catalog.title == catalog_id) else {
        return Err(anyhow!("catalog not found"));
    };
    catalogs.remove(index);
    reindex_catalog_identifiers(registry);
    Ok(())
}

fn backup_manifest_bytes(manifest_path: &str) -> Option<Vec<u8>> {
    let path = Path::new(manifest_path);
    if !path.exists() {
        return None;
    }
    std::fs::read(path).ok()
}

fn restore_manifest_backup(manifest_path: &str, backup: Option<&[u8]>) {
    let Some(backup) = backup else {
        return;
    };
    let _ = std::fs::write(manifest_path, backup);
}

fn cleanup_replaced_manifest_file_if_orphan(registry: &CommandRegistry, catalog_id: &str, old_manifest_path: &str) {
    let current_manifest_path = registry
        .config
        .catalogs
        .as_ref()
        .and_then(|catalogs| catalogs.iter().find(|catalog| catalog.title == catalog_id))
        .map(|catalog| catalog.manifest_path.as_str());
    if current_manifest_path.is_some_and(|current_path| current_path == old_manifest_path) {
        return;
    }
    let old_path = Path::new(old_manifest_path);
    if old_path.exists() {
        let _ = std::fs::remove_file(old_path);
    }
}

fn persist_catalog_manifest_by_title(registry: &mut CommandRegistry, catalog_title: Option<&str>) -> Result<()> {
    let Some(catalog_title) = catalog_title else {
        return Ok(());
    };
    let Some(catalogs) = registry.config.catalogs.as_mut() else {
        return Ok(());
    };
    let Some(catalog) = catalogs.iter_mut().find(|catalog| catalog.title == catalog_title) else {
        return Ok(());
    };
    persist_catalog_manifest(catalog)
}

fn persist_catalog_manifest(catalog: &mut RegistryCatalog) -> Result<()> {
    let Some(manifest) = catalog.manifest.as_ref() else {
        return Ok(());
    };
    let manifest_bytes = to_stdvec(manifest).map_err(|error| anyhow!("failed to serialize manifest: {error}"))?;
    let mut manifest_path = catalog.manifest_path.clone();
    if manifest_path.trim().is_empty() {
        let catalogs_path = crate::default_catalogs_path();
        std::fs::create_dir_all(&catalogs_path)?;
        manifest_path = catalogs_path
            .join(format!("{}.bin", catalog.title.to_snake_case()))
            .to_string_lossy()
            .to_string();
    } else if let Some(parent) = Path::new(&manifest_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_manifest_bytes_atomically(&manifest_path, &manifest_bytes)?;
    catalog.manifest_path = manifest_path;
    Ok(())
}

fn write_manifest_bytes_atomically(manifest_path: &str, manifest_bytes: &[u8]) -> Result<()> {
    let manifest_path_ref = Path::new(manifest_path);
    let parent_directory = manifest_path_ref.parent().unwrap_or_else(|| Path::new("."));
    let temp_manifest_path = build_temp_manifest_path(parent_directory);

    let mut temp_file = File::create(&temp_manifest_path)?;
    temp_file.write_all(manifest_bytes)?;
    temp_file.sync_all()?;
    drop(temp_file);

    std::fs::rename(&temp_manifest_path, manifest_path_ref)?;
    Ok(())
}

fn build_temp_manifest_path(parent_directory: &Path) -> std::path::PathBuf {
    let process_identifier = std::process::id();
    let timestamp_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    parent_directory.join(format!(".catalog-manifest-{process_identifier}-{timestamp_nanos}.tmp"))
}

fn reindex_catalog_identifiers(registry: &mut CommandRegistry) {
    let mut canonical_id_to_catalog_index = HashMap::<String, usize>::new();
    if let Some(catalogs) = registry.config.catalogs.as_mut() {
        for (catalog_index, catalog) in catalogs.iter_mut().enumerate() {
            if let Some(manifest) = catalog.manifest.as_mut() {
                for command in &mut manifest.commands {
                    command.catalog_identifier = catalog_index;
                    canonical_id_to_catalog_index.insert(command.canonical_id(), catalog_index);
                }
            }
        }
    }
    for command in &mut registry.commands {
        if let Some(index) = canonical_id_to_catalog_index.get(&command.canonical_id()) {
            command.catalog_identifier = *index;
        }
    }
}
