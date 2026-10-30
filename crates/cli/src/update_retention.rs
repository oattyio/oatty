//! Preserve native update archives, including their licenses and corresponding source.
//!
//! The updater calls this hook after downloading and before extracting or installing.
//! A completed, durable copy is required before installation may proceed. Each attempt
//! receives a separate directory; successful retention is never pruned automatically.

use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, anyhow, bail};
use tempfile::{Builder, NamedTempFile};

/// Tracks the complete archive retained by one native update command.
///
/// Construction has no filesystem effects. The platform data directory is resolved
/// only when the updater supplies an actual downloaded archive to [`Self::verify`].
#[derive(Clone, Default)]
pub(crate) struct ArchiveRetention {
    root: Option<PathBuf>,
    saved_path: Arc<Mutex<Option<PathBuf>>>,
}

impl ArchiveRetention {
    /// Retains the downloaded archive or rejects the update before extraction.
    pub(crate) fn verify(&self, archive: &Path) -> self_update::Result<()> {
        self.retain(archive)
            .map_err(|error| self_update::Error::archive_verification_rejected(format!("Could not save release archive: {error:#}")))
    }

    /// Returns the published archive path, if retention has completed successfully.
    pub(crate) fn saved_path(&self) -> Result<Option<PathBuf>> {
        self.saved_path
            .lock()
            .map(|path| path.clone())
            .map_err(|_| anyhow!("Release archive retention state is unavailable"))
    }

    /// Adds the retained path to a subsequent extraction or installation error.
    pub(crate) fn complete<T>(&self, result: self_update::Result<T>) -> Result<T> {
        result.map_err(|error| {
            let error = anyhow!(error);
            match self.saved_path() {
                Ok(Some(path)) => error.context(format!("Update failed; release archive remains saved at {}", path.display())),
                Ok(None) => error,
                Err(state_error) => error.context(state_error),
            }
        })
    }

    fn retain(&self, archive: &Path) -> Result<()> {
        // self_update rejects unsafe original asset names before downloading, so this
        // hook's filename is the actual selected asset name, not a preflight guess.
        let name = archive
            .file_name()
            .and_then(|name| name.to_str())
            .context("Release archive filename is unavailable")?;
        validate_archive_name(name)?;
        let mut saved_path = self
            .saved_path
            .lock()
            .map_err(|_| anyhow!("Release archive retention state is unavailable"))?;
        let root = match &self.root {
            Some(root) => root.clone(),
            None => release_directory(dirs_next::data_local_dir())?,
        };
        let path = persist_archive(archive, &root, name)?;
        *saved_path = Some(path);
        Ok(())
    }

    #[cfg(test)]
    fn in_directory(root: PathBuf) -> Self {
        Self {
            root: Some(root),
            ..Self::default()
        }
    }
}

fn release_directory(local_data: Option<PathBuf>) -> Result<PathBuf> {
    Ok(local_data
        .context("Local application data directory is unavailable; the native update was not installed")?
        .join("oatty")
        .join("releases"))
}

fn validate_archive_name(name: &str) -> Result<()> {
    let safe_characters = name
        .bytes()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, b'-' | b'_' | b'.'));
    let tag = [
        "-x86_64-unknown-linux-gnu.tar.gz",
        "-x86_64-apple-darwin.zip",
        "-aarch64-apple-darwin.zip",
        "-x86_64-pc-windows-msvc.zip",
    ]
    .iter()
    .find_map(|suffix| name.strip_prefix("oatty-")?.strip_suffix(suffix));
    if !safe_characters || tag.is_none_or(|tag| tag.is_empty() || tag.starts_with('.') || tag.ends_with('.') || tag.contains("..")) {
        bail!("Unsupported or unsafe release archive filename: {name:?}");
    }
    Ok(())
}

fn persist_archive(archive: &Path, root: &Path, name: &str) -> Result<PathBuf> {
    fs::create_dir_all(root).with_context(|| format!("Could not create release archive directory {}", root.display()))?;
    let attempt = Builder::new()
        .prefix("release-")
        .tempdir_in(root)
        .context("Could not create release retention attempt")?;
    let mut copy = NamedTempFile::new_in(attempt.path()).context("Could not create release archive copy")?;
    let mut source = File::open(archive).context("Could not read downloaded release archive")?;
    io::copy(&mut source, &mut copy).context("Could not copy complete release archive")?;
    copy.flush().context("Could not flush release archive copy")?;
    copy.as_file().sync_all().context("Could not synchronize release archive copy")?;
    let path = attempt.path().join(name);
    copy.persist_noclobber(&path)
        .map_err(|error| error.error)
        .context("Could not publish release archive without overwriting")?;
    Ok(attempt.keep().join(name))
}

#[cfg(test)]
mod tests;
