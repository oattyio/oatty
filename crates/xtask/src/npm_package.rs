//! Pack script-free npm packages from verified legal files and unchanged executables.
//!
//! Ported from `scripts/package-npm.py`. Packaging always runs `npm pack` against an
//! isolated staging directory with lifecycle scripts explicitly disabled, and every
//! produced archive is re-verified before its path is returned. Behavior is
//! fail-closed throughout: licensing materials must verify before packing, every
//! archive is re-verified against the current legal bytes afterwards, signed
//! executable bytes must stay byte-for-byte identical, and tar metadata is normalized
//! so that npm archives built on Windows still carry Unix execute bits.
//!
//! `npm_publish` consumes [`npm_command`], [`root_manifest`], [`load_platforms`], and
//! [`verify_artifact`], so keep their signatures stable.
//!
//! # Testability
//!
//! Licensing materials and the npm toolchain are reachable only through
//! [`crate::licenses::verify`], [`crate::licenses::inventory`], and a child process.
//! [`PackagingHooks`] injects all three so unit tests run against deterministic
//! stand-ins instead of the real materials and the real npm installation.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use anyhow::{Context as _, Result, anyhow, bail};
use flate2::Compression;
use flate2::GzBuilder;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tar::Header;

/// The `package/` prefix every npm tarball member carries.
const PACKAGE_PREFIX: &str = "package/";

/// The mode every shipped executable member must carry.
const EXECUTABLE_MODE: u32 = 0o755;

/// The static installed-version marker shipped inside every native payload.
const INSTALLED_VERSION_MARKER: &str = "bin/.oatty-installed-version";

/// The file name of the installed-version marker inside the `bin` directory.
const INSTALLED_VERSION_MARKER_FILE_NAME: &str = ".oatty-installed-version";

/// The published launcher script inside the umbrella package.
const LAUNCHER_NAME: &str = "npm/bin/oatty.js";

/// The canonical platform-to-package mapping shipped with the umbrella package.
const PLATFORM_INDEX_NAME: &str = "npm/platforms.json";

/// The documentation file shipped with every package.
const README_NAME: &str = "README.md";

/// The first-party license file shipped with every package.
const LICENSE_NAME: &str = "LICENSE";

/// The three source files that make up the umbrella launcher package.
const UMBRELLA_FILES: [&str; 3] = [LAUNCHER_NAME, PLATFORM_INDEX_NAME, README_NAME];

/// First-party checkout text files whose line endings are canonicalized to LF.
///
/// Upstream legal materials keep their authoritative bytes verbatim, so they are
/// deliberately absent from this list.
const CANONICAL_TEXT_FILES: [&str; 4] = [LICENSE_NAME, LAUNCHER_NAME, PLATFORM_INDEX_NAME, README_NAME];

/// The exact prepack licensing guard the root manifest must declare.
const REQUIRED_PREPACK_GUARD: &str = "cargo xtask licenses verify";

/// The npm lifecycle hooks that must never run while publishing this package.
const FORBIDDEN_LIFECYCLE_SCRIPTS: [&str; 9] = [
    "preinstall",
    "install",
    "postinstall",
    "prepare",
    "postpack",
    "prepublish",
    "prepublishOnly",
    "publish",
    "postpublish",
];

/// The canonical npm platform keys.
const PLATFORM_KEYS: [&str; 4] = ["darwin-x64", "darwin-arm64", "linux-x64", "win32-x64"];

/// The canonical platform contract for every supported key.
///
/// Each entry is `(key, target triple, os, cpu)`. The package name, executable name,
/// and `libc` value are derived from those values.
const CANONICAL_PLATFORMS: [(&str, &str, &str, &str); 4] = [
    ("darwin-x64", "x86_64-apple-darwin", "darwin", "x64"),
    ("darwin-arm64", "aarch64-apple-darwin", "darwin", "arm64"),
    ("linux-x64", "x86_64-unknown-linux-gnu", "linux", "x64"),
    ("win32-x64", "x86_64-pc-windows-msvc", "win32", "x64"),
];

/// One canonical native npm platform contract.
///
/// The fields mirror `npm/platforms.json`, with `libc` present only for
/// glibc-based Linux targets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Platform {
    /// The npm package name, for example `@oattyio/cli-darwin-arm64`.
    #[serde(rename = "package")]
    pub package_name: String,
    /// The Rust target triple, for example `aarch64-apple-darwin`.
    #[serde(rename = "target")]
    pub target_triple: String,
    /// The npm `os` value, for example `darwin`.
    pub os: String,
    /// The npm `cpu` value, for example `arm64`.
    pub cpu: String,
    /// The npm `libc` value when the platform targets glibc.
    pub libc: Option<String>,
    /// The bundled executable name, `oatty` or `oatty.exe`.
    pub binary: String,
}

/// The npm executable invocation, honoring the documented `NPM_CLI` override.
///
/// `NPM_CLI` may name an `npm-cli.js` file to run with Node, which lets a CI job pin
/// a specific npm version. On Windows npm is normally a `cmd` shim, so the sibling
/// `node_modules/npm/bin/npm-cli.js` behind the shim is used when present.
///
/// # Returns
///
/// Returns the argv prefix used to invoke npm, either `node <npm-cli.js>` or `<npm>`
/// depending on the environment.
///
/// # Errors
///
/// Returns an error on Windows when `NPM_CLI` is unset and no npm shim exposing
/// `node_modules/npm/bin/npm-cli.js` can be found. The message is exactly
/// `Cannot locate npm-cli.js; set NPM_CLI to its absolute path` so callers can
/// surface it verbatim.
pub(crate) fn npm_command() -> anyhow::Result<Vec<String>> {
    if let Some(override_path) = env::var_os("NPM_CLI").filter(|value| !value.is_empty()) {
        return Ok(vec!["node".to_owned(), override_path.to_string_lossy().into_owned()]);
    }
    platform_npm_command()
}

/// Return the platform-specific npm invocation used when `NPM_CLI` is unset.
///
/// # Returns
///
/// Returns `node <npm-cli.js>` on Windows when the npm shim exposes the CLI file, and
/// the plain `npm` program everywhere else.
///
/// # Errors
///
/// Returns an error on Windows when no npm shim exposing
/// `node_modules/npm/bin/npm-cli.js` can be found.
fn platform_npm_command() -> Result<Vec<String>> {
    if env::consts::OS != "windows" {
        return Ok(vec!["npm".to_owned()]);
    }
    let cli_path = find_executable_on_path("npm")
        .and_then(|wrapper| wrapper.parent().map(Path::to_path_buf))
        .map(|directory| directory.join("node_modules").join("npm").join("bin").join("npm-cli.js"));
    if let Some(cli_path) = cli_path.filter(|candidate| candidate.is_file()) {
        return Ok(vec!["node".to_owned(), cli_path.to_string_lossy().into_owned()]);
    }
    Err(anyhow!("Cannot locate npm-cli.js; set NPM_CLI to its absolute path"))
}

/// Locate an executable on the `PATH`, honoring `PATHEXT` on Windows.
///
/// # Arguments
///
/// * `program` - The executable name to locate, for example `npm`.
///
/// # Returns
///
/// Returns the first matching absolute path, or `None` when no candidate exists.
fn find_executable_on_path(program: &str) -> Option<PathBuf> {
    let search_path = env::var_os("PATH")?;
    let extensions = env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned());
    env::split_paths(&search_path).find_map(|directory| find_executable_in_directory(&directory, program, &extensions))
}

/// Locate an executable inside one `PATH` directory.
///
/// # Arguments
///
/// * `directory` - The `PATH` directory to inspect.
/// * `program` - The executable name to locate.
/// * `extensions` - The Windows `PATHEXT` extension list, for example `.EXE;.CMD`.
///
/// # Returns
///
/// Returns the matching path, preferring the extensionless name, or `None`.
fn find_executable_in_directory(directory: &Path, program: &str, extensions: &str) -> Option<PathBuf> {
    let direct = directory.join(program);
    if direct.is_file() {
        return Some(direct);
    }
    extensions
        .split(';')
        .filter(|extension| !extension.is_empty())
        .map(|extension| directory.join(format!("{program}{extension}")))
        .find(|candidate| candidate.is_file())
}

/// Read and validate the canonical four supported package/target contracts.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
///
/// # Returns
///
/// Returns a map from platform key (for example `darwin-x64`) to its contract.
///
/// # Errors
///
/// Returns an error when platforms.json is missing, malformed, or deviates from the
/// canonical contract.
pub(crate) fn load_platforms(root: &Path) -> Result<BTreeMap<String, Platform>> {
    let platform_index_path = root.join(PLATFORM_INDEX_NAME);
    let encoded = fs::read(&platform_index_path).with_context(|| format!("failed to read {}", platform_index_path.display()))?;
    let platforms: BTreeMap<String, Platform> =
        serde_json::from_slice(&encoded).with_context(|| format!("failed to parse {}", platform_index_path.display()))?;
    let keys_match = platforms.len() == PLATFORM_KEYS.len() && PLATFORM_KEYS.iter().all(|key| platforms.contains_key(*key));
    if !keys_match {
        bail!("Expected exactly four canonical npm platforms");
    }
    for (key, platform) in &platforms {
        if platform != &canonical_platform(key)? {
            bail!("Invalid canonical platform metadata: {key}");
        }
    }
    Ok(platforms)
}

/// Build the canonical contract for one supported platform key.
///
/// # Arguments
///
/// * `key` - The npm platform key, for example `linux-x64`.
///
/// # Returns
///
/// Returns the exact contract `npm/platforms.json` must declare for that key.
///
/// # Errors
///
/// Returns an error when the key is not one of the four canonical platform keys.
fn canonical_platform(key: &str) -> Result<Platform> {
    let (_, target_triple, os, cpu) = CANONICAL_PLATFORMS
        .iter()
        .find(|(candidate_key, _, _, _)| *candidate_key == key)
        .ok_or_else(|| anyhow!("Invalid canonical platform key: {key}"))?;
    Ok(Platform {
        package_name: format!("@oattyio/cli-{key}"),
        target_triple: (*target_triple).to_owned(),
        os: (*os).to_owned(),
        cpu: (*cpu).to_owned(),
        libc: (*os == "linux").then(|| "glibc".to_owned()),
        binary: if *os == "win32" {
            "oatty.exe".to_owned()
        } else {
            "oatty".to_owned()
        },
    })
}

/// Require exact native optional versions and prohibit runtime dependencies/hooks.
///
/// The root manifest is the published launcher contract, so it must pin every
/// canonical native package to the exact release version, declare the licensing
/// prepack guard, avoid forbidden lifecycle hooks, and carry the Oatty identity.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
///
/// # Returns
///
/// Returns the parsed root package.json manifest.
///
/// # Errors
///
/// Returns an error when the root manifest deviates from the published package
/// contract.
pub(crate) fn root_manifest(root: &Path) -> Result<Value> {
    let manifest_path = root.join("package.json");
    let encoded = fs::read(&manifest_path).with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: Value = serde_json::from_slice(&encoded).with_context(|| format!("failed to parse {}", manifest_path.display()))?;

    let version = manifest
        .get("version")
        .cloned()
        .context("Root package manifest must declare a version")?;
    let mut expected_optional_dependencies = Map::new();
    for platform in load_platforms(root)?.values() {
        expected_optional_dependencies.insert(platform.package_name.clone(), version.clone());
    }
    let expected_optional_dependencies = Value::Object(expected_optional_dependencies);
    let has_unexpected_optional_dependencies = manifest.get("optionalDependencies") != Some(&expected_optional_dependencies);
    let has_runtime_dependencies = manifest
        .get("dependencies")
        .is_some_and(|dependencies| !is_absent_dependency_list(dependencies));
    if has_unexpected_optional_dependencies || has_runtime_dependencies {
        bail!("Root package must contain only exact-version native optional dependencies");
    }

    let scripts = manifest.get("scripts").and_then(Value::as_object);
    let prepack_guard = scripts.and_then(|entries| entries.get("prepack")).and_then(Value::as_str);
    if prepack_guard != Some(REQUIRED_PREPACK_GUARD) {
        bail!("Required source prepack licensing guard is missing");
    }
    if scripts.is_some_and(|entries| entries.keys().any(|name| FORBIDDEN_LIFECYCLE_SCRIPTS.contains(&name.as_str()))) {
        bail!("npm lifecycle hooks are forbidden");
    }

    let identity_matches = manifest.get("repository") == Some(&repository_object())
        && manifest.get("name").and_then(Value::as_str) == Some("oatty")
        && manifest.get("license").and_then(Value::as_str) == Some("MIT");
    if !identity_matches {
        bail!("Incorrect Oatty package identity, repository, or license");
    }
    Ok(manifest)
}

/// Report whether a dependency declaration is absent or explicitly empty.
///
/// # Arguments
///
/// * `dependencies` - The `dependencies` value from a package manifest.
///
/// # Returns
///
/// Returns `true` for a missing `null` value or an empty object.
fn is_absent_dependency_list(dependencies: &Value) -> bool {
    match dependencies {
        Value::Null => true,
        Value::Object(entries) => entries.is_empty(),
        _ => false,
    }
}

/// Build the exact repository object every published manifest must declare.
///
/// # Returns
///
/// Returns the `{type, url}` object matching the canonical Oatty repository.
fn repository_object() -> Value {
    json!({
        "type": "git",
        "url": "git+https://github.com/oattyio/oatty.git",
    })
}

/// Pack one signed/notarized binary unchanged, with legal files and version marker.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `binary` - The native executable to embed.
/// * `target` - The Rust target triple the executable was built for.
/// * `destination` - The directory to write the tarball into.
///
/// # Returns
///
/// Returns the absolute path of the packed, verified tarball.
///
/// # Errors
///
/// Returns an error when licensing verification fails or the packed artifact changes
/// executable bytes.
pub(crate) fn package_binary(root: &Path, binary: &Path, target: &str, destination: &Path) -> Result<PathBuf> {
    package_binary_with(root, binary, target, destination, &PackagingHooks::production())
}

/// Pack only the launcher, canonical mapping, documentation, and legal materials.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `destination` - The directory to write the tarball into.
///
/// # Returns
///
/// Returns the absolute path of the packed, verified tarball.
///
/// # Errors
///
/// Returns an error when licensing verification fails or verification of the packed
/// artifact fails.
pub(crate) fn package_umbrella(root: &Path, destination: &Path) -> Result<PathBuf> {
    package_umbrella_with(root, destination, &PackagingHooks::production())
}

/// Validate metadata, exact package layout, legal bytes, and executable mode.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `archive` - The npm tarball to validate.
/// * `platform` - The platform contract for a native payload, or `None` for the
///   umbrella launcher package.
///
/// # Returns
///
/// Returns the parsed package.json manifest from the archive.
///
/// # Errors
///
/// Returns an error when the archive layout, manifest, or byte contents deviate from
/// the expected package contract.
pub(crate) fn verify_artifact(root: &Path, archive: &Path, platform: Option<&Platform>) -> Result<Value> {
    let (_, manifest) = verify_artifact_with(root, archive, platform, &crate::licenses::inventory)?;
    Ok(manifest)
}

/// The replaceable collaborators npm packaging depends on.
///
/// Production code uses [`PackagingHooks::production`]. Unit tests substitute a no-op
/// licensing verifier, a deterministic licensing inventory, a fixed npm argv prefix,
/// and an in-process npm runner so they never require the real licensing materials,
/// the real npm toolchain, or network access.
struct PackagingHooks<'a> {
    /// Rejects missing, stale, edited, or extra licensing materials.
    verify_licenses: &'a dyn Fn(&Path) -> Result<()>,
    /// Returns the relative-path-to-sha256 inventory of generated materials.
    inventory_legal_files: &'a dyn Fn(&Path) -> Result<BTreeMap<String, String>>,
    /// Returns the npm argv prefix to execute.
    npm_command: &'a dyn Fn() -> Result<Vec<String>>,
    /// Executes the npm argv prefix in the supplied working directory.
    run_npm: &'a dyn Fn(&[String], &Path) -> Result<CommandOutcome>,
}

impl PackagingHooks<'static> {
    /// Build the hooks used outside tests.
    ///
    /// # Returns
    ///
    /// Returns hooks backed by the real licensing module and the real npm process.
    fn production() -> Self {
        Self {
            verify_licenses: &crate::licenses::verify,
            inventory_legal_files: &crate::licenses::inventory,
            npm_command: &npm_command,
            run_npm: &run_npm_command,
        }
    }
}

/// The captured outcome of one npm child process.
struct CommandOutcome {
    /// Whether npm reported a successful exit status.
    exit_success: bool,
    /// The standard output captured from npm.
    standard_output: Vec<u8>,
    /// The standard error captured from npm, decoded lossily for messages.
    standard_error: String,
}

/// Execute npm and capture its output.
///
/// Lifecycle scripts and the npm cache are pinned through the environment so a
/// staging directory can never execute package scripts or touch a shared cache.
///
/// # Arguments
///
/// * `arguments` - The full argv prefix, whose first element is the program.
/// * `working_directory` - The isolated staging directory npm runs inside.
///
/// # Returns
///
/// Returns the captured exit status and output.
///
/// # Errors
///
/// Returns an error when the child process cannot be spawned.
fn run_npm_command(arguments: &[String], working_directory: &Path) -> Result<CommandOutcome> {
    let (program, program_arguments) = arguments.split_first().ok_or_else(|| anyhow!("The npm command is empty"))?;
    let output = Command::new(program)
        .args(program_arguments)
        .current_dir(working_directory)
        .env("npm_config_ignore_scripts", "true")
        .env("npm_config_cache", working_directory.join(".npm-cache"))
        .output()
        .with_context(|| format!("failed to execute {program} in {}", working_directory.display()))?;
    Ok(CommandOutcome {
        exit_success: output.status.success(),
        standard_output: output.stdout,
        standard_error: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// Pack one native payload using the supplied collaborators.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `binary` - The native executable to embed unchanged.
/// * `target` - The Rust target triple the executable was built for.
/// * `destination` - The directory to write the tarball into.
/// * `hooks` - The licensing and npm collaborators, injectable for tests.
///
/// # Returns
///
/// Returns the absolute path of the packed, verified tarball.
///
/// # Errors
///
/// Returns an error when licensing verification fails, the target is unsupported, npm
/// fails, or the packed executable bytes differ from the input bytes.
fn package_binary_with(root: &Path, binary: &Path, target: &str, destination: &Path, hooks: &PackagingHooks<'_>) -> Result<PathBuf> {
    (hooks.verify_licenses)(root)?;
    let platforms = load_platforms(root)?;
    let platform = platforms
        .values()
        .find(|candidate| candidate.target_triple == target)
        .ok_or_else(|| anyhow!("Unsupported Rust target: {target}"))?;

    let stage_directory = tempfile::Builder::new()
        .prefix("oatty-npm-native-")
        .tempdir()
        .context("failed to create the npm staging directory")?;
    let stage = stage_directory.path();
    stage_legal_files(root, stage, hooks.inventory_legal_files)?;
    write_manifest(stage, &payload_manifest(root, platform)?)?;

    let bin_directory = stage.join("bin");
    fs::create_dir(&bin_directory).with_context(|| format!("failed to create {}", bin_directory.display()))?;
    let staged_executable = bin_directory.join(&platform.binary);
    fs::copy(binary, &staged_executable)
        .with_context(|| format!("failed to stage {} as {}", binary.display(), staged_executable.display()))?;
    set_executable_mode(&staged_executable)?;

    let version = root_manifest(root)?
        .get("version")
        .and_then(Value::as_str)
        .context("Root package manifest must declare a version")?
        .to_owned();
    write_document(
        &bin_directory.join(INSTALLED_VERSION_MARKER_FILE_NAME),
        format!("{version}\n").as_bytes(),
    )?;

    let archive = pack_stage_with(stage, destination, hooks)?;
    let (contents, _) = verify_artifact_with(root, &archive, Some(platform), hooks.inventory_legal_files)?;
    let binary_name = format!("bin/{}", platform.binary);
    let original_bytes = fs::read(binary).with_context(|| format!("failed to read {}", binary.display()))?;
    if contents.files.get(&binary_name).map(Vec::as_slice) != Some(original_bytes.as_slice()) {
        bail!("npm packing changed signed executable bytes");
    }
    Ok(archive)
}

/// Pack the umbrella launcher package using the supplied collaborators.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `destination` - The directory to write the tarball into.
/// * `hooks` - The licensing and npm collaborators, injectable for tests.
///
/// # Returns
///
/// Returns the absolute path of the packed, verified tarball.
///
/// # Errors
///
/// Returns an error when licensing verification fails, npm fails, or the packed
/// artifact deviates from the launcher package contract.
fn package_umbrella_with(root: &Path, destination: &Path, hooks: &PackagingHooks<'_>) -> Result<PathBuf> {
    (hooks.verify_licenses)(root)?;

    let stage_directory = tempfile::Builder::new()
        .prefix("oatty-npm-launcher-")
        .tempdir()
        .context("failed to create the npm staging directory")?;
    let stage = stage_directory.path();
    stage_legal_files(root, stage, hooks.inventory_legal_files)?;
    write_manifest(stage, &umbrella_manifest(root)?)?;
    for name in UMBRELLA_FILES {
        write_document(&stage.join(name), &source_bytes(root, name)?)?;
    }
    set_executable_mode(&stage.join(LAUNCHER_NAME))?;

    let archive = pack_stage_with(stage, destination, hooks)?;
    verify_artifact_with(root, &archive, None, hooks.inventory_legal_files)?;
    Ok(archive)
}

/// Construct dependency-free metadata for one exact-version native payload.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `platform` - The platform contract being packed.
///
/// # Returns
///
/// Returns the payload manifest written into the staging directory.
///
/// # Errors
///
/// Returns an error when the root manifest deviates from its contract or does not
/// declare a version.
fn payload_manifest(root: &Path, platform: &Platform) -> Result<Value> {
    let version = root_manifest(root)?
        .get("version")
        .cloned()
        .context("Root package manifest must declare a version")?;
    let mut files = Vec::with_capacity(2 + crate::licenses::GENERATED_MATERIALS.len());
    files.push(Value::from("bin"));
    files.push(Value::from(LICENSE_NAME));
    files.extend(crate::licenses::GENERATED_MATERIALS.iter().map(|name| Value::from(*name)));

    let mut manifest = Map::new();
    manifest.insert("name".to_owned(), Value::from(platform.package_name.clone()));
    manifest.insert("version".to_owned(), version);
    manifest.insert(
        "description".to_owned(),
        Value::from(format!("Oatty native executable for {}", platform.target_triple)),
    );
    manifest.insert("license".to_owned(), Value::from("MIT"));
    manifest.insert("repository".to_owned(), repository_object());
    manifest.insert("os".to_owned(), json!([platform.os]));
    manifest.insert("cpu".to_owned(), json!([platform.cpu]));
    manifest.insert("oattyTarget".to_owned(), Value::from(platform.target_triple.clone()));
    manifest.insert("files".to_owned(), Value::Array(files));
    if let Some(libc) = &platform.libc {
        manifest.insert("libc".to_owned(), json!([libc]));
    }
    Ok(Value::Object(manifest))
}

/// Return published launcher metadata without development-only npm commands.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
///
/// # Returns
///
/// Returns the validated root manifest with its `scripts` entry removed.
///
/// # Errors
///
/// Returns an error when the root manifest deviates from its contract.
fn umbrella_manifest(root: &Path) -> Result<Value> {
    let mut manifest = root_manifest(root)?;
    let Value::Object(entries) = &mut manifest else {
        bail!("Root package manifest must be a JSON object");
    };
    entries.remove("scripts");
    Ok(manifest)
}

/// Return every required legal/source filename with authoritative bytes.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `inventory_legal_files` - Returns the generated-material inventory.
///
/// # Returns
///
/// Returns `LICENSE` followed by the sorted generated-material relative paths.
///
/// # Errors
///
/// Returns an error when the generated-material inventory cannot be produced.
fn legal_files(root: &Path, inventory_legal_files: &dyn Fn(&Path) -> Result<BTreeMap<String, String>>) -> Result<Vec<String>> {
    let mut names = Vec::with_capacity(1 + crate::licenses::GENERATED_MATERIALS.len());
    names.push(LICENSE_NAME.to_owned());
    names.extend(inventory_legal_files(root)?.into_keys());
    Ok(names)
}

/// Read one first-party file, canonicalizing only owned checkout text.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `name` - The package-relative file name to read.
///
/// # Returns
///
/// Returns the file bytes, with CRLF sequences converted to LF for owned text.
///
/// # Errors
///
/// Returns an error when the file cannot be read.
fn source_bytes(root: &Path, name: &str) -> Result<Vec<u8>> {
    let path = root.join(name);
    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if CANONICAL_TEXT_FILES.contains(&name) {
        Ok(canonicalize_line_endings(&bytes))
    } else {
        Ok(bytes)
    }
}

/// Convert every CRLF sequence to a single LF without touching other bytes.
///
/// # Arguments
///
/// * `bytes` - The raw file bytes to canonicalize.
///
/// # Returns
///
/// Returns the canonicalized bytes; upstream legal bytes are never passed here.
fn canonicalize_line_endings(bytes: &[u8]) -> Vec<u8> {
    let mut canonical = Vec::with_capacity(bytes.len());
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
            continue;
        }
        canonical.push(*byte);
    }
    canonical
}

/// Copy the complete verified notices and sources into an isolated package.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `stage` - The isolated staging directory.
/// * `inventory_legal_files` - Returns the generated-material inventory.
///
/// # Errors
///
/// Returns an error when the inventory or any legal file cannot be read or written.
fn stage_legal_files(root: &Path, stage: &Path, inventory_legal_files: &dyn Fn(&Path) -> Result<BTreeMap<String, String>>) -> Result<()> {
    for name in legal_files(root, inventory_legal_files)? {
        write_document(&stage.join(&name), &source_bytes(root, &name)?)?;
    }
    Ok(())
}

/// Write stable, readable package metadata to the staging directory.
///
/// # Arguments
///
/// * `stage` - The isolated staging directory.
/// * `manifest` - The manifest to serialize with two-space indentation.
///
/// # Errors
///
/// Returns an error when serialization or writing fails.
fn write_manifest(stage: &Path, manifest: &Value) -> Result<()> {
    let mut document = serde_json::to_string_pretty(manifest).context("failed to serialize the npm package manifest")?;
    document.push('\n');
    write_document(&stage.join("package.json"), document.as_bytes())
}

/// Write one file, creating every missing parent directory first.
///
/// # Arguments
///
/// * `path` - The destination path to write.
/// * `bytes` - The exact bytes to write.
///
/// # Errors
///
/// Returns an error when a parent directory or the file cannot be created.
fn write_document(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

/// Mark one staged path as executable.
///
/// Windows cannot express Unix execute bits in the filesystem, so archive metadata is
/// normalized by [`normalize_executable_mode`] instead.
///
/// # Arguments
///
/// * `path` - The staged file to mark executable.
///
/// # Errors
///
/// Returns an error when the permissions cannot be read or updated.
#[cfg(unix)]
fn set_executable_mode(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    let mut permissions = fs::metadata(path)
        .with_context(|| format!("failed to read permissions for {}", path.display()))?
        .permissions();
    permissions.set_mode(EXECUTABLE_MODE);
    fs::set_permissions(path, permissions).with_context(|| format!("failed to mark {} executable", path.display()))
}

/// Mark one staged path as executable where the platform supports it.
///
/// # Arguments
///
/// * `_path` - The staged file to mark executable; ignored on Windows.
///
/// # Errors
///
/// Never returns an error on Windows.
#[cfg(not(unix))]
fn set_executable_mode(_path: &Path) -> Result<()> {
    Ok(())
}

/// Run guarded npm pack with all lifecycle execution explicitly disabled.
///
/// # Arguments
///
/// * `stage` - The isolated staging directory npm packs.
/// * `destination` - The directory receiving the tarball.
/// * `hooks` - The licensing and npm collaborators, injectable for tests.
///
/// # Returns
///
/// Returns the absolute path of the single packed tarball.
///
/// # Errors
///
/// Returns an error when npm fails, reports anything other than exactly one plain
/// artifact filename, or the packed executable cannot be normalized.
fn pack_stage_with(stage: &Path, destination: &Path, hooks: &PackagingHooks<'_>) -> Result<PathBuf> {
    fs::create_dir_all(destination).with_context(|| format!("failed to create {}", destination.display()))?;
    let resolved_destination = fs::canonicalize(destination).with_context(|| format!("failed to resolve {}", destination.display()))?;

    let mut arguments = (hooks.npm_command)()?;
    arguments.extend([
        "pack".to_owned(),
        "--ignore-scripts".to_owned(),
        "--json".to_owned(),
        "--pack-destination".to_owned(),
        resolved_destination.to_string_lossy().into_owned(),
    ]);
    let outcome = (hooks.run_npm)(&arguments, stage)?;
    if !outcome.exit_success {
        bail!("npm pack failed in {}: {}", stage.display(), outcome.standard_error.trim());
    }

    let file_name = parse_pack_report(&outcome.standard_output)?;
    let archive = resolved_destination.join(file_name);
    normalize_executable_mode(&archive, &staged_executable_name(stage))?;
    Ok(archive)
}

/// Determine which executable the staged tree ships.
///
/// # Arguments
///
/// * `stage` - The isolated staging directory.
///
/// # Returns
///
/// Returns the npm-relative executable path, matching the launcher, Windows payload,
/// and POSIX payload layouts in that order.
fn staged_executable_name(stage: &Path) -> String {
    if stage.join(LAUNCHER_NAME).exists() {
        LAUNCHER_NAME.to_owned()
    } else if stage.join("bin").join("oatty.exe").exists() {
        "bin/oatty.exe".to_owned()
    } else {
        "bin/oatty".to_owned()
    }
}

/// Parse the `npm pack --json` report and return its single artifact filename.
///
/// npm 11 prints a JSON list while npm 12 prints an object of values, so both shapes
/// are accepted. Exactly one entry with a plain file name is required.
///
/// # Arguments
///
/// * `standard_output` - The raw standard output of `npm pack --json`.
///
/// # Returns
///
/// Returns the artifact file name, guaranteed to contain no path separators.
///
/// # Errors
///
/// Returns an error when the report is not JSON, does not contain exactly one entry,
/// or names anything other than a plain file.
fn parse_pack_report(standard_output: &[u8]) -> Result<String> {
    let report: Value = serde_json::from_slice(standard_output).context("Expected exactly one npm pack artifact filename")?;
    let packages: Vec<&Value> = match &report {
        Value::Array(entries) => entries.iter().collect(),
        Value::Object(entries) => entries.values().collect(),
        _ => bail!("Expected exactly one npm pack artifact filename"),
    };
    if packages.len() != 1 {
        bail!("Expected exactly one npm pack artifact filename");
    }
    let file_name = packages[0]
        .get("filename")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Expected exactly one npm pack artifact filename"))?;
    if !is_plain_file_name(file_name) {
        bail!("Expected exactly one npm pack artifact filename");
    }
    Ok(file_name.to_owned())
}

/// Report whether a name is a bare file name without any directory component.
///
/// # Arguments
///
/// * `file_name` - The artifact name reported by npm.
///
/// # Returns
///
/// Returns `true` when the name is non-empty and contains no separators.
fn is_plain_file_name(file_name: &str) -> bool {
    !file_name.is_empty() && file_name != "." && file_name != ".." && !file_name.contains(['/', '\\'])
}

/// Ensure Unix executable metadata even when npm pack runs on Windows.
///
/// Only tar metadata changes; signed executable content stays byte-for-byte unchanged.
/// Existing mode-755 npm output is retained exactly. Rewritten archives are
/// deterministic: the gzip header carries a fixed zero timestamp and no file name,
/// source member metadata is preserved, and the executable member is forced to mode
/// 755.
///
/// # Arguments
///
/// * `archive` - The npm tarball to normalize in place.
/// * `executable` - The npm-relative executable path, for example `bin/oatty`.
///
/// # Errors
///
/// Returns an error when the archive cannot be read or rewritten, or when the
/// executable member is absent.
fn normalize_executable_mode(archive: &Path, executable: &str) -> Result<()> {
    let executable_path = Path::new(PACKAGE_PREFIX).join(executable);
    let members = read_archive_members(archive)?;
    let staged_executable = members
        .iter()
        .find(|member| member.path == executable_path)
        .ok_or_else(|| anyhow!("Archive is missing the npm executable member: {}", executable_path.display()))?;
    if staged_executable.header.mode()? == EXECUTABLE_MODE {
        return Ok(());
    }
    rewrite_archive_with_executable_mode(archive, members, &executable_path)
}

/// Rewrite every member, forcing the executable mode and fixing the gzip header.
///
/// # Arguments
///
/// * `archive` - The tarball to rewrite in place.
/// * `members` - The members read from the original archive.
/// * `executable_path` - The full archive path of the executable member, if any.
///
/// # Errors
///
/// Returns an error when the archive cannot be rewritten.
fn rewrite_archive_with_executable_mode(archive: &Path, members: Vec<ArchiveMember>, executable_path: &Path) -> Result<()> {
    let forced = members
        .into_iter()
        .map(|member| {
            let ArchiveMember { path, mut header, data } = member;
            if path == executable_path {
                header.set_mode(EXECUTABLE_MODE);
            }
            ArchiveMember { path, header, data }
        })
        .collect();
    write_archive_members(archive, forced)
}

/// Write every supplied member into a deterministic tar.gz archive.
///
/// The gzip header carries a fixed zero timestamp and no file name, and each member
/// header is written with its own preserved metadata. This is the single writer used
/// by archive normalization and by the test fixtures.
///
/// # Arguments
///
/// * `archive` - The tarball to create or replace.
/// * `members` - The members to write, in order.
///
/// # Errors
///
/// Returns an error when the archive cannot be written.
fn write_archive_members(archive: &Path, members: Vec<ArchiveMember>) -> Result<()> {
    let output = fs::File::create(archive).with_context(|| format!("failed to rewrite {}", archive.display()))?;
    let encoder = GzBuilder::new().mtime(0).write(output, Compression::default());
    let mut builder = tar::Builder::new(encoder);

    for member in members {
        let ArchiveMember { path, mut header, data } = member;
        let content: Box<dyn Read> = match data {
            Some(bytes) => Box::new(Cursor::new(bytes)),
            None => Box::new(std::io::empty()),
        };
        builder
            .append_data(&mut header, &path, content)
            .with_context(|| format!("failed to append {} to {}", path.display(), archive.display()))?;
    }

    let encoder = builder.into_inner().context("failed to finish the normalized tar stream")?;
    let output = encoder.finish().context("failed to finish the normalized gzip stream")?;
    output.sync_all().with_context(|| format!("failed to flush {}", archive.display()))
}

/// One member read from an npm tarball.
struct ArchiveMember {
    /// The full archive path including the `package/` prefix.
    path: PathBuf,
    /// The source header, carrying every preserved piece of member metadata.
    header: Header,
    /// The member bytes for regular files; `None` for every other entry type.
    data: Option<Vec<u8>>,
}

/// Read every member of an npm tarball, including non-regular entries.
///
/// # Arguments
///
/// * `archive` - The npm tarball to read.
///
/// # Returns
///
/// Returns every member in archive order with its header and content.
///
/// # Errors
///
/// Returns an error when the archive cannot be opened or any member is unreadable.
fn read_archive_members(archive: &Path) -> Result<Vec<ArchiveMember>> {
    let archive_file = fs::File::open(archive).with_context(|| format!("failed to open {}", archive.display()))?;
    let mut reader = tar::Archive::new(GzDecoder::new(archive_file));
    let mut members = Vec::new();
    for entry in reader.entries().with_context(|| format!("failed to read {}", archive.display()))? {
        let mut entry = entry.with_context(|| format!("failed to read an entry from {}", archive.display()))?;
        let path = entry
            .path()
            .with_context(|| format!("failed to read an entry path from {}", archive.display()))?
            .into_owned();
        let header = entry.header().clone();
        let data = if header.entry_type().is_file() {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            Some(bytes)
        } else {
            None
        };
        members.push(ArchiveMember { path, header, data });
    }
    Ok(members)
}

/// The verified byte contents and modes of an npm tarball.
#[derive(Debug, Default)]
struct NpmArchiveContents {
    /// File bytes keyed by npm-relative path.
    files: BTreeMap<String, Vec<u8>>,
    /// Unix modes keyed by npm-relative path.
    modes: BTreeMap<String, u32>,
}

/// Read safe regular npm archive entries, rejecting aliases and duplicate paths.
///
/// # Arguments
///
/// * `archive` - The npm tarball to read.
///
/// # Returns
///
/// Returns file bytes and modes keyed by npm-relative path.
///
/// # Errors
///
/// Returns an error for entries outside `package/`, for `..` components, for
/// non-regular entries, and for duplicate paths.
fn read_artifact(archive: &Path) -> Result<NpmArchiveContents> {
    let archive_file = fs::File::open(archive).with_context(|| format!("failed to open {}", archive.display()))?;
    let mut reader = tar::Archive::new(GzDecoder::new(archive_file));
    let mut contents = NpmArchiveContents::default();
    for entry in reader.entries().with_context(|| format!("failed to read {}", archive.display()))? {
        let mut entry = entry.with_context(|| format!("failed to read an entry from {}", archive.display()))?;
        let entry_path = entry.path()?.into_owned();
        let entry_name = entry_path.to_string_lossy().into_owned();
        let escapes_package_root =
            !entry_name.starts_with(PACKAGE_PREFIX) || entry_path.components().any(|component| matches!(component, Component::ParentDir));
        if escapes_package_root {
            bail!("Unsafe npm archive entry: {entry_name}");
        }
        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            continue;
        }
        let name = entry_name.strip_prefix(PACKAGE_PREFIX).unwrap_or_default().to_owned();
        if !entry_type.is_file() || contents.files.contains_key(&name) {
            bail!("Non-regular or duplicate npm archive entry: {entry_name}");
        }
        let mode = entry.header().mode()?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        contents.modes.insert(name.clone(), mode);
        contents.files.insert(name, bytes);
    }
    Ok(contents)
}

/// Validate an artifact using the supplied licensing inventory.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `archive` - The npm tarball to validate.
/// * `platform` - The platform contract for a native payload, or `None` for the
///   umbrella launcher package.
/// * `inventory_legal_files` - Returns the generated-material inventory.
///
/// # Returns
///
/// Returns the verified archive contents together with the parsed manifest.
///
/// # Errors
///
/// Returns an error when the archive layout, manifest, or byte contents deviate from
/// the expected package contract.
fn verify_artifact_with(
    root: &Path,
    archive: &Path,
    platform: Option<&Platform>,
    inventory_legal_files: &dyn Fn(&Path) -> Result<BTreeMap<String, String>>,
) -> Result<(NpmArchiveContents, Value)> {
    let contents = read_artifact(archive)?;
    let manifest_bytes = contents
        .files
        .get("package.json")
        .ok_or_else(|| anyhow!("npm archive is missing package.json: {}", archive.display()))?;
    let manifest: Value =
        serde_json::from_slice(manifest_bytes).with_context(|| format!("failed to parse the manifest inside {}", archive.display()))?;
    let expected_manifest = match platform {
        Some(platform) => payload_manifest(root, platform)?,
        None => umbrella_manifest(root)?,
    };
    if manifest != expected_manifest {
        bail!("Unexpected npm manifest in {}", archive.display());
    }

    let legal_file_names = legal_files(root, inventory_legal_files)?;
    let mut expected_names: BTreeSet<String> = legal_file_names.iter().cloned().collect();
    expected_names.insert("package.json".to_owned());
    match platform {
        Some(platform) => verify_payload_layout(&contents, &manifest, platform, &mut expected_names)?,
        None => verify_umbrella_layout(root, &contents, &mut expected_names)?,
    }

    let actual_names: BTreeSet<String> = contents.files.keys().cloned().collect();
    if actual_names != expected_names {
        let difference: Vec<&String> = actual_names.symmetric_difference(&expected_names).collect();
        bail!("Missing or unexpected npm package files: {difference:?}");
    }
    for name in &legal_file_names {
        let expected_bytes = source_bytes(root, name)?;
        if contents.files.get(name).map(Vec::as_slice) != Some(expected_bytes.as_slice()) {
            bail!("Changed npm legal/source bytes: {name}");
        }
    }
    Ok((contents, manifest))
}

/// Validate the native payload layout and add its entries to the expected set.
///
/// # Arguments
///
/// * `contents` - The verified archive contents.
/// * `manifest` - The parsed archive manifest.
/// * `platform` - The platform contract the payload was packed for.
/// * `expected_names` - The expected npm-relative entry names, extended in place.
///
/// # Errors
///
/// Returns an error when the executable or the installed-version marker is missing,
/// not executable, or does not match the packed version.
fn verify_payload_layout(
    contents: &NpmArchiveContents,
    manifest: &Value,
    platform: &Platform,
    expected_names: &mut BTreeSet<String>,
) -> Result<()> {
    let binary_name = format!("bin/{}", platform.binary);
    expected_names.insert(binary_name.clone());
    expected_names.insert(INSTALLED_VERSION_MARKER.to_owned());
    let binary_present = contents.files.get(&binary_name).is_some_and(|bytes| !bytes.is_empty());
    if !binary_present || contents.modes.get(&binary_name) != Some(&EXECUTABLE_MODE) {
        bail!("Native executable is missing or not mode 755");
    }
    let version = manifest
        .get("version")
        .and_then(Value::as_str)
        .context("Packed manifest must declare a version")?;
    let expected_marker = format!("{version}\n");
    if contents.files.get(INSTALLED_VERSION_MARKER).map(Vec::as_slice) != Some(expected_marker.as_bytes()) {
        bail!("Invalid static installed-version marker");
    }
    Ok(())
}

/// Validate the umbrella launcher layout and add its entries to the expected set.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `contents` - The verified archive contents.
/// * `expected_names` - The expected npm-relative entry names, extended in place.
///
/// # Errors
///
/// Returns an error when the launcher is not executable or any launcher file byte
/// differs from its authoritative source.
fn verify_umbrella_layout(root: &Path, contents: &NpmArchiveContents, expected_names: &mut BTreeSet<String>) -> Result<()> {
    expected_names.extend(UMBRELLA_FILES.map(str::to_owned));
    if contents.modes.get(LAUNCHER_NAME) != Some(&EXECUTABLE_MODE) {
        bail!("Launcher is not mode 755");
    }
    for name in UMBRELLA_FILES {
        let expected_bytes = source_bytes(root, name)?;
        if contents.files.get(name).map(Vec::as_slice) != Some(expected_bytes.as_slice()) {
            bail!("Changed launcher package content: {name}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use flate2::GzBuilder;
    use serde_json::json;
    use sha2::{Digest as _, Sha256};
    use tempfile::TempDir;

    use super::*;

    /// The fixture release version shared by every test manifest.
    const FIXTURE_VERSION: &str = "0.1.12";

    /// The fixture upstream notice, mirroring a generated licensing material.
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

    /// The generated-material keys the fixture inventory reports.
    const FIXTURE_LEGAL_KEYS: [&str; 3] = [
        "THIRD-PARTY-NOTICES.txt",
        "third-party-licenses/example/LICENSE",
        "third-party-sources/example.crate",
    ];

    /// Serializes tests that read or mutate process environment variables.
    static ENVIRONMENT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// A licensing verifier that always succeeds, standing in for real materials.
    fn no_licensing_verification(_root: &Path) -> Result<()> {
        Ok(())
    }

    /// A licensing verifier mirroring the real stale-material rejection.
    fn fixture_licensing_verification(root: &Path) -> Result<()> {
        if fs::read(root.join("THIRD-PARTY-NOTICES.txt"))? != FIXTURE_NOTICE {
            bail!("Missing or changed licensing output/source; run npm run licenses:prepare");
        }
        Ok(())
    }

    /// A deterministic inventory over the fixture generated materials.
    fn fixture_inventory(root: &Path) -> Result<BTreeMap<String, String>> {
        let mut inventory = BTreeMap::new();
        for name in FIXTURE_LEGAL_KEYS {
            let bytes = fs::read(root.join(name))?;
            inventory.insert(name.to_owned(), hex::encode(Sha256::digest(&bytes)));
        }
        Ok(inventory)
    }

    /// The npm argv prefix used by packaging tests.
    fn fixture_npm_command() -> Result<Vec<String>> {
        Ok(vec!["fixture-npm".to_owned()])
    }

    /// The report shape the fake npm prints.
    #[derive(Clone, Copy, Default)]
    enum PackReportShape {
        /// A JSON list, as printed by npm 11.
        #[default]
        List,
        /// A JSON object of values, as printed by npm 12.
        Object,
    }

    /// An in-process npm replacement that packs staging trees deterministically.
    #[derive(Default)]
    struct FakeNpm {
        /// The JSON report shape to print.
        report_shape: PackReportShape,
        /// Raw JSON to print instead of packing, for malformed-report tests.
        raw_report: Option<String>,
        /// A standard error message that simulates a failed npm invocation.
        failure: Option<String>,
        /// Archive-path-to-mode overrides applied while packing.
        mode_overrides: RefCell<BTreeMap<String, u32>>,
        /// Member whose packed bytes are altered, simulating a mutating toolchain.
        tampered_member: Option<String>,
        /// Every npm argv prefix observed by the fake.
        invocations: RefCell<Vec<Vec<String>>>,
    }

    impl FakeNpm {
        /// Build a fake npm that always fails with the supplied message.
        fn failing(message: &str) -> Self {
            Self {
                failure: Some(message.to_owned()),
                ..Self::default()
            }
        }

        /// Build a fake npm that prints the supplied raw report.
        fn reporting(report: &str) -> Self {
            Self {
                raw_report: Some(report.to_owned()),
                ..Self::default()
            }
        }

        /// Record an invocation and emulate `npm pack`.
        fn run(&self, arguments: &[String], working_directory: &Path) -> Result<CommandOutcome> {
            self.invocations.borrow_mut().push(arguments.to_vec());
            if let Some(message) = &self.failure {
                return Ok(CommandOutcome {
                    exit_success: false,
                    standard_output: Vec::new(),
                    standard_error: message.clone(),
                });
            }
            if let Some(report) = &self.raw_report {
                return Ok(CommandOutcome {
                    exit_success: true,
                    standard_output: report.as_bytes().to_vec(),
                    standard_error: String::new(),
                });
            }
            let destination = pack_destination(arguments)?;
            let manifest = read_json_file(&working_directory.join("package.json"))?;
            let name = manifest
                .get("name")
                .and_then(Value::as_str)
                .context("fixture package name is missing")?;
            let version = manifest
                .get("version")
                .and_then(Value::as_str)
                .context("fixture package version is missing")?;
            let file_name = format!("{}-{}.tgz", npm_artifact_name(name), version);
            let archive = destination.join(&file_name);
            write_npm_style_archive(working_directory, &archive, &self.mode_overrides.borrow())?;
            if let Some(member_name) = &self.tampered_member {
                replace_archive_member_data(&archive, member_name, b"tampered executable bytes\n")?;
            }
            let report = match self.report_shape {
                PackReportShape::List => json!([{ "filename": file_name, "name": name, "version": version }]),
                PackReportShape::Object => json!({ "0": { "filename": file_name, "name": name, "version": version } }),
            };
            Ok(CommandOutcome {
                exit_success: true,
                standard_output: serde_json::to_vec(&report)?,
                standard_error: String::new(),
            })
        }
    }

    /// Convert a package name into the artifact prefix npm uses for tarball names.
    ///
    /// npm strips the scope marker and flattens the scope separator, so
    /// `@oattyio/cli-linux-x64` becomes `oattyio-cli-linux-x64`.
    fn npm_artifact_name(package_name: &str) -> String {
        package_name.replace('@', "").replace('/', "-")
    }

    /// Replace one archive member's bytes while preserving every other member.
    fn replace_archive_member_data(archive: &Path, member_name: &str, replacement: &[u8]) -> Result<()> {
        let target = Path::new(member_name);
        let mut rewritten = Vec::new();
        for member in read_archive_members(archive)? {
            let ArchiveMember { path, header, data } = member;
            let data = if path == target { Some(replacement.to_vec()) } else { data };
            rewritten.push(ArchiveMember { path, header, data });
        }
        write_archive_members(archive, rewritten)
    }

    /// Read the `--pack-destination` value from an npm argv prefix.
    fn pack_destination(arguments: &[String]) -> Result<PathBuf> {
        let position = arguments
            .iter()
            .position(|argument| argument == "--pack-destination")
            .context("fixture npm was not told where to pack")?;
        let destination = arguments.get(position + 1).context("fixture npm received no pack destination")?;
        Ok(PathBuf::from(destination))
    }

    /// Read and parse one JSON document.
    fn read_json_file(path: &Path) -> Result<Value> {
        let encoded = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
        serde_json::from_slice(&encoded).with_context(|| format!("failed to parse {}", path.display()))
    }

    /// Write one npm-style tarball from a staging tree with deterministic metadata.
    fn write_npm_style_archive(stage: &Path, archive: &Path, mode_overrides: &BTreeMap<String, u32>) -> Result<()> {
        let mut relative_paths = Vec::new();
        collect_relative_files(stage, Path::new(""), &mut relative_paths)?;
        relative_paths.sort();

        if let Some(parent) = archive.parent() {
            fs::create_dir_all(parent)?;
        }
        let archive_file = fs::File::create(archive).with_context(|| format!("failed to create {}", archive.display()))?;
        let encoder = GzBuilder::new().mtime(0).write(archive_file, Compression::default());
        let mut builder = tar::Builder::new(encoder);
        for relative_path in relative_paths {
            let staged_path = stage.join(&relative_path);
            let archive_name = format!("{PACKAGE_PREFIX}{}", relative_path.to_string_lossy().replace('\\', "/"));
            let metadata = fs::metadata(&staged_path)?;
            let mut header = tar::Header::new_ustar();
            header.set_path(&archive_name)?;
            header.set_mode(mode_overrides.get(&archive_name).copied().unwrap_or_else(|| staged_mode(&metadata)));
            header.set_size(metadata.len());
            header.set_mtime(0);
            header.set_uid(0);
            header.set_gid(0);
            header.set_entry_type(tar::EntryType::Regular);
            header.set_cksum();
            builder.append(&header, fs::File::open(&staged_path)?)?;
        }
        let encoder = builder.into_inner()?;
        encoder.finish()?.sync_all()?;
        Ok(())
    }

    /// Report the Unix mode of a staged file, or a portable default elsewhere.
    #[cfg(unix)]
    fn staged_mode(metadata: &fs::Metadata) -> u32 {
        use std::os::unix::fs::PermissionsExt as _;

        metadata.permissions().mode() & 0o777
    }

    /// Report the default mode of a staged file where Unix modes are unavailable.
    #[cfg(not(unix))]
    fn staged_mode(_metadata: &fs::Metadata) -> u32 {
        0o644
    }

    /// Collect every regular file below a directory as a relative path.
    fn collect_relative_files(directory: &Path, prefix: &Path, collected: &mut Vec<PathBuf>) -> Result<()> {
        let mut entries = fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let relative_path = prefix.join(entry.file_name());
            let metadata = entry.metadata()?;
            if metadata.is_dir() {
                collect_relative_files(&entry.path(), &relative_path, collected)?;
            } else if metadata.is_file() {
                collected.push(relative_path);
            }
        }
        Ok(())
    }

    /// Write a minimal tar.gz archive from raw member descriptions.
    fn write_raw_archive(archive: &Path, members: &[(&str, tar::EntryType, &[u8])]) -> Result<()> {
        let archive_file = fs::File::create(archive)?;
        let encoder = GzBuilder::new().mtime(0).write(archive_file, Compression::default());
        let mut builder = tar::Builder::new(encoder);
        for (name, entry_type, data) in members {
            let mut header = tar::Header::new_ustar();
            header.set_mode(0o644);
            header.set_size(data.len() as u64);
            header.set_mtime(0);
            header.set_entry_type(*entry_type);
            set_raw_member_name(&mut header, name)?;
            header.set_cksum();
            builder.append(&header, *data)?;
        }
        let encoder = builder.into_inner()?;
        encoder.finish()?.sync_all()?;
        Ok(())
    }

    /// Write one member name without the library path validation that rejects `..`.
    ///
    /// Malicious-archive fixtures need names an honest writer would never emit, so the
    /// raw ustar name field is filled directly.
    fn set_raw_member_name(header: &mut tar::Header, name: &str) -> Result<()> {
        let ustar = header.as_ustar_mut().context("fixture header is not in ustar format")?;
        let bytes = name.as_bytes();
        if bytes.len() > ustar.name.len() {
            bail!("fixture archive name is too long: {name}");
        }
        ustar.name[..bytes.len()].copy_from_slice(bytes);
        Ok(())
    }

    /// Build the canonical root package manifest the fixture publishes.
    fn fixture_root_manifest() -> Value {
        json!({
            "name": "oatty",
            "version": FIXTURE_VERSION,
            "description": "Fixture Oatty package",
            "license": "MIT",
            "repository": { "type": "git", "url": "git+https://github.com/oattyio/oatty.git" },
            "scripts": { "prepack": REQUIRED_PREPACK_GUARD },
            "optionalDependencies": {
                "@oattyio/cli-darwin-x64": FIXTURE_VERSION,
                "@oattyio/cli-darwin-arm64": FIXTURE_VERSION,
                "@oattyio/cli-linux-x64": FIXTURE_VERSION,
                "@oattyio/cli-win32-x64": FIXTURE_VERSION,
            },
        })
    }

    /// Build the canonical platform index the fixture publishes.
    fn fixture_platforms() -> Value {
        json!({
            "darwin-x64": {
                "package": "@oattyio/cli-darwin-x64",
                "target": "x86_64-apple-darwin",
                "os": "darwin",
                "cpu": "x64",
                "binary": "oatty"
            },
            "darwin-arm64": {
                "package": "@oattyio/cli-darwin-arm64",
                "target": "aarch64-apple-darwin",
                "os": "darwin",
                "cpu": "arm64",
                "binary": "oatty"
            },
            "linux-x64": {
                "package": "@oattyio/cli-linux-x64",
                "target": "x86_64-unknown-linux-gnu",
                "os": "linux",
                "cpu": "x64",
                "binary": "oatty",
                "libc": "glibc"
            },
            "win32-x64": {
                "package": "@oattyio/cli-win32-x64",
                "target": "x86_64-pc-windows-msvc",
                "os": "win32",
                "cpu": "x64",
                "binary": "oatty.exe"
            },
        })
    }

    /// A complete source tree satisfying every npm packaging contract.
    struct Fixture {
        /// The temporary directory holding the fixture tree.
        directory: TempDir,
    }

    impl Fixture {
        /// Create the fixture tree with all first-party and legal materials.
        fn new() -> Result<Self> {
            let directory = TempDir::new().context("failed to create the fixture directory")?;
            let fixture = Self { directory };
            let root = fixture.root();
            write_document(
                &root.join("package.json"),
                serde_json::to_string_pretty(&fixture_root_manifest())?.as_bytes(),
            )?;
            write_document(
                &root.join(PLATFORM_INDEX_NAME),
                serde_json::to_string_pretty(&fixture_platforms())?.as_bytes(),
            )?;
            write_document(&root.join(LAUNCHER_NAME), FIXTURE_LAUNCHER)?;
            write_document(&root.join(README_NAME), FIXTURE_README)?;
            write_document(&root.join(LICENSE_NAME), FIXTURE_LICENSE)?;
            write_document(&root.join("THIRD-PARTY-NOTICES.txt"), FIXTURE_NOTICE)?;
            write_document(&root.join("third-party-licenses/example/LICENSE"), FIXTURE_UPSTREAM_LICENSE)?;
            write_document(&root.join("third-party-sources/example.crate"), FIXTURE_COVERED_SOURCE)?;
            Ok(fixture)
        }

        /// Return the fixture root directory.
        fn root(&self) -> &Path {
            self.directory.path()
        }

        /// Join one fixture-relative path.
        fn path(&self, relative_path: &str) -> PathBuf {
            self.root().join(relative_path)
        }

        /// Return a scratch directory path inside the fixture.
        fn scratch(&self, name: &str) -> PathBuf {
            self.root().join("scratch").join(name)
        }

        /// Write an executable fixture binary and return its path.
        fn write_binary(&self, bytes: &[u8]) -> Result<PathBuf> {
            let binary = self.scratch("binary").join("oatty");
            write_document(&binary, bytes)?;
            set_executable_mode(&binary)?;
            Ok(binary)
        }

        /// Load the validated fixture platform contracts.
        fn platforms(&self) -> Result<BTreeMap<String, Platform>> {
            load_platforms(self.root())
        }

        /// Rewrite the fixture root manifest with one mutation applied.
        fn rewrite_root_manifest(&self, mutate: impl FnOnce(&mut Map<String, Value>)) -> Result<()> {
            let manifest_path = self.path("package.json");
            let Value::Object(mut manifest) = read_json_file(&manifest_path)? else {
                bail!("fixture root manifest is not a JSON object");
            };
            mutate(&mut manifest);
            write_document(&manifest_path, serde_json::to_string_pretty(&Value::Object(manifest))?.as_bytes())
        }

        /// Rewrite the fixture platform index with one mutation applied.
        fn rewrite_platforms(&self, mutate: impl FnOnce(&mut Map<String, Value>)) -> Result<()> {
            let platform_path = self.path(PLATFORM_INDEX_NAME);
            let Value::Object(mut platforms) = read_json_file(&platform_path)? else {
                bail!("fixture platform index is not a JSON object");
            };
            mutate(&mut platforms);
            write_document(&platform_path, serde_json::to_string_pretty(&Value::Object(platforms))?.as_bytes())
        }

        /// Convert one fixture file to a CRLF checkout.
        fn convert_to_crlf(&self, relative_path: &str) -> Result<()> {
            let path = self.path(relative_path);
            let bytes = fs::read(&path)?;
            let mut converted = Vec::with_capacity(bytes.len() + bytes.len() / 16);
            for byte in bytes {
                if byte == b'\n' {
                    converted.push(b'\r');
                }
                converted.push(byte);
            }
            write_document(&path, &converted)
        }

        /// Rewrite one packed archive member with new content or metadata.
        fn rewrite_member(&self, archive: &Path, member_name: &str, mode: Option<u32>, data: Option<&[u8]>) -> Result<()> {
            let target = Path::new(member_name);
            let mut rewritten = Vec::new();
            for member in read_archive_members(archive)? {
                let ArchiveMember {
                    path,
                    mut header,
                    data: original,
                } = member;
                let matches_target = path == target;
                if let (true, Some(mode)) = (matches_target, mode) {
                    header.set_mode(mode);
                }
                let data = match (matches_target, data) {
                    (true, Some(replacement)) => Some(replacement.to_vec()),
                    _ => original,
                };
                rewritten.push(ArchiveMember { path, header, data });
            }
            write_archive_members(archive, rewritten)
        }
    }

    /// Build hooks that use the fake npm and the supplied verifier.
    fn fixture_hooks<'a>(
        run_npm: &'a dyn Fn(&[String], &Path) -> Result<CommandOutcome>,
        verify_licenses: &'a dyn Fn(&Path) -> Result<()>,
    ) -> PackagingHooks<'a> {
        PackagingHooks {
            verify_licenses,
            inventory_legal_files: &fixture_inventory,
            npm_command: &fixture_npm_command,
            run_npm,
        }
    }

    /// Assert that one packaging error carries the expected message.
    fn assert_error_contains(outcome: Result<impl std::fmt::Debug>, expected: &str) {
        let error = outcome.unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "expected {expected:?} in {:?}",
            error.to_string()
        );
    }

    #[test]
    fn npm_distribution_all_four_metadata_legal_bytes_modes_and_determinism() -> Result<()> {
        let fixture = Fixture::new()?;
        let binary = fixture.write_binary(FIXTURE_BINARY)?;
        let fake_npm = FakeNpm::default();
        let run_npm = |arguments: &[String], working_directory: &Path| fake_npm.run(arguments, working_directory);
        let hooks = fixture_hooks(&run_npm, &no_licensing_verification);
        let destination = fixture.scratch("dist");
        let platforms = fixture.platforms()?;
        assert_eq!(platforms.len(), 4);

        for platform in platforms.values() {
            let binary_name = format!("bin/{}", platform.binary);
            let archive = package_binary_with(fixture.root(), &binary, &platform.target_triple, &destination, &hooks)?;
            let (contents, manifest) = verify_artifact_with(fixture.root(), &archive, Some(platform), &fixture_inventory)?;
            assert_eq!(contents.files.get(&binary_name).map(Vec::as_slice), Some(FIXTURE_BINARY));
            assert_eq!(manifest, payload_manifest(fixture.root(), platform)?);
            assert_eq!(contents.modes.get(&binary_name), Some(&EXECUTABLE_MODE));

            let repeated = package_binary_with(fixture.root(), &binary, &platform.target_triple, &fixture.scratch("repeat"), &hooks)?;
            assert_eq!(fs::read(&archive)?, fs::read(&repeated)?);
        }

        let umbrella = package_umbrella_with(fixture.root(), &destination, &hooks)?;
        let (contents, manifest) = verify_artifact_with(fixture.root(), &umbrella, None, &fixture_inventory)?;
        assert_eq!(manifest, umbrella_manifest(fixture.root())?);
        assert_eq!(contents.modes.get(LAUNCHER_NAME), Some(&EXECUTABLE_MODE));
        let repeated_umbrella = package_umbrella_with(fixture.root(), &fixture.scratch("repeat"), &hooks)?;
        assert_eq!(fs::read(&umbrella)?, fs::read(&repeated_umbrella)?);

        let first_invocation = fake_npm.invocations.borrow()[0].clone();
        assert_eq!(first_invocation.len(), 6);
        assert_eq!(first_invocation[0], "fixture-npm");
        assert_eq!(first_invocation[1], "pack");
        assert_eq!(first_invocation[2], "--ignore-scripts");
        assert_eq!(first_invocation[3], "--json");
        assert_eq!(first_invocation[4], "--pack-destination");
        Ok(())
    }

    #[test]
    fn npm_distribution_payload_and_umbrella_manifests_match_contract() -> Result<()> {
        let fixture = Fixture::new()?;
        let platforms = fixture.platforms()?;
        let linux_manifest = payload_manifest(fixture.root(), &platforms["linux-x64"])?;
        assert_eq!(
            linux_manifest,
            json!({
                "name": "@oattyio/cli-linux-x64",
                "version": FIXTURE_VERSION,
                "description": "Oatty native executable for x86_64-unknown-linux-gnu",
                "license": "MIT",
                "repository": { "type": "git", "url": "git+https://github.com/oattyio/oatty.git" },
                "os": ["linux"],
                "cpu": ["x64"],
                "oattyTarget": "x86_64-unknown-linux-gnu",
                "files": ["bin", "LICENSE", "THIRD-PARTY-NOTICES.txt", "third-party-licenses", "third-party-sources"],
                "libc": ["glibc"],
            })
        );
        let darwin_manifest = payload_manifest(fixture.root(), &platforms["darwin-arm64"])?;
        assert_eq!(darwin_manifest.get("libc"), None);
        assert_eq!(darwin_manifest["os"], json!(["darwin"]));
        assert_eq!(darwin_manifest["files"][0], json!("bin"));

        let umbrella = umbrella_manifest(fixture.root())?;
        assert_eq!(umbrella.get("scripts"), None);
        assert_eq!(umbrella["version"], json!(FIXTURE_VERSION));
        assert_eq!(umbrella["optionalDependencies"]["@oattyio/cli-win32-x64"], json!(FIXTURE_VERSION));
        Ok(())
    }

    #[test]
    fn npm_distribution_windows_mode_normalization_preserves_every_file_byte() -> Result<()> {
        let fixture = Fixture::new()?;
        let binary = fixture.write_binary(FIXTURE_BINARY)?;
        let platforms = fixture.platforms()?;
        let platform = platforms.values().next().context("fixture has no platforms")?;

        // Recreate what npm produces on Windows: a 0644 executable member.
        let stage = fixture.scratch("stage");
        stage_legal_files(fixture.root(), &stage, &fixture_inventory)?;
        write_manifest(&stage, &payload_manifest(fixture.root(), platform)?)?;
        fs::create_dir_all(stage.join("bin"))?;
        fs::copy(&binary, stage.join("bin").join("oatty"))?;
        write_document(&stage.join(INSTALLED_VERSION_MARKER), format!("{FIXTURE_VERSION}\n").as_bytes())?;
        let archive = fixture.scratch("windows-mode").join("oatty-mode.tgz");
        write_npm_style_archive(&stage, &archive, &BTreeMap::from([("package/bin/oatty".to_owned(), 0o644)]))?;

        let before = read_artifact(&archive)?;
        assert_eq!(before.modes.get("bin/oatty"), Some(&0o644));
        normalize_executable_mode(&archive, "bin/oatty")?;
        let after = read_artifact(&archive)?;
        assert_eq!(before.files, after.files);
        assert_eq!(after.modes.get("bin/oatty"), Some(&EXECUTABLE_MODE));
        verify_artifact_with(fixture.root(), &archive, Some(platform), &fixture_inventory)?;

        // An already-normalized archive is retained byte for byte.
        let normalized_bytes = fs::read(&archive)?;
        normalize_executable_mode(&archive, "bin/oatty")?;
        assert_eq!(normalized_bytes, fs::read(&archive)?);

        // The packaging pipeline normalizes npm's 0644 output as well.
        let fake_npm = FakeNpm::default();
        fake_npm.mode_overrides.borrow_mut().insert("package/bin/oatty".to_owned(), 0o644);
        let run_npm = |arguments: &[String], working_directory: &Path| fake_npm.run(arguments, working_directory);
        let hooks = fixture_hooks(&run_npm, &no_licensing_verification);
        let packed = package_binary_with(fixture.root(), &binary, &platform.target_triple, &fixture.scratch("dist"), &hooks)?;
        let packed_contents = verify_artifact_with(fixture.root(), &packed, Some(platform), &fixture_inventory)?.0;
        assert_eq!(packed_contents.modes.get("bin/oatty"), Some(&EXECUTABLE_MODE));
        assert_eq!(packed_contents.files.get("bin/oatty").map(Vec::as_slice), Some(FIXTURE_BINARY));
        Ok(())
    }

    #[test]
    fn npm_distribution_first_party_crlf_checkout_produces_identical_archives() -> Result<()> {
        let fixture = Fixture::new()?;
        let binary = fixture.write_binary(FIXTURE_BINARY)?;
        let fake_npm = FakeNpm::default();
        let run_npm = |arguments: &[String], working_directory: &Path| fake_npm.run(arguments, working_directory);
        let hooks = fixture_hooks(&run_npm, &no_licensing_verification);
        let platforms = fixture.platforms()?;
        let platform = platforms.values().next().context("fixture has no platforms")?;

        let umbrella = package_umbrella_with(fixture.root(), &fixture.scratch("lf"), &hooks)?;
        let native = package_binary_with(fixture.root(), &binary, &platform.target_triple, &fixture.scratch("lf"), &hooks)?;

        for name in CANONICAL_TEXT_FILES {
            fixture.convert_to_crlf(name)?;
        }
        let crlf_umbrella = package_umbrella_with(fixture.root(), &fixture.scratch("crlf"), &hooks)?;
        let crlf_native = package_binary_with(fixture.root(), &binary, &platform.target_triple, &fixture.scratch("crlf"), &hooks)?;
        assert_eq!(fs::read(&umbrella)?, fs::read(&crlf_umbrella)?);
        assert_eq!(fs::read(&native)?, fs::read(&crlf_native)?);
        Ok(())
    }

    #[test]
    fn npm_distribution_stale_legal_material_aborts_before_npm_pack() -> Result<()> {
        let fixture = Fixture::new()?;
        write_document(&fixture.path("THIRD-PARTY-NOTICES.txt"), b"changed")?;
        let fake_npm = FakeNpm::default();
        let run_npm = |arguments: &[String], working_directory: &Path| fake_npm.run(arguments, working_directory);
        let hooks = fixture_hooks(&run_npm, &fixture_licensing_verification);

        let outcome = package_umbrella_with(fixture.root(), &fixture.scratch("invalid"), &hooks);
        assert_error_contains(outcome, "Missing or changed");
        assert!(
            fake_npm.invocations.borrow().is_empty(),
            "npm pack must never run for stale materials"
        );
        Ok(())
    }

    #[test]
    fn npm_distribution_pack_report_accepts_list_and_object_shapes() -> Result<()> {
        for report_shape in [PackReportShape::List, PackReportShape::Object] {
            let fixture = Fixture::new()?;
            let fake_npm = FakeNpm {
                report_shape,
                ..FakeNpm::default()
            };
            let run_npm = |arguments: &[String], working_directory: &Path| fake_npm.run(arguments, working_directory);
            let hooks = fixture_hooks(&run_npm, &no_licensing_verification);
            let archive = package_umbrella_with(fixture.root(), &fixture.scratch("dist"), &hooks)?;
            assert_eq!(archive.file_name().and_then(|name| name.to_str()), Some("oatty-0.1.12.tgz"));
        }
        Ok(())
    }

    #[test]
    fn npm_distribution_pack_report_rejects_malformed_artifacts() -> Result<()> {
        let malformed_reports = [
            "[]",
            r#"[{"filename": "one.tgz"}, {"filename": "two.tgz"}]"#,
            r#"[{"filename": "nested/oatty-0.1.12.tgz"}]"#,
            r#"[{"filename": "../escape.tgz"}]"#,
            r#"[{"name": "oatty"}]"#,
            "not json",
        ];
        for report in malformed_reports {
            let fixture = Fixture::new()?;
            let fake_npm = FakeNpm::reporting(report);
            let run_npm = |arguments: &[String], working_directory: &Path| fake_npm.run(arguments, working_directory);
            let hooks = fixture_hooks(&run_npm, &no_licensing_verification);
            let outcome = package_umbrella_with(fixture.root(), &fixture.scratch("dist"), &hooks);
            assert_error_contains(outcome, "Expected exactly one npm pack artifact filename");
        }
        Ok(())
    }

    #[test]
    fn npm_distribution_pack_failure_reports_standard_error() -> Result<()> {
        let fixture = Fixture::new()?;
        let fake_npm = FakeNpm::failing("npm error code EACCES");
        let run_npm = |arguments: &[String], working_directory: &Path| fake_npm.run(arguments, working_directory);
        let hooks = fixture_hooks(&run_npm, &no_licensing_verification);
        let error = package_umbrella_with(fixture.root(), &fixture.scratch("dist"), &hooks).unwrap_err();
        assert!(error.to_string().contains("npm pack failed"), "unexpected error: {error}");
        assert!(error.to_string().contains("npm error code EACCES"), "unexpected error: {error}");
        Ok(())
    }

    #[test]
    fn npm_distribution_verify_rejects_unsafe_non_regular_and_duplicate_entries() -> Result<()> {
        let fixture = Fixture::new()?;
        let scratch = fixture.scratch("unsafe");
        fs::create_dir_all(&scratch)?;

        let outside = scratch.join("outside.tgz");
        write_raw_archive(&outside, &[("outside.txt", tar::EntryType::Regular, b"nope".as_slice())])?;
        assert_error_contains(
            verify_artifact(fixture.root(), &outside, None),
            "Unsafe npm archive entry: outside.txt",
        );

        let traversal = scratch.join("traversal.tgz");
        write_raw_archive(&traversal, &[("package/../escape", tar::EntryType::Regular, b"nope".as_slice())])?;
        assert_error_contains(
            verify_artifact(fixture.root(), &traversal, None),
            "Unsafe npm archive entry: package/../escape",
        );

        let symlink = scratch.join("symlink.tgz");
        write_raw_archive(&symlink, &[("package/LICENSE", tar::EntryType::Symlink, b"".as_slice())])?;
        assert_error_contains(
            verify_artifact(fixture.root(), &symlink, None),
            "Non-regular or duplicate npm archive entry: package/LICENSE",
        );

        let duplicate = scratch.join("duplicate.tgz");
        write_raw_archive(
            &duplicate,
            &[
                ("package/LICENSE", tar::EntryType::Regular, b"first".as_slice()),
                ("package/LICENSE", tar::EntryType::Regular, b"second".as_slice()),
            ],
        )?;
        assert_error_contains(
            verify_artifact(fixture.root(), &duplicate, None),
            "Non-regular or duplicate npm archive entry: package/LICENSE",
        );
        Ok(())
    }

    #[test]
    fn npm_distribution_verify_rejects_wrong_modes_marker_and_legal_bytes() -> Result<()> {
        let fixture = Fixture::new()?;
        let binary = fixture.write_binary(FIXTURE_BINARY)?;
        let fake_npm = FakeNpm::default();
        let run_npm = |arguments: &[String], working_directory: &Path| fake_npm.run(arguments, working_directory);
        let hooks = fixture_hooks(&run_npm, &no_licensing_verification);
        let platforms = fixture.platforms()?;
        let platform = platforms.values().next().context("fixture has no platforms")?;

        let archive = package_binary_with(fixture.root(), &binary, &platform.target_triple, &fixture.scratch("dist"), &hooks)?;
        fixture.rewrite_member(&archive, "package/bin/oatty", Some(0o644), None)?;
        assert_error_contains(
            verify_artifact_with(fixture.root(), &archive, Some(platform), &fixture_inventory),
            "Native executable is missing or not mode 755",
        );

        let umbrella = package_umbrella_with(fixture.root(), &fixture.scratch("umbrella"), &hooks)?;
        fixture.rewrite_member(&umbrella, "package/npm/bin/oatty.js", Some(0o644), None)?;
        assert_error_contains(
            verify_artifact_with(fixture.root(), &umbrella, None, &fixture_inventory),
            "Launcher is not mode 755",
        );

        let marker_archive = package_binary_with(fixture.root(), &binary, &platform.target_triple, &fixture.scratch("marker"), &hooks)?;
        fixture.rewrite_member(&marker_archive, "package/bin/.oatty-installed-version", None, Some(b"9.9.9\n"))?;
        assert_error_contains(
            verify_artifact_with(fixture.root(), &marker_archive, Some(platform), &fixture_inventory),
            "Invalid static installed-version marker",
        );

        let legal_archive = package_umbrella_with(fixture.root(), &fixture.scratch("legal"), &hooks)?;
        write_document(&fixture.path("THIRD-PARTY-NOTICES.txt"), b"changed notice\n")?;
        assert_error_contains(
            verify_artifact_with(fixture.root(), &legal_archive, None, &fixture_inventory),
            "Changed npm legal/source bytes: THIRD-PARTY-NOTICES.txt",
        );
        Ok(())
    }

    #[test]
    fn npm_distribution_changed_executable_bytes_are_rejected() -> Result<()> {
        let fixture = Fixture::new()?;
        let binary = fixture.write_binary(FIXTURE_BINARY)?;
        let fake_npm = FakeNpm {
            tampered_member: Some("package/bin/oatty".to_owned()),
            ..FakeNpm::default()
        };
        let run_npm = |arguments: &[String], working_directory: &Path| fake_npm.run(arguments, working_directory);
        let hooks = fixture_hooks(&run_npm, &no_licensing_verification);
        let platforms = fixture.platforms()?;
        let platform = platforms.values().next().context("fixture has no platforms")?;
        let outcome = package_binary_with(fixture.root(), &binary, &platform.target_triple, &fixture.scratch("dist"), &hooks);
        assert_error_contains(outcome, "npm packing changed signed executable bytes");
        Ok(())
    }

    #[test]
    fn npm_distribution_unsupported_target_is_rejected() -> Result<()> {
        let fixture = Fixture::new()?;
        let binary = fixture.write_binary(FIXTURE_BINARY)?;
        let fake_npm = FakeNpm::default();
        let run_npm = |arguments: &[String], working_directory: &Path| fake_npm.run(arguments, working_directory);
        let hooks = fixture_hooks(&run_npm, &no_licensing_verification);
        let outcome = package_binary_with(
            fixture.root(),
            &binary,
            "aarch64-unknown-linux-musl",
            &fixture.scratch("dist"),
            &hooks,
        );
        assert_error_contains(outcome, "Unsupported Rust target: aarch64-unknown-linux-musl");
        assert!(fake_npm.invocations.borrow().is_empty());
        Ok(())
    }

    #[test]
    fn npm_distribution_root_manifest_requires_the_cargo_xtask_prepack_guard() -> Result<()> {
        let fixture = Fixture::new()?;
        fixture.rewrite_root_manifest(|manifest| {
            manifest.insert("scripts".to_owned(), json!({ "prepack": "python3 scripts/licenses.py verify" }));
        })?;
        assert_error_contains(root_manifest(fixture.root()), "Required source prepack licensing guard is missing");

        let fixture = Fixture::new()?;
        fixture.rewrite_root_manifest(|manifest| {
            manifest.remove("scripts");
        })?;
        assert_error_contains(root_manifest(fixture.root()), "Required source prepack licensing guard is missing");
        Ok(())
    }

    #[test]
    fn npm_distribution_root_manifest_rejects_dependencies_hooks_and_identity() -> Result<()> {
        let fixture = Fixture::new()?;
        fixture.rewrite_root_manifest(|manifest| {
            manifest.insert("dependencies".to_owned(), json!({ "left-pad": "1.0.0" }));
        })?;
        assert_error_contains(
            root_manifest(fixture.root()),
            "Root package must contain only exact-version native optional dependencies",
        );

        let fixture = Fixture::new()?;
        fixture.rewrite_root_manifest(|manifest| {
            manifest.insert(
                "scripts".to_owned(),
                json!({ "prepack": REQUIRED_PREPACK_GUARD, "postinstall": "node install.js" }),
            );
        })?;
        assert_error_contains(root_manifest(fixture.root()), "npm lifecycle hooks are forbidden");

        let fixture = Fixture::new()?;
        fixture.rewrite_root_manifest(|manifest| {
            manifest.insert("optionalDependencies".to_owned(), json!({}));
        })?;
        assert_error_contains(
            root_manifest(fixture.root()),
            "Root package must contain only exact-version native optional dependencies",
        );

        let fixture = Fixture::new()?;
        fixture.rewrite_root_manifest(|manifest| {
            manifest.insert(
                "repository".to_owned(),
                json!({ "type": "git", "url": "https://example.com/oatty.git" }),
            );
        })?;
        assert_error_contains(
            root_manifest(fixture.root()),
            "Incorrect Oatty package identity, repository, or license",
        );

        let fixture = Fixture::new()?;
        fixture.rewrite_root_manifest(|manifest| {
            manifest.insert("license".to_owned(), json!("Apache-2.0"));
        })?;
        assert_error_contains(
            root_manifest(fixture.root()),
            "Incorrect Oatty package identity, repository, or license",
        );
        Ok(())
    }

    #[test]
    fn npm_distribution_load_platforms_rejects_non_canonical_metadata() -> Result<()> {
        let fixture = Fixture::new()?;
        fixture.rewrite_platforms(|platforms| {
            platforms.insert(
                "darwin-x64".to_owned(),
                json!({
                    "package": "@oattyio/cli-darwin-x64",
                    "target": "x86_64-apple-darwin",
                    "os": "darwin",
                    "cpu": "arm64",
                    "binary": "oatty"
                }),
            );
        })?;
        assert_error_contains(load_platforms(fixture.root()), "Invalid canonical platform metadata: darwin-x64");

        let fixture = Fixture::new()?;
        fixture.rewrite_platforms(|platforms| {
            platforms.remove("win32-x64");
        })?;
        assert_error_contains(load_platforms(fixture.root()), "Expected exactly four canonical npm platforms");
        Ok(())
    }

    #[test]
    fn npm_distribution_npm_command_honors_the_environment_override() -> Result<()> {
        let _guard = ENVIRONMENT_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = env::var_os("NPM_CLI");
        // SAFETY: the environment is only mutated while the test lock is held and
        // no other packaging test reads it concurrently.
        unsafe {
            env::set_var("NPM_CLI", "/opt/npm/bin/npm-cli.js");
        }
        assert_eq!(npm_command()?, vec!["node".to_owned(), "/opt/npm/bin/npm-cli.js".to_owned()]);
        unsafe {
            env::remove_var("NPM_CLI");
        }
        if env::consts::OS != "windows" {
            assert_eq!(npm_command()?, vec!["npm".to_owned()]);
        }
        if let Some(previous) = previous {
            unsafe {
                env::set_var("NPM_CLI", previous);
            }
        }
        Ok(())
    }

    #[test]
    fn npm_distribution_release_native_binary_installs_and_reports_marker() -> Result<()> {
        let _guard = ENVIRONMENT_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let Ok(binary) = env::var("OATTY_NPM_TEST_BINARY") else {
            return Ok(());
        };
        let Ok(target) = env::var("OATTY_NPM_TEST_TARGET") else {
            return Ok(());
        };
        let root = crate::context::project_root()?;
        let platforms = load_platforms(&root)?;
        let (host_key, platform) = platforms
            .iter()
            .find(|(_, candidate)| candidate.target_triple == target)
            .context("OATTY_NPM_TEST_TARGET is not a canonical platform target")?;
        assert_eq!(
            host_platform_key().as_deref(),
            Some(host_key.as_str()),
            "release smoke tests must run on the packaged target host"
        );

        let destination = TempDir::new()?;
        let native_archive = package_binary(&root, Path::new(&binary), &target, destination.path())?;
        let umbrella_archive = package_umbrella(&root, destination.path())?;
        let version = root_manifest(&root)?
            .get("version")
            .and_then(Value::as_str)
            .context("Root package manifest must declare a version")?
            .to_owned();

        let native_contents = read_artifact(&native_archive)?;
        assert_eq!(
            native_contents.files.get(INSTALLED_VERSION_MARKER).map(Vec::as_slice),
            Some(format!("{version}\n").as_bytes())
        );
        let binary_bytes = fs::read(&binary)?;
        assert_eq!(
            native_contents.files.get(&format!("bin/{}", platform.binary)).map(Vec::as_slice),
            Some(binary_bytes.as_slice())
        );
        let umbrella_contents = read_artifact(&umbrella_archive)?;
        assert_eq!(
            umbrella_contents.files.get(LAUNCHER_NAME).map(Vec::as_slice),
            Some(source_bytes(&root, LAUNCHER_NAME)?.as_slice())
        );
        Ok(())
    }

    /// Report the canonical platform key of the running host, if supported.
    fn host_platform_key() -> Option<String> {
        let os = match env::consts::OS {
            "macos" => "darwin",
            "windows" => "win32",
            other => other,
        };
        let cpu = match env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "arm64",
            _ => return None,
        };
        Some(format!("{os}-{cpu}"))
    }
}
