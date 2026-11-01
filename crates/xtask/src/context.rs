//! Shared workspace context helpers for the auxiliary task modules.

use std::path::PathBuf;

use anyhow::{Context, Result};

/// Locate the workspace root directory.
///
/// The auxiliary task modules operate on repository files relative to the
/// workspace root. It is resolved from the crate's own location rather than
/// the current working directory so that `cargo xtask` tasks behave the same
/// way regardless of where they are invoked from.
///
/// # Returns
///
/// Returns the absolute path of the workspace root.
///
/// # Errors
///
/// Returns an error when the crate's build-time manifest location cannot be
/// resolved to a directory containing a `Cargo.toml` workspace manifest.
pub fn project_root() -> Result<PathBuf> {
    let manifest_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_directory
        .parent()
        .and_then(|parent| parent.parent())
        .context("failed to resolve the workspace root from the crate location")?
        .to_path_buf();
    if !root.join("Cargo.toml").is_file() {
        anyhow::bail!("resolved workspace root does not contain Cargo.toml: {}", root.display());
    }
    Ok(root)
}
