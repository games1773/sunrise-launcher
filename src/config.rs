//! Settings and instances, stored as one JSON file in the data folder.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{paths, Result};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The official launcher's game folder; `None` means "find it".
    pub minecraft_dir: Option<PathBuf>,
    /// The 26.3 server jar, needed once by the preparer.
    pub server_jar: Option<PathBuf>,
    /// Java 25 home to prepare with; `None` means "whatever the preparer finds".
    pub java_home: Option<PathBuf>,
    /// Copy the ~380 MB of sounds while preparing (the game is silent without them).
    pub copy_sounds: bool,
    /// The offline name the game plays as.
    pub player_name: String,
    /// The build tag picked last, reused for new instances.
    pub last_build: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            minecraft_dir: None,
            server_jar: None,
            java_home: None,
            copy_sounds: true,
            player_name: default_player_name(),
            last_build: None,
        }
    }
}

fn default_player_name() -> String {
    "Player".to_string()
}

/// Vanilla's rule for a player name: 3-16 letters, digits or `_`.
pub fn valid_player_name(name: &str) -> bool {
    let name = name.trim();
    let length = name.chars().count();
    (3..=16).contains(&length)
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub game_dir: PathBuf,
    /// `None` follows whichever build is installed newest.
    #[serde(default)]
    pub build_tag: Option<String>,
    /// The world opened at start, if any.
    #[serde(default)]
    pub world: Option<String>,
    #[serde(default)]
    pub seed: Option<String>,
    #[serde(default = "default_render_distance")]
    pub render_distance: u32,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub creative: bool,
    #[serde(default)]
    pub extra_args: String,
    /// `host[:port]` joined at start; an offline-mode server takes the offline name.
    #[serde(default)]
    pub connect: Option<String>,
    #[serde(default)]
    pub last_played: Option<u64>,
    #[serde(default)]
    pub play_seconds: u64,
}

fn default_render_distance() -> u32 {
    12
}

fn default_width() -> u32 {
    1280
}

fn default_height() -> u32 {
    720
}

impl Instance {
    pub fn new(name: &str, game_dir: PathBuf) -> Self {
        Self {
            id: paths::slug(name),
            name: name.trim().to_string(),
            game_dir,
            build_tag: None,
            world: None,
            seed: None,
            render_distance: 12,
            width: default_width(),
            height: default_height(),
            fullscreen: false,
            creative: false,
            extra_args: String::new(),
            connect: None,
            last_played: None,
            play_seconds: 0,
        }
    }

    pub fn worlds_dir(&self) -> PathBuf {
        self.game_dir.join("saves")
    }

    /// Folders in `saves/` that look like worlds.
    pub fn worlds(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir(self.worlds_dir()) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join("level.dat").is_file() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        out.push(name.to_string());
                    }
                }
            }
        }
        out.sort();
        out
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub settings: Settings,
    pub instances: Vec<Instance>,
    pub selected_instance: Option<String>,
}

impl Config {
    pub fn load(data: &Path) -> Self {
        let file = paths::config_file(data);
        let Ok(text) = std::fs::read_to_string(&file) else {
            let mut config = Config::default();
            config.ensure_instance(data);
            return config;
        };
        match serde_json::from_str::<Config>(&text) {
            Ok(mut config) => {
                config.ensure_instance(data);
                config
            }
            Err(err) => {
                // Keep the broken file instead of overwriting the player's setup.
                let backup = file.with_extension(format!("json.broken-{}", crate::now_secs()));
                let _ = std::fs::rename(&file, &backup);
                eprintln!("sunrise: sunrise.json could not be read ({err}); kept as {}", backup.display());
                let mut config = Config::default();
                config.ensure_instance(data);
                config
            }
        }
    }

    pub fn save(&self, data: &Path) -> Result<()> {
        paths::ensure_dir(data)?;
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let file = paths::config_file(data);
        std::fs::write(&file, text).map_err(|e| format!("could not save {}: {e}", file.display()))?;
        // The file can hold a session token: keep it to this user on Unix. On
        // Windows the data folder already lives under the user's %APPDATA%.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    fn ensure_instance(&mut self, data: &Path) {
        if self.instances.is_empty() {
            let dir = paths::instances_dir(data).join("main");
            let mut instance = Instance::new("Main", dir);
            instance.id = "main".to_string();
            self.instances.push(instance);
        }
        if self
            .selected_instance
            .as_ref()
            .map(|id| !self.instances.iter().any(|i| &i.id == id))
            .unwrap_or(true)
        {
            self.selected_instance = Some(self.instances[0].id.clone());
        }
    }

    pub fn selected(&self) -> &Instance {
        let id = self.selected_instance.clone().unwrap_or_default();
        self.instances
            .iter()
            .find(|i| i.id == id)
            .unwrap_or(&self.instances[0])
    }

    pub fn selected_mut(&mut self) -> &mut Instance {
        let id = self.selected_instance.clone().unwrap_or_default();
        if self.instances.iter().any(|i| i.id == id) {
            self.instances.iter_mut().find(|i| i.id == id).unwrap()
        } else {
            &mut self.instances[0]
        }
    }

    pub fn add_instance(&mut self, data: &Path, name: &str) -> String {
        let taken: Vec<String> = self.instances.iter().map(|i| i.id.clone()).collect();
        let id = paths::unique_slug(&paths::slug(name), &taken);
        let mut instance = Instance::new(name, paths::instances_dir(data).join(&id));
        instance.id = id.clone();
        instance.build_tag = None;
        self.instances.push(instance);
        self.selected_instance = Some(id.clone());
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trips() {
        let dir = std::env::temp_dir().join(format!("sunrise-config-{}", crate::now_secs()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut config = Config::default();
        config.ensure_instance(&dir);
        config.add_instance(&dir, "Second Player");
        config.save(&dir).unwrap();
        let loaded = Config::load(&dir);
        assert_eq!(loaded.instances.len(), 2);
        assert_eq!(loaded.instances[1].name, "Second Player");
        assert_eq!(loaded.selected().id, "second-player");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn player_names_follow_vanillas_rule() {
        assert!(valid_player_name("Alex"));
        assert!(valid_player_name("Player_1"));
        assert!(!valid_player_name("ab"));
        assert!(!valid_player_name("this-name-is-too-long"));
        assert!(!valid_player_name("space name"));
        assert!(!valid_player_name(""));
    }

    #[test]
    fn offline_name_and_server_persist() {
        let dir = std::env::temp_dir().join(format!("sunrise-offline-{}", crate::now_secs()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut config = Config::default();
        config.ensure_instance(&dir);
        config.settings.player_name = "Steve".into();
        config.selected_mut().connect = Some("192.168.1.20:25565".into());
        config.save(&dir).unwrap();
        let loaded = Config::load(&dir);
        assert_eq!(loaded.settings.player_name, "Steve");
        assert_eq!(
            loaded.selected().connect.as_deref(),
            Some("192.168.1.20:25565")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
