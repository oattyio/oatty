//! Package the signed release executable unchanged, with verified legal materials.
//!
//! This module is a behavior-preserving port of `scripts/package-release.py`. It
//! stages the release executable next to the first-party license and every
//! generated licensing material, writes a platform-appropriate archive, and then
//! reads every entry back to prove that the executable and the legal bytes
//! survived the round trip unchanged.
//!
//! # Archive formats
//!
//! * `*-unknown-linux-gnu` targets use a deterministic `.tar.gz` written with the
//!   `tar` and `flate2` crates; gzip timestamps are pinned to zero.
//! * macOS Apple targets packaged on a macOS host use
//!   `ditto -c -k --sequesterRsrc`, preserving the existing resource-fork aware,
//!   notarizable zip format.
//! * Every other combination uses the `zip` crate with deflate compression.
//!
//! # Testability
//!
//! Licensing verification and the generated materials are workspace inputs, so
//! [`package_release_with`] injects the verifier. Unit tests package a fixture
//! tree with a no-op verifier instead of running the real licensing pipeline.

use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use flate2::Compression;
use flate2::GzBuilder;
use flate2::read::GzDecoder;
use zip::write::{FileOptions, SimpleFileOptions};
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// The release targets accepted by the packaging step.
const TARGETS: [&str; 4] = [
    "x86_64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
];

/// The first-party license file copied into every release archive.
const LICENSE_NAME: &str = "LICENSE";

/// The temporary staging directory prefix used while assembling an archive.
const STAGE_PREFIX: &str = "oatty-package-";

/// The program used to build resource-fork aware macOS archives.
const DITTO_PROGRAM: &str = "ditto";

/// Package the signed release executable unchanged, with verified legal materials.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `binary` - The release executable to package.
/// * `target` - The Rust target triple the executable was built for.
/// * `version` - The release version embedded in the archive name.
/// * `destination` - The directory to write the archive into.
///
/// # Returns
///
/// Returns the absolute path of the written, read-back verified archive.
///
/// # Errors
///
/// Returns an error when the target is unsupported, licensing verification
/// fails, the archive cannot be written, or the read-back verification detects
/// changed or unexpected bytes.
pub(crate) fn package_release(root: &Path, binary: &Path, target: &str, version: &str, destination: &Path) -> Result<PathBuf> {
    package_release_with(root, binary, target, version, destination, &crate::licenses::verify)
}

/// Package a release archive using the supplied licensing verifier.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `binary` - The release executable to package.
/// * `target` - The Rust target triple the executable was built for.
/// * `version` - The release version embedded in the archive name.
/// * `destination` - The directory to write the archive into.
/// * `verify_licenses` - Rejects missing, stale, or edited licensing materials;
///   injectable so tests can bypass the real pipeline.
///
/// # Returns
///
/// Returns the absolute path of the written, read-back verified archive.
///
/// # Errors
///
/// Returns an error when the target is unsupported, licensing verification
/// fails, the archive cannot be written, or the read-back verification detects
/// changed or unexpected bytes.
fn package_release_with(
    root: &Path,
    binary: &Path,
    target: &str,
    version: &str,
    destination: &Path,
    verify_licenses: &dyn Fn(&Path) -> Result<()>,
) -> Result<PathBuf> {
    if !TARGETS.contains(&target) {
        bail!("Unsupported Rust target: {target}");
    }
    verify_licenses(root)?;

    let binary_name = release_binary_name(target);
    let extension = release_archive_extension(target);
    fs::create_dir_all(destination).with_context(|| format!("failed to create {}", destination.display()))?;
    let resolved_destination = fs::canonicalize(destination).with_context(|| format!("failed to resolve {}", destination.display()))?;
    let archive = resolved_destination.join(format!("oatty-{version}-{target}{extension}"));

    let stage_directory = tempfile::Builder::new()
        .prefix(STAGE_PREFIX)
        .tempdir()
        .context("failed to create the release staging directory")?;
    let stage = stage_directory.path();
    copy_file(binary, &stage.join(binary_name))?;
    copy_file(&root.join(LICENSE_NAME), &stage.join(LICENSE_NAME))?;
    for material in crate::licenses::GENERATED_MATERIALS {
        let source = root.join(material);
        let staged = stage.join(material);
        if source.is_dir() {
            copy_directory_recursively(&source, &staged)?;
        } else {
            copy_file(&source, &staged)?;
        }
    }

    if extension == ".tar.gz" {
        write_tarball(stage, &archive)?;
    } else if uses_resource_fork_zip(target) {
        write_resource_fork_zip(stage, &archive)?;
    } else {
        write_zip(stage, &archive)?;
    }
    verify_archive(&archive, binary_name, binary, root)?;
    Ok(archive)
}

/// Return the executable name embedded at the archive root for one target.
///
/// # Arguments
///
/// * `target` - The Rust target triple being packaged.
///
/// # Returns
///
/// Returns `oatty.exe` for MSVC Windows targets and `oatty` everywhere else.
fn release_binary_name(target: &str) -> &'static str {
    if target.ends_with("windows-msvc") { "oatty.exe" } else { "oatty" }
}

/// Return the archive extension used for one target.
///
/// # Arguments
///
/// * `target` - The Rust target triple being packaged.
///
/// # Returns
///
/// Returns `.tar.gz` for glibc Linux targets and `.zip` everywhere else.
fn release_archive_extension(target: &str) -> &'static str {
    if target.ends_with("linux-gnu") { ".tar.gz" } else { ".zip" }
}

/// Report whether the host builds macOS archives through `ditto`.
///
/// The Python pipeline selected `ditto` for Apple targets only when it ran on
/// macOS. The check stays dynamic so packaging another platform's archive on a
/// non-Apple host still produces a zip.
///
/// # Arguments
///
/// * `target` - The Rust target triple being packaged.
///
/// # Returns
///
/// Returns `true` when the host is macOS and the target is an Apple target.
fn uses_resource_fork_zip(target: &str) -> bool {
    std::env::consts::OS == "macos" && target.ends_with("apple-darwin")
}

/// Copy one file, creating every missing parent directory first.
///
/// Equivalent to Python's `shutil.copy2`: file contents and permission bits are
/// preserved, which keeps the staged executable executable.
///
/// # Arguments
///
/// * `source` - The file to copy.
/// * `destination` - The path to copy it to.
///
/// # Errors
///
/// Returns an error when a parent directory or the file cannot be created.
fn copy_file(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::copy(source, destination).with_context(|| format!("failed to copy {} to {}", source.display(), destination.display()))?;
    Ok(())
}

/// Copy one directory tree, preserving the source layout exactly.
///
/// # Arguments
///
/// * `source` - The directory to copy.
/// * `destination` - The directory to copy it to, created when missing.
///
/// # Errors
///
/// Returns an error when the directory cannot be read or any nested file cannot
/// be copied.
fn copy_directory_recursively(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination).with_context(|| format!("failed to create {}", destination.display()))?;
    for entry in sorted_directory_entries(source)? {
        let target = destination.join(entry.file_name());
        if entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?
            .is_dir()
        {
            copy_directory_recursively(&entry.path(), &target)?;
        } else {
            copy_file(&entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Read one directory's entries in stable name order.
///
/// Python's `sorted(stage.iterdir())` and `tarfile.add` both sort names, so the
/// archives stay reproducible across runs.
///
/// # Arguments
///
/// * `directory` - The directory to list.
///
/// # Returns
///
/// Returns the entries sorted by file name.
///
/// # Errors
///
/// Returns an error when the directory cannot be read.
fn sorted_directory_entries(directory: &Path) -> Result<Vec<fs::DirEntry>> {
    let mut entries = fs::read_dir(directory)
        .with_context(|| format!("failed to read {}", directory.display()))?
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("failed to read {}", directory.display()))?;
    entries.sort_by_key(fs::DirEntry::file_name);
    Ok(entries)
}

/// Write the staged tree as a deterministic tar.gz archive.
///
/// Every staged top-level entry keeps its plain name, and directories recurse
/// with `parent/child` names, matching Python's `output.add(item, arcname=item.name)`.
///
/// # Arguments
///
/// * `stage` - The staging directory holding the archive layout.
/// * `archive` - The archive file to create.
///
/// # Errors
///
/// Returns an error when the archive cannot be written or finished.
fn write_tarball(stage: &Path, archive: &Path) -> Result<()> {
    let output = fs::File::create(archive).with_context(|| format!("failed to create {}", archive.display()))?;
    let encoder = GzBuilder::new().mtime(0).write(output, Compression::default());
    let mut builder = tar::Builder::new(encoder);
    append_directory_entries(&mut builder, stage, Path::new(""))?;

    let encoder = builder.into_inner().context("failed to finish the release tar stream")?;
    let output = encoder.finish().context("failed to finish the release gzip stream")?;
    output.sync_all().with_context(|| format!("failed to flush {}", archive.display()))
}

/// Append every entry below a staged directory to the tar builder.
///
/// # Arguments
///
/// * `builder` - The tar builder receiving the entries.
/// * `directory` - The staged directory to walk.
/// * `prefix` - The archive path accumulated so far; empty at the archive root.
///
/// # Errors
///
/// Returns an error when a directory cannot be read or an entry cannot be added.
fn append_directory_entries<W: Write>(builder: &mut tar::Builder<W>, directory: &Path, prefix: &Path) -> Result<()> {
    for entry in sorted_directory_entries(directory)? {
        let archive_name = prefix.join(entry.file_name());
        builder
            .append_path_with_name(entry.path(), &archive_name)
            .with_context(|| format!("failed to archive {}", entry.path().display()))?;
        if entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?
            .is_dir()
        {
            append_directory_entries(builder, &entry.path(), &archive_name)?;
        }
    }
    Ok(())
}

/// Write the staged tree with `ditto`, preserving macOS resource forks.
///
/// # Arguments
///
/// * `stage` - The staging directory holding the archive layout.
/// * `archive` - The zip file to create.
///
/// # Errors
///
/// Returns an error when `ditto` cannot be executed or exits unsuccessfully.
fn write_resource_fork_zip(stage: &Path, archive: &Path) -> Result<()> {
    let output = Command::new(DITTO_PROGRAM)
        .args(["-c", "-k", "--sequesterRsrc"])
        .arg(stage)
        .arg(archive)
        .output()
        .with_context(|| format!("failed to execute {DITTO_PROGRAM}"))?;
    if !output.status.success() {
        bail!(
            "{DITTO_PROGRAM} failed to create {}: {}",
            archive.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// Write every staged file into a deflated zip archive.
///
/// Only regular files are written, keyed by their staging-relative POSIX path,
/// matching Python's `stage.rglob('*')` selection. Every entry records the
/// staged file's `stat` mode so executable bits survive Unix extraction,
/// mirroring what CPython's `zipfile` packaging recorded: Unix hosts preserve
/// the filesystem mode including the executable bit, while Windows hosts
/// emulate `os.stat` (Windows cannot express an executable bit; the recorded
/// mode reflects readability instead).
///
/// # Arguments
///
/// * `stage` - The staging directory holding the archive layout.
/// * `archive` - The zip file to create.
///
/// # Errors
///
/// Returns an error when the staged tree or archive cannot be written or
/// finished.
fn write_zip(stage: &Path, archive: &Path) -> Result<()> {
    let output = fs::File::create(archive).with_context(|| format!("failed to create {}", archive.display()))?;
    let mut writer = ZipWriter::new(output);
    let options: SimpleFileOptions = FileOptions::default().compression_method(CompressionMethod::Deflated);

    let mut relative_paths = Vec::new();
    collect_relative_files(stage, Path::new(""), &mut relative_paths)?;
    relative_paths.sort();
    for relative_path in relative_paths {
        let source = stage.join(&relative_path);
        let recorded_mode = recorded_file_mode(&source).with_context(|| format!("failed to read the mode of {}", source.display()))?;
        let name = relative_path.to_string_lossy().replace('\\', "/");
        let bytes = fs::read(&source).with_context(|| format!("failed to read {}", source.display()))?;
        writer
            .start_file(name, options.unix_permissions(recorded_mode))
            .with_context(|| format!("failed to add an entry to {}", archive.display()))?;
        writer
            .write_all(&bytes)
            .with_context(|| format!("failed to write an entry to {}", archive.display()))?;
    }
    writer.finish().with_context(|| format!("failed to finish {}", archive.display()))?;
    Ok(())
}

/// Report the staged file's `stat` mode recorded in zip entries.
///
/// # Arguments
///
/// * `path` - The staged file whose recorded mode is needed.
///
/// # Returns
///
/// Returns the full `stat` mode. Unix hosts report the filesystem mode, which
/// carries the executable bit unchanged; Windows hosts emulate CPython's
/// `os.stat` result because Windows cannot express an executable bit.
fn recorded_file_mode(path: &Path) -> std::io::Result<u32> {
    let permissions = fs::metadata(path)?.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        Ok(permissions.mode())
    }
    #[cfg(not(unix))]
    {
        if permissions.readonly() { Ok(0o100444) } else { Ok(0o100666) }
    }
}

/// Collect every regular file below a directory as a staging-relative path.
///
/// # Arguments
///
/// * `directory` - The directory to walk.
/// * `prefix` - The relative path accumulated so far.
/// * `collected` - The destination collection, extended in place.
///
/// # Errors
///
/// Returns an error when a directory cannot be read.
fn collect_relative_files(directory: &Path, prefix: &Path, collected: &mut Vec<PathBuf>) -> Result<()> {
    for entry in sorted_directory_entries(directory)? {
        let relative_path = prefix.join(entry.file_name());
        if entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?
            .is_dir()
        {
            collect_relative_files(&entry.path(), &relative_path, collected)?;
        } else {
            collected.push(relative_path);
        }
    }
    Ok(())
}

/// Read back every archive entry, enforcing the root layout and byte identity.
///
/// # Arguments
///
/// * `archive` - The archive that was just written.
/// * `binary_name` - The executable name expected at the archive root.
/// * `binary` - The original executable whose bytes must be preserved.
/// * `root` - The workspace root directory supplying the authoritative bytes.
///
/// # Errors
///
/// Returns an error when the executable bytes or root path changed, when the
/// entry set differs from the expected legal materials, or when any legal file
/// byte differs from its source.
fn verify_archive(archive: &Path, binary_name: &str, binary: &Path, root: &Path) -> Result<()> {
    let mut contents = read_archive_contents(archive)?;
    let expected = expected_archive_entries(root)?;

    let original_bytes = fs::read(binary).with_context(|| format!("failed to read {}", binary.display()))?;
    if contents.remove(binary_name).as_deref() != Some(original_bytes.as_slice()) {
        bail!("Packaged executable bytes or root path changed");
    }

    let actual: BTreeSet<String> = contents.keys().cloned().collect();
    if actual != expected {
        bail!("Release archive is missing or has unexpected licensing entries");
    }
    for name in &expected {
        let expected_bytes = fs::read(root.join(name)).with_context(|| format!("failed to read {}", root.join(name).display()))?;
        if contents.get(name) != Some(&expected_bytes) {
            bail!("Packaged license/source bytes changed: {name}");
        }
    }
    Ok(())
}

/// Read every file entry from a release archive.
///
/// # Arguments
///
/// * `archive` - The archive to read.
///
/// # Returns
///
/// Returns the entry bytes keyed by archive path. Later duplicate entries win, as
/// they did in the Python dictionary comprehension.
///
/// # Errors
///
/// Returns an error when the archive cannot be opened or an entry unreadable.
fn read_archive_contents(archive: &Path) -> Result<std::collections::BTreeMap<String, Vec<u8>>> {
    if is_tarball(archive) {
        read_tarball_contents(archive)
    } else {
        read_zip_contents(archive)
    }
}

/// Report whether a release archive is a gzip-compressed tarball.
///
/// # Arguments
///
/// * `archive` - The archive path to inspect.
///
/// # Returns
///
/// Returns `true` when the archive file name ends with `.tar.gz`.
fn is_tarball(archive: &Path) -> bool {
    archive.file_name().is_some_and(|name| name.to_string_lossy().ends_with(".tar.gz"))
}

/// Read every regular file member of a tar.gz archive.
///
/// # Arguments
///
/// * `archive` - The archive to read.
///
/// # Returns
///
/// Returns the member bytes keyed by member name.
///
/// # Errors
///
/// Returns an error when the archive cannot be opened or a member cannot be read.
fn read_tarball_contents(archive: &Path) -> Result<std::collections::BTreeMap<String, Vec<u8>>> {
    let archive_file = fs::File::open(archive).with_context(|| format!("failed to open {}", archive.display()))?;
    let mut reader = tar::Archive::new(GzDecoder::new(archive_file));
    let mut contents = std::collections::BTreeMap::new();
    for entry in reader.entries().with_context(|| format!("failed to read {}", archive.display()))? {
        let mut entry = entry.with_context(|| format!("failed to read an entry from {}", archive.display()))?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let name = entry
            .path()
            .with_context(|| format!("failed to read an entry path from {}", archive.display()))?
            .to_string_lossy()
            .into_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        contents.insert(name, bytes);
    }
    Ok(contents)
}

/// Read every non-directory zip entry that is not a macOS resource fork.
///
/// # Arguments
///
/// * `archive` - The zip archive to read.
///
/// # Returns
///
/// Returns the entry bytes keyed by entry name, skipping `__MACOSX/` members and
/// directory entries exactly as the Python pipeline did.
///
/// # Errors
///
/// Returns an error when the archive cannot be opened or an entry cannot be read.
fn read_zip_contents(archive: &Path) -> Result<std::collections::BTreeMap<String, Vec<u8>>> {
    let archive_file = fs::File::open(archive).with_context(|| format!("failed to open {}", archive.display()))?;
    let mut reader = ZipArchive::new(archive_file).with_context(|| format!("failed to read {}", archive.display()))?;
    let mut contents = std::collections::BTreeMap::new();
    for index in 0..reader.len() {
        let mut entry = reader
            .by_index(index)
            .with_context(|| format!("failed to read an entry from {}", archive.display()))?;
        if entry.is_dir() {
            continue;
        }
        let entry_name = entry
            .name()
            .with_context(|| format!("failed to read an entry name from {}", archive.display()))?
            .into_owned();
        if entry_name.starts_with("__MACOSX/") {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        contents.insert(entry_name, bytes);
    }
    Ok(contents)
}

/// Build the exact entry set every release archive must contain.
///
/// The set is `LICENSE`, the first generated material, and every file below each
/// remaining generated material, expressed as workspace-relative POSIX paths.
/// This mirrors `expected` in the Python `verify_archive` helper.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
///
/// # Returns
///
/// Returns the expected archive-relative entry names.
///
/// # Errors
///
/// Returns an error when a material directory cannot be walked.
fn expected_archive_entries(root: &Path) -> Result<BTreeSet<String>> {
    let mut expected = BTreeSet::new();
    expected.insert(LICENSE_NAME.to_owned());
    let Some((first_material, directory_materials)) = crate::licenses::GENERATED_MATERIALS.split_first() else {
        return Ok(expected);
    };
    expected.insert((*first_material).to_owned());

    let mut files = Vec::new();
    for material in directory_materials {
        collect_files_below(&root.join(material), &mut files)?;
    }
    for file in files {
        expected.insert(relative_posix_path(root, &file)?);
    }
    Ok(expected)
}

/// Collect every regular file below a path without following nested symlink loops.
///
/// Python's `Path.rglob('*')` yields nothing when the path is not a directory, so
/// a non-directory material contributes no entries.
///
/// # Arguments
///
/// * `path` - The material path to walk.
/// * `collected` - The destination collection, extended in place.
///
/// # Errors
///
/// Returns an error when a directory cannot be read.
fn collect_files_below(path: &Path, collected: &mut Vec<PathBuf>) -> Result<()> {
    if !path.is_dir() {
        return Ok(());
    }
    for entry in sorted_directory_entries(path)? {
        let entry_path = entry.path();
        if entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry_path.display()))?
            .is_dir()
        {
            collect_files_below(&entry_path, collected)?;
        } else {
            collected.push(entry_path);
        }
    }
    Ok(())
}

/// Express one path relative to the workspace root using forward slashes.
///
/// # Arguments
///
/// * `root` - The workspace root directory.
/// * `path` - A path below the workspace root.
///
/// # Returns
///
/// Returns the POSIX-style relative path.
///
/// # Errors
///
/// Returns an error when the path is not below the workspace root.
fn relative_posix_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .with_context(|| format!("{} is not inside {}", path.display(), root.display()))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;
    use tempfile::TempDir;

    use super::*;

    /// The fixture executable bytes, including bytes that must stay verbatim.
    const FIXTURE_BINARY: &[u8] = b"unchanged signed executable\x00\xff";

    /// The fixture first-party license bytes.
    const FIXTURE_LICENSE: &[u8] = b"MIT License\n\nFixture first-party license text.\n";

    /// The fixture generated notices bytes.
    const FIXTURE_NOTICE: &[u8] = b"Complete fixture notice\n";

    /// The fixture vendored upstream license bytes.
    const FIXTURE_UPSTREAM_LICENSE: &[u8] = b"Fixture upstream license\n";

    /// The fixture covered-source bytes.
    const FIXTURE_COVERED_SOURCE: &[u8] = b"Fixture covered source\x00\xff";

    /// The fixture release version embedded in every archive name.
    const FIXTURE_VERSION: &str = "v0.0.0";

    /// The fixture upstream license path relative to the workspace root.
    const FIXTURE_UPSTREAM_LICENSE_PATH: &str = "third-party-licenses/example/LICENSE";

    /// The fixture covered source path relative to the workspace root.
    const FIXTURE_COVERED_SOURCE_PATH: &str = "third-party-sources/example.crate";

    /// Return the fixture file relative path and bytes for one directory material.
    fn fixture_directory_material(material: &str) -> (&'static str, &'static [u8]) {
        match material {
            "third-party-licenses" => (FIXTURE_UPSTREAM_LICENSE_PATH, FIXTURE_UPSTREAM_LICENSE),
            "third-party-sources" => (FIXTURE_COVERED_SOURCE_PATH, FIXTURE_COVERED_SOURCE),
            other => panic!("unexpected directory material in the fixture: {other}"),
        }
    }

    /// A licensing verifier that always succeeds, standing in for real materials.
    fn no_licensing_verification(_root: &Path) -> Result<()> {
        Ok(())
    }

    /// A source tree carrying the first-party license and every generated material.
    struct Fixture {
        /// The temporary directory holding the fixture tree.
        directory: TempDir,
    }

    impl Fixture {
        /// Create the fixture tree and its licensing materials.
        fn new() -> Result<Self> {
            let directory = TempDir::new().context("failed to create the fixture directory")?;
            let fixture = Self { directory };
            let root = fixture.root();
            write_fixture_file(&root.join(LICENSE_NAME), FIXTURE_LICENSE)?;
            let (notice_material, directory_materials) = crate::licenses::GENERATED_MATERIALS
                .split_first()
                .context("the licensing materials are empty")?;
            write_fixture_file(&root.join(notice_material), FIXTURE_NOTICE)?;
            for material in directory_materials {
                let (relative_path, bytes) = fixture_directory_material(material);
                write_fixture_file(&root.join(relative_path), bytes)?;
            }
            Ok(fixture)
        }

        /// Return the fixture root directory.
        fn root(&self) -> &Path {
            self.directory.path()
        }

        /// Write the signed fixture executable and return its path.
        fn binary(&self) -> Result<PathBuf> {
            let binary = self.root().join("scratch/signed-binary");
            write_fixture_file(&binary, FIXTURE_BINARY)?;
            set_executable_mode(&binary)?;
            Ok(binary)
        }

        /// Build a staging directory that mirrors a packaged release archive.
        fn stage(&self, binary: &Path, stage: &Path) -> Result<()> {
            copy_file(binary, &stage.join("oatty"))?;
            for name in std::iter::once(LICENSE_NAME).chain(crate::licenses::GENERATED_MATERIALS) {
                let source = self.root().join(name);
                if source.is_dir() {
                    copy_directory_recursively(&source, &stage.join(name))?;
                } else {
                    copy_file(&source, &stage.join(name))?;
                }
            }
            Ok(())
        }
    }

    /// Write one fixture file, creating every missing parent directory first.
    fn write_fixture_file(path: &Path, bytes: &[u8]) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
    }

    /// Mark one fixture path as executable where the platform supports it.
    #[cfg(unix)]
    fn set_executable_mode(path: &Path) -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).with_context(|| format!("failed to mark {} executable", path.display()))
    }

    /// Mark one fixture path as executable where the platform supports it.
    #[cfg(not(unix))]
    fn set_executable_mode(_path: &Path) -> Result<()> {
        Ok(())
    }

    /// Assert that one release packaging error carries the expected message.
    fn assert_error_contains(outcome: Result<impl std::fmt::Debug>, expected: &str) {
        let error = outcome.unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "expected {expected:?} in {:?}",
            error.to_string()
        );
    }

    /// Return the archive index of one entry name.
    fn zip_entry_index(reader: &mut zip::ZipArchive<fs::File>, name: &str) -> Result<usize> {
        for index in 0..reader.len() {
            let entry_name = reader
                .by_index(index)?
                .name()
                .expect("archive entry names are readable")
                .into_owned();
            if entry_name == name {
                return Ok(index);
            }
        }
        bail!("{name} is missing from the zip archive");
    }

    #[test]
    fn release_fallback_zip_records_staged_file_modes() -> Result<()> {
        // Pick a target whose packaging path never uses ditto on any host so
        // the fallback zip writer is exercised. On apple-darwin hosts the
        // Windows MSVC target is packaged right here too, so both are safe.
        let zip_target = "x86_64-pc-windows-msvc";
        let fixture = Fixture::new()?;
        let binary = fixture.binary()?;
        let destination = TempDir::new()?;
        let archive = package_release_with(
            fixture.root(),
            &binary,
            zip_target,
            FIXTURE_VERSION,
            destination.path(),
            &no_licensing_verification,
        )?;

        let mut reader = zip::ZipArchive::new(fs::File::open(&archive)?)?;
        let executable = release_binary_name(zip_target);
        let binary_index = zip_entry_index(&mut reader, executable)?;
        let binary_mode = reader.by_index(binary_index)?.unix_mode().unwrap_or_default();
        assert_eq!(binary_mode & 0o777, 0o755, "the packaged executable must stay executable");

        let license_index = zip_entry_index(&mut reader, LICENSE_NAME)?;
        let license_mode = reader.by_index(license_index)?.unix_mode().unwrap_or_default();
        assert_eq!(license_mode & 0o777, 0o644, "legal files must keep their readable mode");
        Ok(())
    }

    #[test]
    fn release_archive_keeps_binary_at_root_and_legal_materials() -> Result<()> {
        let fixture = Fixture::new()?;
        let binary = fixture.binary()?;
        let destination_root = TempDir::new()?;
        let destination = destination_root.path().join("nested/dist");

        for target in TARGETS {
            let archive = package_release_with(
                fixture.root(),
                &binary,
                target,
                FIXTURE_VERSION,
                &destination,
                &no_licensing_verification,
            )?;
            let expected_name = format!("oatty-{FIXTURE_VERSION}-{target}{}", release_archive_extension(target));
            assert_eq!(
                Some(expected_name.as_str()),
                archive.file_name().map(|name| name.to_str().unwrap_or_default())
            );
            assert!(archive.is_file(), "{archive:?} should exist after a verified read-back");

            let contents = read_archive_contents(&archive)?;
            let binary_name = release_binary_name(target);
            assert_eq!(
                contents.get(binary_name).map(Vec::as_slice),
                Some(FIXTURE_BINARY),
                "{target} must keep the executable at the archive root"
            );
            for name in [
                LICENSE_NAME,
                crate::licenses::GENERATED_MATERIALS[0],
                FIXTURE_UPSTREAM_LICENSE_PATH,
                FIXTURE_COVERED_SOURCE_PATH,
            ] {
                let source = fs::read(fixture.root().join(name))?;
                assert_eq!(
                    contents.get(name).map(Vec::as_slice),
                    Some(source.as_slice()),
                    "{target} must archive {name} byte-for-byte"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn release_archive_verification_rejects_changed_license_bytes() -> Result<()> {
        let fixture = Fixture::new()?;
        let binary = fixture.binary()?;
        let scratch = TempDir::new()?;
        let stage = scratch.path().join("stage");
        fs::create_dir(&stage)?;
        fixture.stage(&binary, &stage)?;
        write_fixture_file(&stage.join(LICENSE_NAME), b"tampered first-party license\n")?;
        let archive = scratch.path().join("tampered.tar.gz");
        write_tarball(&stage, &archive)?;

        assert_error_contains(
            verify_archive(&archive, "oatty", &binary, fixture.root()),
            "Packaged license/source bytes changed: LICENSE",
        );
        Ok(())
    }

    #[test]
    fn release_archive_verification_rejects_missing_binary() -> Result<()> {
        let fixture = Fixture::new()?;
        let binary = fixture.binary()?;
        let scratch = TempDir::new()?;
        let stage = scratch.path().join("stage");
        fs::create_dir(&stage)?;
        fixture.stage(&binary, &stage)?;
        fs::remove_file(stage.join("oatty"))?;
        let archive = scratch.path().join("missing.tar.gz");
        write_tarball(&stage, &archive)?;

        assert_error_contains(
            verify_archive(&archive, "oatty", &binary, fixture.root()),
            "Packaged executable bytes or root path changed",
        );
        Ok(())
    }

    #[test]
    fn release_package_requires_verified_licensing_materials() -> Result<()> {
        let fixture = Fixture::new()?;
        let binary = fixture.binary()?;
        let destination = TempDir::new()?;
        let stale = |_: &Path| -> Result<()> { Err(anyhow!("Stale licensing inputs; run npm run licenses:prepare")) };

        let error = package_release_with(
            fixture.root(),
            &binary,
            "x86_64-unknown-linux-gnu",
            FIXTURE_VERSION,
            destination.path(),
            &stale,
        )
        .unwrap_err();
        assert_eq!("Stale licensing inputs; run npm run licenses:prepare", error.to_string());
        assert!(
            fs::read_dir(destination.path())?.next().is_none(),
            "no archive may be written before licensing verifies"
        );
        Ok(())
    }

    #[test]
    fn release_package_rejects_unsupported_target() -> Result<()> {
        let fixture = Fixture::new()?;
        let binary = fixture.binary()?;
        let destination = TempDir::new()?;

        assert_error_contains(
            package_release_with(
                fixture.root(),
                &binary,
                "aarch64-unknown-linux-musl",
                FIXTURE_VERSION,
                destination.path(),
                &no_licensing_verification,
            ),
            "Unsupported Rust target: aarch64-unknown-linux-musl",
        );
        assert!(
            fs::read_dir(destination.path())?.next().is_none(),
            "unsupported targets must not create an archive"
        );
        Ok(())
    }
}
