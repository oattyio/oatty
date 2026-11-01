//! Validate Cargo, npm, and release tag versions for a release.
//!
//! Ported from the inline Python step that previously lived in the release
//! workflow's `metadata` job. The check is fail-closed: a release must not
//! proceed when the workspace manifests or the tag disagree.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use toml::Table;

use crate::context::project_root;

/// The npm-facing package manifest fields used by the check.
#[derive(Debug, Deserialize)]
struct NpmManifest {
    /// The npm-distributed version, for example `0.1.12`.
    version: String,
}

/// The `[workspace.package]` table of the workspace manifest.
#[derive(Debug, Deserialize)]
struct WorkspacePackage {
    /// The workspace-wide crate version, for example `0.1.12`.
    version: String,
}

/// Read the npm-distributed version from the root package manifest.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
///
/// # Returns
///
/// Returns the `version` field of `package.json`.
fn npm_version(root: &std::path::Path) -> Result<String> {
    let manifest = std::fs::read_to_string(root.join("package.json")).context("failed to read package.json")?;
    let parsed: NpmManifest = serde_json::from_str(&manifest).context("failed to parse package.json")?;
    Ok(parsed.version)
}

/// Read the workspace crate version from the manifest.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
///
/// # Returns
///
/// Returns the `[workspace.package].version` field of `Cargo.toml`.
fn cargo_version(root: &std::path::Path) -> Result<String> {
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).context("failed to read Cargo.toml")?;
    let table: Table = toml::from_str(&manifest).context("failed to parse Cargo.toml")?;
    let workspace = table
        .get("workspace")
        .and_then(|value| value.as_table())
        .context("Cargo.toml is missing the workspace table")?;
    let package: WorkspacePackage = workspace
        .get("package")
        .context("Cargo.toml is missing workspace.package")?
        .clone()
        .try_into()
        .context("failed to parse workspace.package")?;
    Ok(package.version)
}

/// Validate that Cargo, npm, and the release tag agree on the version.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `release_reference` - The Git release reference, for example a tag ref.
/// * `release_tag` - The tag name carried by the release reference, for
///   example `v0.1.12`.
///
/// # Returns
///
/// Returns the aligned version when the workspace and tag versions match.
///
/// # Errors
///
/// Returns an error when the Cargo version differs from the npm version, or
/// when a tag reference carries a tag other than `v<version>`.
fn verified_release_version(root: &std::path::Path, release_reference: &str, release_tag: &str) -> Result<String> {
    let npm_package_version = npm_version(root)?;
    let workspace_cargo_version = cargo_version(root)?;
    if workspace_cargo_version != npm_package_version {
        bail!("Cargo version {workspace_cargo_version} does not match npm version {npm_package_version}");
    }
    if release_reference.starts_with("refs/tags/") {
        let expected_tag = format!("v{npm_package_version}");
        if release_tag != expected_tag {
            bail!("Release tag must be {expected_tag}, got {release_tag}");
        }
    }
    Ok(npm_package_version)
}

/// Validate the release manifests and tag in the release environment.
///
/// The environment must provide `RELEASE_REF` and `RELEASE_TAG`; a tag
/// reference must equal `v` plus the package version.
///
/// # Returns
///
/// Returns `Ok(())` when the workspace and tag versions are aligned.
///
/// # Errors
///
/// Returns an error when the required environment variables are missing or
/// the versions disagree.
pub(crate) fn verify_release_versions() -> Result<()> {
    let release_reference = std::env::var("RELEASE_REF").context("RELEASE_REF is required")?;
    let release_tag = std::env::var("RELEASE_TAG").context("RELEASE_TAG is required")?;
    let root = project_root()?;
    let version = verified_release_version(&root, &release_reference, &release_tag)?;
    println!("Release versions are aligned: Cargo, npm, and tag all reference {version}.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Write a minimal workspace checkout with the given aligned version.
    fn prepare_workspace(root: &std::path::Path, version: &str) {
        fs::write(
            root.join("Cargo.toml"),
            format!("[workspace]\nmembers = []\n[workspace.package]\nversion = \"{version}\"\n"),
        )
        .unwrap();
        fs::write(root.join("package.json"), format!(r#"{{"version": "{version}"}}"#)).unwrap();
    }

    #[test]
    fn aligned_versions_and_matching_tag_are_accepted() {
        let directory = tempfile::tempdir().unwrap();
        prepare_workspace(directory.path(), "1.2.3");
        let version = verified_release_version(directory.path(), "refs/tags/v1.2.3", "v1.2.3").unwrap();
        assert_eq!(version, "1.2.3");
    }

    #[test]
    fn misaligned_manifest_versions_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("Cargo.toml"),
            "[workspace]\n[workspace.package]\nversion = \"0.2.0\"\n",
        )
        .unwrap();
        fs::write(directory.path().join("package.json"), r#"{"version": "0.1.0"}"#).unwrap();
        let outcome = verified_release_version(directory.path(), "refs/tags/v0.1.0", "v0.1.0");
        assert!(outcome.unwrap_err().to_string().contains("does not match npm version"));
    }

    #[test]
    fn wrong_tag_reports_the_expected_version() {
        let directory = tempfile::tempdir().unwrap();
        prepare_workspace(directory.path(), "1.2.3");
        let outcome = verified_release_version(directory.path(), "refs/tags/v9.9.9", "v9.9.9");
        assert!(outcome.unwrap_err().to_string().contains("Release tag must be v1.2.3"));
    }

    #[test]
    fn non_tag_references_skip_tag_validation() {
        let directory = tempfile::tempdir().unwrap();
        prepare_workspace(directory.path(), "1.2.3");
        let version = verified_release_version(directory.path(), "refs/heads/main", "ignored").unwrap();
        assert_eq!(version, "1.2.3");
    }
}
