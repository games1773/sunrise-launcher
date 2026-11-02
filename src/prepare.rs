//! Preparing Horizon's game files from the player's own Minecraft install.
//!
//! Resources live beside the build, in `<build>/.local/horizon-client/<version>/`,
//! and are ready when that folder holds `horizon-resources.json`. The shipped
//! preparer has no `--check` flag, so Sunrise looks for that marker itself.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::detect::Detection;
use crate::{builds, Result};

/// Where a build keeps its prepared resources.
pub fn resources_dir(build_dir: &Path) -> PathBuf {
    build_dir
        .join(".local")
        .join("horizon-client")
        .join(builds::game_version(build_dir))
}

pub fn marker(build_dir: &Path) -> PathBuf {
    resources_dir(build_dir).join("horizon-resources.json")
}

pub fn is_prepared(build_dir: &Path) -> bool {
    marker(build_dir).is_file()
}

/// What still has to happen before the game can start.
#[derive(Clone, Debug, Default)]
pub struct SetupStatus {
    pub build_installed: bool,
    pub minecraft_found: bool,
    pub java_found: bool,
    pub prepared: bool,
}

impl SetupStatus {
    pub fn ready(&self) -> bool {
        self.build_installed && self.minecraft_found && self.java_found && self.prepared
    }

    pub fn score(&self) -> (usize, usize) {
        let done = [
            self.build_installed,
            self.minecraft_found,
            self.java_found,
            self.prepared,
        ]
        .iter()
        .filter(|v| **v)
        .count();
        (done, 4)
    }
}

pub fn status(build_dir: Option<&Path>, detection: Option<&Detection>) -> SetupStatus {
    SetupStatus {
        build_installed: build_dir
            .map(|dir| builds::client_exe(dir).is_file())
            .unwrap_or(false),
        minecraft_found: detection.map(|d| d.has_game_version()).unwrap_or(false),
        java_found: detection.map(|d| d.java_ready()).unwrap_or(false),
        prepared: build_dir.map(is_prepared).unwrap_or(false),
    }
}

fn base_command(prepare: &Path, build_dir: &Path, java_home: Option<&Path>) -> Command {
    let mut cmd = Command::new(prepare);
    cmd.current_dir(build_dir);
    if let Some(home) = java_home {
        cmd.env("JAVA_HOME", home);
    }
    cmd
}

/// `horizon-prepare --prepare --root <build> [--minecraft-dir <mc>] [--server-jar <jar>]`
pub fn prepare_command(
    build_dir: &Path,
    minecraft_dir: Option<&Path>,
    server_jar: Option<&Path>,
    java_home: Option<&Path>,
) -> Result<Command> {
    let prepare = builds::prepare_exe(build_dir);
    if !prepare.is_file() {
        return Err(format!(
            "this build has no preparer at {}; install it again",
            prepare.display()
        ));
    }
    let mut cmd = base_command(&prepare, build_dir, java_home);
    cmd.arg("--prepare").arg("--root").arg(build_dir);
    if let Some(dir) = minecraft_dir {
        cmd.arg("--minecraft-dir").arg(dir);
    }
    if let Some(jar) = server_jar {
        cmd.arg("--server-jar").arg(jar);
    }
    Ok(cmd)
}

/// `horizon-prepare sounds --root <build> [--minecraft-dir <mc>]`
pub fn sounds_command(
    build_dir: &Path,
    minecraft_dir: Option<&Path>,
    java_home: Option<&Path>,
) -> Result<Command> {
    let prepare = builds::prepare_exe(build_dir);
    if !prepare.is_file() {
        return Err(format!(
            "this build has no preparer at {}; install it again",
            prepare.display()
        ));
    }
    let mut cmd = base_command(&prepare, build_dir, java_home);
    // This build's parser only takes the subcommand as the first argument; before
    // --root it answers "unrecognized argument: sounds" and exits 2.
    cmd.arg("sounds").arg("--root").arg(build_dir);
    if let Some(dir) = minecraft_dir {
        cmd.arg("--minecraft-dir").arg(dir);
    }
    Ok(cmd)
}

/// Human-readable reason the preparer cannot run yet, if any.
pub fn blocker(build_dir: Option<&Path>, detection: Option<&Detection>) -> Option<String> {
    let Some(build_dir) = build_dir else {
        return Some("Install a Horizon build first.".to_string());
    };
    if !builds::client_exe(build_dir).is_file() {
        return Some("Install a Horizon build first.".to_string());
    }
    let (Some(detection), true) = (detection, true) else {
        return Some("Looking for your Minecraft install...".to_string());
    };
    if !detection.has_game_version() {
        return Some(format!(
            "Sunrise needs your own copy of Minecraft: Java Edition {} - play it once in the official launcher, or point Sunrise at that folder.",
            crate::detect::GAME_VERSION
        ));
    }
    if !detection.java_ready() {
        return Some(format!(
            "Preparing needs Java {} - install it, or point Sunrise at a Java {} home.",
            crate::detect::REQUIRED_JAVA_MAJOR,
            crate::detect::REQUIRED_JAVA_MAJOR
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sounds_subcommand_comes_first() {
        // The shipped parser treats its first argument as the subcommand, so
        // `--root X sounds` is refused with exit code 2.
        let dir = std::env::temp_dir().join(format!("sunrise-sounds-{}", crate::now_secs()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(builds::prepare_exe(&dir), b"").unwrap();

        let sounds = sounds_command(&dir, None, None).unwrap();
        let args: Vec<String> = sounds
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(args.first().map(String::as_str), Some("sounds"));
        assert!(args
            .windows(2)
            .any(|w| w[0] == "--root" && w[1] == dir.display().to_string()));

        let prepare = prepare_command(&dir, None, None, None).unwrap();
        let args: Vec<String> = prepare
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert!(args.contains(&"--prepare".to_string()));
        assert!(!args.contains(&"sounds".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resources_are_ready_only_with_the_marker() {
        let dir = std::env::temp_dir().join(format!("sunrise-prep-{}", crate::now_secs()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("research-lock.json"), r#"{"version":"26.3"}"#).unwrap();
        assert!(!is_prepared(&dir));
        let res = resources_dir(&dir);
        std::fs::create_dir_all(&res).unwrap();
        std::fs::write(res.join("horizon-resources.json"), "{}").unwrap();
        assert!(is_prepared(&dir));
        assert_eq!(resources_dir(&dir).file_name().unwrap(), "26.3");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
