//! Exercise packed, script-free npm installations against a private loopback registry.
//!
//! Ported from `scripts/tests/test_npm_distribution.py`. These tests spawn the real
//! `npm` and `node` toolchains exactly like the Python suite: fixture packages are
//! packed with `npm pack`, served by a loopback registry shim, installed with
//! `npm install --ignore-scripts` (locally and globally), and driven through the
//! published `npm/bin/oatty.js` launcher.
//!
//! # Harness
//!
//! [`Harness`] stands in for the Python class fixture: it writes an isolated base
//! directory, packs one native payload per canonical platform plus the umbrella
//! launcher package, starts an in-process HTTP registry (packuments and tarballs), and
//! performs the local and global installs several tests share.
//!
//! Every npm invocation is pinned through CLI flags and environment variables so the
//! developer's `~/.npmrc`, npm prefix, and npm cache are never touched: `--prefix`,
//! `--cache`, `--registry`, `--userconfig`, `--globalconfig`, plus
//! `npm_config_prefix`, `npm_config_cache`, and `npm_config_ignore_scripts`.
//!
//! Tests early-return quietly when `npm` or `node` is unavailable so ordinary
//! developer machines without a Node toolchain still pass.

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha512};
use tempfile::TempDir;

/// The fixture release version every generated manifest declares.
const FIXTURE_VERSION: &str = "3.2.1";

/// The scripted native executable substituted for a signed release binary.
///
/// The launcher invokes it directly, so it doubles as the "unchanged executable bytes"
/// that installation must preserve and as the fixture behavior driver.
const FIXTURE_BINARY: &str = r#"#!/usr/bin/env node
const fs = require('node:fs');
const args = process.argv.slice(2);
if (args[0] === '--version') { console.log('oatty ' + fs.readFileSync(require('node:path').join(__dirname, '.oatty-installed-version'), 'utf8').trim()); }
else if (args[0] === 'update' && args[1] === '--check') {
  const version = fs.readFileSync(require('node:path').join(__dirname, '.oatty-installed-version'), 'utf8').trim();
  console.log('Installed version: ' + version);
} else if (args[0] === 'status') { process.exit(Number(args[1])); }
else if (args[0] === 'signal') { process.kill(process.pid, args[1]); }
else if (args[0] === 'wait') { console.log('ready'); setInterval(() => {}, 1000); }
else { console.log(JSON.stringify({args, cwd:process.cwd(), environment:process.env.OATTY_FIXTURE_VALUE})); }
"#;

/// The fixture first-party license bytes shared by every packed package.
const FIXTURE_LICENSE: &[u8] = b"MIT License\n\nFixture first-party license text.\n";

/// The fixture documentation bytes packed into the umbrella package.
const FIXTURE_README: &[u8] = b"# Oatty fixture\n\nFixture launcher documentation.\n";

/// How long one npm invocation may run before it is killed.
const NPM_TIMEOUT: Duration = Duration::from_secs(240);

/// How long one node launcher invocation may run before it is killed.
const LAUNCHER_TIMEOUT: Duration = Duration::from_secs(30);

/// The SIGINT signal number on POSIX hosts.
#[cfg(unix)]
const SIGINT: i32 = 2;

/// The SIGTERM signal number on POSIX hosts.
#[cfg(unix)]
const SIGTERM: i32 = 15;

/// One canonical platform the fixture packages.
struct FixturePlatform {
    /// The npm platform key, for example `linux-x64`.
    key: &'static str,
    /// The published native package name.
    package_name: &'static str,
    /// The Rust target triple encoded in `oattyTarget`.
    target: &'static str,
    /// The npm `os` value.
    os: &'static str,
    /// The npm `cpu` value.
    cpu: &'static str,
    /// The npm `libc` value for glibc platforms.
    libc: Option<&'static str>,
    /// The bundled executable name.
    binary: &'static str,
}

/// The four canonical platform contracts the fixture mirrors.
const FIXTURE_PLATFORMS: [FixturePlatform; 4] = [
    FixturePlatform {
        key: "darwin-x64",
        package_name: "@oattyio/cli-darwin-x64",
        target: "x86_64-apple-darwin",
        os: "darwin",
        cpu: "x64",
        libc: None,
        binary: "oatty",
    },
    FixturePlatform {
        key: "darwin-arm64",
        package_name: "@oattyio/cli-darwin-arm64",
        target: "aarch64-apple-darwin",
        os: "darwin",
        cpu: "arm64",
        libc: None,
        binary: "oatty",
    },
    FixturePlatform {
        key: "linux-x64",
        package_name: "@oattyio/cli-linux-x64",
        target: "x86_64-unknown-linux-gnu",
        os: "linux",
        cpu: "x64",
        libc: Some("glibc"),
        binary: "oatty",
    },
    FixturePlatform {
        key: "win32-x64",
        package_name: "@oattyio/cli-win32-x64",
        target: "x86_64-pc-windows-msvc",
        os: "win32",
        cpu: "x64",
        libc: None,
        binary: "oatty.exe",
    },
];

/// One packed fixture tarball and the metadata the registry publishes for it.
struct Tarball {
    /// The reported artifact file name inside the destination directory.
    file_name: String,
    /// The absolute path of the packed tarball.
    path: PathBuf,
    /// The manifest the tarball was packed from.
    manifest: Value,
    /// The npm `sha512` subresource integrity value of the tarball bytes.
    integrity: String,
}

/// The captured outcome of one child process.
struct ProcessOutcome {
    /// The exit status of the child process.
    status: ExitStatus,
    /// The child's standard output, decoded lossily.
    stdout: String,
    /// The child's standard error, decoded lossily.
    stderr: String,
}

/// Options for one `npm install` against the loopback registry.
#[derive(Default)]
struct InstallOptions<'a> {
    /// Whether to install globally into the prefix.
    globally: bool,
    /// Whether to omit optional dependencies.
    omit_optional: bool,
    /// The archive to install; the umbrella package when absent.
    archive: Option<&'a Tarball>,
    /// Extra npm platform-selection flags, for example `--os=linux`.
    platform_flags: Vec<String>,
}

/// The registry data served to npm over loopback HTTP.
#[derive(Default)]
struct RegistryState {
    /// Packument bytes keyed by package name.
    packuments: BTreeMap<String, Vec<u8>>,
    /// Tarball bytes keyed by artifact file name.
    tarballs: BTreeMap<String, Vec<u8>>,
    /// Every request path the registry observed, in order.
    requests: Mutex<Vec<String>>,
}

impl RegistryState {
    /// Return a snapshot of every request path the registry observed.
    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }
}

/// The per-test fixture: packed tarballs, a loopback registry, and the installs.
struct Harness {
    /// The isolated base directory holding every generated path.
    base: TempDir,
    /// The fixture release version.
    version: &'static str,
    /// The canonical platform contracts the fixture published.
    platforms: &'static [FixturePlatform; 4],
    /// The canonical host platform key reported by the local Node runtime.
    host_key: Option<String>,
    /// The scripted native executable that all payloads embed.
    binary: PathBuf,
    /// The integrated registry state, including the observed requests.
    registry_state: Arc<RegistryState>,
    /// The base URL of the loopback registry.
    registry_address: String,
    /// An empty user npm configuration that shadows `~/.npmrc`.
    user_config: PathBuf,
    /// An empty global npm configuration.
    global_config: PathBuf,
    /// The umbrella package tarball.
    umbrella: Tarball,
    /// The local installation's `oatty` module directory, when the host is supported.
    local_module: Option<PathBuf>,
    /// The global installation's `oatty` module directory, when the host is supported.
    global_module: Option<PathBuf>,
}

impl Harness {
    /// Build the fixture, or report `None` when the Node toolchain is unavailable.
    ///
    /// # Returns
    ///
    /// Returns a ready harness, or `None` when `npm` or `node` cannot be executed.
    ///
    /// # Errors
    ///
    /// Returns an error when packing, the registry shim, or the installs fail.
    fn new() -> Result<Option<Self>> {
        if !toolchain_available() {
            return Ok(None);
        }
        let base = TempDir::new().context("failed to create the integration base directory")?;
        let version = FIXTURE_VERSION;
        let dist = base.path().join("dist");
        fs::create_dir_all(&dist)?;
        let pack_cache = base.path().join("pack-cache");

        let binary = base.path().join("oatty");
        write_file(&binary, FIXTURE_BINARY.as_bytes())?;
        set_executable(&binary)?;

        let mut payloads = Vec::new();
        for platform in FIXTURE_PLATFORMS.iter() {
            payloads.push(pack_payload(base.path(), &dist, &pack_cache, platform, version)?);
        }
        let umbrella = pack_umbrella(base.path(), &dist, &pack_cache, version)?;

        let listener = TcpListener::bind(("127.0.0.1", 0)).context("failed to bind the loopback npm registry")?;
        let registry_address = format!("http://{}", listener.local_addr()?);
        let mut registry_state = RegistryState::default();
        for tarball in payloads.iter().chain(std::iter::once(&umbrella)) {
            registry_state.tarballs.insert(tarball.file_name.clone(), fs::read(&tarball.path)?);
            let package_name = tarball
                .manifest
                .get("name")
                .and_then(Value::as_str)
                .context("fixture tarball has no package name")?;
            registry_state.packuments.insert(
                package_name.to_owned(),
                serde_json::to_vec(&packument(tarball, &registry_address)?)?,
            );
        }
        let registry_state = Arc::new(registry_state);
        serve_registry(listener, Arc::clone(&registry_state));

        let user_config = base.path().join("empty.npmrc");
        write_file(&user_config, b"")?;
        let global_config = base.path().join("empty-global.npmrc");
        write_file(&global_config, b"")?;

        let host_key = host_platform_key();
        let mut harness = Self {
            base,
            version,
            platforms: &FIXTURE_PLATFORMS,
            host_key,
            binary,
            registry_state,
            registry_address,
            user_config,
            global_config,
            umbrella,
            local_module: None,
            global_module: None,
        };
        if harness.host_platform().is_some() {
            harness.local_module = Some(harness.install("local", InstallOptions::default())?);
            harness.global_module = Some(harness.install(
                "global",
                InstallOptions {
                    globally: true,
                    ..InstallOptions::default()
                },
            )?);
        }
        Ok(Some(harness))
    }

    /// Return the base directory of the fixture.
    fn base(&self) -> &Path {
        self.base.path()
    }

    /// Return the canonical host platform contract, if it is supported.
    fn host_platform(&self) -> Option<&FixturePlatform> {
        let host_key = self.host_key.as_deref()?;
        self.platforms.iter().find(|platform| platform.key == host_key)
    }

    /// Return the local installation's `oatty` module directory.
    fn local_module(&self) -> Option<&Path> {
        self.local_module.as_deref()
    }

    /// Return the global installation's `oatty` module directory.
    fn global_module(&self) -> Option<&Path> {
        self.global_module.as_deref()
    }

    /// Return the scripted native executable bytes every payload embeds.
    fn binary_bytes(&self) -> Result<Vec<u8>> {
        fs::read(&self.binary).with_context(|| format!("failed to read {}", self.binary.display()))
    }

    /// Return the launcher script path of an installed `oatty` module.
    fn launcher_script(&self, module: &Path) -> PathBuf {
        module.join("npm").join("bin").join("oatty.js")
    }

    /// Build a command that runs the installed launcher.
    fn launcher_command(&self, module: &Path) -> Command {
        let mut command = Command::new("node");
        command.arg(self.launcher_script(module));
        command
    }

    /// Run the installed launcher with the supplied arguments.
    fn launcher(&self, module: &Path, arguments: &[&str]) -> Result<ProcessOutcome> {
        let mut command = self.launcher_command(module);
        command.args(arguments);
        run_process(command, LAUNCHER_TIMEOUT, "node launcher")
    }

    /// Resolve the installed directory of one native payload package.
    ///
    /// npm may hoist the optional package next to the `oatty` module or keep it nested
    /// inside it, so both layouts are probed exactly like the Python suite.
    fn native_directory(&self, module: &Path, platform: &FixturePlatform) -> PathBuf {
        let nested = module.join("node_modules").join(platform.package_name);
        if nested.exists() {
            return nested;
        }
        module.parent().map(|parent| parent.join(platform.package_name)).unwrap_or(nested)
    }

    /// Install a real tarball against the loopback registry and an empty cache.
    ///
    /// # Arguments
    ///
    /// * `label` - The fixture-unique installation label used for prefixes and caches.
    /// * `options` - The installation mode, archive, and platform-selection flags.
    ///
    /// # Returns
    ///
    /// Returns the installed `oatty` module directory.
    ///
    /// # Errors
    ///
    /// Returns an error when npm cannot be spawned or reports a non-zero exit status.
    fn install(&self, label: &str, options: InstallOptions<'_>) -> Result<PathBuf> {
        let prefix = self.base().join(label);
        let cache = self.base().join(format!("{label}-cache"));
        fs::create_dir_all(&prefix)?;
        fs::create_dir_all(&cache)?;
        let archive = options.archive.unwrap_or(&self.umbrella);
        let mut arguments = vec![
            "install".to_owned(),
            "--ignore-scripts".to_owned(),
            "--prefix".to_owned(),
            prefix.display().to_string(),
            "--cache".to_owned(),
            cache.display().to_string(),
            "--registry".to_owned(),
            self.registry_address.clone(),
            "--no-audit".to_owned(),
            "--no-fund".to_owned(),
            "--userconfig".to_owned(),
            self.user_config.display().to_string(),
            "--globalconfig".to_owned(),
            self.global_config.display().to_string(),
        ];
        if options.globally {
            arguments.push("--global".to_owned());
        }
        if options.omit_optional {
            arguments.push("--omit=optional".to_owned());
        }
        arguments.extend(options.platform_flags);
        arguments.push(archive.path.display().to_string());

        let mut command = npm_command(&arguments, None);
        command.env("npm_config_prefix", &prefix).env("npm_config_cache", &cache);
        let outcome = run_process(command, NPM_TIMEOUT, "npm install")?;
        if !outcome.status.success() {
            bail!(
                "npm install failed for {label}\ncommand: npm {}\nstdout:\n{}\nstderr:\n{}",
                arguments.join(" "),
                outcome.stdout,
                outcome.stderr
            );
        }
        let module = if options.globally && !cfg!(windows) {
            prefix.join("lib").join("node_modules").join("oatty")
        } else {
            prefix.join("node_modules").join("oatty")
        };
        if !module.is_dir() {
            bail!("npm install did not create {}", module.display());
        }
        Ok(module)
    }

    /// Run `npm view <specification> --json` against the loopback registry.
    fn npm_view(&self, specification: &str) -> Result<ProcessOutcome> {
        let cache = self.base().join("view-cache");
        fs::create_dir_all(&cache)?;
        let arguments = vec![
            "view".to_owned(),
            specification.to_owned(),
            "--json".to_owned(),
            "--registry".to_owned(),
            self.registry_address.clone(),
            "--cache".to_owned(),
            cache.display().to_string(),
            "--userconfig".to_owned(),
            self.user_config.display().to_string(),
        ];
        run_process(npm_command(&arguments, None), NPM_TIMEOUT, "npm view")
    }
}

/// Run the npm command line tool with the supplied arguments.
///
/// Honors `NPM_CLI` exactly like the packaging code so CI can pin an npm version.
fn npm_command(arguments: &[String], working_directory: Option<&Path>) -> Command {
    let prefix = npm_prefix();
    let mut command = Command::new(&prefix[0]);
    command.args(&prefix[1..]).args(arguments);
    if let Some(working_directory) = working_directory {
        command.current_dir(working_directory);
    }
    command
        .env("npm_config_ignore_scripts", "true")
        .env("npm_config_update_notifier", "false")
        .env("npm_config_audit", "false")
        .env("npm_config_fund", "false");
    command
}

/// Return the argv prefix used to invoke npm, honoring `NPM_CLI`.
fn npm_prefix() -> Vec<String> {
    if let Some(cli) = std::env::var_os("NPM_CLI").filter(|value| !value.is_empty()) {
        return vec!["node".to_owned(), cli.to_string_lossy().into_owned()];
    }
    vec!["npm".to_owned()]
}

/// Report whether `npm` and `node` can be executed on this machine.
fn toolchain_available() -> bool {
    let prefix = npm_prefix();
    let mut npm_arguments: Vec<String> = prefix[1..].to_vec();
    npm_arguments.push("--version".to_owned());
    command_succeeds(&prefix[0], &npm_arguments) && command_succeeds("node", &["--version".to_owned()])
}

/// Report whether one executable exits successfully with the supplied arguments.
fn command_succeeds(program: &str, arguments: &[String]) -> bool {
    Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Run a prepared command and capture its output within a time limit.
///
/// # Arguments
///
/// * `command` - The fully configured command to run.
/// * `timeout` - The maximum runtime before the child is killed.
/// * `description` - A human-readable name used in error messages.
///
/// # Errors
///
/// Returns an error when the child cannot be spawned or exceeds the timeout.
fn run_process(mut command: Command, timeout: Duration, description: &str) -> Result<ProcessOutcome> {
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to spawn {description}: {command:?}"))?;
    let stdout = child.stdout.take().context("the child stdout pipe is missing")?;
    let stderr = child.stderr.take().context("the child stderr pipe is missing")?;
    let status = wait_with_timeout(&mut child, timeout, description)?;
    let mut stdout_text = String::new();
    let mut stderr_text = String::new();
    BufReader::new(stdout).read_to_string(&mut stdout_text)?;
    BufReader::new(stderr).read_to_string(&mut stderr_text)?;
    Ok(ProcessOutcome {
        status,
        stdout: stdout_text,
        stderr: stderr_text,
    })
}

/// Wait for a child process, killing it once the timeout elapses.
fn wait_with_timeout(child: &mut Child, timeout: Duration, description: &str) -> Result<ExitStatus> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            bail!("{description} exceeded its {timeout:?} timeout");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

/// Report the signal that terminated a status on POSIX hosts.
#[cfg(unix)]
fn terminated_signal(status: &ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt as _;

    status.signal()
}

/// Report the signal that terminated a status where signals are unavailable.
#[cfg(not(unix))]
fn terminated_signal(_status: &ExitStatus) -> Option<i32> {
    None
}

/// Send a named signal to a process on POSIX hosts.
#[cfg(unix)]
fn send_signal(process_id: u32, signal: &str) -> Result<()> {
    let mut command = Command::new("kill");
    command.args(["-s", signal, &process_id.to_string()]);
    let outcome = run_process(command, Duration::from_secs(10), "kill")?;
    if !outcome.status.success() {
        bail!("failed to send {signal} to {process_id}: {}", outcome.stderr);
    }
    Ok(())
}

/// Send a named signal to a process where signals are unavailable.
#[cfg(not(unix))]
fn send_signal(_process_id: u32, _signal: &str) -> Result<()> {
    Ok(())
}

/// Report the canonical platform key reported by the local Node runtime.
fn host_platform_key() -> Option<String> {
    let output = Command::new("node")
        .args(["-p", r#"process.platform+"-"+process.arch"#])
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok().map(|value| value.trim().to_owned())
}

/// Pack one native payload fixture and return its tarball metadata.
///
/// # Errors
///
/// Returns an error when the staging tree or `npm pack` fails.
fn pack_payload(staging_root: &Path, dist: &Path, pack_cache: &Path, platform: &FixturePlatform, version: &str) -> Result<Tarball> {
    let stage = staging_root.join("stage").join(platform.key);
    let manifest = payload_manifest(platform, version);
    write_json(&stage.join("package.json"), &manifest)?;
    write_file(&stage.join("LICENSE"), FIXTURE_LICENSE)?;
    let executable = stage.join("bin").join(platform.binary);
    write_file(&executable, FIXTURE_BINARY.as_bytes())?;
    set_executable(&executable)?;
    write_file(
        &stage.join("bin").join(".oatty-installed-version"),
        format!("{version}\n").as_bytes(),
    )?;
    pack_stage(&stage, &manifest, dist, pack_cache, platform.key)
}

/// Pack the umbrella launcher fixture and return its tarball metadata.
///
/// # Errors
///
/// Returns an error when the staging tree or `npm pack` fails.
fn pack_umbrella(staging_root: &Path, dist: &Path, pack_cache: &Path, version: &str) -> Result<Tarball> {
    let stage = staging_root.join("stage").join("umbrella");
    let manifest = umbrella_manifest(version);
    write_json(&stage.join("package.json"), &manifest)?;
    write_file(&stage.join("LICENSE"), FIXTURE_LICENSE)?;
    write_file(&stage.join("README.md"), FIXTURE_README)?;
    write_file(
        &stage.join("npm").join("platforms.json"),
        &serde_json::to_vec_pretty(&platforms_document())?,
    )?;
    let launcher = stage.join("npm").join("bin").join("oatty.js");
    write_file(&launcher, &fs::read(project_root()?.join("npm").join("bin").join("oatty.js"))?)?;
    set_executable(&launcher)?;
    pack_stage(&stage, &manifest, dist, pack_cache, "umbrella")
}

/// Run real `npm pack` against one staging tree and describe the artifact.
fn pack_stage(stage: &Path, manifest: &Value, dist: &Path, pack_cache: &Path, label: &str) -> Result<Tarball> {
    fs::create_dir_all(dist)?;
    let cache = pack_cache.join(label);
    fs::create_dir_all(&cache)?;
    let arguments = vec![
        "pack".to_owned(),
        "--ignore-scripts".to_owned(),
        "--json".to_owned(),
        "--pack-destination".to_owned(),
        dist.display().to_string(),
    ];
    let mut command = npm_command(&arguments, Some(stage));
    command.env("npm_config_cache", &cache);
    let outcome = run_process(command, NPM_TIMEOUT, "npm pack")?;
    if !outcome.status.success() {
        bail!(
            "npm pack failed for {}\nstdout:\n{}\nstderr:\n{}",
            stage.display(),
            outcome.stdout,
            outcome.stderr
        );
    }
    let file_name = pack_artifact_name(outcome.stdout.as_bytes())?;
    let path = dist.join(&file_name);
    let bytes = fs::read(&path).with_context(|| format!("npm pack did not produce {}", path.display()))?;
    let integrity = format!("sha512-{}", BASE64_STANDARD.encode(Sha512::digest(&bytes)));
    Ok(Tarball {
        file_name,
        path,
        manifest: manifest.clone(),
        integrity,
    })
}

/// Extract the single artifact file name from an `npm pack --json` report.
///
/// npm 11 prints a JSON list and npm 12 prints an object of values.
fn pack_artifact_name(standard_output: &[u8]) -> Result<String> {
    let report: Value = serde_json::from_slice(standard_output).context("unexpected npm pack report")?;
    let entries: Vec<&Value> = match &report {
        Value::Array(items) => items.iter().collect(),
        Value::Object(entries) => entries.values().collect(),
        _ => bail!("unexpected npm pack report"),
    };
    if entries.len() != 1 {
        bail!("expected exactly one npm pack artifact, received {}", entries.len());
    }
    let file_name = entries[0]
        .get("filename")
        .and_then(Value::as_str)
        .context("npm pack reported no artifact filename")?;
    if !file_name.ends_with(".tgz") || file_name.contains(['/', '\\']) {
        bail!("unexpected npm pack artifact filename: {file_name}");
    }
    Ok(file_name.to_owned())
}

/// Build the registry packument for one fixture tarball.
fn packument(tarball: &Tarball, registry_address: &str) -> Result<Value> {
    let package_name = tarball
        .manifest
        .get("name")
        .and_then(Value::as_str)
        .context("fixture tarball has no package name")?;
    let version = tarball
        .manifest
        .get("version")
        .and_then(Value::as_str)
        .context("fixture tarball has no version")?;
    let mut served = tarball.manifest.clone();
    served["dist"] = json!({
        "tarball": format!("{registry_address}/tarballs/{}", tarball.file_name),
        "integrity": tarball.integrity,
    });
    let mut versions = Map::new();
    versions.insert(version.to_owned(), served);
    let mut dist_tags = Map::new();
    dist_tags.insert("latest".to_owned(), json!(version));
    Ok(json!({
        "name": package_name,
        "dist-tags": Value::Object(dist_tags),
        "versions": Value::Object(versions),
    }))
}

/// Serve npm registry requests until the process exits.
fn serve_registry(listener: TcpListener, state: Arc<RegistryState>) {
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let state = Arc::clone(&state);
            thread::spawn(move || {
                let _ = handle_registry_request(stream, &state);
            });
        }
    });
}

/// Answer one registry request with packuments, tarballs, or a structured E404.
fn handle_registry_request(mut stream: TcpStream, state: &RegistryState) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header == "\r\n" {
            break;
        }
    }
    let target = request_line.split_whitespace().nth(1).unwrap_or("/");
    let path = percent_decode(target.split('?').next().unwrap_or("/"));
    let name = path.trim_start_matches('/').to_owned();
    state
        .requests
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(name.clone());

    let (status, content_type, body) = resolve_registry_response(state, &name);
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(&body)?;
    stream.flush()?;
    Ok(())
}

/// Resolve the status, content type, and body for one registry request path.
fn resolve_registry_response(state: &RegistryState, name: &str) -> (&'static str, &'static str, Vec<u8>) {
    if let Some(file_name) = name.strip_prefix("tarballs/")
        && let Some(bytes) = state.tarballs.get(file_name)
    {
        return ("200 OK", "application/octet-stream", bytes.clone());
    }
    if let Some(packument) = state.packuments.get(name) {
        return ("200 OK", "application/json", packument.clone());
    }
    ("404 Not Found", "application/json", br#"{"error":"not found"}"#.to_vec())
}

/// Decode percent-encoded URL path segments, as npm sends for scoped package names.
fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let decoded_byte = if bytes[index] == b'%' && index + 2 < bytes.len() {
            hex_value(bytes[index + 1])
                .zip(hex_value(bytes[index + 2]))
                .map(|(high, low)| high * 16 + low)
        } else {
            None
        };
        if let Some(decoded_byte) = decoded_byte {
            decoded.push(decoded_byte);
            index += 3;
            continue;
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// Decode one hexadecimal digit.
fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Build the native payload manifest for one fixture platform.
fn payload_manifest(platform: &FixturePlatform, version: &str) -> Value {
    let mut manifest = json!({
        "name": platform.package_name,
        "version": version,
        "description": format!("Oatty native executable for {}", platform.target),
        "license": "MIT",
        "repository": { "type": "git", "url": "git+https://github.com/oattyio/oatty.git" },
        "os": [platform.os],
        "cpu": [platform.cpu],
        "oattyTarget": platform.target,
        "files": ["bin", "LICENSE"],
    });
    if let Some(libc) = platform.libc {
        manifest["libc"] = json!([libc]);
    }
    manifest
}

/// Build the umbrella launcher manifest for the fixture.
fn umbrella_manifest(version: &str) -> Value {
    let mut optional_dependencies = Map::new();
    for platform in FIXTURE_PLATFORMS.iter() {
        optional_dependencies.insert(platform.package_name.to_owned(), json!(version));
    }
    json!({
        "name": "oatty",
        "version": version,
        "description": "Oatty fixture launcher",
        "license": "MIT",
        "repository": { "type": "git", "url": "git+https://github.com/oattyio/oatty.git" },
        "bin": { "oatty": "npm/bin/oatty.js" },
        "files": ["npm/bin/oatty.js", "npm/platforms.json", "README.md", "LICENSE"],
        "engines": { "node": ">=18" },
        "optionalDependencies": Value::Object(optional_dependencies),
    })
}

/// Build the canonical `npm/platforms.json` document the fixture launcher reads.
fn platforms_document() -> Value {
    let mut platforms = Map::new();
    for platform in FIXTURE_PLATFORMS.iter() {
        let mut entry = json!({
            "package": platform.package_name,
            "target": platform.target,
            "os": platform.os,
            "cpu": platform.cpu,
            "binary": platform.binary,
        });
        if let Some(libc) = platform.libc {
            entry["libc"] = json!(libc);
        }
        platforms.insert(platform.key.to_owned(), entry);
    }
    Value::Object(platforms)
}

/// Read the release version declared by the workspace root manifest.
fn project_version() -> Result<String> {
    let encoded = fs::read(project_root()?.join("package.json"))?;
    let manifest: Value = serde_json::from_slice(&encoded)?;
    manifest
        .get("version")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .context("the workspace root manifest declares no version")
}

/// Resolve the workspace root directory from the crate location.
fn project_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .context("failed to resolve the workspace root")
}

/// Write one file, creating its parent directories first.
fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

/// Write one JSON document with a trailing newline.
fn write_json(path: &Path, value: &Value) -> Result<()> {
    let mut document = serde_json::to_string_pretty(value)?;
    document.push('\n');
    write_file(path, document.as_bytes())
}

/// Mark one staged file executable on POSIX hosts.
#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).with_context(|| format!("failed to mark {} executable", path.display()))
}

/// Mark one staged file executable where execute bits are unavailable.
#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}

/// Report whether the POSIX fixture script can be executed directly.
fn fixture_script_supported() -> bool {
    cfg!(unix)
}

/// Resolve the npm shim path for one installation layout.
fn shim_path(module: &Path, globally: bool) -> PathBuf {
    let shim_name = if cfg!(windows) { "oatty.cmd" } else { "oatty" };
    if globally {
        let prefix = module
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| module.to_path_buf());
        if cfg!(windows) {
            return prefix.join(shim_name);
        }
        return prefix.join("bin").join(shim_name);
    }
    module
        .parent()
        .map(|parent| parent.join(".bin").join(shim_name))
        .unwrap_or_else(|| module.join(".bin").join(shim_name))
}

/// Run the installed npm shim for one installation.
fn run_shim(module: &Path, globally: bool) -> Result<ProcessOutcome> {
    let executable = shim_path(module, globally);
    if !executable.exists() {
        bail!("npm did not create the shim {}", executable.display());
    }
    let mut command = Command::new(&executable);
    command.arg("--version");
    run_process(command, LAUNCHER_TIMEOUT, "oatty shim")
}

/// Return the POSIX signal number for a named signal.
#[cfg(unix)]
fn signal_number(name: &str) -> Option<i32> {
    match name {
        "SIGINT" => Some(SIGINT),
        "SIGTERM" => Some(SIGTERM),
        _ => None,
    }
}

/// Return the signal number for a named signal where signals are unavailable.
#[cfg(not(unix))]
fn signal_number(_name: &str) -> Option<i32> {
    None
}

/// Normalize one `npm view --json` response into the registry manifest.
///
/// npm 12 keeps the version result array; npm 11 returns a single manifest
/// object. This mirrors `npm_publish::registry_manifest` so the test asserts
/// on the same normalization the publisher runs in production.
fn npm_single_view(stdout: &str) -> Value {
    let mut metadata: Value = serde_json::from_str(stdout).expect("npm view did not return JSON");
    if let Value::Array(entries) = &metadata {
        assert_eq!(entries.len(), 1, "npm view must return exactly one version result");
        metadata = entries[0].clone();
    }
    assert!(metadata.is_object(), "npm view must return an object or a single-element array");
    metadata
}

/// Mirror `npm_publish::is_not_found_response` for an unpublished specifier.
fn npm_view_is_not_found(stdout: &str) -> bool {
    serde_json::from_str::<Value>(stdout)
        .ok()
        .and_then(|payload| payload.get("error").and_then(|error| error.get("code")).cloned())
        == Some(json!("E404"))
}

#[test]
fn npm_distribution_registry_reads_exact_metadata_and_distinguishes_e404() -> Result<()> {
    let Some(harness) = Harness::new()? else {
        return Ok(());
    };
    let view = harness.npm_view(&format!("oatty@{}", harness.version))?;
    assert!(view.status.success(), "npm view failed:\n{}\n{}", view.stdout, view.stderr);
    let registry = npm_single_view(&view.stdout);
    assert_eq!(registry["version"], json!(harness.version));
    let umbrella_bytes = fs::read(&harness.umbrella.path)?;
    let expected_integrity = format!("sha512-{}", BASE64_STANDARD.encode(Sha512::digest(&umbrella_bytes)));
    assert_eq!(registry["dist"]["integrity"], json!(expected_integrity));

    let missing = harness.npm_view(&format!("@oattyio/not-a-real-package@{}", harness.version))?;
    assert!(!missing.status.success(), "an unpublished package must not resolve");
    assert!(npm_view_is_not_found(&missing.stdout), "npm view must report a structured E404");
    Ok(())
}

#[test]
fn npm_distribution_libc_selection_installs_glibc_payload_and_omits_it_on_musl() -> Result<()> {
    let Some(harness) = Harness::new()? else {
        return Ok(());
    };
    for libc in ["glibc", "musl"] {
        let module = harness.install(
            &format!("libc-{libc}"),
            InstallOptions {
                platform_flags: vec!["--os=linux".to_owned(), "--cpu=x64".to_owned(), format!("--libc={libc}")],
                ..InstallOptions::default()
            },
        )?;
        for platform in harness.platforms.iter() {
            let nested = module.join("node_modules").join(platform.package_name);
            let hoisted = module
                .parent()
                .map(|parent| parent.join(platform.package_name))
                .unwrap_or_else(|| nested.clone());
            assert_eq!(
                libc == "glibc" && platform.key == "linux-x64",
                nested.exists() || hoisted.exists(),
                "unexpected payload selection for {libc} {}",
                platform.key
            );
        }
    }
    Ok(())
}

#[test]
fn npm_distribution_source_lock_bootstraps_without_registry_or_scripts() -> Result<()> {
    let Some(harness) = Harness::new()? else {
        return Ok(());
    };
    let work = harness.base().join("offline-bootstrap");
    fs::create_dir_all(&work)?;
    let project = project_root()?;
    for name in ["package.json", "package-lock.json"] {
        fs::copy(project.join(name), work.join(name)).with_context(|| format!("failed to copy {name}"))?;
    }
    let cache = harness.base().join("offline-cache");
    fs::create_dir_all(&cache)?;
    let arguments = vec![
        "ci".to_owned(),
        "--omit=optional".to_owned(),
        "--ignore-scripts".to_owned(),
        "--offline".to_owned(),
        "--cache".to_owned(),
        cache.display().to_string(),
        "--userconfig".to_owned(),
        harness.user_config.display().to_string(),
        "--globalconfig".to_owned(),
        harness.global_config.display().to_string(),
    ];
    let outcome = run_process(npm_command(&arguments, Some(&work)), NPM_TIMEOUT, "npm ci")?;
    assert!(outcome.status.success(), "npm ci failed:\n{}\n{}", outcome.stdout, outcome.stderr);

    let locked: Value = serde_json::from_slice(&fs::read(work.join("package-lock.json"))?)?;
    let packages = locked
        .get("packages")
        .and_then(Value::as_object)
        .context("the npm source lock has no packages")?;
    assert_eq!(packages.len(), 5);
    let version = project_version()?;
    for platform in harness.platforms.iter() {
        let lock_key = format!("node_modules/{}", platform.package_name);
        let entry = packages
            .get(lock_key.as_str())
            .with_context(|| format!("the npm source lock is missing {}", platform.package_name))?;
        assert_eq!(entry["version"], json!(version));
        assert_eq!(entry["optional"], json!(true));
        assert_eq!(entry["os"], json!([platform.os]));
        assert_eq!(entry["cpu"], json!([platform.cpu]));
        let expected_libc = match platform.libc {
            Some(libc) => json!([libc]),
            None => Value::Null,
        };
        assert_eq!(entry.get("libc").cloned().unwrap_or(Value::Null), expected_libc);
        assert!(entry.get("integrity").is_none(), "the source lock must not pin integrity");
    }
    Ok(())
}

#[test]
fn npm_distribution_local_global_ignore_scripts_installs_run_unchanged_tarballs() -> Result<()> {
    let Some(harness) = Harness::new()? else {
        return Ok(());
    };
    let Some(host) = harness.host_platform() else {
        return Ok(());
    };
    if !fixture_script_supported() {
        return Ok(());
    }
    let binary_bytes = harness.binary_bytes()?;
    let installs = [
        (false, harness.local_module().context("the local fixture install is missing")?),
        (true, harness.global_module().context("the global fixture install is missing")?),
    ];
    for (globally, module) in installs {
        let version = harness.launcher(module, &["--version"])?;
        assert_eq!(version.stdout.trim(), format!("oatty {}", harness.version));

        let check = harness.launcher(module, &["update", "--check"])?;
        assert_eq!(Some(0), check.status.code(), "{}", check.stderr);
        assert!(
            check.stdout.contains(&format!("Installed version: {}", harness.version)),
            "{}",
            check.stdout
        );

        let native = harness.native_directory(module, host).join("bin").join(host.binary);
        assert_eq!(fs::read(&native)?, binary_bytes, "installed payload bytes changed");

        for platform in harness.platforms.iter() {
            let nested = module.join("node_modules").join(platform.package_name);
            let hoisted = module
                .parent()
                .map(|parent| parent.join(platform.package_name))
                .unwrap_or_else(|| nested.clone());
            assert_eq!(
                platform.key == harness.host_key.as_deref().unwrap_or_default(),
                nested.exists() || hoisted.exists(),
                "unexpected payload presence for {}",
                platform.key
            );
        }

        let shim = run_shim(module, globally)?;
        assert_eq!(Some(0), shim.status.code(), "shim failed:\n{}", shim.stderr);
    }
    assert!(
        harness
            .registry_state
            .requests()
            .iter()
            .any(|name| name.starts_with("tarballs/oattyio-cli-")),
        "the install never downloaded a native payload tarball"
    );
    Ok(())
}

#[test]
fn npm_distribution_arguments_cwd_environment_and_status_are_preserved() -> Result<()> {
    let Some(harness) = Harness::new()? else {
        return Ok(());
    };
    let Some(module) = harness.local_module() else {
        return Ok(());
    };
    if !fixture_script_supported() {
        return Ok(());
    }
    let arguments = ["one two", "quote\"", "--flag=x", "💚"];
    let mut command = harness.launcher_command(module);
    command
        .args(arguments)
        .current_dir(harness.base())
        .env("OATTY_FIXTURE_VALUE", "forwarded");
    let outcome = run_process(command, LAUNCHER_TIMEOUT, "node launcher")?;
    assert_eq!(Some(0), outcome.status.code(), "{}", outcome.stderr);
    let forwarded: Value = serde_json::from_str(&outcome.stdout).context("the launcher forwarded no JSON")?;
    assert_eq!(forwarded["args"], json!(arguments));
    assert_eq!(forwarded["cwd"], json!(fs::canonicalize(harness.base())?.display().to_string()));
    assert_eq!(forwarded["environment"], json!("forwarded"));

    let status = harness.launcher(module, &["status", "37"])?;
    assert_eq!(Some(37), status.status.code());
    Ok(())
}

#[test]
fn npm_distribution_child_and_parent_directed_signals_preserve_status() -> Result<()> {
    let Some(harness) = Harness::new()? else {
        return Ok(());
    };
    let Some(module) = harness.local_module() else {
        return Ok(());
    };
    if !fixture_script_supported() {
        return Ok(());
    }
    for (name, number) in [("SIGINT", signal_number("SIGINT")), ("SIGTERM", signal_number("SIGTERM"))] {
        let child_outcome = harness.launcher(module, &["signal", name])?;
        assert_eq!(
            terminated_signal(&child_outcome.status),
            number,
            "{name} was not preserved by the launcher"
        );

        let mut command = harness.launcher_command(module);
        command.arg("wait").stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command.spawn().context("failed to spawn the waiting launcher")?;
        let stdout = child.stdout.take().context("the launcher stdout pipe is missing")?;
        let mut reader = BufReader::new(stdout);
        let mut ready = String::new();
        reader.read_line(&mut ready)?;
        assert_eq!("ready", ready.trim(), "the native fixture never reported readiness");

        send_signal(child.id(), name)?;
        let status = wait_with_timeout(&mut child, Duration::from_secs(10), "waiting launcher")?;
        assert_eq!(terminated_signal(&status), number, "{name} was not forwarded to the child");
    }
    Ok(())
}

#[test]
fn npm_distribution_omit_optional_gives_reinstall_guidance() -> Result<()> {
    let Some(harness) = Harness::new()? else {
        return Ok(());
    };
    let Some(host) = harness.host_platform() else {
        return Ok(());
    };
    if !fixture_script_supported() {
        return Ok(());
    }
    let module = harness.install(
        "local-omit",
        InstallOptions {
            omit_optional: true,
            ..InstallOptions::default()
        },
    )?;
    let outcome = harness.launcher(&module, &["--version"])?;
    assert_eq!(Some(1), outcome.status.code());
    assert!(outcome.stderr.contains("--include=optional"), "{}", outcome.stderr);
    assert!(
        outcome.stderr.contains(&format!("{}@{}", host.package_name, harness.version)),
        "{}",
        outcome.stderr
    );
    Ok(())
}

#[test]
fn npm_distribution_rejects_mismatch_unsupported_musl_and_missing_marker() -> Result<()> {
    let Some(harness) = Harness::new()? else {
        return Ok(());
    };
    let Some(host) = harness.host_platform() else {
        return Ok(());
    };
    let Some(module) = harness.local_module() else {
        return Ok(());
    };
    if !fixture_script_supported() {
        return Ok(());
    }
    let metadata_path = harness.native_directory(module, host).join("package.json");
    let original = fs::read(&metadata_path)?;
    let manifest: Value = serde_json::from_slice(&original)?;
    for (field, value) in [
        ("version", json!("0.0.0")),
        ("os", json!(["unsupported"])),
        ("cpu", json!(["unsupported"])),
        ("oattyTarget", json!("wrong")),
    ] {
        let mut modified = manifest.clone();
        modified[field] = value;
        write_json(&metadata_path, &modified)?;
        let outcome = harness.launcher(module, &[])?;
        assert_eq!(Some(1), outcome.status.code(), "{field} must be rejected");
        assert!(outcome.stderr.contains(&format!("metadata: {field}")), "{}", outcome.stderr);
        assert!(outcome.stderr.contains("--include=optional"), "{}", outcome.stderr);
    }
    write_file(&metadata_path, &original)?;

    let marker = metadata_path
        .parent()
        .map(|directory| directory.join("bin").join(".oatty-installed-version"))
        .context("the payload directory has no parent")?;
    let original_marker = fs::read(&marker)?;
    fs::remove_file(&marker)?;
    let missing_marker = harness.launcher(module, &[])?;
    assert_eq!(Some(1), missing_marker.status.code());
    write_file(&marker, &original_marker)?;

    let launcher_literal = json!(harness.launcher_script(module).display().to_string());
    let overrides = [
        (
            r#"Object.defineProperty(process,'arch',{value:'riscv64'});"#,
            "Unsupported platform",
        ),
        (
            r#"Object.defineProperty(process,'platform',{value:'linux'});Object.defineProperty(process,'arch',{value:'x64'});process.report.getReport=()=>({header:{}});"#,
            "musl",
        ),
    ];
    for (setup, expected) in overrides {
        let script = format!("{setup}require({launcher_literal});");
        let mut command = Command::new("node");
        command.args(["-e", &script]);
        let outcome = run_process(command, LAUNCHER_TIMEOUT, "node -e launcher override")?;
        assert_eq!(Some(1), outcome.status.code());
        assert!(outcome.stderr.contains(expected), "{}", outcome.stderr);
    }
    Ok(())
}
