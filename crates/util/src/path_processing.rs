use std::path::PathBuf;

use dirs_next::home_dir;


pub fn expand_tilde(path: &str) -> PathBuf {
    let p = path.trim();
    if p == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from("~"));
    }
    if let Some(rest) = p.strip_prefix("~/") {
        return home_dir().unwrap_or_else(|| PathBuf::from("~")).join(rest);
    }
    if let Some(rest) = p.strip_prefix("~\\") {
        // Windows-style
        return home_dir().unwrap_or_else(|| PathBuf::from("~")).join(rest);
    }
    PathBuf::from(p)
}
