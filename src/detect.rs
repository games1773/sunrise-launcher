//! Finding the player's own Minecraft: Java Edition 26.3 install and a Java 25
//! runtime, both of which the preparer reads from.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{paths, Settings};

pub const GAME_VERSION: &str = "26.3";
pub const REQUIRED_JAVA_MAJOR: u32 = 25;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MinecraftInstall {
    pub dir: PathBuf,
    pub has_game_version: bool,
    pub version_jar: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JavaInfo {
    pub home: PathBuf,
    pub version: String,
    pub major: u32,
}

impl JavaInfo {
    pub fn is_usable(&self) -> bool {
        self.major == REQUIRED_JAVA_MAJOR || self.major > REQUIRED_JAVA_MAJOR
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Detection {
    pub installs: Vec<MinecraftInstall>,
    /// The install Sunrise would prepare from.
    pub chosen: Option<MinecraftInstall>,
    pub java: Option<JavaInfo>,
    pub server_jar: Option<PathBuf>,
}

impl Detection {
    pub fn has_game_version(&self) -> bool {
        self.chosen.is_some()
    }

    pub fn java_ready(&self) -> bool {
        self.java.as_ref().map(JavaInfo::is_usable).unwrap_or(false)
    }
}

fn version_jar(dir: &Path) -> Option<PathBuf> {
    let candidates = [
        dir.join("versions")
            .join(GAME_VERSION)
            .join(format!("{GAME_VERSION}.jar")),
        dir.join("versions")
            .join(GAME_VERSION)
            .join(format!("{GAME_VERSION}.json")),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

pub fn inspect(dir: &Path) -> MinecraftInstall {
    let jar = version_jar(dir);
    MinecraftInstall {
        dir: dir.to_path_buf(),
        has_game_version: dir
            .join("versions")
            .join(GAME_VERSION)
            .join(format!("{GAME_VERSION}.json"))
            .is_file()
            && dir
                .join("versions")
                .join(GAME_VERSION)
                .join(format!("{GAME_VERSION}.jar"))
                .is_file(),
        version_jar: jar,
    }
}

/// Candidate game folders: the chosen one, the platform default, the usual
/// alternatives.
pub fn candidate_dirs(settings: &Settings) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut push = |path: Option<PathBuf>| {
        if let Some(path) = path {
            if !out.iter().any(|p| p == &path) {
                out.push(path);
            }
        }
    };
    push(settings.minecraft_dir.clone());
    push(paths::default_minecraft_dir());
    push(dirs::home_dir().map(|h| h.join(".minecraft")));
    push(dirs::home_dir().map(|h| h.join("Documents/.minecraft")));
    #[cfg(target_os = "macos")]
    push(dirs::home_dir().map(|h| h.join("Library/Application Support/minecraft")));
    out
}

fn read_release(home: &Path) -> Option<(String, u32)> {
    let text = std::fs::read_to_string(home.join("release")).ok()?;
    let version = text
        .lines()
        .find_map(|line| line.strip_prefix("JAVA_VERSION="))?
        .trim()
        .trim_matches('"')
        .to_string();
    let mut parts = version.split('.');
    let first: u32 = parts.next()?.parse().ok()?;
    let major = if first == 1 {
        parts.next()?.parse().unwrap_or(1)
    } else {
        first
    };
    Some((version, major))
}

fn java_home_from_binary(binary: &Path) -> Option<PathBuf> {
    // <home>/bin/java[.exe]
    let home = binary.parent()?.parent()?.to_path_buf();
    if home.join("release").is_file() || home.join("bin").is_dir() {
        Some(home)
    } else {
        None
    }
}

fn java_candidates(settings: &Settings, minecraft: Option<&Path>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut push = |path: PathBuf| {
        if !out.contains(&path) {
            out.push(path);
        }
    };
    if let Some(home) = &settings.java_home {
        push(home.clone());
    }
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        if !home.is_empty() {
            push(PathBuf::from(home));
        }
    }
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let exe = dir.join(if cfg!(windows) { "java.exe" } else { "java" });
            if exe.is_file() {
                if let Some(home) = java_home_from_binary(&exe) {
                    push(home);
                }
            }
        }
    }
    if let Some(mc) = minecraft {
        let runtime = mc.join("runtime");
        if let Ok(entries) = std::fs::read_dir(&runtime) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let nested = std::fs::read_dir(&path)
                    .map(|e| e.flatten().map(|e| e.path()).collect::<Vec<_>>())
                    .unwrap_or_default();
                push(path.clone());
                for candidate in nested {
                    if candidate.is_dir() {
                        push(candidate);
                    }
                }
            }
        }
    }
    out
}

pub fn detect_java(settings: &Settings, minecraft: Option<&Path>) -> Option<JavaInfo> {
    let mut best: Option<JavaInfo> = None;
    for home in java_candidates(settings, minecraft) {
        let Some((version, major)) = read_release(&home) else {
            continue;
        };
        let info = JavaInfo {
            home,
            version,
            major,
        };
        if info.is_usable() {
            return Some(info);
        }
        if best.is_none() {
            best = Some(info);
        }
    }
    best
}

pub fn detect(settings: &Settings) -> Detection {
    let mut installs: Vec<MinecraftInstall> = Vec::new();
    for dir in candidate_dirs(settings) {
        if !dir.is_dir() {
            continue;
        }
        let install = inspect(&dir);
        if installs.iter().any(|i| i.dir == install.dir) {
            continue;
        }
        installs.push(install);
    }

    let chosen = installs
        .iter()
        .find(|i| i.has_game_version)
        .cloned()
        .or_else(|| {
            settings
                .minecraft_dir
                .as_ref()
                .filter(|dir| dir.is_dir())
                .map(|dir| inspect(dir))
        });

    let java = detect_java(settings, chosen.as_ref().map(|c| c.dir.as_path()));

    let server_jar = settings
        .server_jar
        .clone()
        .filter(|path| path.is_file());

    Detection {
        installs,
        chosen,
        java,
        server_jar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_majors_are_read_from_release_files() {
        let dir = std::env::temp_dir().join(format!("sunrise-java-{}", crate::now_secs()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("release"),
            "IMPLEMENTOR=\"Eclipse Adoptium\"\nJAVA_VERSION=\"25.0.4.1\"\n",
        )
        .unwrap();
        assert_eq!(read_release(&dir), Some(("25.0.4.1".to_string(), 25)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inspect_finds_a_26_3_install() {
        let dir = std::env::temp_dir().join(format!("sunrise-mc-{}", crate::now_secs()));
        let version = dir.join("versions").join(GAME_VERSION);
        std::fs::create_dir_all(&version).unwrap();
        std::fs::write(version.join("26.3.json"), "{}").unwrap();
        std::fs::write(version.join("26.3.jar"), "x").unwrap();
        let install = inspect(&dir);
        assert!(install.has_game_version);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
