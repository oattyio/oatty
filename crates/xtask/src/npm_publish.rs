//! Validate all npm artifacts, publish native payloads, then publish the launcher.
//!
//! This module is a behavior-preserving port of `scripts/publish-npm.py`. Nothing
//! is built or repacked here: the directory supplied by the caller must already
//! contain the locally validated release tarballs. Publication is fail-closed:
//!
//! * Every artifact is re-verified before any registry change, so a stale legal
//!   file, a changed executable, or an unexpected tarball aborts the whole run.
//! * Every version collision, including the umbrella, is detected before the
//!   first publish. An already-published version is accepted only when its
//!   registry integrity and release metadata match these exact local artifacts.
//! * Network and authentication failures are never interpreted as "not yet
//!   published"; only a structured npm `E404` means a version is absent.
//! * Native payloads are published before their exact-version umbrella, and the
//!   umbrella only appears after every payload is visibly published.
//!
//! # Testability
//!
//! Licensing verification and the npm toolchain are reachable only through
//! [`crate::licenses::verify`], [`crate::npm_package::npm_command`], and a child
//! process. [`PublishHooks`] injects all of them, including the retry delay, so
//! unit tests exercise ordering against a deterministic in-process fake npm.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde_json::Value;
use sha2::{Digest as _, Sha512};

/// The exact failure message for an unexpected artifact directory listing.
const EXPECTED_ARTIFACT_SET_MESSAGE: &str = "Expected exactly four native payload tarballs and one umbrella tarball";

/// The artifact file-name prefix of the umbrella launcher package.
const UMBRELLA_PREFIX: &str = "oatty-";

/// The extension every release tarball carries.
const ARTIFACT_EXTENSION: &str = ".tgz";

/// The number of registry lookups attempted after a publish.
const PUBLISH_VISIBILITY_ATTEMPTS: usize = 5;

/// The delay between registry lookups after a publish.
const PUBLISH_RETRY_DELAY: Duration = Duration::from_secs(1);

/// The registry manifest keys that must match the local artifact exactly.
///
/// The order is the Python tuple order so the first reported difference stays
/// stable.
const REGISTRY_MANIFEST_KEYS: [&str; 13] = [
    "name",
    "version",
    "license",
    "repository",
    "os",
    "cpu",
    "libc",
    "oattyTarget",
    "bin",
    "engines",
    "optionalDependencies",
    "dependencies",
    "scripts",
];

/// Publish all validated payloads before exposing their exact-version umbrella.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `directory` - The directory containing validated release tarballs.
///
/// # Errors
///
/// Returns an error when any artifact fails validation, an existing version has
/// different bytes or metadata, a publication fails, or a publication cannot be
/// confirmed as visible in the registry.
pub(crate) fn publish(root: &Path, directory: &Path) -> Result<()> {
    publish_with(root, directory, &PublishHooks::production())
}

/// One validated release artifact together with the manifest parsed from inside it.
struct ReleaseArtifact {
    /// The local tarball.
    archive: PathBuf,
    /// The `package.json` parsed from the tarball.
    manifest: Value,
}

/// The captured outcome of one npm child process.
///
/// This mirrors [`std::process::Output`] closely enough that the production
/// runner can be swapped for an in-process fake without touching callers.
struct NpmOutcome {
    /// Whether npm reported a successful exit status.
    exit_success: bool,
    /// The standard output captured from npm.
    standard_output: Vec<u8>,
    /// The standard error captured from npm.
    standard_error: String,
}

/// The injectable signature of one npm invocation.
///
/// The first argument is the npm argv prefix, the second the command and its
/// arguments, matching how the production runner builds a child process.
type NpmRunner<'a> = dyn Fn(&[String], &[&str]) -> Result<NpmOutcome> + 'a;

/// The replaceable collaborators npm publication depends on.
///
/// Production code uses [`PublishHooks::production`]; unit tests substitute a
/// no-op licensing verifier, a fixed npm argv prefix, an in-process npm runner,
/// and a no-wait retry delay.
struct PublishHooks<'a> {
    /// Rejects missing, stale, edited, or extra licensing materials.
    verify_licenses: &'a dyn Fn(&Path) -> Result<()>,
    /// Returns the npm argv prefix to execute.
    npm_command: &'a dyn Fn() -> Result<Vec<String>>,
    /// Executes the npm argv prefix followed by the supplied arguments.
    run_npm: &'a NpmRunner<'a>,
    /// Waits for the supplied delay between registry lookups.
    sleep: &'a dyn Fn(Duration),
}

impl PublishHooks<'static> {
    /// Build the hooks used outside tests.
    ///
    /// # Returns
    ///
    /// Returns hooks backed by the real licensing module and the real npm process.
    fn production() -> Self {
        Self {
            verify_licenses: &crate::licenses::verify,
            npm_command: &crate::npm_package::npm_command,
            run_npm: &run_npm_command,
            sleep: &sleep_before_retry,
        }
    }
}

/// Execute npm and capture its output.
///
/// # Arguments
///
/// * `prefix` - The npm argv prefix, whose first element is the program.
/// * `arguments` - The npm command and its arguments.
///
/// # Returns
///
/// Returns the captured exit status and output.
///
/// # Errors
///
/// Returns an error when the prefix is empty or the child process cannot be
/// spawned.
fn run_npm_command(prefix: &[String], arguments: &[&str]) -> Result<NpmOutcome> {
    let (program, program_arguments) = prefix.split_first().ok_or_else(|| anyhow!("The npm command is empty"))?;
    let output = Command::new(program)
        .args(program_arguments)
        .args(arguments)
        .output()
        .with_context(|| format!("failed to execute {program}"))?;
    Ok(NpmOutcome {
        exit_success: output.status.success(),
        standard_output: output.stdout,
        standard_error: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// Wait before the next registry visibility lookup.
///
/// # Arguments
///
/// * `delay` - The delay requested by the visibility retry loop.
fn sleep_before_retry(delay: Duration) {
    std::thread::sleep(delay);
}

/// Compute the npm sha512 SRI value for the unchanged local archive bytes.
///
/// # Arguments
///
/// * `archive` - The tarball whose published integrity is being computed.
///
/// # Returns
///
/// Returns the `sha512-<base64>` subresource-integrity string.
///
/// # Errors
///
/// Returns an error when the archive cannot be read.
fn artifact_integrity(archive: &Path) -> Result<String> {
    let bytes = fs::read(archive).with_context(|| format!("failed to read {}", archive.display()))?;
    Ok(format!("sha512-{}", BASE64_STANDARD.encode(Sha512::digest(&bytes))))
}

/// Return exact registry metadata, or `None` only for a structured npm `E404`.
///
/// # Arguments
///
/// * `name` - The package name to look up, for example `@oatty/cli-linux-x64`.
/// * `version` - The exact release version to look up.
/// * `hooks` - The npm collaborators, injectable for tests.
///
/// # Returns
///
/// Returns the published manifest for the exact version, or `None` when npm
/// reported a structured `E404`.
///
/// # Errors
///
/// Returns an error for every other non-zero exit status, for unparseable output,
/// and for a response that is not a JSON object.
fn registry_manifest(name: &str, version: &str, hooks: &PublishHooks<'_>) -> Result<Option<Value>> {
    let prefix = (hooks.npm_command)()?;
    let specifier = format!("{name}@{version}");
    let outcome = (hooks.run_npm)(&prefix, &["view", specifier.as_str(), "--json"])?;
    if !outcome.exit_success {
        if is_not_found_response(&outcome.standard_output) {
            return Ok(None);
        }
        bail!("npm view failed for {name}@{version}: {}", failure_detail(&outcome));
    }

    let mut metadata: Value = serde_json::from_slice(&outcome.standard_output)
        .with_context(|| format!("failed to parse the npm view response for {name}@{version}"))?;
    // npm 12 keeps the version result array; npm 11 unwraps a single version.
    if let Value::Array(entries) = &metadata
        && entries.len() == 1
    {
        metadata = entries[0].clone();
    }
    if !metadata.is_object() {
        bail!("Unexpected registry response for {name}@{version}");
    }
    Ok(Some(metadata))
}

/// Report whether npm's standard output carries a structured `E404` marker.
///
/// # Arguments
///
/// * `standard_output` - The raw standard output of a failed `npm view`.
///
/// # Returns
///
/// Returns `true` only when the output parses as JSON with `error.code == "E404"`.
fn is_not_found_response(standard_output: &[u8]) -> bool {
    serde_json::from_slice::<Value>(standard_output)
        .ok()
        .as_ref()
        .and_then(|payload| payload.get("error"))
        .and_then(|error| error.get("code"))
        .and_then(Value::as_str)
        == Some("E404")
}

/// Return the most useful diagnostic an npm invocation produced.
///
/// Standard error wins when present, matching `result.stderr.strip() or
/// result.stdout.strip()`.
///
/// # Arguments
///
/// * `outcome` - The failed npm invocation.
///
/// # Returns
///
/// Returns the trimmed standard error, or the trimmed standard output when the
/// standard error is empty.
fn failure_detail(outcome: &NpmOutcome) -> String {
    let standard_error = outcome.standard_error.trim();
    if standard_error.is_empty() {
        String::from_utf8_lossy(&outcome.standard_output).trim().to_owned()
    } else {
        standard_error.to_owned()
    }
}

/// Reject an already-published version with different bytes or metadata.
///
/// # Arguments
///
/// * `archive` - The local tarball whose integrity must match.
/// * `expected` - The manifest parsed from the local tarball.
/// * `registry` - The manifest the registry reports for the exact version.
///
/// # Errors
///
/// Returns an error naming the first differing field, or when the published
/// integrity differs from the local archive bytes.
fn verify_registry(archive: &Path, expected: &Value, registry: &Value) -> Result<()> {
    let (name, version) = manifest_identity(expected)?;
    let published_integrity = registry.get("dist").and_then(|dist| dist.get("integrity"));
    if published_integrity != Some(&Value::String(artifact_integrity(archive)?)) {
        bail!("Published integrity differs for {name}@{version}");
    }
    for key in REGISTRY_MANIFEST_KEYS {
        if manifest_field(registry, key) != manifest_field(expected, key) {
            bail!("Published {key} differs for {name}@{version}");
        }
    }
    Ok(())
}

/// Read one manifest field, treating an explicit `null` as an absent field.
///
/// Python's `dict.get` cannot distinguish a missing key from a JSON `null`, so
/// the comparison has to normalize both to absence as well.
///
/// # Arguments
///
/// * `manifest` - The manifest to read.
/// * `key` - The field name to look up.
///
/// # Returns
///
/// Returns the field value, or `None` when it is absent or `null`.
fn manifest_field<'a>(manifest: &'a Value, key: &str) -> Option<&'a Value> {
    manifest.get(key).filter(|value| !value.is_null())
}

/// Publish once, or verify an existing version; require confirmed registry visibility.
///
/// # Arguments
///
/// * `archive` - The local tarball to publish.
/// * `manifest` - The manifest parsed from the local tarball.
/// * `hooks` - The npm collaborators, injectable for tests.
///
/// # Errors
///
/// Returns an error when the publish fails, when an existing version differs, or
/// when the version never becomes visible in the registry.
fn ensure_published(archive: &Path, manifest: &Value, hooks: &PublishHooks<'_>) -> Result<()> {
    let (name, version) = manifest_identity(manifest)?;
    if let Some(registry) = registry_manifest(&name, &version, hooks)? {
        return verify_registry(archive, manifest, &registry);
    }

    publish_artifact(archive, &name, &version, hooks)?;
    for attempt in 0..PUBLISH_VISIBILITY_ATTEMPTS {
        if let Some(registry) = registry_manifest(&name, &version, hooks)? {
            return verify_registry(archive, manifest, &registry);
        }
        if attempt + 1 < PUBLISH_VISIBILITY_ATTEMPTS {
            (hooks.sleep)(PUBLISH_RETRY_DELAY);
        }
    }
    bail!("Published package not visible: {name}@{version}")
}

/// Publish one tarball with provenance and without lifecycle scripts.
///
/// # Arguments
///
/// * `archive` - The local tarball to publish.
/// * `name` - The package name, used only for diagnostics.
/// * `version` - The package version, used only for diagnostics.
/// * `hooks` - The npm collaborators, injectable for tests.
///
/// # Errors
///
/// Returns an error when the archive cannot be resolved or npm exits non-zero.
fn publish_artifact(archive: &Path, name: &str, version: &str, hooks: &PublishHooks<'_>) -> Result<()> {
    // npm cannot resolve `\\?\`-prefixed paths (npm turns `\\?\D:\...` into
    // `file:D:/?/D/...`, see issue #97), so normalize before building npm argv.
    let resolved = fs::canonicalize(archive)
        .map(|resolved| crate::paths::normalize_windows_extended_length_path(&resolved))
        .with_context(|| format!("failed to resolve {}", archive.display()))?;
    let resolved = resolved.to_string_lossy().into_owned();
    let prefix = (hooks.npm_command)()?;
    let outcome = (hooks.run_npm)(
        &prefix,
        &[
            "publish",
            resolved.as_str(),
            "--provenance",
            "--access",
            "public",
            "--ignore-scripts",
        ],
    )?;
    if !outcome.exit_success {
        bail!("npm publish failed for {name}@{version}: {}", failure_detail(&outcome));
    }
    Ok(())
}

/// Read the package name and version a packed manifest declares.
///
/// # Arguments
///
/// * `manifest` - A validated packed manifest.
///
/// # Returns
///
/// Returns the `(name, version)` pair.
///
/// # Errors
///
/// Returns an error when either field is missing or not a string.
fn manifest_identity(manifest: &Value) -> Result<(String, String)> {
    let name = manifest
        .get("name")
        .and_then(Value::as_str)
        .context("Packed manifest must declare a name")?;
    let version = manifest
        .get("version")
        .and_then(Value::as_str)
        .context("Packed manifest must declare a version")?;
    Ok((name.to_owned(), version.to_owned()))
}

/// Validate the complete artifact set a release directory must contain.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `directory` - The directory containing the release tarballs.
/// * `verify_licenses` - Rejects missing, stale, or edited licensing materials;
///   injectable so tests can bypass the real pipeline.
///
/// # Returns
///
/// Returns every artifact in publication order: the native payloads followed by
/// the umbrella, each with the manifest parsed from inside the tarball.
///
/// # Errors
///
/// Returns an error when licensing verification fails, the directory does not
/// contain exactly the expected tarball set, or any artifact fails validation.
fn collect_artifacts_with(root: &Path, directory: &Path, verify_licenses: &dyn Fn(&Path) -> Result<()>) -> Result<Vec<ReleaseArtifact>> {
    verify_licenses(root)?;
    let root_manifest = crate::npm_package::root_manifest(root)?;
    let version = root_manifest
        .get("version")
        .and_then(Value::as_str)
        .context("Root package manifest must declare a version")?
        .to_owned();

    let platforms = crate::npm_package::load_platforms(root)?;
    let mut releases = Vec::with_capacity(platforms.len() + 1);
    for platform in platforms.values() {
        releases.push((directory.join(native_artifact_name(platform, &version)), Some(platform)));
    }
    releases.push((directory.join(umbrella_artifact_name(&version)), None));

    let expected_names: BTreeSet<String> = releases
        .iter()
        .filter_map(|(archive, _)| archive.file_name().map(|name| name.to_string_lossy().into_owned()))
        .collect();
    if expected_names != tarball_names(directory)? {
        bail!("{EXPECTED_ARTIFACT_SET_MESSAGE}");
    }

    let mut verified = Vec::with_capacity(releases.len());
    for (archive, platform) in releases {
        let manifest = crate::npm_package::verify_artifact(root, &archive, platform)?;
        verified.push(ReleaseArtifact { archive, manifest });
    }
    Ok(verified)
}

/// Return the tarball file name of one native payload package.
///
/// npm strips the scope marker and flattens the scope separator, so
/// `@oatty/cli-linux-x64` becomes `oatty-cli-linux-x64-<version>.tgz`.
///
/// # Arguments
///
/// * `platform` - The platform contract being published.
/// * `version` - The exact release version.
///
/// # Returns
///
/// Returns the artifact file name.
fn native_artifact_name(platform: &crate::npm_package::Platform, version: &str) -> String {
    format!("{}-{version}{ARTIFACT_EXTENSION}", npm_artifact_name(&platform.package_name))
}

/// Return the tarball file name of the umbrella launcher package.
///
/// # Arguments
///
/// * `version` - The exact release version.
///
/// # Returns
///
/// Returns the artifact file name.
fn umbrella_artifact_name(version: &str) -> String {
    format!("{UMBRELLA_PREFIX}{version}{ARTIFACT_EXTENSION}")
}

/// Convert a package name into the artifact prefix npm uses for tarball names.
///
/// # Arguments
///
/// * `package_name` - The package name, for example `@oatty/cli-darwin-x64`.
///
/// # Returns
///
/// Returns the flattened artifact prefix, for example `oatty-cli-darwin-x64`.
fn npm_artifact_name(package_name: &str) -> String {
    package_name.replace('@', "").replace('/', "-")
}

/// List the tarball-looking entries in a release directory.
///
/// This mirrors Python's `directory.glob('*.tgz')`: any entry name ending in
/// `.tgz` participates in the comparison, including hidden entries.
///
/// # Arguments
///
/// * `directory` - The directory to list.
///
/// # Returns
///
/// Returns the matching entry names.
///
/// # Errors
///
/// Returns an error when the directory cannot be read.
fn tarball_names(directory: &Path) -> Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(directory).with_context(|| format!("failed to read {}", directory.display()))? {
        let entry = entry.with_context(|| format!("failed to read an entry from {}", directory.display()))?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if !name.ends_with(ARTIFACT_EXTENSION) {
            continue;
        }
        names.insert(name);
    }
    Ok(names)
}

/// Publish validated payloads and the umbrella using the supplied collaborators.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `directory` - The directory containing validated release tarballs.
/// * `hooks` - The licensing and npm collaborators, injectable for tests.
///
/// # Errors
///
/// Returns an error when validation fails, an existing version differs, or any
/// publication cannot be confirmed.
fn publish_with(root: &Path, directory: &Path, hooks: &PublishHooks<'_>) -> Result<()> {
    let releases = collect_artifacts_with(root, directory, hooks.verify_licenses)?;

    // Detect every version collision, including the umbrella, before any publish.
    for release in &releases {
        let (name, version) = manifest_identity(&release.manifest)?;
        if let Some(registry) = registry_manifest(&name, &version, hooks)? {
            verify_registry(&release.archive, &release.manifest, &registry)?;
        }
    }

    for release in &releases {
        ensure_published(&release.archive, &release.manifest, hooks)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::{RefCell, RefMut};
    use std::collections::BTreeMap;

    use flate2::Compression;
    use flate2::GzBuilder;
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;

    /// The fixture release version shared by every artifact.
    const FIXTURE_VERSION: &str = "0.1.12";

    /// The fixture generated notices bytes.
    const FIXTURE_NOTICE: &[u8] = b"Complete fixture notice\n";

    /// The fixture vendored upstream license bytes.
    const FIXTURE_UPSTREAM_LICENSE: &[u8] = b"Fixture upstream license\n";

    /// The fixture covered-source bytes, including bytes that must stay verbatim.
    const FIXTURE_COVERED_SOURCE: &[u8] = b"Fixture covered source\x00\xff";

    /// The fixture launcher bytes.
    const FIXTURE_LAUNCHER: &[u8] = b"#!/usr/bin/env node\nconsole.log('oatty fixture launcher');\n";

    /// The fixture documentation bytes.
    const FIXTURE_README: &[u8] = b"# Oatty fixture\n\nLegal fixture documentation.\n";

    /// The fixture first-party license bytes.
    const FIXTURE_LICENSE: &[u8] = b"MIT License\n\nFixture first-party license text.\n";

    /// The fixture executable bytes.
    const FIXTURE_BINARY: &[u8] = b"\x7fELF fixture signed executable bytes\n";

    /// The prepack licensing guard the fixture root manifest declares.
    const FIXTURE_PREPACK_GUARD: &str = "cargo xtask licenses verify";

    /// The canonical Oatty repository the fixture manifests declare.
    const FIXTURE_REPOSITORY_URL: &str = "git+https://github.com/oattyio/oatty.git";

    /// A licensing verifier that always succeeds, standing in for real materials.
    fn no_licensing_verification(_root: &Path) -> Result<()> {
        Ok(())
    }

    /// The npm argv prefix used by publication tests.
    fn fixture_npm_command() -> Result<Vec<String>> {
        Ok(vec!["fixture-npm".to_owned()])
    }

    /// A retry delay that never waits, so visibility tests stay fast.
    fn no_retry_wait(_delay: Duration) {}

    /// Write one fixture file, creating every missing parent directory first.
    fn write_document(path: &Path, bytes: &[u8]) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
    }

    /// Write one fixture JSON document with a trailing newline.
    fn write_json_document(path: &Path, value: &Value) -> Result<()> {
        let mut document = serde_json::to_string_pretty(value)?;
        document.push('\n');
        write_document(path, document.as_bytes())
    }

    /// Build the canonical root package manifest the fixture publishes.
    fn fixture_root_manifest() -> Value {
        json!({
            "name": "oatty",
            "version": FIXTURE_VERSION,
            "description": "Fixture Oatty package",
            "license": "MIT",
            "repository": { "type": "git", "url": FIXTURE_REPOSITORY_URL },
            "scripts": { "prepack": FIXTURE_PREPACK_GUARD },
            "optionalDependencies": {
                "@oatty/cli-darwin-x64": FIXTURE_VERSION,
                "@oatty/cli-darwin-arm64": FIXTURE_VERSION,
                "@oatty/cli-linux-x64": FIXTURE_VERSION,
                "@oatty/cli-win32-x64": FIXTURE_VERSION,
            },
        })
    }

    /// Build the canonical platform index the fixture publishes.
    fn fixture_platforms() -> Value {
        json!({
            "darwin-x64": {
                "package": "@oatty/cli-darwin-x64",
                "target": "x86_64-apple-darwin",
                "os": "darwin",
                "cpu": "x64",
                "binary": "oatty"
            },
            "darwin-arm64": {
                "package": "@oatty/cli-darwin-arm64",
                "target": "aarch64-apple-darwin",
                "os": "darwin",
                "cpu": "arm64",
                "binary": "oatty"
            },
            "linux-x64": {
                "package": "@oatty/cli-linux-x64",
                "target": "x86_64-unknown-linux-gnu",
                "os": "linux",
                "cpu": "x64",
                "binary": "oatty",
                "libc": "glibc"
            },
            "win32-x64": {
                "package": "@oatty/cli-win32-x64",
                "target": "x86_64-pc-windows-msvc",
                "os": "win32",
                "cpu": "x64",
                "binary": "oatty.exe"
            },
        })
    }

    /// Build the exact native payload manifest included in a fixture tarball.
    fn fixture_payload_manifest(platform: &crate::npm_package::Platform, version: &str) -> Value {
        let mut files = vec![Value::from("bin"), Value::from("LICENSE")];
        files.extend(crate::licenses::GENERATED_MATERIALS.iter().map(|name| Value::from(*name)));
        let mut manifest = json!({
            "name": platform.package_name,
            "version": version,
            "description": format!("Oatty native executable for {}", platform.target_triple),
            "license": "MIT",
            "repository": { "type": "git", "url": FIXTURE_REPOSITORY_URL },
            "os": [platform.os],
            "cpu": [platform.cpu],
            "oattyTarget": platform.target_triple,
            "files": files,
        });
        if let Some(libc) = &platform.libc {
            manifest["libc"] = json!([libc]);
        }
        manifest
    }

    /// Build the umbrella manifest: the root manifest without development scripts.
    fn fixture_umbrella_manifest() -> Value {
        let mut manifest = fixture_root_manifest();
        let Value::Object(entries) = &mut manifest else {
            panic!("the fixture root manifest must be a JSON object");
        };
        entries.remove("scripts");
        manifest
    }

    /// Write one deterministic npm-style tarball from raw member descriptions.
    fn write_tarball(archive: &Path, entries: &[(&str, &[u8], u32)]) -> Result<()> {
        let output = fs::File::create(archive).with_context(|| format!("failed to create {}", archive.display()))?;
        let encoder = GzBuilder::new().mtime(0).write(output, Compression::default());
        let mut builder = tar::Builder::new(encoder);
        for (name, data, mode) in entries {
            let mut header = tar::Header::new_ustar();
            header.set_path(format!("package/{name}"))?;
            header.set_mode(*mode);
            header.set_size(data.len() as u64);
            header.set_mtime(0);
            header.set_uid(0);
            header.set_gid(0);
            header.set_entry_type(tar::EntryType::Regular);
            header.set_cksum();
            builder.append(&header, *data)?;
        }
        let encoder = builder.into_inner()?;
        encoder.finish()?.sync_all()?;
        Ok(())
    }

    /// Write one fixture native payload tarball.
    fn write_payload_tarball(archive: &Path, platform: &crate::npm_package::Platform, version: &str) -> Result<()> {
        let manifest = serde_json::to_vec(&fixture_payload_manifest(platform, version))?;
        let binary_name = format!("bin/{}", platform.binary);
        let marker = format!("{version}\n");
        write_tarball(
            archive,
            &[
                ("package.json", manifest.as_slice(), 0o644),
                ("LICENSE", FIXTURE_LICENSE, 0o644),
                ("THIRD-PARTY-NOTICES.txt", FIXTURE_NOTICE, 0o644),
                ("third-party-licenses/example/LICENSE", FIXTURE_UPSTREAM_LICENSE, 0o644),
                ("third-party-sources/example.crate", FIXTURE_COVERED_SOURCE, 0o644),
                (binary_name.as_str(), FIXTURE_BINARY, 0o755),
                ("bin/.oatty-installed-version", marker.as_bytes(), 0o644),
            ],
        )
    }

    /// Write the fixture umbrella tarball.
    fn write_umbrella_tarball(archive: &Path, root: &Path) -> Result<()> {
        let manifest = serde_json::to_vec(&fixture_umbrella_manifest())?;
        let platform_index = fs::read(root.join("npm/platforms.json"))?;
        write_tarball(
            archive,
            &[
                ("package.json", manifest.as_slice(), 0o644),
                ("LICENSE", FIXTURE_LICENSE, 0o644),
                ("THIRD-PARTY-NOTICES.txt", FIXTURE_NOTICE, 0o644),
                ("third-party-licenses/example/LICENSE", FIXTURE_UPSTREAM_LICENSE, 0o644),
                ("third-party-sources/example.crate", FIXTURE_COVERED_SOURCE, 0o644),
                ("npm/bin/oatty.js", FIXTURE_LAUNCHER, 0o755),
                ("npm/platforms.json", platform_index.as_slice(), 0o644),
                ("README.md", FIXTURE_README, 0o644),
            ],
        )
    }

    /// A complete source tree and release directory satisfying every publication contract.
    struct Fixture {
        /// The temporary directory holding the fixture tree; the prefix keeps
        /// it alive for the test because drop removes the tree.
        _directory: TempDir,
        /// The workspace root the artifacts were packed from.
        root: PathBuf,
        /// The directory holding the release tarballs.
        dist: PathBuf,
        /// The release version every artifact carries.
        version: String,
        /// Artifact file names mapped to their manifests plus published integrity.
        artifacts: Vec<(String, Value)>,
    }

    impl Fixture {
        /// Create the fixture tree, write all five tarballs, and validate them.
        fn new() -> Result<Self> {
            let directory = TempDir::new().context("failed to create the fixture directory")?;
            let root = directory.path().join("source");
            let dist = directory.path().join("dist");
            write_json_document(&root.join("package.json"), &fixture_root_manifest())?;
            write_json_document(&root.join("npm/platforms.json"), &fixture_platforms())?;
            write_document(&root.join("npm/bin/oatty.js"), FIXTURE_LAUNCHER)?;
            write_document(&root.join("README.md"), FIXTURE_README)?;
            write_document(&root.join("LICENSE"), FIXTURE_LICENSE)?;
            write_document(&root.join("THIRD-PARTY-NOTICES.txt"), FIXTURE_NOTICE)?;
            write_document(&root.join("third-party-licenses/example/LICENSE"), FIXTURE_UPSTREAM_LICENSE)?;
            write_document(&root.join("third-party-sources/example.crate"), FIXTURE_COVERED_SOURCE)?;
            fs::create_dir_all(&dist)?;

            let version = FIXTURE_VERSION.to_owned();
            for platform in crate::npm_package::load_platforms(&root)?.values() {
                write_payload_tarball(&dist.join(native_artifact_name(platform, &version)), platform, &version)?;
            }
            write_umbrella_tarball(&dist.join(umbrella_artifact_name(&version)), &root)?;

            let releases = collect_artifacts_with(&root, &dist, &no_licensing_verification)?;
            let mut artifacts = Vec::with_capacity(releases.len());
            for release in releases {
                let file_name = release
                    .archive
                    .file_name()
                    .context("the fixture artifact has no file name")?
                    .to_string_lossy()
                    .into_owned();
                let mut manifest = release.manifest;
                manifest["dist"] = json!({ "integrity": artifact_integrity(&release.archive)? });
                artifacts.push((file_name, manifest));
            }
            Ok(Self {
                _directory: directory,
                root,
                dist,
                version,
                artifacts,
            })
        }

        /// Return the fixture workspace root.
        fn root(&self) -> &Path {
            &self.root
        }

        /// Return the fixture release directory.
        fn dist(&self) -> &Path {
            &self.dist
        }

        /// Return the fixture release version.
        fn version(&self) -> &str {
            &self.version
        }

        /// Return every artifact in publication order as file-name/manifest pairs.
        fn artifacts(&self) -> BTreeMap<String, Value> {
            self.artifacts.iter().cloned().collect()
        }

        /// Return one artifact manifest by tarball file name.
        fn artifact(&self, file_name: &str) -> Result<&Value> {
            self.artifacts
                .iter()
                .find(|(name, _)| name == file_name)
                .map(|(_, manifest)| manifest)
                .with_context(|| format!("the fixture has no artifact named {file_name}"))
        }

        /// Return the first published artifact, matching Python's insertion order.
        fn first_artifact(&self) -> &Value {
            &self.artifacts.first().expect("the fixture always contains five artifacts").1
        }
    }

    /// The mutable registry, artifact, and invocation state of the fake npm.
    #[derive(Default)]
    struct FakeState {
        /// Every npm argv recorded by the fake, without the program prefix.
        calls: Vec<Vec<String>>,
        /// Published manifests keyed by `name@version`.
        registry: BTreeMap<String, Value>,
        /// Local artifact manifests keyed by tarball file name.
        artifacts: BTreeMap<String, Value>,
        /// Simulates a network or authentication failure on every lookup.
        network_failure: bool,
        /// Simulates a published version that is not yet visible in the registry.
        invisible: Option<String>,
        /// Simulates a failing publish for the named package.
        fail_publish: Option<String>,
    }

    /// An in-process replacement for the fake npm script used by the Python suite.
    struct FakeNpm {
        /// The shared state the fake mutates while serving `view` and `publish`.
        state: RefCell<FakeState>,
    }

    impl FakeNpm {
        /// Build a fake npm serving the supplied local artifacts.
        fn new(artifacts: BTreeMap<String, Value>) -> Self {
            Self {
                state: RefCell::new(FakeState {
                    artifacts,
                    ..FakeState::default()
                }),
            }
        }

        /// Borrow the fake state mutably so tests can stage registry entries.
        fn state(&self) -> RefMut<'_, FakeState> {
            self.state.borrow_mut()
        }

        /// Return a copy of every recorded invocation.
        fn calls(&self) -> Vec<Vec<String>> {
            self.state.borrow().calls.clone()
        }

        /// Return a copy of every recorded publish invocation.
        fn publish_calls(&self) -> Vec<Vec<String>> {
            self.state
                .borrow()
                .calls
                .iter()
                .filter(|call| call.first().map(String::as_str) == Some("publish"))
                .cloned()
                .collect()
        }

        /// Record an invocation and emulate `npm view` or `npm publish`.
        fn run(&self, _prefix: &[String], arguments: &[&str]) -> Result<NpmOutcome> {
            let mut state = self.state.borrow_mut();
            state.calls.push(arguments.iter().map(|argument| (*argument).to_owned()).collect());
            match arguments.first().copied() {
                Some("view") => {
                    if state.network_failure {
                        return structured_failure("E401");
                    }
                    let specifier = arguments.get(1).copied().unwrap_or_default();
                    let visible = state.invisible.as_deref() != Some(specifier);
                    match state.registry.get(specifier) {
                        Some(manifest) if visible => Ok(succeeded(&serde_json::to_vec(manifest)?)),
                        _ => structured_failure("E404"),
                    }
                }
                Some("publish") => {
                    let archive = arguments.get(1).copied().unwrap_or_default();
                    let Some(file_name) = Path::new(archive).file_name().and_then(|name| name.to_str()) else {
                        return Ok(failed());
                    };
                    let Some(artifact) = state.artifacts.get(file_name).cloned() else {
                        return Ok(failed());
                    };
                    let (name, version) = manifest_identity(&artifact)?;
                    if state.fail_publish.as_deref() == Some(name.as_str()) {
                        return Ok(failed());
                    }
                    state.registry.insert(format!("{name}@{version}"), artifact);
                    Ok(succeeded(&[]))
                }
                _ => Ok(failed()),
            }
        }
    }

    /// Build a successful fake outcome with the supplied standard output.
    fn succeeded(standard_output: &[u8]) -> NpmOutcome {
        NpmOutcome {
            exit_success: true,
            standard_output: standard_output.to_vec(),
            standard_error: String::new(),
        }
    }

    /// Build a failed fake outcome with empty output, mirroring a silent crash.
    fn failed() -> NpmOutcome {
        NpmOutcome {
            exit_success: false,
            standard_output: Vec::new(),
            standard_error: String::new(),
        }
    }

    /// Build a failed fake outcome carrying an npm structured error on standard output.
    fn structured_failure(code: &str) -> Result<NpmOutcome> {
        Ok(NpmOutcome {
            exit_success: false,
            standard_output: serde_json::to_vec(&json!({ "error": { "code": code } }))?,
            standard_error: String::new(),
        })
    }

    /// Run one closure with hooks backed by the fake npm and no licensing checks.
    fn with_hooks<T>(fake: &FakeNpm, action: impl FnOnce(&PublishHooks<'_>) -> Result<T>) -> Result<T> {
        let run_npm = |prefix: &[String], arguments: &[&str]| fake.run(prefix, arguments);
        action(&fixture_hooks(&run_npm))
    }

    /// Build production-shaped hooks whose only substitute is the npm runner.
    fn fixture_hooks<'a>(run_npm: &'a NpmRunner<'a>) -> PublishHooks<'a> {
        PublishHooks {
            verify_licenses: &no_licensing_verification,
            npm_command: &fixture_npm_command,
            run_npm,
            sleep: &no_retry_wait,
        }
    }

    /// Build one fake npm outcome with the supplied body and exit status.
    fn registry_response(body: &Value, exit_success: bool) -> Result<NpmOutcome> {
        Ok(NpmOutcome {
            exit_success,
            standard_output: serde_json::to_vec(body)?,
            standard_error: String::new(),
        })
    }

    /// Assert that one publication error carries the expected message.
    fn assert_error_contains(outcome: Result<impl std::fmt::Debug>, expected: &str) {
        let error = outcome.unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "expected {expected:?} in {:?}",
            error.to_string()
        );
    }

    /// Publish the fixture release directory through the fake npm.
    fn run_publish(fixture: &Fixture, fake: &FakeNpm) -> Result<()> {
        with_hooks(fake, |hooks| publish_with(fixture.root(), fixture.dist(), hooks))
    }

    #[test]
    fn npm_distribution_registry_manifest_normalizes_shapes_and_structured_errors() -> Result<()> {
        let manifest = json!({ "name": "oatty", "version": FIXTURE_VERSION });

        // npm 12 keeps the single version result in a list; npm 11 returns it directly.
        for body in [json!([manifest.clone()]), manifest.clone()] {
            let run = move |_: &[String], _: &[&str]| registry_response(&body, true);
            let published =
                registry_manifest("oatty", FIXTURE_VERSION, &fixture_hooks(&run))?.context("the fixture registry metadata is missing")?;
            assert_eq!(manifest, published);
        }

        let not_found = |_: &[String], _: &[&str]| registry_response(&json!({ "error": { "code": "E404" } }), false);
        assert!(
            registry_manifest("oatty", FIXTURE_VERSION, &fixture_hooks(&not_found))?.is_none(),
            "a structured E404 is the only proof that a version is unpublished"
        );

        let unauthorized = |_: &[String], _: &[&str]| {
            Ok(NpmOutcome {
                exit_success: false,
                standard_output: serde_json::to_vec(&json!({ "error": { "code": "E401" } }))?,
                standard_error: "unauthorized\n".to_owned(),
            })
        };
        assert_error_contains(
            registry_manifest("oatty", FIXTURE_VERSION, &fixture_hooks(&unauthorized)),
            "npm view failed for oatty@0.1.12: unauthorized",
        );
        Ok(())
    }

    #[test]
    fn npm_distribution_native_first_root_last_and_idempotent_retry() -> Result<()> {
        let fixture = Fixture::new()?;
        let fake = FakeNpm::new(fixture.artifacts());
        run_publish(&fixture, &fake)?;

        let published = fake.publish_calls();
        assert_eq!(5, published.len(), "all four payloads and the umbrella must be published once");
        let umbrella_name = umbrella_artifact_name(fixture.version());
        assert_eq!(
            Some(umbrella_name.as_str()),
            Path::new(&published[4][1]).file_name().and_then(|name| name.to_str()),
            "the umbrella must be published last"
        );
        for call in &published {
            for flag in ["--provenance", "--access", "public", "--ignore-scripts"] {
                assert!(call.iter().any(|argument| argument == flag), "{call:?} must carry {flag}");
            }
        }
        let calls = fake.calls();
        for (index, call) in calls.iter().enumerate() {
            if call.first().map(String::as_str) == Some("publish") {
                assert_eq!(
                    Some("view"),
                    calls.get(index + 1).and_then(|next| next.first()).map(String::as_str),
                    "every publish must be followed by a visibility lookup"
                );
            }
        }

        fake.state().calls.clear();
        run_publish(&fixture, &fake)?;
        assert!(fake.publish_calls().is_empty(), "a repeated release must not republish");
        Ok(())
    }

    #[test]
    fn npm_distribution_existing_root_collision_prevents_all_payload_publication() -> Result<()> {
        let fixture = Fixture::new()?;
        let fake = FakeNpm::new(fixture.artifacts());
        let mut umbrella = fixture.artifact(&umbrella_artifact_name(fixture.version()))?.clone();
        umbrella["dist"] = json!({ "integrity": "sha512-old-release" });
        fake.state().registry.insert(format!("oatty@{}", fixture.version()), umbrella);

        let error = run_publish(&fixture, &fake).unwrap_err();
        assert!(
            error.to_string().contains("Published integrity differs"),
            "expected an integrity collision, got {error}"
        );
        assert!(fake.publish_calls().is_empty(), "a collision anywhere must block every publish");
        Ok(())
    }

    #[test]
    fn npm_distribution_network_failure_is_not_treated_as_unpublished() -> Result<()> {
        let fixture = Fixture::new()?;
        let fake = FakeNpm::new(fixture.artifacts());
        fake.state().network_failure = true;

        let error = run_publish(&fixture, &fake).unwrap_err();
        assert!(
            error.to_string().contains("npm view failed"),
            "expected a view failure, got {error}"
        );
        assert!(fake.publish_calls().is_empty(), "network failures must never publish");
        Ok(())
    }

    #[test]
    fn npm_distribution_payload_failure_blocks_umbrella() -> Result<()> {
        let fixture = Fixture::new()?;
        let fake = FakeNpm::new(fixture.artifacts());
        fake.state().fail_publish = Some("@oatty/cli-linux-x64".to_owned());

        let error = run_publish(&fixture, &fake).unwrap_err();
        assert!(
            error.to_string().contains("npm publish failed for @oatty/cli-linux-x64@"),
            "expected the payload publish failure, got {error}"
        );
        assert!(
            !fake.state().registry.contains_key(&format!("oatty@{}", fixture.version())),
            "the umbrella must not be registered when a payload fails"
        );
        let umbrella_name = umbrella_artifact_name(fixture.version());
        assert!(
            fake.publish_calls()
                .iter()
                .all(|call| Path::new(&call[1]).file_name().and_then(|name| name.to_str()) != Some(umbrella_name.as_str())),
            "the umbrella must never be published after a payload failure"
        );
        Ok(())
    }

    #[test]
    fn npm_distribution_unconfirmed_payload_visibility_blocks_umbrella() -> Result<()> {
        let fixture = Fixture::new()?;
        let fake = FakeNpm::new(fixture.artifacts());
        fake.state().invisible = Some(format!("@oatty/cli-darwin-x64@{}", fixture.version()));

        let error = run_publish(&fixture, &fake).unwrap_err();
        assert!(
            error.to_string().contains("not visible"),
            "expected a visibility failure, got {error}"
        );
        assert!(
            !fake.state().registry.contains_key(&format!("oatty@{}", fixture.version())),
            "the umbrella must not be registered while a payload is invisible"
        );
        Ok(())
    }

    #[test]
    fn npm_distribution_changed_existing_integrity_or_manifest_fails() -> Result<()> {
        let fixture = Fixture::new()?;
        let fake = FakeNpm::new(fixture.artifacts());
        let first = fixture.first_artifact().clone();

        for changed in [json!({ "dist": { "integrity": "sha512-different" } }), json!({ "os": ["wrong"] })] {
            let mut entry = first.clone();
            let Value::Object(entry_fields) = &mut entry else {
                bail!("the fixture artifact must be a JSON object");
            };
            let Value::Object(changed_fields) = changed else {
                bail!("the fixture mutation must be a JSON object");
            };
            for (key, value) in changed_fields {
                entry_fields.insert(key, value);
            }

            let (name, version) = manifest_identity(&entry)?;
            {
                let mut state = fake.state();
                state.registry.clear();
                state.calls.clear();
                state.registry.insert(format!("{name}@{version}"), entry);
            }

            let error = run_publish(&fixture, &fake).unwrap_err();
            let message = error.to_string();
            assert!(
                message.contains("Published") && message.contains("differs"),
                "expected a published-metadata difference, got {message}"
            );
            assert!(
                fake.publish_calls().is_empty(),
                "changed existing metadata must block every publish"
            );
        }
        Ok(())
    }

    #[test]
    fn npm_distribution_missing_artifact_fails_before_registry_calls() -> Result<()> {
        let fixture = Fixture::new()?;
        let fake = FakeNpm::new(fixture.artifacts());
        let empty = TempDir::new()?;

        let error = with_hooks(&fake, |hooks| publish_with(fixture.root(), empty.path(), hooks)).unwrap_err();
        assert!(
            error.to_string().contains("exactly four"),
            "expected the artifact-set failure, got {error}"
        );
        assert!(fake.calls().is_empty(), "a missing artifact must fail before any registry call");
        Ok(())
    }
}
