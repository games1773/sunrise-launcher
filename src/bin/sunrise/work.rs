//! Starting the background work: the jobs each request spawns, and the state
//! it leaves behind while they run.
//!
//! Kept apart from the state itself so `app` reads without the network and
//! process calls it triggers; both files are `impl App`.

use std::path::PathBuf;

use sunrise::auth::Session;
use sunrise::config::valid_player_name;
use sunrise::{auth, builds, detect, launch, microsoft, mojang, prepare, Event, Job};

use crate::app::{App, Busy, InstallState, Level, Tab};

impl App {
    // ------------------------------------------------------------- background

    pub(crate) fn start_detect(&mut self) {
        self.pending_detect = true;
        let tx = self.tx.clone();
        let settings = self.config.settings.clone();
        std::thread::spawn(move || {
            let detection = detect::detect(&settings);
            let _ = tx.send(Event::DetectionDone {
                detection: Box::new(detection),
                error: None,
            });
        });
    }

    pub(crate) fn start_builds(&mut self, force: bool) {
        self.pending_builds = true;
        let tx = self.tx.clone();
        let data = self.data.clone();
        std::thread::spawn(move || {
            let _ = match builds::fetch_builds(&data, force) {
                Ok(builds) => tx.send(Event::BuildsFetched {
                    builds,
                    error: None,
                }),
                Err(err) => tx.send(Event::BuildsFetched {
                    builds: Vec::new(),
                    error: Some(err),
                }),
            };
        });
    }

    pub(crate) fn start_caps(&mut self, tag: String) {
        let Some(build) = self.installed.iter().find(|b| b.tag == tag).cloned() else {
            return;
        };
        self.pending_caps = true;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = match launch::probe_caps(&build.dir) {
                Ok(caps) => tx.send(Event::CapsProbed {
                    tag: build.tag,
                    caps: Some(caps),
                    error: None,
                }),
                Err(err) => tx.send(Event::CapsProbed {
                    tag: build.tag,
                    caps: None,
                    error: Some(err),
                }),
            };
        });
    }

    pub(crate) fn start_install(&mut self, tag: String) {
        let Some(remote) = self.builds.iter().find(|b| b.tag == tag).cloned() else {
            self.notice("That build is not in the list any more; refresh it.");
            return;
        };
        self.install = Some(InstallState {
            tag: tag.clone(),
            done: 0,
            total: remote.size,
            message: "Starting".to_string(),
        });
        let tx = self.tx.clone();
        let data = self.data.clone();
        std::thread::spawn(move || {
            let mut progress = |p: builds::Progress| {
                let _ = tx.send(Event::InstallProgress {
                    tag: tag.clone(),
                    done: p.done,
                    total: p.total,
                    message: p.message,
                });
            };
            let result = builds::install(&data, &remote, &mut progress);
            let _ = tx.send(Event::InstallFinished {
                tag: remote.tag.clone(),
                error: result.err(),
            });
        });
    }

    /// Preparing needs the 26.3 server jar once. Sunrise fetches it from Mojang
    /// and checks Horizon's pin, so there is nothing for the player to find.
    pub(crate) fn start_prepare(&mut self) {
        if self.jar_fetch.is_some() {
            self.notice("The server jar is still downloading.");
            return;
        }
        let Some(build) = self.current_build().cloned() else {
            self.builds_window = true;
            return;
        };
        if let Some(reason) = prepare::blocker(Some(&build.dir), self.detection.as_ref()) {
            self.notice(reason);
            return;
        }
        let minecraft = self
            .detection
            .as_ref()
            .and_then(|d| d.chosen.as_ref())
            .map(|c| c.dir.clone());

        if let Some(jar) = self.server_jar.clone().filter(|p| p.is_file()) {
            self.begin_prepare(build, minecraft, jar);
            return;
        }

        let pinned = builds::pinned_server_sha1(&build.dir).unwrap_or_default();
        if let Some(path) = mojang::cached(&self.data, &build.version, &pinned) {
            self.server_jar = Some(path.clone());
            self.begin_prepare(build, minecraft, path);
            return;
        }

        let data = self.data.clone();
        let version = build.version.clone();
        let tx = self.tx.clone();
        self.pending_prepare = true;
        self.jar_fetch = Some(mojang::Progress {
            done: 0,
            total: 0,
            message: format!("Asking Mojang for the {version} server jar"),
        });
        std::thread::spawn(move || {
            let mut progress = |p: mojang::Progress| {
                let _ = tx.send(Event::ServerJarProgress {
                    done: p.done,
                    total: p.total,
                    message: p.message,
                });
            };
            let event = match mojang::ensure(&data, &version, &pinned, &mut progress) {
                Ok(path) => Event::ServerJarReady {
                    path: Some(path),
                    error: None,
                },
                Err(err) => Event::ServerJarReady {
                    path: None,
                    error: Some(err),
                },
            };
            let _ = tx.send(event);
        });
    }

    pub(crate) fn begin_prepare(
        &mut self,
        build: builds::InstalledBuild,
        minecraft: Option<PathBuf>,
        server_jar: PathBuf,
    ) {
        let cmd = match prepare::prepare_command(
            &build.dir,
            minecraft.as_deref(),
            Some(&server_jar),
            self.config.settings.java_home.as_deref(),
        ) {
            Ok(cmd) => cmd,
            Err(err) => {
                self.notice(err);
                return;
            }
        };
        self.busy = Some(Busy::Prepare);
        self.sounds_after_prepare = self.config.settings.copy_sounds;
        self.line(format!("Preparing {}", build.tag), Level::Info);
        if let Err(err) = sunrise::spawn_streamed(cmd, Job::Prepare, self.tx.clone(), None) {
            self.busy = None;
            self.line(err.clone(), Level::Error);
            self.notice(err);
        }
    }

    pub(crate) fn start_sounds(&mut self) {
        let Some(build) = self.current_build().cloned() else {
            return;
        };
        let minecraft = self
            .detection
            .as_ref()
            .and_then(|d| d.chosen.as_ref())
            .map(|c| c.dir.clone());
        let Ok(cmd) = prepare::sounds_command(
            &build.dir,
            minecraft.as_deref(),
            self.config.settings.java_home.as_deref(),
        ) else {
            return;
        };
        self.busy = Some(Busy::Sounds);
        let _ = sunrise::spawn_streamed(cmd, Job::Sounds, self.tx.clone(), None);
    }

    pub(crate) fn start_play(&mut self) {
        let Some(build) = self.current_build().cloned() else {
            self.builds_window = true;
            return;
        };
        let Some((tag, caps)) = self.caps.clone() else {
            self.start_caps(build.tag.clone());
            self.notice("Asking the build what it takes...");
            return;
        };
        if tag != build.tag {
            self.start_caps(build.tag.clone());
            self.notice("Asking the build what it takes...");
            return;
        }
        if !prepare::is_prepared(&build.dir) {
            self.start_prepare();
            return;
        }
        let session = self.session().cloned();
        // A session that can renew itself does so before the game starts, so a
        // day-old sign-in never reaches the server as an expired token.
        if let Some(session) = session.as_ref().filter(|s| s.needs_renewal()) {
            self.pending_play = true;
            self.start_renewal(session.clone());
            return;
        }
        if self.online() && session.is_none() {
            self.notice("Sign in, or switch the account back to Offline.");
            self.tab = Tab::Setup;
            return;
        }
        if session.is_none() && !valid_player_name(&self.config.settings.player_name) {
            self.notice("Pick a name: 3-16 letters, digits or _");
            self.tab = Tab::Setup;
            return;
        }
        if let Some(session) = &session {
            if session.looks_stale() {
                self.notice("That session is over a day old; online-mode servers may refuse it.");
            }
        }

        let instance = self.config.selected().clone();
        let spec = launch::GameSpec {
            build_dir: build.dir.clone(),
            game_dir: instance.game_dir.clone(),
            home: instance.game_dir.clone(),
            width: instance.width,
            height: instance.height,
            fullscreen: instance.fullscreen,
            render_distance: instance.render_distance,
            world: instance.world.clone(),
            seed: instance.seed.clone(),
            creative: instance.creative,
            extra_args: instance.extra_args.clone(),
            name: if session.is_some() {
                None
            } else {
                Some(self.config.settings.player_name.trim().to_string())
            },
            session,
            server: instance
                .connect
                .clone()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        };

        self.busy = Some(Busy::Game);
        match launch::spawn(&spec, &caps, self.tx.clone()) {
            Ok(_) => {
                if let Some(instance) = self
                    .config
                    .instances
                    .iter_mut()
                    .find(|i| i.id == instance.id)
                {
                    instance.last_played = Some(sunrise::now_secs());
                    self.dirty = true;
                }
            }
            Err(err) => {
                self.busy = None;
                self.line(err.clone(), Level::Error);
                self.notice(err);
            }
        }
    }

    pub(crate) fn pick_minecraft(&mut self) {
        let mut dialog =
            rfd::FileDialog::new().set_title("Your official launcher's .minecraft folder");
        if let Some(current) = &self.config.settings.minecraft_dir {
            dialog = dialog.set_directory(current);
        }
        if let Some(dir) = dialog.pick_folder() {
            self.config.settings.minecraft_dir = Some(dir);
            self.dirty = true;
            self.start_detect();
        }
    }

    // ------------------------------------------------------------- account

    /// Read a session from a file the player picked, then let Mojang say whether
    /// it still works before Sunrise keeps it.
    pub(crate) fn pick_session(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("A Minecraft session JSON")
            .add_filter("Session", &["json"]);
        if let Some(dir) = dirs::download_dir() {
            dialog = dialog.set_directory(dir);
        }
        if let Some(file) = dialog.pick_file() {
            let text = match std::fs::read_to_string(&file) {
                Ok(text) => text,
                Err(err) => {
                    self.notice(format!("Could not read {}: {err}", file.display()));
                    return;
                }
            };
            self.import_session_text(&text);
        }
    }

    pub(crate) fn import_session_text(&mut self, text: &str) {
        match auth::parse_session(text) {
            Ok(session) => self.start_session_check(session),
            Err(err) => {
                self.line(err.clone(), Level::Error);
                self.notice(err);
            }
        }
    }

    /// Sign in with Microsoft: ask for a code, show it, wait. The browser is
    /// opened from the window, by the player, so nothing takes the code away
    /// before it has been read.
    pub(crate) fn start_sign_in(&mut self) {
        if self.sign_in.is_some() {
            self.notice("A sign-in is already waiting for the browser.");
            return;
        }
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.sign_in = Some(crate::app::SignIn {
            user_code: String::new(),
            url: String::new(),
            cancel: cancel.clone(),
        });
        self.line("Asking Microsoft for a sign-in code...", Level::Info);

        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let code = match microsoft::begin() {
                Ok(code) => code,
                Err(err) => {
                    let _ = tx.send(Event::SignInCode {
                        code: None,
                        error: Some(err),
                    });
                    return;
                }
            };
            let _ = tx.send(Event::SignInCode {
                code: Some(Box::new(code.clone())),
                error: None,
            });

            let stop = cancel.clone();
            let waited = microsoft::wait(&code, move || {
                stop.load(std::sync::atomic::Ordering::Relaxed)
            });
            let event = match waited {
                Ok(Some(tokens)) => {
                    match microsoft::session_from_microsoft(
                        &tokens.access_token,
                        tokens.refresh_token,
                    ) {
                        Ok(checked) => Event::SessionChecked {
                            session: Some(Box::new(checked.session)),
                            warning: checked.warning,
                            error: None,
                        },
                        Err(err) => Event::SessionChecked {
                            session: None,
                            warning: None,
                            error: Some(err),
                        },
                    }
                }
                Ok(None) => Event::SignInEnded { error: None },
                Err(err) => Event::SignInEnded { error: Some(err) },
            };
            let _ = tx.send(event);
        });
    }

    /// Renew a session from its refresh token, keeping the player's hand out of it.
    pub(crate) fn start_renewal(&mut self, session: Session) {
        if self.session_check {
            return;
        }
        self.session_check = true;
        let tx = self.tx.clone();
        self.line("Renewing the Microsoft sign-in...", Level::Info);
        std::thread::spawn(move || {
            let event = match microsoft::refresh(&session) {
                Ok(checked) => Event::SessionChecked {
                    session: Some(Box::new(checked.session)),
                    warning: checked.warning,
                    error: None,
                },
                Err(err) => Event::SessionChecked {
                    session: None,
                    warning: None,
                    error: Some(err),
                },
            };
            let _ = tx.send(event);
        });
    }

    pub(crate) fn start_session_check(&mut self, session: Session) {
        self.session_check = true;
        let tx = self.tx.clone();
        self.line("Checking the session with Mojang...", Level::Info);
        std::thread::spawn(move || {
            let event = match auth::check(&session) {
                Ok(checked) => Event::SessionChecked {
                    session: Some(Box::new(checked.session)),
                    warning: checked.warning,
                    error: None,
                },
                Err(err) => Event::SessionChecked {
                    session: None,
                    warning: None,
                    error: Some(err),
                },
            };
            let _ = tx.send(event);
        });
    }

    pub(crate) fn pick_java(&mut self) {
        if let Some(dir) = rfd::FileDialog::new()
            .set_title("A Java 25 home folder")
            .pick_folder()
        {
            self.config.settings.java_home = Some(dir);
            self.dirty = true;
            self.start_detect();
        }
    }
}
