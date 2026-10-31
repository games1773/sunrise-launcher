//! Sunrise Launcher: a launcher for Horizon, the Rust write of Minecraft: Java Edition 26.3.
//!
//! Horizon ships builds on GitHub and documents a small Launch API for launchers
//! (`docs/api/launch.md` upstream). This crate wraps the parts Sunrise needs:
//! finding and installing builds, preparing the game files from the player's own
//! Minecraft install, and starting `horizon-client` with its output captured.

pub mod builds;
pub mod config;
pub mod detect;
pub mod launch;
pub mod paths;
pub mod prepare;

use std::sync::mpsc::Sender;
use std::time::{SystemTime, UNIX_EPOCH};

pub use builds::{InstalledBuild, RemoteBuild};
pub use config::{Config, Instance, Settings};
pub use detect::Detection;

pub type Result<T> = std::result::Result<T, String>;

/// A long-running child process Sunrise follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Job {
    Prepare,
    Sounds,
    Game,
}

impl Job {
    pub fn label(self) -> &'static str {
        match self {
            Job::Prepare => "prepare",
            Job::Sounds => "sounds",
            Job::Game => "game",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Job::Prepare => "Preparing game files",
            Job::Sounds => "Copying sounds",
            Job::Game => "Playing",
        }
    }
}

/// Everything a background worker reports back to the UI.
#[derive(Clone, Debug)]
pub enum Event {
    Log {
        job: Job,
        line: String,
        err: bool,
    },
    Finished {
        job: Job,
        code: Option<i32>,
        seconds: f32,
    },
    InstallProgress {
        tag: String,
        done: u64,
        total: u64,
        message: String,
    },
    InstallFinished {
        tag: String,
        error: Option<String>,
    },
    BuildsFetched {
        builds: Vec<RemoteBuild>,
        error: Option<String>,
    },
    DetectionDone {
        detection: Box<Detection>,
        error: Option<String>,
    },
    GameSpawned {
        pid: u32,
        command: String,
    },
    /// A build answered `--launch-api` and `--help`.
    CapsProbed {
        tag: String,
        caps: Option<crate::launch::Caps>,
        error: Option<String>,
    },
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Spawn a process with both output streams forwarded line by line, then report
/// its exit code. Used for the preparer and for the game itself.
pub fn spawn_streamed(
    mut cmd: std::process::Command,
    job: Job,
    tx: Sender<Event>,
) -> Result<u32> {
    use std::process::Stdio;

    let program = cmd.get_program().to_string_lossy().to_string();
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // The preparer is a console program; without this a console window flashes
    // open next to the launcher.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let started = std::time::Instant::now();
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("could not start {program}: {e}"))?;

    let pid = child.id();

    fn forward<R: std::io::Read + Send + 'static>(
        pipe: R,
        job: Job,
        is_err: bool,
        tx: Sender<Event>,
    ) {
        std::thread::spawn(move || {
            use std::io::BufRead;
            let reader = std::io::BufReader::new(pipe);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                if tx
                    .send(Event::Log {
                        job,
                        line,
                        err: is_err,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    if let Some(out) = child.stdout.take() {
        forward(out, job, false, tx.clone());
    }
    if let Some(err) = child.stderr.take() {
        forward(err, job, true, tx.clone());
    }

    std::thread::spawn(move || {
        let status = child.wait();
        let code = status.ok().and_then(|s| s.code());
        let _ = tx.send(Event::Finished {
            job,
            code,
            seconds: started.elapsed().as_secs_f32(),
        });
    });

    Ok(pid)
}

/// Split a command line the way the game's extra arguments expect: quotes group
/// a word with spaces, everything else splits on whitespace.
pub fn split_args(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut has_word = false;
    for ch in line.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => current.push(ch),
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
                has_word = true;
            }
            None if ch.is_whitespace() => {
                if has_word || !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                    has_word = false;
                }
            }
            None => {
                current.push(ch);
                has_word = true;
            }
        }
    }
    if has_word || !current.is_empty() {
        out.push(current);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_quoted_args() {
        assert_eq!(
            split_args("--server-thread -Dfoo=\"a b\" 'c d'"),
            vec!["--server-thread", "-Dfoo=a b", "c d"]
        );
        assert!(split_args("   ").is_empty());
    }
}
