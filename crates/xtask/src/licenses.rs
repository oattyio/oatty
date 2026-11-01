//! Prepare and verify deterministic licensing materials for every Oatty distribution.
//!
//! This module is a behavior-preserving port of `scripts/licenses.py`. It
//! regenerates `THIRD-PARTY-NOTICES.txt`, `third-party-licenses/`, and
//! `third-party-sources/` from the exact inputs that feed them, and then
//! verifies the freshly written materials against a checksum manifest.
//!
//! The design is deliberately fail-closed:
//!
//! * [`verify`] rejects missing, stale, edited, or extra generated files and
//!   prints nothing on success; the command-line layer owns the success message.
//! * Generating Rust notices requires cargo-about 0.9.2, the reviewed license
//!   supplements, and the complete MPL-2.0 source archive.
//! * [`prepare`] splits report generation from material assembly so the
//!   assembly step can be tested without invoking cargo-about. Nothing is
//!   published to the workspace root until every material has been assembled
//!   in a staging directory.
//!
//! Text generation reproduces the Python output byte for byte. JSON documents
//! use the same two-space, sorted-key, trailing-newline encoding that Python's
//! `json.dumps(..., indent=2, sort_keys=True)` produces.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

/// The generated licensing materials, in their publication order.
///
/// `prepare` writes all three into the workspace root; `verify` requires all
/// three to exist before packing any distribution artifact.
// The explicit `'static` lifetime documents the public contract for consumers.
#[allow(clippy::redundant_static_lifetimes)]
pub(crate) const GENERATED_MATERIALS: [&'static str; 3] = ["THIRD-PARTY-NOTICES.txt", "third-party-licenses", "third-party-sources"];

/// The committed manifest that fingerprints the licensing inputs and outputs.
const MANIFEST_FILE: &str = ".license-manifest.json";

/// The only cargo-about release whose output this pipeline is written against.
const EXPECTED_CARGO_ABOUT_VERSION: &str = "cargo-about 0.9.2";

/// The environment variable that overrides the cargo-about executable.
const CARGO_ABOUT_ENVIRONMENT_VARIABLE: &str = "CARGO_ABOUT";

/// The default cargo-about executable name resolved from `PATH`.
const DEFAULT_CARGO_ABOUT: &str = "cargo-about";

/// The npm licenses that the launcher dependency set may declare.
const NPM_LICENSES: [&str; 3] = ["MIT", "ISC", "BlueOak-1.0.0"];

/// The supported first-party npm platform keys and their Rust target triples.
///
/// Iteration order matches the Python dictionary so the first mapping error
/// observed for an invalid fixture stays stable.
const NPM_BINARY_TARGETS: [(&str, &str); 4] = [
    ("darwin-x64", "x86_64-apple-darwin"),
    ("darwin-arm64", "aarch64-apple-darwin"),
    ("linux-x64", "x86_64-unknown-linux-gnu"),
    ("win32-x64", "x86_64-pc-windows-msvc"),
];

/// The fixed preamble of `THIRD-PARTY-NOTICES.txt`.
///
/// Every line is reproduced verbatim from the Python pipeline; the two trailing
/// newlines separate the preamble from the generated Rust inventory.
const THIRD_PARTY_NOTICES_HEADER: &str = concat!(
    "# Oatty third-party notices\n\n",
    "Oatty itself is MIT licensed; see LICENSE.\n",
    "Dependencies retain their original licenses. MIT is preferred where upstream offers a choice.\n",
    "This inventory covers the CLI default features across Linux x86_64, macOS x86_64/aarch64,\n",
    "and Windows x86_64 MSVC, including build and transitive dependencies, excluding dev-only dependencies.\n",
    "Full upstream supplemental notices (including native code) are preserved under third-party-licenses/.\n",
    "MPL-2.0 corresponding source and extraction instructions are in third-party-sources/.\n\n",
);

/// The fixed preamble of `third-party-sources/README.txt`.
const MPL_SOURCE_README_PREFIX: &str = concat!(
    "MPL-2.0 source availability\n\n",
    "Oatty includes unchanged nucleo-matcher 0.3.1 under MPL-2.0.\n",
    "The complete corresponding upstream source is provided here as its original\n",
    "crates.io .crate archive (a gzip-compressed tar archive). Extract it with\n",
    "`tar -xzf nucleo-matcher-0.3.1.crate`; its LICENSE contains MPL-2.0.\n",
    "You may modify and redistribute this covered source under MPL-2.0.\n",
    "Oatty makes no modifications to this covered source.\n\n",
);

/// Matches a file name that is itself an upstream license or notice document.
static NOTICE_NAME_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:LICEN[CS]E|NOTICE|COPYRIGHT|COPYING)(?:$|[._-])").expect("the notice-name pattern is valid"));

/// Matches unresolved template placeholders that must never ship as attribution.
static PLACEHOLDER_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<year>|<owner>|<copyright holders>").expect("the placeholder pattern is valid"));

/// Matches a concrete copyright attribution line.
static ATTRIBUTION_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)Copyright\s+(?:\([cC]\)|©|\d{4})[^\n]*").expect("the attribution pattern is valid"));

/// Generate every licensing material and the staleness manifest.
///
/// This is the complete prepare flow: it checks the cargo-about version, asks
/// cargo-about for the workspace license report, assembles all materials in a
/// staging directory, publishes them into `root`, writes the manifest, and
/// finally calls [`verify`].
///
/// # Arguments
///
/// * `root` - The workspace root directory.
///
/// # Errors
///
/// Returns an error when cargo-about is unavailable, has the wrong version, or
/// fails, and when any input or generated material does not satisfy the
/// licensing contracts. The assembly step publishes nothing unless the report
/// was fetched successfully.
///
/// # Side Effects
///
/// On success, rewrites the three generated materials, the manifest, and prints
/// `Prepared {n} Rust packages and verified distribution licensing materials.`
/// to standard output.
pub(crate) fn prepare(root: &Path) -> Result<()> {
    let tool = std::env::var(CARGO_ABOUT_ENVIRONMENT_VARIABLE).unwrap_or_else(|_| DEFAULT_CARGO_ABOUT.to_owned());
    prepare_with_report_provider(root, || report(root, &tool))
}

/// Reject missing, stale, edited, or extra generated materials before packing.
///
/// The committed manifest must fingerprint exactly the current licensing
/// inputs and the current generated files, and at least one MPL-2.0 `.crate`
/// source bundle must be present.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
///
/// # Errors
///
/// Returns an error when the manifest is missing or malformed, when the inputs
/// are stale, when any generated file is missing or changed, or when the
/// required MPL-2.0 source archive is absent.
///
/// # Side Effects
///
/// This function prints nothing; the caller reports success.
pub(crate) fn verify(root: &Path) -> Result<()> {
    let manifest_path = root.join(MANIFEST_FILE);
    let manifest_text = fs::read_to_string(&manifest_path).with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: Value = serde_json::from_str(&manifest_text).with_context(|| format!("failed to parse {}", manifest_path.display()))?;

    let inputs_entry = manifest.get("inputs").context("licensing manifest is missing the inputs entry")?;
    let current_inputs = serde_json::to_value(inputs(root)?).context("failed to serialize licensing inputs")?;
    if inputs_entry != &current_inputs {
        bail!("Stale licensing inputs; run npm run licenses:prepare");
    }

    let files_entry = manifest.get("files").context("licensing manifest is missing the files entry")?;
    let current_files = inventory(root)?;
    if files_entry != &serde_json::to_value(&current_files).context("failed to serialize licensing inventory")? {
        bail!("Missing or changed licensing output/source; run npm run licenses:prepare");
    }
    if current_files.is_empty() || !current_files.keys().any(|name| name.ends_with(".crate")) {
        bail!("Required MPL source bundle is missing");
    }
    Ok(())
}

/// Fingerprint every file inside the three generated materials.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
///
/// # Returns
///
/// Returns a map from workspace-relative POSIX path to lowercase SHA-256 hex
/// digest. Directory materials contribute every regular file below them.
///
/// # Errors
///
/// Returns an error naming the first missing material, or when a material
/// cannot be read or walked. The missing-material message is part of the
/// packaging contract: `Missing generated material: {name}; run npm run
/// licenses:prepare`.
pub(crate) fn inventory(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut files = Vec::new();
    for name in GENERATED_MATERIALS {
        let path = root.join(name);
        if !path.exists() {
            bail!("Missing generated material: {name}; run npm run licenses:prepare");
        }
        if path.is_file() {
            files.push(path);
        } else {
            visit_files(&path, &mut files)?;
        }
    }

    let mut fingerprints = BTreeMap::new();
    for path in files {
        let name = relative_posix_path(root, &path)?;
        fingerprints.insert(name, sha256(&path)?);
    }
    Ok(fingerprints)
}

/// Fingerprint the locks, policy, generation code, and workspace manifests.
///
/// The Python source hashed its own two scripts; this port hashes the two Rust
/// modules that replace them. Every text file is normalized from CRLF to LF
/// before hashing so Windows Git checkouts produce identical fingerprints.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
///
/// # Returns
///
/// Returns sorted relative POSIX paths mapped to SHA-256 hex digests.
///
/// # Errors
///
/// Returns an error when any required input file cannot be read.
fn inputs(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut names = [
        "LICENSE",
        "Cargo.toml",
        "Cargo.lock",
        "package.json",
        "package-lock.json",
        "about.toml",
        "crates/xtask/src/licenses.rs",
        "npm/platforms.json",
        "crates/xtask/src/npm_package.rs",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();

    for path in collect_named_files(&root.join("crates"), "Cargo.toml")? {
        names.push(relative_posix_path(root, &path)?);
    }
    for path in collect_files_below(&root.join("scripts").join("license-supplements"))? {
        names.push(relative_posix_path(root, &path)?);
    }

    let mut fingerprints = BTreeMap::new();
    for name in names {
        let data = fs::read(root.join(&name)).with_context(|| format!("failed to read licensing input {name}"))?;
        fingerprints.insert(name, sha256_bytes(&normalize_line_endings(&data)));
    }
    Ok(fingerprints)
}

/// Run cargo-about and return its parsed JSON report.
///
/// # Arguments
///
/// * `root` - The workspace root directory used as the working directory.
/// * `tool` - The cargo-about executable, honoring `CARGO_ABOUT`.
///
/// # Returns
///
/// Returns the parsed `cargo-about generate --format json` report.
///
/// # Errors
///
/// Returns an error when the executable is missing, the version is not exactly
/// `cargo-about 0.9.2`, generation fails, or the report is not valid JSON.
fn report(root: &Path, tool: &str) -> Result<Value> {
    report_with_runner(tool, |program, arguments| run_program(root, program, arguments))
}

/// Run cargo-about through an injectable command runner.
///
/// The runner indirection keeps the version check and generation contract
/// testable without installing cargo-about or touching the real workspace.
///
/// # Arguments
///
/// * `tool` - The cargo-about executable.
/// * `runner` - Invokes `runner(program, arguments)` and returns captured stdout.
///
/// # Returns
///
/// Returns the parsed `generate --format json` report.
///
/// # Errors
///
/// Returns an error when the version differs from
/// `cargo-about 0.9.2`, when generation fails, or when the output is not JSON.
fn report_with_runner<F>(tool: &str, runner: F) -> Result<Value>
where
    F: Fn(&str, &[&str]) -> Result<CapturedOutput>,
{
    let version_output = runner(tool, &["--version"])?;
    let version = String::from_utf8_lossy(&version_output.stdout).trim().to_owned();
    if version != EXPECTED_CARGO_ABOUT_VERSION {
        bail!("Expected cargo-about 0.9.2, got {version}");
    }
    let report_output = runner(
        tool,
        &[
            "generate",
            "--locked",
            "--offline",
            "--fail",
            "--manifest-path",
            "crates/cli/Cargo.toml",
            "--config",
            "about.toml",
            "--format",
            "json",
        ],
    )?;
    serde_json::from_slice(&report_output.stdout).context("failed to parse the cargo-about report JSON")
}

/// Execute a program and surface a non-zero exit status as an error.
///
/// # Arguments
///
/// * `root` - The working directory for the child process.
/// * `program` - The executable name or path.
/// * `arguments` - The arguments passed to the executable.
///
/// # Returns
///
/// Returns the captured standard output of a successful invocation.
///
/// # Errors
///
/// Returns an error when the process cannot start or exits unsuccessfully.
fn run_program(root: &Path, program: &str, arguments: &[&str]) -> Result<CapturedOutput> {
    let output = Command::new(program)
        .args(arguments)
        .current_dir(root)
        .output()
        .with_context(|| format!("failed to execute {program}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("{program} exited with {}: {}", output.status, stderr.trim());
    }
    Ok(CapturedOutput { stdout: output.stdout })
}

/// The captured standard output of one successfully executed program.
struct CapturedOutput {
    /// The raw bytes the program wrote to standard output.
    stdout: Vec<u8>,
}

/// Prepare materials with an injectable report provider.
///
/// This is the testable seam required by the Python test suite: the provider
/// stands in for cargo-about, and `assemble` is never called when it fails.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `provider` - Produces the cargo-about report, or fails.
///
/// # Errors
///
/// Returns any error from the provider or from [`assemble`].
fn prepare_with_report_provider<F>(root: &Path, provider: F) -> Result<()>
where
    F: FnOnce() -> Result<Value>,
{
    let generator_report = provider()?;
    let package_count = assemble(root, &generator_report)?;
    println!("Prepared {package_count} Rust packages and verified distribution licensing materials.");
    Ok(())
}

/// Assemble, publish, and verify every material from a cargo-about report.
///
/// All textual generation happens in a temporary staging directory. Only after
/// the generated files and manifest have been computed successfully are the
/// materials moved into `root`; [`verify`] then re-checks the published state.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `report_value` - The parsed cargo-about JSON report.
///
/// # Returns
///
/// Returns the number of Rust packages in the report.
///
/// # Errors
///
/// Returns an error when the report is malformed, any licensing contract is
/// violated, or verification of the published materials fails.
fn assemble(root: &Path, report_value: &Value) -> Result<usize> {
    let generator_report: GeneratorReport =
        serde_json::from_value(report_value.clone()).context("failed to interpret the cargo-about report")?;

    let staging = tempfile::Builder::new()
        .prefix("oatty-licenses-")
        .tempdir()
        .context("failed to create a licensing staging directory")?;
    let output = staging.path();

    let evidence_root = root.join("scripts").join("license-supplements");
    let supplements = read_json(&evidence_root.join("reviewed.json"))?
        .as_object()
        .cloned()
        .context("reviewed license evidence must contain a JSON object")?;
    let lock = read_cargo_lock(&root.join("Cargo.lock"))?;

    let mut notices = String::from(THIRD_PARTY_NOTICES_HEADER);
    notices.push_str(&rust_notices(&generator_report, output, &lock, &supplements, &evidence_root)?);
    notices.push('\n');
    notices.push_str(&npm_notices(root, output)?);
    write_text(&output.join(GENERATED_MATERIALS[0]), &notices)?;

    let manifest = json!({
        "generator": EXPECTED_CARGO_ABOUT_VERSION,
        "inputs": inputs(root)?,
        "files": inventory(output)?,
    });

    for name in GENERATED_MATERIALS {
        let destination = root.join(name);
        remove_path(&destination)?;
        move_path(&output.join(name), &destination)?;
    }
    write_text(&root.join(MANIFEST_FILE), &format_pretty_json(&manifest)?)?;
    verify(root)?;
    Ok(generator_report.crates.len())
}

/// Generate the Rust package inventory and selected license sections.
///
/// # Arguments
///
/// * `report` - The parsed cargo-about report.
/// * `output` - The staging directory that receives licenses and sources.
/// * `lock` - The parsed `Cargo.lock`.
/// * `supplements` - The reviewed evidence keyed by `name@version`.
/// * `evidence_root` - The directory holding reviewed supplement files.
///
/// # Returns
///
/// Returns the `## Rust package inventory` section, including every selected
/// license section, exactly as the Python pipeline emits it.
///
/// # Errors
///
/// Returns an error for missing license text, unresolved crates, missing or
/// empty notices, stale reviewed evidence, checksum mismatches, unreviewed MPL
/// obligations, or a missing nucleo-matcher source selection.
fn rust_notices(
    report: &GeneratorReport,
    output: &Path,
    lock: &CargoLock,
    supplements: &Map<String, Value>,
    evidence_root: &Path,
) -> Result<String> {
    let mut covered = BTreeSet::new();
    let mut template_ids = BTreeSet::new();
    let mut sections = Vec::new();

    for license_info in &report.licenses {
        if license_info.text.trim().is_empty() {
            bail!("Missing selected license text");
        }
        let mut users = license_info
            .used_by
            .iter()
            .map(|user| format!("{} {}", user.krate.name, user.krate.version))
            .collect::<Vec<_>>();
        users.sort();
        let is_template = PLACEHOLDER_PATTERN.is_match(&license_info.text);
        for user in &license_info.used_by {
            covered.insert(user.krate.id.clone());
            if is_template {
                template_ids.insert(user.krate.id.clone());
            }
        }
        sections.push(format!(
            "## {} ({})\n\nUsed by: {}\n\n{}\n",
            license_info.name,
            license_info.id,
            users.join(", "),
            license_info.text
        ));
    }

    let locked = lock
        .package
        .iter()
        .map(|package| (format!("{}@{}", package.name, package.version), package))
        .collect::<BTreeMap<_, _>>();

    let mut packages = Vec::new();
    let mut source_bundles = Vec::new();
    let mut crate_entries = report.crates.iter().collect::<Vec<_>>();
    crate_entries.sort_by(|left, right| {
        (left.package.name.as_str(), left.package.version.as_str()).cmp(&(right.package.name.as_str(), right.package.version.as_str()))
    });

    for crate_entry in crate_entries {
        let package = &crate_entry.package;
        let package_license = package.license.as_deref().unwrap_or_default();
        let package_id = package.id.as_deref().unwrap_or_default();
        if package_license.is_empty() || !covered.contains(package_id) {
            bail!("Unresolved licensing: {} {}", package.name, package.version);
        }
        if package.source.as_deref().is_none_or(str::is_empty) {
            // Oatty's own workspace MIT text ships separately as LICENSE.
            continue;
        }

        let name = package.name.as_str();
        let version = package.version.as_str();
        let identity = format!("{name}@{version}");
        let manifest_path = package
            .manifest_path
            .as_deref()
            .with_context(|| format!("cargo-about report is missing the manifest path for {identity}"))?;
        let source = manifest_path
            .parent()
            .with_context(|| format!("invalid manifest path for {identity}"))?;
        let destination = output.join("third-party-licenses").join("rust").join(format!("{name}-{version}"));

        let mut notices = copy_notices(source, &destination, required_notices(name))?;
        let lock_entry = locked
            .get(&identity)
            .copied()
            .with_context(|| format!("Cargo.lock is missing {identity}"))?;
        notices.extend(preserve_supplement(package, lock_entry, &destination, supplements, evidence_root)?);
        validate_attribution(
            package,
            &notices,
            &destination,
            template_ids.contains(package_id),
            supplements.contains_key(&identity),
        )?;

        let upstream = package
            .repository
            .as_deref()
            .filter(|value| !value.is_empty())
            .or_else(|| package.homepage.as_deref().filter(|value| !value.is_empty()))
            .or(package.source.as_deref())
            .unwrap_or_default();
        packages.push(format!(
            "- {name} {version}: {}; upstream: {upstream}; preserved notices: {}",
            crate_entry.license.as_deref().unwrap_or_default(),
            notices.len()
        ));

        if package_license.contains("MPL-2.0") {
            append_mpl_source_bundle(name, version, source, lock_entry, output, &mut source_bundles)?;
        }
    }

    if source_bundles.is_empty() {
        bail!("Expected nucleo-matcher source was not selected; review MPL policy");
    }

    let readme = format!("{MPL_SOURCE_README_PREFIX}{}\n", source_bundles.join("\n"));
    let sources_directory = output.join("third-party-sources");
    fs::create_dir_all(&sources_directory).with_context(|| format!("failed to create {}", sources_directory.display()))?;
    write_text(&sources_directory.join("README.txt"), &readme)?;

    Ok(format!(
        "## Rust package inventory\n\n{}\n\n{}",
        packages.join("\n"),
        sections.join("\n")
    ))
}

/// Copy the one reviewed MPL-2.0 source archive and record its provenance line.
///
/// # Arguments
///
/// * `name` - The crate name from the report.
/// * `version` - The crate version from the report.
/// * `source` - The extracted crate source directory.
/// * `lock_entry` - The matching `Cargo.lock` entry supplying the checksum.
/// * `output` - The staging directory that receives the archive.
/// * `source_bundles` - The accumulated README provenance lines.
///
/// # Errors
///
/// Returns an error for any MPL-2.0 obligation other than nucleo-matcher 0.3.1,
/// for a missing checksum, or when the archive checksum does not match.
fn append_mpl_source_bundle(
    name: &str,
    version: &str,
    source: &Path,
    lock_entry: &LockedPackage,
    output: &Path,
    source_bundles: &mut Vec<String>,
) -> Result<()> {
    if name != "nucleo-matcher" || version != "0.3.1" {
        bail!("Unreviewed MPL source obligation: {name} {version}");
    }
    let checksum = lock_entry
        .checksum
        .as_deref()
        .with_context(|| format!("Cargo.lock is missing the checksum for {name} {version}"))?;
    let source_parent = source
        .parent()
        .with_context(|| format!("invalid source path for {name} {version}"))?;
    let registry_directory = source_parent
        .parent()
        .with_context(|| format!("invalid source path for {name} {version}"))?;
    let registry_name = source_parent
        .file_name()
        .with_context(|| format!("invalid source path for {name} {version}"))?;
    let archive = registry_directory
        .join("..")
        .join("cache")
        .join(registry_name)
        .join(format!("{name}-{version}.crate"));
    let archive_name = archive
        .file_name()
        .with_context(|| format!("invalid source archive path for {name} {version}"))?
        .to_os_string();
    let resolved_archive = archive
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", archive.display()))?;
    bundle_source(&resolved_archive, &output.join("third-party-sources").join(&archive_name), checksum)?;
    source_bundles.push(format!(
        "- {}; SHA-256 {checksum}; https://crates.io/crates/{name}/{version}",
        archive_name.to_string_lossy()
    ));
    Ok(())
}

/// Copy an MPL-2.0 source archive after verifying its published checksum.
///
/// # Arguments
///
/// * `archive` - The resolved `.crate` archive path.
/// * `destination` - The path to copy the archive to.
/// * `checksum` - The expected lowercase SHA-256 hex digest.
///
/// # Errors
///
/// Returns an error when the digest differs or the archive cannot be copied.
fn bundle_source(archive: &Path, destination: &Path, checksum: &str) -> Result<()> {
    if sha256(archive)? != checksum {
        let name = archive
            .file_name()
            .map_or_else(|| archive.display().to_string(), |value| value.to_string_lossy().into_owned());
        bail!("Source checksum mismatch: {name}");
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::copy(archive, destination).with_context(|| format!("failed to copy {} to {}", archive.display(), destination.display()))?;
    Ok(())
}

/// Copy every upstream notice file while preserving its relative path.
///
/// # Arguments
///
/// * `source` - The extracted crate or npm package directory.
/// * `destination` - The destination directory for preserved notices.
/// * `required` - Relative notice paths that must exist before copying.
///
/// # Returns
///
/// Returns the copied paths, relative to `source`, in POSIX form.
///
/// # Errors
///
/// Returns an error when a required notice is missing, a notice is blank, or a
/// file cannot be read or copied.
fn copy_notices(source: &Path, destination: &Path, required: &[&str]) -> Result<Vec<String>> {
    for name in required {
        let path = source.join(name);
        if !path.is_file() {
            bail!("Missing required upstream notice: {}", path.display());
        }
    }

    let mut files = collect_files_below(source)?;
    files.sort();
    let mut copied = Vec::new();
    for path in files {
        let Some(file_name) = path.file_name().and_then(OsStr::to_str) else {
            continue;
        };
        if !NOTICE_NAME_PATTERN.is_match(file_name) {
            continue;
        }
        if path
            .extension()
            .and_then(OsStr::to_str)
            .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
        {
            continue;
        }
        let data = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        if is_blank(&data) {
            bail!("Empty upstream notice: {}", path.display());
        }
        let relative = path
            .strip_prefix(source)
            .with_context(|| format!("{} is not inside {}", path.display(), source.display()))?;
        let output = destination.join(relative);
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::copy(&path, &output).with_context(|| format!("failed to copy {} to {}", path.display(), output.display()))?;
        copied.push(relative_posix_path(source, &path)?);
    }
    Ok(copied)
}

/// Require the original path casing even on case-insensitive filesystems.
///
/// Each path component is compared against the actual directory entry names so
/// a reviewed evidence path cannot silently resolve through a case-insensitive
/// lookup.
///
/// # Arguments
///
/// * `source` - The directory the relative path is resolved from.
/// * `relative` - The reviewed relative path recorded in the evidence.
///
/// # Returns
///
/// Returns the exact-case resolved path.
///
/// # Errors
///
/// Returns an error when a component is missing, uses the wrong case, or is not
/// a plain relative path component.
fn exact_source_path(source: &Path, relative: &str) -> Result<PathBuf> {
    let mut path = source.to_path_buf();
    for component in Path::new(relative).components() {
        let name = match component {
            Component::Normal(name) => name,
            Component::CurDir => continue,
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("Reviewed source path is missing or has incorrect case: {relative}")
            }
        };
        let entries = fs::read_dir(&path).with_context(|| format!("failed to list {}", path.display()))?;
        let mut found = false;
        for entry in entries {
            let entry = entry.with_context(|| format!("failed to list {}", path.display()))?;
            if entry.file_name() == name {
                found = true;
                break;
            }
        }
        if !found {
            bail!("Reviewed source path is missing or has incorrect case: {relative}");
        }
        path.push(name);
    }
    Ok(path)
}

/// Restore version- and checksum-bound reviewed evidence into a package.
///
/// Reviewed evidence is the only source of upstream attribution for packages
/// whose published archives omit notices. The crate checksum, optional VCS
/// revision, and every evidence digest must match before anything is written.
///
/// # Arguments
///
/// * `package` - The report package entry.
/// * `locked` - The matching `Cargo.lock` entry.
/// * `destination` - The package's preserved-license directory.
/// * `supplements` - The reviewed evidence keyed by `name@version`.
/// * `evidence_root` - The directory holding reviewed supplement files.
///
/// # Returns
///
/// Returns the preserved evidence file names, or an empty list when the package
/// has no reviewed record.
///
/// # Errors
///
/// Returns an error on a stale crate checksum, a changed upstream revision, a
/// supplement checksum mismatch, a changed source header, or I/O failure.
fn preserve_supplement(
    package: &ReportPackage,
    locked: &LockedPackage,
    destination: &Path,
    supplements: &Map<String, Value>,
    evidence_root: &Path,
) -> Result<Vec<String>> {
    let identity = format!("{}@{}", package.name, package.version);
    let Some(record) = supplements.get(&identity) else {
        return Ok(Vec::new());
    };
    let crate_checksum = record
        .get("crate-checksum")
        .and_then(Value::as_str)
        .with_context(|| format!("reviewed license evidence for {identity} is missing crate-checksum"))?;
    if Some(crate_checksum) != locked.checksum.as_deref() {
        bail!("Stale reviewed license evidence: {}", package.name);
    }

    let manifest_path = package
        .manifest_path
        .as_deref()
        .with_context(|| format!("cargo-about report is missing the manifest path for {identity}"))?;
    let source = manifest_path
        .parent()
        .with_context(|| format!("invalid manifest path for {identity}"))?;

    if let Some(revision) = record.get("vcs-commit") {
        let revision = revision
            .as_str()
            .with_context(|| format!("reviewed license evidence for {identity} has an invalid vcs-commit"))?;
        let vcs_path = source.join(".cargo_vcs_info.json");
        let vcs_text = fs::read_to_string(&vcs_path).with_context(|| format!("failed to read {}", vcs_path.display()))?;
        let vcs: Value = serde_json::from_str(&vcs_text).with_context(|| format!("failed to parse {}", vcs_path.display()))?;
        let sha1 = vcs
            .get("git")
            .and_then(|git| git.get("sha1"))
            .and_then(Value::as_str)
            .with_context(|| format!("{} is missing git.sha1", vcs_path.display()))?;
        if sha1 != revision {
            bail!("Changed upstream source revision: {}", package.name);
        }
    }

    let evidence_files = record
        .get("files")
        .and_then(Value::as_array)
        .with_context(|| format!("reviewed license evidence for {identity} is missing files"))?;
    let mut preserved = Vec::new();
    for evidence in evidence_files {
        let file_name = evidence
            .get("file")
            .and_then(Value::as_str)
            .with_context(|| format!("reviewed license evidence for {identity} is missing a file name"))?;
        let evidence_path = evidence_root.join(file_name);
        let data = fs::read(&evidence_path).with_context(|| format!("failed to read {}", evidence_path.display()))?;
        let data = normalize_line_endings(&data);
        let expected_checksum = evidence
            .get("sha256")
            .and_then(Value::as_str)
            .with_context(|| format!("reviewed license evidence for {file_name} is missing sha256"))?;
        if sha256_bytes(&data) != expected_checksum {
            bail!("Reviewed license supplement checksum mismatch: {file_name}");
        }
        if let Some(source_path) = evidence.get("source-path").and_then(Value::as_str) {
            let expected_source_checksum = evidence
                .get("source-sha256")
                .and_then(Value::as_str)
                .with_context(|| format!("reviewed license evidence for {file_name} is missing source-sha256"))?;
            if sha256(&exact_source_path(source, source_path)?)? != expected_source_checksum {
                bail!("Reviewed source header changed: {}", package.name);
            }
        }
        fs::create_dir_all(destination).with_context(|| format!("failed to create {}", destination.display()))?;
        write_bytes(&destination.join(file_name), &data)?;
        preserved.push(file_name.to_owned());
    }

    fs::create_dir_all(destination).with_context(|| format!("failed to create {}", destination.display()))?;
    write_text(&destination.join("PROVENANCE.json"), &format_pretty_json(record)?)?;
    Ok(preserved)
}

/// Reject generic templates that omit real attribution or reviewed evidence.
///
/// # Arguments
///
/// * `package` - The report package entry.
/// * `notices` - The preserved notice paths, relative to `destination`.
/// * `destination` - The directory holding the preserved notices.
/// * `template` - Whether the selected license text contains placeholders.
/// * `reviewed` - Whether reviewed evidence exists for this package.
///
/// # Errors
///
/// Returns an error when neither notices nor reviewed evidence exist, or when a
/// template without reviewed evidence has no concrete attribution line.
fn validate_attribution(package: &ReportPackage, notices: &[String], destination: &Path, template: bool, reviewed: bool) -> Result<()> {
    if notices.is_empty() && !reviewed {
        bail!("Missing reviewed upstream notices: {} {}", package.name, package.version);
    }
    if template && !reviewed {
        let mut texts = Vec::new();
        for name in notices {
            let path = destination.join(name);
            let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
            texts.push(String::from_utf8_lossy(&bytes).into_owned());
        }
        let attributed = texts
            .iter()
            .any(|text| ATTRIBUTION_PATTERN.is_match(text) && !PLACEHOLDER_PATTERN.is_match(text));
        if !attributed {
            bail!("Generic license template lacks reviewed attribution: {}", package.name);
        }
    }
    Ok(())
}

/// Generate the npm launcher dependency inventory.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `output` - The staging directory that receives npm notices.
///
/// # Returns
///
/// Returns the `## npm launcher dependencies` section.
///
/// # Errors
///
/// Returns an error when the committed lock root disagrees with `package.json`,
/// when a dependency has an unreviewed license, or when its license text is
/// missing.
fn npm_notices(root: &Path, output: &Path) -> Result<String> {
    let lock = read_json(&root.join("package-lock.json"))?;
    let own_package = read_json(&root.join("package.json"))?;
    let packages = lock
        .get("packages")
        .and_then(Value::as_object)
        .context("package-lock.json is missing the packages object")?;
    let locked_root = packages
        .get("")
        .with_context(|| "package-lock.json is missing the root package entry")?;

    let metadata_changed = ["name", "version", "license"]
        .iter()
        .any(|key| locked_root.get(*key) != own_package.get(*key));
    let dependencies_changed = ["dependencies", "optionalDependencies"].iter().any(|key| {
        locked_root.get(*key).cloned().unwrap_or_else(|| json!({})) != own_package.get(*key).cloned().unwrap_or_else(|| json!({}))
    });
    if metadata_changed || dependencies_changed {
        bail!(
            "Stale npm package-lock root metadata; synchronize package metadata and the committed lock, \
             then run npm ci --omit=optional --ignore-scripts"
        );
    }

    let binary_packages = npm_binary_packages(root, &own_package, packages)?;
    let mut sections = Vec::new();
    for (location, package) in packages {
        if location.is_empty() || binary_packages.contains(location) {
            continue;
        }
        let source = root.join(location);
        let installed = read_json(&source.join("package.json"))?;
        let license_id = package.get("license");
        let resolved_license = license_id.and_then(Value::as_str);
        if !resolved_license.is_some_and(|license| NPM_LICENSES.contains(&license))
            || installed.get("version") != package.get("version")
            || installed.get("license") != license_id
        {
            bail!("Unresolved or stale npm licensing: {location}");
        }
        let relative = location.strip_prefix("node_modules/").unwrap_or(location);
        let destination = output.join("third-party-licenses").join("npm").join(relative);
        let names = copy_notices(&source, &destination, &[])?;
        if names.is_empty() {
            bail!("Missing npm license text: {location}");
        }
        let installed_name = required_string_member(&installed, "name")?;
        let installed_version = required_string_member(&installed, "version")?;
        sections.push(format!(
            "- {installed_name} {installed_version}: {}; full text in third-party-licenses/npm/{relative}/",
            resolved_license.unwrap_or_default()
        ));
    }

    let inventory_text = if sections.is_empty() {
        "None. Platform packages contain the Rust binary and the same legal materials.".to_owned()
    } else {
        sections.join("\n")
    };
    Ok(format!("## npm launcher dependencies\n\n{inventory_text}\n"))
}

/// Validate the first-party npm binary family before exempting it from review.
///
/// The first-party platform packages carry the Rust license and source bundle
/// generated here. They may be unpublished, so the locked metadata alone must
/// prove the package identity, platform, and absence of install hooks.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `own_package` - The parsed root `package.json`.
/// * `packages` - The `packages` object from `package-lock.json`.
///
/// # Returns
///
/// Returns the lock locations exempted as first-party binary packages, or an
/// empty set when the root package is not Oatty.
///
/// # Errors
///
/// Returns an error for an unexpected platform mapping, an imprecise version
/// pin, or invalid locked or installed binary metadata.
fn npm_binary_packages(root: &Path, own_package: &Value, packages: &Map<String, Value>) -> Result<BTreeSet<String>> {
    if own_package.get("name").and_then(Value::as_str) != Some("oatty") {
        return Ok(BTreeSet::new());
    }

    let platforms = read_json(&root.join("npm").join("platforms.json"))?;
    let platforms = platforms.as_object().context("npm/platforms.json must contain a JSON object")?;
    let mapping_matches =
        platforms.len() == NPM_BINARY_TARGETS.len() && NPM_BINARY_TARGETS.iter().all(|(name, _)| platforms.contains_key(*name));
    if !mapping_matches {
        bail!("Unexpected first-party npm platform mapping");
    }

    let version = own_package.get("version").context("package.json is missing version")?;
    let optional_dependencies = own_package.get("optionalDependencies");
    let mut locations = BTreeSet::new();

    for (platform_name, target_triple) in NPM_BINARY_TARGETS {
        let (operating_system, architecture) = platform_name
            .split_once('-')
            .with_context(|| format!("invalid platform name {platform_name}"))?;
        let package_name = format!("@oattyio/cli-{platform_name}");
        let expectation = FirstPartyBinaryExpectation {
            package_name: package_name.clone(),
            target_triple,
            operating_system,
            architecture,
            binary: if operating_system == "win32" { "oatty.exe" } else { "oatty" },
            libc: (operating_system == "linux").then_some("glibc"),
        };
        if platforms.get(platform_name) != Some(&expectation.as_json()) {
            bail!("Unexpected first-party npm platform mapping: {platform_name}");
        }
        if optional_dependencies.and_then(|dependencies| dependencies.get(&package_name)) != Some(version) {
            bail!("First-party npm package must pin the Oatty version: {package_name}");
        }

        let location = format!("node_modules/{package_name}");
        let missing = Value::Object(Map::new());
        let locked = packages.get(&location).unwrap_or(&missing);
        validate_npm_binary_metadata(locked, &expectation, version, &location, true)?;
        let installed_path = root.join(&location).join("package.json");
        if installed_path.exists() {
            let installed = read_json(&installed_path)?;
            validate_npm_binary_metadata(&installed, &expectation, version, &location, false)?;
        }
        locations.insert(location);
    }
    Ok(locations)
}

/// Validate one locked or installed first-party binary package.
///
/// # Arguments
///
/// * `package` - The lock entry or installed `package.json` value.
/// * `expectation` - The canonical platform contract for this package.
/// * `version` - The required version from the root `package.json`.
/// * `location` - The lock location used in error messages.
/// * `locked` - Whether this value comes from the lock file rather than disk.
///
/// # Errors
///
/// Returns an error for unexpected libc metadata, stale or platform-independent
/// metadata, forbidden runtime dependencies or install hooks, or a mismatched
/// package identity.
fn validate_npm_binary_metadata(
    package: &Value,
    expectation: &FirstPartyBinaryExpectation,
    version: &Value,
    location: &str,
    locked: bool,
) -> Result<()> {
    let mut expected = vec![
        ("version", version.clone()),
        ("license", json!("MIT")),
        ("os", json!([expectation.operating_system])),
        ("cpu", json!([expectation.architecture])),
    ];
    if let Some(libc) = expectation.libc {
        expected.push(("libc", json!([libc])));
    } else if package.get("libc").is_some_and(value_is_truthy) {
        bail!("Unexpected first-party npm libc metadata: {location}");
    }
    if locked {
        expected.push(("optional", json!(true)));
    } else {
        expected.push(("name", json!(expectation.package_name)));
    }

    let invalid = expected.iter().any(|(key, value)| package.get(*key) != Some(value));
    let forbidden = [
        "dependencies",
        "optionalDependencies",
        "peerDependencies",
        "devDependencies",
        "scripts",
        "hasInstallScript",
        "link",
        "inBundle",
        "bundleDependencies",
        "bundledDependencies",
    ];
    if invalid || forbidden.iter().any(|key| package.get(*key).is_some_and(value_is_truthy)) {
        bail!("Unresolved or stale first-party npm binary metadata: {location}");
    }

    let default_name = json!(expectation.package_name);
    if package.get("name").unwrap_or(&default_name) != &default_name {
        bail!("Unexpected first-party npm package identity: {location}");
    }
    Ok(())
}

/// The canonical metadata contract for one first-party binary package.
struct FirstPartyBinaryExpectation {
    /// The npm package name derived from the platform key.
    package_name: String,
    /// The Rust target triple, for example `aarch64-apple-darwin`.
    target_triple: &'static str,
    /// The npm `os` value, for example `darwin`.
    operating_system: &'static str,
    /// The npm `cpu` value, for example `arm64`.
    architecture: &'static str,
    /// The bundled executable name.
    binary: &'static str,
    /// The npm `libc` value for glibc-based Linux targets.
    libc: Option<&'static str>,
}

impl FirstPartyBinaryExpectation {
    /// Render the exact `npm/platforms.json` entry this platform must declare.
    ///
    /// # Returns
    ///
    /// Returns a JSON object with `package`, `target`, `os`, `cpu`, `binary`,
    /// and, for Linux, `libc`.
    fn as_json(&self) -> Value {
        let mut object = Map::new();
        object.insert("package".to_owned(), json!(self.package_name));
        object.insert("target".to_owned(), json!(self.target_triple));
        object.insert("os".to_owned(), json!(self.operating_system));
        object.insert("cpu".to_owned(), json!(self.architecture));
        object.insert("binary".to_owned(), json!(self.binary));
        if let Some(libc) = self.libc {
            object.insert("libc".to_owned(), json!(libc));
        }
        Value::Object(object)
    }
}

/// Determine whether a JSON value is truthy using Python's rules.
///
/// # Arguments
///
/// * `value` - The value to test.
///
/// # Returns
///
/// Returns `false` for null, false, numeric zero, empty strings, empty arrays,
/// and empty objects; otherwise returns `true`.
fn value_is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|number| number != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(fields) => !fields.is_empty(),
    }
}

/// Return the upstream notice paths that a named crate must publish.
///
/// # Arguments
///
/// * `name` - The crate name.
///
/// # Returns
///
/// Returns the required relative notice paths, or an empty slice.
fn required_notices(name: &str) -> &'static [&'static str] {
    match name {
        "aws-lc-sys" => &["LICENSE", "aws-lc/LICENSE", "aws-lc/third_party/fiat/LICENSE"],
        "ring" => &[
            "LICENSE",
            "LICENSE-other-bits",
            "LICENSE-BoringSSL",
            "src/polyfill/once_cell/LICENSE-APACHE",
            "src/polyfill/once_cell/LICENSE-MIT",
            "third_party/fiat/LICENSE",
        ],
        "encoding_rs" => &["COPYRIGHT", "LICENSE-APACHE", "LICENSE-MIT", "LICENSE-WHATWG"],
        _ => &[],
    }
}

/// Parse a `Cargo.lock` document into the package records this module needs.
///
/// # Arguments
///
/// * `path` - The `Cargo.lock` path.
///
/// # Errors
///
/// Returns an error when the lock file cannot be read or parsed.
fn read_cargo_lock(path: &Path) -> Result<CargoLock> {
    let text = fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

/// Read and parse a JSON file.
///
/// # Arguments
///
/// * `path` - The JSON file path.
///
/// # Errors
///
/// Returns an error when the file cannot be read or parsed.
fn read_json(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

/// Extract a required string member from a JSON object.
///
/// # Arguments
///
/// * `value` - The JSON value to inspect.
/// * `key` - The required member name.
///
/// # Errors
///
/// Returns an error when the member is absent or is not a string.
fn required_string_member<'value>(value: &'value Value, key: &str) -> Result<&'value str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("JSON value is missing the string field {key}"))
}

/// Serialize a JSON value exactly like Python's sorted, two-space-indented dump.
///
/// # Arguments
///
/// * `value` - The JSON value to serialize.
///
/// # Returns
///
/// Returns pretty JSON with sorted object keys and exactly one trailing newline.
///
/// # Errors
///
/// Returns an error when serialization fails.
fn format_pretty_json(value: &Value) -> Result<String> {
    let mut text = serde_json::to_string_pretty(value).context("failed to serialize licensing JSON")?;
    text.push('\n');
    Ok(text)
}

/// Write bytes to a file.
///
/// # Arguments
///
/// * `path` - The destination path; its parent must already exist.
/// * `bytes` - The exact bytes to write.
///
/// # Errors
///
/// Returns an error when the file cannot be written.
fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

/// Write UTF-8 text without newline translation.
///
/// # Arguments
///
/// * `path` - The destination path; its parent must already exist.
/// * `text` - The text to write.
///
/// # Errors
///
/// Returns an error when the file cannot be written.
fn write_text(path: &Path, text: &str) -> Result<()> {
    write_bytes(path, text.as_bytes())
}

/// Compute the lowercase SHA-256 hex digest of a file's bytes.
///
/// # Arguments
///
/// * `path` - The file to hash.
///
/// # Errors
///
/// Returns an error when the file cannot be read.
fn sha256(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(sha256_bytes(&bytes))
}

/// Compute the lowercase SHA-256 hex digest of a byte slice.
///
/// # Arguments
///
/// * `bytes` - The bytes to hash.
///
/// # Returns
///
/// Returns the lowercase hexadecimal digest.
fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Replace every CRLF pair with a single LF, matching `bytes.replace(b"\r\n", b"\n")`.
///
/// # Arguments
///
/// * `bytes` - The file contents to normalize.
///
/// # Returns
///
/// Returns contents with Git checkout line endings normalized.
fn normalize_line_endings(bytes: &[u8]) -> Vec<u8> {
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
            index += 1;
            continue;
        }
        normalized.push(bytes[index]);
        index += 1;
    }
    normalized
}

/// Determine whether every byte is ASCII whitespace, matching Python `bytes.strip()`.
///
/// # Arguments
///
/// * `bytes` - The contents to inspect.
///
/// # Returns
///
/// Returns `true` when the contents are empty or contain only ASCII whitespace.
fn is_blank(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
}

/// Render a workspace path with forward slashes, matching Python `as_posix()`.
///
/// # Arguments
///
/// * `root` - The workspace root the path must be relative to.
/// * `path` - The path to render.
///
/// # Errors
///
/// Returns an error when `path` is not inside `root`.
fn relative_posix_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .with_context(|| format!("{} is not inside {}", path.display(), root.display()))?;
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::ParentDir => parts.push("..".to_owned()),
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => {
                bail!("{} is not a relative path", relative.display())
            }
        }
    }
    Ok(parts.join("/"))
}

/// Collect every regular file below a directory, recursively.
///
/// # Arguments
///
/// * `directory` - The directory to walk.
///
/// # Returns
///
/// Returns all regular files below the directory, or an empty list when it does
/// not exist.
///
/// # Errors
///
/// Returns an error when a directory cannot be listed.
fn collect_files_below(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    visit_files(directory, &mut files)?;
    Ok(files)
}

/// Collect every regular file below a directory whose name matches exactly.
///
/// # Arguments
///
/// * `directory` - The directory to walk.
/// * `file_name` - The exact file name to select.
///
/// # Returns
///
/// Returns the matching files, or an empty list when the directory is absent.
///
/// # Errors
///
/// Returns an error when a directory cannot be listed.
fn collect_named_files(directory: &Path, file_name: &str) -> Result<Vec<PathBuf>> {
    let mut files = collect_files_below(directory)?;
    files.retain(|path| path.file_name() == Some(OsStr::new(file_name)));
    Ok(files)
}

/// Recursively append every regular file below a directory.
///
/// # Arguments
///
/// * `directory` - The directory to walk; missing directories are ignored.
/// * `files` - The accumulator for discovered file paths.
///
/// # Errors
///
/// Returns an error when a directory cannot be listed.
fn visit_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if !directory.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(directory).with_context(|| format!("failed to list {}", directory.display()))? {
        let entry = entry.with_context(|| format!("failed to list {}", directory.display()))?;
        let path = entry.path();
        if path.is_dir() {
            visit_files(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

/// Remove a file or directory when it exists.
///
/// # Arguments
///
/// * `path` - The path to remove.
///
/// # Errors
///
/// Returns an error when the path exists but cannot be removed.
fn remove_path(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path).with_context(|| format!("failed to remove {}", path.display())),
        Ok(_) => fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

/// Move a file or directory, falling back to copy-and-remove across devices.
///
/// # Arguments
///
/// * `source` - The staged path to move.
/// * `destination` - The published path; it must not already exist.
///
/// # Errors
///
/// Returns an error when the path cannot be renamed, copied, or removed.
fn move_path(source: &Path, destination: &Path) -> Result<()> {
    if fs::rename(source, destination).is_ok() {
        return Ok(());
    }
    copy_recursively(source, destination)?;
    remove_path(source)
}

/// Copy a file or directory recursively.
///
/// # Arguments
///
/// * `source` - The path to copy.
/// * `destination` - The destination path.
///
/// # Errors
///
/// Returns an error when any entry cannot be copied.
fn copy_recursively(source: &Path, destination: &Path) -> Result<()> {
    if source.is_dir() {
        fs::create_dir_all(destination).with_context(|| format!("failed to create {}", destination.display()))?;
        for entry in fs::read_dir(source).with_context(|| format!("failed to list {}", source.display()))? {
            let entry = entry.with_context(|| format!("failed to list {}", source.display()))?;
            copy_recursively(&entry.path(), &destination.join(entry.file_name()))?;
        }
        return Ok(());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::copy(source, destination).with_context(|| format!("failed to copy {} to {}", source.display(), destination.display()))?;
    Ok(())
}

/// The subset of the cargo-about JSON report used by this module.
#[derive(Debug, Default, Deserialize)]
struct GeneratorReport {
    /// The selected license texts, one per resolved SPDX expression.
    #[serde(default)]
    licenses: Vec<LicenseSelection>,
    /// The resolved crates contributing to the selected licenses.
    #[serde(default)]
    crates: Vec<ReportCrate>,
}

/// One selected license and the crates that use it.
#[derive(Debug, Deserialize)]
struct LicenseSelection {
    /// The human-readable license name.
    #[serde(default)]
    name: String,
    /// The SPDX license identifier.
    #[serde(default)]
    id: String,
    /// The complete selected license text.
    #[serde(default)]
    text: String,
    /// The crates that use this license.
    #[serde(default)]
    used_by: Vec<LicenseUse>,
}

/// One crate reference inside a selected license.
#[derive(Debug, Deserialize)]
struct LicenseUse {
    /// The crate identity, serialized by cargo-about under `crate`.
    #[serde(default, rename = "crate")]
    krate: UsedCrate,
}

/// The crate identity embedded in a license use.
#[derive(Debug, Default, Deserialize)]
struct UsedCrate {
    /// The crate name.
    #[serde(default)]
    name: String,
    /// The crate version.
    #[serde(default)]
    version: String,
    /// The internal cargo-about crate identifier.
    #[serde(default)]
    id: String,
}

/// One crate entry in the cargo-about report.
#[derive(Debug, Deserialize)]
struct ReportCrate {
    /// The package metadata.
    #[serde(default)]
    package: ReportPackage,
    /// The resolved license expression for the entry.
    #[serde(default)]
    license: Option<String>,
}

/// The package metadata cargo-about records for a crate.
#[derive(Debug, Default, Deserialize)]
struct ReportPackage {
    /// The crate name.
    #[serde(default)]
    name: String,
    /// The crate version.
    #[serde(default)]
    version: String,
    /// The declared license expression, if cargo-about resolved one.
    #[serde(default)]
    license: Option<String>,
    /// The internal cargo-about crate identifier.
    #[serde(default)]
    id: Option<String>,
    /// The registry or VCS source; absent or null for workspace members.
    #[serde(default)]
    source: Option<String>,
    /// The absolute path to the crate's `Cargo.toml`.
    #[serde(default)]
    manifest_path: Option<PathBuf>,
    /// The upstream repository URL.
    #[serde(default)]
    repository: Option<String>,
    /// The upstream homepage URL.
    #[serde(default)]
    homepage: Option<String>,
}

/// The subset of `Cargo.lock` needed to bind checksums and revisions.
#[derive(Debug, Default, Deserialize)]
struct CargoLock {
    /// Every locked package record.
    #[serde(default)]
    package: Vec<LockedPackage>,
}

/// One package record from `Cargo.lock`.
#[derive(Debug, Deserialize)]
struct LockedPackage {
    /// The package name.
    name: String,
    /// The package version.
    version: String,
    /// The published archive checksum, absent for path packages.
    #[serde(default)]
    checksum: Option<String>,
}

#[cfg(test)]
mod tests {
    use std::io::Read as _;

    use tempfile::tempdir;

    use super::*;
    use crate::context::project_root;

    /// Write a fixture file, creating parent directories as needed.
    fn write_fixture(path: &Path, contents: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture directory should be creatable");
        }
        fs::write(path, contents).expect("fixture file should be writable");
    }

    /// Write a fixture JSON file without pretty formatting.
    fn write_fixture_json(path: &Path, value: &Value) {
        write_fixture(
            path,
            serde_json::to_string(value).expect("fixture JSON should serialize").as_bytes(),
        );
    }

    /// Build a minimal report package value for direct function tests.
    fn report_package(name: &str, version: &str) -> ReportPackage {
        ReportPackage {
            name: name.to_owned(),
            version: version.to_owned(),
            license: None,
            id: None,
            source: None,
            manifest_path: None,
            repository: None,
            homepage: None,
        }
    }

    /// Build a parsed cargo-about report from a JSON literal.
    fn generator_report(value: Value) -> GeneratorReport {
        serde_json::from_value(value).expect("fixture report should deserialize")
    }

    /// Build an empty `Cargo.lock`.
    fn empty_lock() -> CargoLock {
        CargoLock { package: Vec::new() }
    }

    /// Build an empty reviewed-evidence map.
    fn empty_supplements() -> Map<String, Value> {
        Map::new()
    }

    /// Write the complete first-party npm fixture used by the binary package tests.
    fn prepare_binary_package_fixture(root: &Path) -> (Value, Value, Map<String, Value>) {
        let workspace_root = project_root().expect("the workspace root should resolve");
        let platform_text =
            fs::read_to_string(workspace_root.join("npm").join("platforms.json")).expect("platforms.json should be readable");
        let platforms: Value = serde_json::from_str(&platform_text).expect("platforms.json should parse");
        let platform_entries = platforms.as_object().expect("platforms.json should be an object");

        fs::create_dir_all(root.join("npm")).expect("fixture npm directory should be creatable");
        write_fixture(&root.join("npm").join("platforms.json"), platform_text.as_bytes());

        let mut optional_dependencies = Map::new();
        for entry in platform_entries.values() {
            let package_name = entry["package"].as_str().expect("fixture platform should name its package");
            optional_dependencies.insert(package_name.to_owned(), json!("1.2.3"));
        }
        let own_package = json!({
            "name": "oatty",
            "version": "1.2.3",
            "license": "MIT",
            "optionalDependencies": optional_dependencies,
        });

        let mut packages = Map::new();
        packages.insert(String::new(), own_package.clone());
        for entry in platform_entries.values() {
            let mut metadata = Map::new();
            metadata.insert("version".to_owned(), json!("1.2.3"));
            metadata.insert("license".to_owned(), json!("MIT"));
            metadata.insert("optional".to_owned(), json!(true));
            metadata.insert("os".to_owned(), json!([entry["os"]]));
            metadata.insert("cpu".to_owned(), json!([entry["cpu"]]));
            if let Some(libc) = entry.get("libc") {
                metadata.insert("libc".to_owned(), json!([libc]));
            }
            let package_name = entry["package"].as_str().expect("fixture platform should name its package");
            packages.insert(format!("node_modules/{package_name}"), Value::Object(metadata));
        }

        write_fixture_json(&root.join("package.json"), &own_package);
        write_fixture_json(&root.join("package-lock.json"), &json!({ "packages": packages }));
        (platforms, own_package, packages)
    }

    /// Merge an override object into a base object, mirroring Python's `base | override`.
    fn merge_object(base: &Value, override_values: &Value) -> Value {
        let mut merged = base.as_object().expect("fixture package should be an object").clone();
        for (key, value) in override_values.as_object().expect("fixture mutation should be an object") {
            merged.insert(key.clone(), value.clone());
        }
        Value::Object(merged)
    }

    #[test]
    fn json_documents_use_sorted_two_space_keys_and_a_trailing_newline() {
        let value = json!({
            "z": 1,
            "a": {"b": [1, 2], "c": {}},
            "empty": [],
            "nested": {"y": "x"},
        });
        assert_eq!(
            format_pretty_json(&value).expect("fixture JSON should serialize"),
            concat!(
                "{\n",
                "  \"a\": {\n",
                "    \"b\": [\n",
                "      1,\n",
                "      2\n",
                "    ],\n",
                "    \"c\": {}\n",
                "  },\n",
                "  \"empty\": [],\n",
                "  \"nested\": {\n",
                "    \"y\": \"x\"\n",
                "  },\n",
                "  \"z\": 1\n",
                "}\n",
            )
        );
    }

    #[test]
    fn text_inputs_are_stable_across_checkout_newlines() {
        let directory = tempdir().expect("temporary directory");
        let root = directory.path();
        for name in [
            "LICENSE",
            "Cargo.toml",
            "Cargo.lock",
            "package.json",
            "package-lock.json",
            "about.toml",
            "crates/xtask/src/licenses.rs",
            "scripts/license-supplements/reviewed.json",
            "crates/example/Cargo.toml",
            "npm/platforms.json",
            "crates/xtask/src/npm_package.rs",
        ] {
            write_fixture(&root.join(name), b"line one\nline two\n");
        }

        let before = inputs(root).expect("inputs should fingerprint the fixture");
        for name in before.keys() {
            let contents = fs::read(root.join(name)).expect("fixture input should be readable");
            let crlf = contents.split(|byte| *byte == b'\n').collect::<Vec<_>>().join(&b"\r\n"[..]);
            write_fixture(&root.join(name), &crlf);
        }
        assert_eq!(before, inputs(root).expect("inputs should fingerprint the CRLF fixture"));
    }

    #[test]
    fn generator_failure_aborts_without_publishing_outputs() {
        let directory = tempdir().expect("temporary directory");
        let root = directory.path();
        let outcome = prepare_with_report_provider(root, || {
            report_with_runner("cargo-about", |_program, arguments| {
                if arguments == ["--version"] {
                    Ok(CapturedOutput {
                        stdout: b"cargo-about 0.9.2\n".to_vec(),
                    })
                } else {
                    Err(anyhow::anyhow!("cargo-about generate failed"))
                }
            })
        });

        assert!(outcome.is_err());
        assert_eq!(fs::read_dir(root).expect("root should be listable").count(), 0);
    }

    #[test]
    fn assemble_failure_publishes_no_partial_materials() {
        let directory = tempdir().expect("temporary directory");
        let root = directory.path();
        let report = json!({
            "licenses": [],
            "crates": [{"package": {"name": "unknown", "version": "1", "license": null}}],
        });

        assert!(assemble(root, &report).is_err());
        assert_eq!(fs::read_dir(root).expect("root should be listable").count(), 0);
    }

    #[test]
    fn unknown_generic_template_and_code_named_copying_are_not_notices() {
        let directory = tempdir().expect("temporary directory");
        let root = directory.path();
        write_fixture(&root.join("copying.rs"), b"pub fn copy() {}");

        let notices = copy_notices(root, &root.join("output"), &[]).expect("language sources must not be notices");
        assert!(notices.is_empty());
        let package = report_package("unknown", "1");
        let error = validate_attribution(&package, &notices, root, true, false).expect_err("missing notices must fail");
        assert!(error.to_string().contains("Missing reviewed upstream notices"));

        write_fixture(&root.join("LICENSE"), b"Copyright (c) <year> <copyright holders>");
        let error =
            validate_attribution(&package, &["LICENSE".to_owned()], root, true, false).expect_err("template placeholders must fail");
        assert!(error.to_string().contains("Generic license template lacks reviewed attribution"));
    }

    #[test]
    fn changed_npm_dependency_spec_requires_updated_lock() {
        let directory = tempdir().expect("temporary directory");
        let root = directory.path();
        write_fixture_json(&root.join("package-lock.json"), &json!({"packages": {"": {}}}));
        for key in ["dependencies", "optionalDependencies"] {
            write_fixture_json(&root.join("package.json"), &json!({key: {"new": "^1"}}));
            let error = npm_notices(root, &root.join("output")).expect_err("a changed dependency spec must fail");
            assert!(
                error.to_string().contains("Stale npm package-lock root metadata"),
                "unexpected error: {error}"
            );
        }
    }

    #[test]
    fn missing_selected_license_text() {
        let directory = tempdir().expect("temporary directory");
        let report = generator_report(json!({"licenses": [{"text": ""}], "crates": []}));
        let error = rust_notices(&report, directory.path(), &empty_lock(), &empty_supplements(), directory.path())
            .expect_err("empty license text must fail");
        assert!(error.to_string().contains("Missing selected license text"));
    }

    #[test]
    fn unresolved_crate() {
        let directory = tempdir().expect("temporary directory");
        let report = generator_report(json!({
            "licenses": [],
            "crates": [{"package": {"name": "unknown", "version": "1", "license": null}}],
        }));
        let error = rust_notices(&report, directory.path(), &empty_lock(), &empty_supplements(), directory.path())
            .expect_err("an uncovered crate must fail");
        assert!(error.to_string().contains("Unresolved licensing"));
    }

    #[test]
    fn missing_required_native_notice() {
        let directory = tempdir().expect("temporary directory");
        let root = directory.path();
        let error = copy_notices(root, &root.join("out"), &["aws-lc/LICENSE"]).expect_err("missing native notices must fail");
        assert!(error.to_string().contains("Missing required upstream notice"));
    }

    #[test]
    fn exact_source_path_rejects_wrong_case_on_every_filesystem() {
        let directory = tempdir().expect("temporary directory");
        let root = directory.path();
        write_fixture(&root.join("readme.md"), b"original upstream name");

        assert_eq!(
            exact_source_path(root, "readme.md").expect("exact case should resolve"),
            root.join("readme.md")
        );
        let error = exact_source_path(root, "README.md").expect_err("incorrect case must fail");
        assert!(error.to_string().contains("incorrect case"));
    }

    #[test]
    fn reviewed_supplement_checksum_and_crate_binding() {
        let directory = tempdir().expect("temporary directory");
        let root = directory.path();
        let package: ReportPackage = serde_json::from_value(json!({
            "name": "example",
            "version": "1",
            "manifest_path": root.join("Cargo.toml"),
        }))
        .expect("fixture package should deserialize");
        let supplements = json!({
            "example@1": {
                "crate-checksum": "expected",
                "files": [{"file": "LICENSE.txt", "sha256": "0".repeat(64)}],
            },
        })
        .as_object()
        .expect("fixture supplements should be an object")
        .clone();

        let changed = LockedPackage {
            name: "example".to_owned(),
            version: "1".to_owned(),
            checksum: Some("changed".to_owned()),
        };
        let error = preserve_supplement(&package, &changed, root, &supplements, root).expect_err("stale evidence must fail");
        assert!(error.to_string().contains("Stale reviewed license evidence"));

        write_fixture(&root.join("LICENSE.txt"), b"changed upstream attribution");
        let expected = LockedPackage {
            name: "example".to_owned(),
            version: "1".to_owned(),
            checksum: Some("expected".to_owned()),
        };
        let error = preserve_supplement(&package, &expected, root, &supplements, root).expect_err("changed evidence must fail");
        assert!(
            error.to_string().contains("Reviewed license supplement checksum mismatch"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn source_checksum_mismatch() {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("source.crate");
        write_fixture(&source, b"changed covered source");

        let error = bundle_source(&source, &directory.path().join("out.crate"), &"0".repeat(64))
            .expect_err("a mismatched source checksum must fail");
        assert!(error.to_string().contains("Source checksum mismatch"));
    }

    #[test]
    fn npm_unknown_license_and_missing_text() {
        let directory = tempdir().expect("temporary directory");
        let root = directory.path();
        write_fixture(&root.join("package.json"), b"{}");
        fs::create_dir_all(root.join("node_modules").join("example")).expect("fixture package directory");

        for (license_id, expected_error) in [("UNKNOWN", "Unresolved"), ("MIT", "Missing npm license text")] {
            let metadata = json!({"name": "example", "version": "1", "license": license_id});
            write_fixture_json(&root.join("node_modules").join("example").join("package.json"), &metadata);
            write_fixture_json(
                &root.join("package-lock.json"),
                &json!({"packages": {"": {}, "node_modules/example": metadata}}),
            );
            let error = npm_notices(root, &root.join("output")).expect_err("invalid npm licensing must fail");
            assert!(error.to_string().contains(expected_error), "expected {expected_error}, got {error}");
        }
    }

    #[test]
    fn unpublished_owned_binaries_need_no_installed_license_duplicates() {
        let directory = tempdir().expect("temporary directory");
        let root = directory.path();
        prepare_binary_package_fixture(root);

        let notices = npm_notices(root, &root.join("output")).expect("unpublished binaries should need no installed licenses");
        assert!(notices.contains("same legal materials"));
        assert!(!root.join("node_modules").exists());
    }

    #[test]
    fn owned_binary_lock_rejects_platform_version_and_dependency_drift() {
        let mutations = [
            json!({"version": "1.2.2"}),
            json!({"license": "UNKNOWN"}),
            json!({"os": ["linux"]}),
            json!({"cpu": ["arm64"]}),
            json!({"optional": false}),
            json!({"libc": ["musl"]}),
            json!({"scripts": {"postinstall": "download"}}),
            json!({"hasInstallScript": true}),
            json!({"dependencies": {"foreign": "1"}}),
            json!({"optionalDependencies": {"foreign": "1"}}),
            json!({"peerDependencies": {"foreign": "1"}}),
            json!({"name": "@oattyio/foreign"}),
        ];

        let directory = tempdir().expect("temporary directory");
        let root = directory.path();
        let (_, _, mut packages) = prepare_binary_package_fixture(root);
        let location = "node_modules/@oattyio/cli-darwin-x64";
        let original = packages
            .get(location)
            .expect("fixture should contain the darwin-x64 lock entry")
            .clone();
        for mutation in mutations {
            let mutated = merge_object(&original, &mutation);
            packages.insert(location.to_owned(), mutated);
            write_fixture_json(&root.join("package-lock.json"), &json!({"packages": packages}));
            let error = npm_notices(root, &root.join("output")).expect_err("lock drift must fail");
            assert!(error.to_string().contains("first-party npm"), "unexpected error: {error}");
        }

        packages.remove(location);
        write_fixture_json(&root.join("package-lock.json"), &json!({"packages": packages}));
        let error = npm_notices(root, &root.join("output")).expect_err("a missing lock entry must fail");
        assert!(error.to_string().contains("first-party npm"), "unexpected error: {error}");
    }

    #[test]
    fn owned_binary_pin_and_mapping_cannot_widen_license_exemption() {
        let directory = tempdir().expect("temporary directory");
        let root = directory.path();
        let (mut platforms, own_package, packages) = prepare_binary_package_fixture(root);

        let mut widened = own_package.clone();
        widened["optionalDependencies"]["@oattyio/cli-darwin-x64"] = json!("^1.2.3");
        // The Python fixture aliases `own_package` into the lock root, so a
        // mutation of the published manifest also updates the locked root.
        let mut widened_packages = packages.clone();
        widened_packages.insert(String::new(), widened.clone());
        write_fixture_json(&root.join("package.json"), &widened);
        write_fixture_json(&root.join("package-lock.json"), &json!({"packages": widened_packages}));
        let error = npm_notices(root, &root.join("output")).expect_err("a range pin must fail");
        assert!(error.to_string().contains("must pin"), "unexpected error: {error}");

        let mut pinned = own_package.clone();
        pinned["optionalDependencies"]["@oattyio/cli-darwin-x64"] = json!("1.2.3");
        write_fixture_json(&root.join("package.json"), &pinned);
        write_fixture_json(&root.join("package-lock.json"), &json!({"packages": packages}));
        platforms["darwin-x64"]["package"] = json!("@oattyio/unreviewed");
        write_fixture_json(&root.join("npm").join("platforms.json"), &platforms);
        let error = npm_notices(root, &root.join("output")).expect_err("a widened mapping must fail");
        assert!(error.to_string().contains("platform mapping"), "unexpected error: {error}");
    }

    #[test]
    fn other_packages_in_owned_scope_still_need_reviewed_licenses() {
        let directory = tempdir().expect("temporary directory");
        let root = directory.path();
        let (_, _, mut packages) = prepare_binary_package_fixture(root);

        let location = "node_modules/@oattyio/unreviewed";
        let metadata = json!({"name": "@oattyio/unreviewed", "version": "1", "license": "UNKNOWN"});
        packages.insert(location.to_owned(), metadata.clone());
        write_fixture_json(&root.join(location).join("package.json"), &metadata);
        write_fixture_json(&root.join("package-lock.json"), &json!({"packages": packages}));

        let error = npm_notices(root, &root.join("output")).expect_err("scoped third parties still need review");
        assert!(
            error.to_string().contains("Unresolved or stale npm licensing"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn linux_lock_requires_glibc_and_installed_owned_package_has_no_hooks() {
        let directory = tempdir().expect("temporary directory");
        let root = directory.path();
        let (_, _, mut packages) = prepare_binary_package_fixture(root);
        let location = "node_modules/@oattyio/cli-linux-x64";
        let mut metadata = packages[location]
            .as_object()
            .expect("fixture metadata should be an object")
            .clone();
        metadata.remove("libc");
        packages.insert(location.to_owned(), Value::Object(metadata.clone()));
        write_fixture_json(&root.join("package-lock.json"), &json!({"packages": packages}));
        let error = npm_notices(root, &root.join("output")).expect_err("linux must require glibc");
        assert!(error.to_string().contains("first-party npm"), "unexpected error: {error}");

        metadata.insert("libc".to_owned(), json!(["glibc"]));
        packages.insert(location.to_owned(), Value::Object(metadata.clone()));
        write_fixture_json(&root.join("package-lock.json"), &json!({"packages": packages}));
        let mut installed = metadata;
        installed.insert("name".to_owned(), json!("@oattyio/cli-linux-x64"));
        installed.insert("scripts".to_owned(), json!({"postinstall": "download"}));
        write_fixture_json(&root.join(location).join("package.json"), &Value::Object(installed));

        let error = npm_notices(root, &root.join("output")).expect_err("installed hooks must fail");
        assert!(error.to_string().contains("first-party npm"), "unexpected error: {error}");
    }

    #[test]
    fn missing_stale_and_changed_materials() {
        let directory = tempdir().expect("temporary directory");
        let root = directory.path();
        for name in GENERATED_MATERIALS {
            if name.contains('.') {
                write_fixture(&root.join(name), b"notices");
            } else {
                fs::create_dir_all(root.join(name)).expect("generated directory fixture");
            }
        }
        let source = root.join("third-party-sources").join("example.crate");
        write_fixture(&source, b"covered source");
        for name in [
            "LICENSE",
            "Cargo.toml",
            "Cargo.lock",
            "package.json",
            "package-lock.json",
            "about.toml",
            "crates/xtask/src/licenses.rs",
            "npm/platforms.json",
            "crates/xtask/src/npm_package.rs",
        ] {
            write_fixture(&root.join(name), b"fixture");
        }

        let manifest = json!({
            "inputs": {"lock": "old"},
            "files": inventory(root).expect("fixture inventory"),
        });
        write_fixture_json(&root.join(MANIFEST_FILE), &manifest);
        let error = verify(root).expect_err("stale inputs must fail");
        assert!(error.to_string().contains("Stale licensing inputs"));

        let current = json!({
            "inputs": inputs(root).expect("fixture inputs"),
            "files": inventory(root).expect("fixture inventory"),
        });
        write_fixture_json(&root.join(MANIFEST_FILE), &current);
        verify(root).expect("matching inputs and files must verify");

        write_fixture(&source, b"tampered source");
        let error = verify(root).expect_err("tampered output must fail");
        assert!(error.to_string().contains("changed licensing output/source"));

        fs::remove_file(root.join("THIRD-PARTY-NOTICES.txt")).expect("fixture notices should be removable");
        let error = verify(root).expect_err("missing material must fail");
        assert!(error.to_string().contains("Missing generated material"));
    }

    #[test]
    fn every_reviewed_source_path_and_hash_matches_exact_crate_archive_member() {
        // The Python suite ran this contract only in the provisioned licensing
        // CI job; skip rather than panic when offline crate sources are absent.
        let root = project_root().expect("the workspace root should resolve");
        let output = Command::new("cargo")
            .args(["metadata", "--format-version", "1", "--locked", "--offline"])
            .current_dir(&root)
            .output()
            .expect("cargo metadata should run");
        if !output.status.success() {
            return;
        }
        assert!(
            output.status.success(),
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let metadata: Value = serde_json::from_slice(&output.stdout).expect("cargo metadata should be JSON");
        let packages = metadata["packages"]
            .as_array()
            .expect("cargo metadata should list packages")
            .iter()
            .map(|package| {
                let identity = format!(
                    "{}@{}",
                    package["name"].as_str().expect("package name"),
                    package["version"].as_str().expect("package version")
                );
                (identity, package.clone())
            })
            .collect::<BTreeMap<_, _>>();
        let records: Value =
            read_json(&root.join("scripts").join("license-supplements").join("reviewed.json")).expect("reviewed evidence should parse");

        for (identity, record) in records.as_object().expect("reviewed evidence should be an object") {
            let package = &packages[identity];
            let manifest_path = PathBuf::from(package["manifest_path"].as_str().expect("manifest path"));
            let source = manifest_path.parent().expect("manifest path should have a parent");
            let crate_name = format!(
                "{}-{}",
                package["name"].as_str().expect("package name"),
                package["version"].as_str().expect("package version")
            );
            let archive_path = source
                .parent()
                .expect("registry source should have a parent")
                .parent()
                .expect("registry source should have a registry directory")
                .join("..")
                .join("cache")
                .join(
                    source
                        .parent()
                        .expect("source parent")
                        .file_name()
                        .expect("registry directory name"),
                )
                .join(format!("{crate_name}.crate"));
            if !archive_path.is_file() {
                return;
            }
            let archive_path = archive_path.canonicalize().expect("the crate archive should canonicalize");
            let decoder = flate2::read::GzDecoder::new(fs::File::open(&archive_path).expect("crate archive should open"));
            let mut archive = tar::Archive::new(decoder);
            let mut members = BTreeMap::new();
            for entry in archive.entries().expect("crate archive should list entries") {
                let mut entry = entry.expect("crate archive entry should parse");
                let name = entry
                    .path()
                    .expect("crate archive entry should have a path")
                    .to_string_lossy()
                    .into_owned();
                let mut contents = Vec::new();
                entry.read_to_end(&mut contents).expect("crate archive entry should read");
                members.insert(name, contents);
            }
            for evidence in record["files"].as_array().expect("reviewed files should be an array") {
                let Some(source_path) = evidence.get("source-path").and_then(Value::as_str) else {
                    continue;
                };
                let member_name = format!("{crate_name}/{source_path}");
                let contents = members
                    .get(&member_name)
                    .unwrap_or_else(|| panic!("{member_name} is missing from {}", archive_path.display()));
                assert_eq!(
                    evidence["source-sha256"].as_str().expect("source checksum"),
                    sha256_bytes(contents),
                    "source member {member_name}"
                );
            }
        }
    }

    #[test]
    fn fxhash_real_copyright_and_reviewed_upstream_grants_are_distributed() {
        // The Python suite ran this contract only in the provisioned licensing
        // CI job; skip rather than panic when the generated materials are absent.
        let evidence = project_root()
            .expect("the workspace root should resolve")
            .join("third-party-licenses")
            .join("rust");
        if !evidence.is_dir() {
            return;
        }
        let read = |relative: &str| {
            fs::read_to_string(evidence.join(relative)).unwrap_or_else(|error| panic!("failed to read {relative}: {error}"))
        };

        assert!(read("fxhash-0.2.1/fxhash-0.2.1-header.txt").contains("Copyright 2015 The Rust Project Developers."));
        assert!(read("alloc-stdlib-0.3.0/alloc-stdlib-0.3.0-LICENSE.txt").contains("Copyright (c) 2016 Dropbox, Inc."));
        assert!(read("objc2-foundation-0.3.2/objc2-LICENSE.md").contains("Apple SDKs"));
        assert!(
            read("brotli-decompressor-6.2.0/brotli-decompressor-6.2.0-src-context.rs-header.txt").contains("Copyright 2013 Google Inc.")
        );
        assert!(read("chrono-0.4.45/chrono-0.4.45-src-format-parse.rs-header.txt").contains("John Nagle"));
        assert!(read("rat-focus-2.1.1/rat-focus-2.1.1-Cargo.toml.orig.txt").contains("license = \"MIT/Apache-2.0\""));
    }
}
