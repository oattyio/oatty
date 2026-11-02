//! Path normalizers shared by the auxiliary task modules.
//!
//! The npm tooling in this crate hands absolute paths to npm and other Node
//! tools as command-line arguments. `std::fs::canonicalize` returns those
//! paths with the Windows extended-length prefix (`\\?\`) attached, and npm
//! cannot cope with it: its file-URL conversion turns `\\?\D:\dir\file.tgz`
//! into `file:D:/?/D/dir/file.tgz`, which then fails resolution with
//! `ENOENT` (see issue #97). Every path that reaches npm as an argument must
//! therefore be normalized here first.

use std::path::{Path, PathBuf};

/// Normalize a canonicalized path so it is safe to hand to external tools.
///
/// Windows `fs::canonicalize` prefixes NT path space results with the
/// extended-length marker (`\\?\`) and spells UNC roots as `\\?\UNC\...`.
/// Most callers—but notably npm—expect ordinary DOS paths. This helper
/// converts both spellings back to their ordinary forms:
///
/// * `\\?\D:\archive\tarball.tgz` becomes `D:\archive\tarball.tgz`
/// * `\\?\UNC\server\share\file.tgz` becomes `\\server\share\file.tgz`
///
/// Unprefixed paths (and every path on macOS, Linux, and other
/// non-Windows hosts) are returned exactly as given, so calling this helper
/// unconditionally is always safe.
///
/// # Arguments
///
/// * `path` - The path to normalize, typically one produced by
///   `fs::canonicalize`.
///
/// # Returns
///
/// Returns the equivalent plain path, or the input unchanged when no
/// extended-length prefix is present.
pub(crate) fn normalize_windows_extended_length_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        if let Some(target) = text.strip_prefix(r"\\?\UNC\") {
            PathBuf::from(format!(r"\\{target}"))
        } else if let Some(target) = text.strip_prefix(r"\\?\") {
            PathBuf::from(target)
        } else {
            path.to_path_buf()
        }
    }

    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn windows_extended_length_prefixes_are_converted_to_plain_paths() {
        let drive_form = normalize_windows_extended_length_path(Path::new(r"\\?\D:\dist\oatty-1.0.0.tgz"));
        assert_eq!(drive_form, PathBuf::from(r"D:\dist\oatty-1.0.0.tgz"));

        let unc_form = normalize_windows_extended_length_path(Path::new(r"\\?\UNC\server\share\oatty-1.0.0.tgz"));
        assert_eq!(unc_form, PathBuf::from(r"\\server\share\oatty-1.0.0.tgz"));

        let already_plain = normalize_windows_extended_length_path(Path::new(r"D:\dist\oatty-1.0.0.tgz"));
        assert_eq!(already_plain, PathBuf::from(r"D:\dist\oatty-1.0.0.tgz"));
    }

    #[cfg(not(windows))]
    #[test]
    fn paths_are_returned_unchanged_on_non_windows_hosts() {
        let unix_path = Path::new("/dist/oatty-1.0.0.tgz");
        let normalized = normalize_windows_extended_length_path(unix_path);
        assert_eq!(normalized, unix_path.to_path_buf());
    }
}
