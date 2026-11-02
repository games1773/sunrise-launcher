//! Where Sunrise keeps its files.

use std::path::{Path, PathBuf};

/// `SUNRISE_DATA_DIR` wins (handy for tests and for a portable install),
/// otherwise the platform's data folder.
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("SUNRISE_DATA_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Sunrise")
}

pub fn config_file(data: &Path) -> PathBuf {
    data.join("sunrise.json")
}

/// One folder per installed Horizon build, named after its release tag.
pub fn builds_dir(data: &Path) -> PathBuf {
    data.join("builds")
}

pub fn downloads_dir(data: &Path) -> PathBuf {
    data.join("downloads")
}

/// One folder per instance: worlds, options, packs.
pub fn instances_dir(data: &Path) -> PathBuf {
    data.join("instances")
}

pub fn logs_dir(data: &Path) -> PathBuf {
    data.join("logs")
}

/// The official launcher's default game folder, per platform.
pub fn default_minecraft_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(".minecraft"))
    } else if cfg!(target_os = "macos") {
        dirs::home_dir().map(|h| h.join("Library/Application Support/minecraft"))
    } else {
        dirs::home_dir().map(|h| h.join(".minecraft"))
    }
}

pub fn ensure_dir(path: &Path) -> crate::Result<()> {
    std::fs::create_dir_all(path).map_err(|e| format!("could not create {}: {e}", path.display()))
}

/// Turn a display name into a folder-safe id.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch.is_whitespace() || ch == '-' || ch == '_') && !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "instance".to_string()
    } else {
        trimmed
    }
}

pub fn unique_slug(base: &str, taken: &[String]) -> String {
    if !taken.iter().any(|t| t == base) {
        return base.to_string();
    }
    for n in 2..1000 {
        let candidate = format!("{base}-{n}");
        if !taken.iter().any(|t| t == &candidate) {
            return candidate;
        }
    }
    format!("{base}-{}", crate::now_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_folder_safe() {
        assert_eq!(slug("My World!"), "my-world");
        assert_eq!(slug("  "), "instance");
        assert_eq!(unique_slug("a", &["a".into()]), "a-2");
    }
}
