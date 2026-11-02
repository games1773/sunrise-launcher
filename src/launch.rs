//! Starting `horizon-client` the way Horizon's Launch API describes.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::{builds, split_args, Result};

/// What a build says it accepts, read from the build itself.
#[derive(Clone, Debug, Default)]
pub struct Caps {
    pub launch_api: String,
    pub horizon_label: String,
    pub minecraft: String,
    pub help: String,
}

impl Caps {
    /// A flag is passed only when this build's own help lists it, so a build
    /// that drops an option is not handed one it would refuse.
    pub fn supports(&self, flag: &str) -> bool {
        self.help.contains(flag)
    }
}

#[derive(Deserialize)]
struct LaunchApiJson {
    #[serde(default)]
    launch_api: String,
    #[serde(default)]
    horizon: String,
    #[serde(default)]
    minecraft: String,
}

fn hidden(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

/// Ask the build what it is and which options it takes.
pub fn probe_caps(build_dir: &Path) -> Result<Caps> {
    let client = builds::client_exe(build_dir);
    if !client.is_file() {
        return Err(format!("no horizon-client in {}", build_dir.display()));
    }

    let mut api_cmd = Command::new(&client);
    api_cmd.arg("--launch-api");
    hidden(&mut api_cmd);
    let api = api_cmd
        .output()
        .map_err(|e| format!("could not run {}: {e}", client.display()))?;
    let parsed: LaunchApiJson = serde_json::from_slice(&api.stdout)
        .map_err(|e| format!("{} did not answer --launch-api: {e}", client.display()))?;

    let mut help_cmd = Command::new(&client);
    help_cmd.arg("--help");
    hidden(&mut help_cmd);
    let help = help_cmd
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).to_string())
        .unwrap_or_default();

    Ok(Caps {
        launch_api: parsed.launch_api,
        horizon_label: parsed.horizon,
        minecraft: if parsed.minecraft.is_empty() {
            builds::game_version(build_dir)
        } else {
            parsed.minecraft
        },
        help,
    })
}

#[derive(Clone, Debug, Default)]
pub struct GameSpec {
    pub build_dir: PathBuf,
    pub game_dir: PathBuf,
    pub home: PathBuf,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub render_distance: u32,
    pub world: Option<String>,
    pub seed: Option<String>,
    pub creative: bool,
    pub extra_args: String,
    /// The offline player name handed over as `--name`.
    pub name: Option<String>,
    /// A Microsoft session. When present the game signs in with it, so no
    /// offline name is passed and online-mode servers accept the player.
    pub session: Option<crate::auth::Session>,
    /// `host[:port]` joined at start, for offline-mode servers.
    pub server: Option<String>,
}

impl GameSpec {
    pub fn resources_dir(&self) -> PathBuf {
        crate::prepare::resources_dir(&self.build_dir)
    }
}

/// The arguments handed to the client, in the order the Launch API lists them.
pub fn build_args(spec: &GameSpec, caps: &Caps) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "--resources".into(),
        spec.resources_dir().display().to_string(),
        "--game-dir".into(),
        spec.game_dir.display().to_string(),
        "--home".into(),
        spec.home.display().to_string(),
    ];

    if spec.width > 0 && spec.height > 0 {
        args.push("--width".into());
        args.push(spec.width.to_string());
        args.push("--height".into());
        args.push(spec.height.to_string());
    }
    if spec.fullscreen {
        args.push("--fullscreen".into());
    }

    let server = spec
        .server
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(server) = server {
        if caps.supports("--quickPlayMultiplayer") {
            args.push("--quickPlayMultiplayer".into());
            args.push(server.to_string());
        } else if caps.supports("--server") {
            args.push("--server".into());
            args.push(server.to_string());
        }
    } else if let Some(world) = spec
        .world
        .as_deref()
        .map(str::trim)
        .filter(|w| !w.is_empty())
    {
        let existing = spec.game_dir.join("saves").join(world).is_dir();
        if existing {
            // Vanilla's quick play: straight into an existing world.
            args.push("--quickPlaySingleplayer".into());
            args.push(world.to_string());
        } else {
            args.push("--world".into());
            args.push(world.to_string());
            if let Some(seed) = spec
                .seed
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                args.push("--seed".into());
                args.push(seed.to_string());
            }
        }
    }

    // A session carries the account's own name, so the offline name would only
    // fight it.
    if spec.session.is_none() {
        if let Some(name) = spec
            .name
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
        {
            if caps.supports("--name") {
                args.push("--name".into());
                args.push(name.to_string());
            }
        }
    }
    if spec.session.is_some() && caps.supports("--session-stdin") {
        args.push("--session-stdin".into());
    }

    if spec.render_distance > 0 && spec.render_distance != 12 && caps.supports("--render-distance")
    {
        args.push("--render-distance".into());
        args.push(spec.render_distance.to_string());
    }
    if spec.creative && caps.supports("--creative") {
        args.push("--creative".into());
    }

    for extra in split_args(&spec.extra_args) {
        if !args.contains(&extra) {
            args.push(extra);
        }
    }

    args
}

/// Start the game with its output captured. Without a session it plays offline
/// with the name `--name` gives it; with one, the session goes over stdin the way
/// the Launch API asks, and is never put on the command line.
pub fn spawn(
    spec: &GameSpec,
    caps: &Caps,
    tx: std::sync::mpsc::Sender<crate::Event>,
) -> Result<u32> {
    let client = builds::client_exe(&spec.build_dir);
    if !client.is_file() {
        return Err(format!("no horizon-client in {}", spec.build_dir.display()));
    }
    if !crate::prepare::is_prepared(&spec.build_dir) {
        return Err("the game files are not prepared yet".into());
    }

    let args = build_args(spec, caps);

    crate::paths::ensure_dir(&spec.game_dir)?;

    let mut cmd = Command::new(&client);
    cmd.current_dir(&spec.game_dir).args(&args);
    #[cfg(windows)]
    {
        // horizon-client is a windowed program; no console, no flashing window.
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let command_line = format!(
        "{} {}",
        client.display(),
        args.iter()
            .map(|a| if a.contains(' ') {
                format!("\"{a}\"")
            } else {
                a.clone()
            })
            .collect::<Vec<_>>()
            .join(" ")
    );

    let payload = if args.iter().any(|a| a == "--session-stdin") {
        spec.session.as_ref().map(|s| s.handover_json())
    } else {
        None
    };
    let pid = crate::spawn_streamed(cmd, crate::Job::Game, tx.clone(), payload)?;
    let _ = tx.send(crate::Event::GameSpawned {
        pid,
        command: command_line,
    });
    Ok(pid)
}

/// What an exit code means, in Horizon's Launch API: 0 done, 1 failed or a
/// crash, 2 a refused command line, 101 a panic.
pub fn exit_hint(code: Option<i32>) -> String {
    match code {
        Some(0) => "The game closed normally.".to_string(),
        Some(2) => "The game refused a command-line option; Sunrise should not have passed it. Please report it.".to_string(),
        Some(101) => "The game crashed (panic). The last line above names its crash report.".to_string(),
        Some(code) => format!("The game exited with code {code}. On 1 or 101 the last line above names the crash report."),
        None => "The game was stopped by a signal.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(help: &str) -> Caps {
        Caps {
            launch_api: "0.1.0".into(),
            horizon_label: "Nightly".into(),
            minecraft: "26.3".into(),
            help: help.to_string(),
        }
    }

    fn spec() -> GameSpec {
        GameSpec {
            build_dir: PathBuf::from("/builds/nightly"),
            game_dir: PathBuf::from("/instances/main"),
            home: PathBuf::from("/instances/main"),
            width: 1280,
            height: 720,
            fullscreen: false,
            render_distance: 16,
            world: None,
            seed: None,
            creative: false,
            extra_args: String::new(),
            name: None,
            session: None,
            server: None,
        }
    }

    #[test]
    fn core_arguments_follow_the_launch_api() {
        let args = build_args(&spec(), &caps(""));
        assert_eq!(args[0], "--resources");
        assert_eq!(
            PathBuf::from(&args[1]),
            PathBuf::from("/builds/nightly").join(".local/horizon-client/26.3")
        );
        assert!(args.contains(&"--game-dir".to_string()));
        assert!(args.contains(&"--home".to_string()));
        assert!(args.contains(&"--width".to_string()));
    }

    #[test]
    fn optional_flags_need_this_build_to_list_them() {
        let without = build_args(&spec(), &caps("--resources PATH"));
        assert!(!without.contains(&"--render-distance".to_string()));

        let with = build_args(
            &spec(),
            &caps("--resources PATH\n--render-distance N\n--creative"),
        );
        assert!(with.contains(&"--render-distance".to_string()));
        assert!(with.contains(&"16".to_string()));
    }

    #[test]
    fn extra_args_are_split_and_appended() {
        let mut spec = spec();
        spec.extra_args = "--pregen --foo=\"a b\"".into();
        let args = build_args(&spec, &caps(""));
        assert_eq!(args[args.len() - 2], "--pregen");
        assert_eq!(args[args.len() - 1], "--foo=a b");
    }

    #[test]
    fn offline_name_and_server_are_passed() {
        let mut spec = spec();
        spec.name = Some("Alex".into());
        spec.server = Some("192.168.1.20:25565".into());
        let args = build_args(
            &spec,
            &caps("--name NAME\n--quickPlayMultiplayer HOST[:PORT]"),
        );
        let name_at = args.iter().position(|a| a == "--name").unwrap();
        assert_eq!(args[name_at + 1], "Alex");
        let server_at = args
            .iter()
            .position(|a| a == "--quickPlayMultiplayer")
            .unwrap();
        assert_eq!(args[server_at + 1], "192.168.1.20:25565");
    }

    #[test]
    fn offline_name_is_skipped_when_the_build_does_not_take_it() {
        let mut spec = spec();
        spec.name = Some("Alex".into());
        let args = build_args(&spec, &caps("--resources PATH"));
        assert!(!args.contains(&"--name".to_string()));
    }

    #[test]
    fn a_session_replaces_the_offline_name() {
        let mut spec = spec();
        spec.name = Some("Player".into());
        spec.session = Some(crate::auth::Session {
            name: "Alex".into(),
            uuid: "069a79f444e94726a5befca90e38aaf5".into(),
            access_token: "tok".into(),
            user_type: "msa".into(),
            xuid: None,
            checked_at: 1,
            ..Default::default()
        });
        let args = build_args(
            &spec,
            &caps("--name NAME\n--session-stdin\n--quickPlayMultiplayer HOST[:PORT]"),
        );
        assert!(args.contains(&"--session-stdin".to_string()));
        assert!(!args.contains(&"--name".to_string()));
        // The token must never travel on the command line.
        assert!(!args.iter().any(|a| a == "tok"));
    }

    #[test]
    fn a_build_without_session_stdin_is_not_asked_for_one() {
        let mut spec = spec();
        spec.session = Some(crate::auth::Session::default());
        let args = build_args(&spec, &caps("--resources PATH\n--name NAME"));
        assert!(!args.contains(&"--session-stdin".to_string()));
    }

    #[test]
    fn exit_codes_are_explained() {
        assert!(exit_hint(Some(2)).contains("refused"));
        assert!(exit_hint(Some(101)).contains("crash"));
    }
}
