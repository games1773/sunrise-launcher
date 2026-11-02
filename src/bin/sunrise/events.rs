//! Applying what the background work reports to the window's state.

use std::time::Duration;

use eframe::egui;
use sunrise::config::AccountMode;
use sunrise::{launch, mojang, Event, Job};

use crate::app::{App, Level};

impl App {
    pub(crate) fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Log { line, .. } => {
                    let level = classify(&line);
                    self.line(line, level);
                }
                Event::Finished { job, code, seconds } => match job {
                    Job::Prepare => {
                        self.busy = None;
                        self.refresh_installed();
                        if code == Some(0) {
                            self.line(format!("Game files prepared in {seconds:.0}s"), Level::Info);
                            if self.sounds_after_prepare {
                                self.sounds_after_prepare = false;
                                self.start_sounds();
                            }
                        } else {
                            self.line(
                                format!("Preparation failed (exit code {code:?}); see the log."),
                                Level::Error,
                            );
                            self.notice("Preparation failed - the console says why.");
                        }
                    }
                    Job::Sounds => {
                        self.busy = None;
                        if code == Some(0) {
                            self.line("Sounds copied.", Level::Info);
                        } else {
                            self.line(
                                "Copying sounds stopped early; the game will be silent.",
                                Level::Warn,
                            );
                        }
                    }
                    Job::Game => {
                        self.busy = None;
                        self.game_pid = None;
                        let hint = launch::exit_hint(code);
                        let level = if code == Some(0) {
                            Level::Info
                        } else {
                            Level::Warn
                        };
                        self.line(hint, level);
                        if let Some(instance) = self.config.instances.iter_mut().find(|i| {
                            i.id == self.config.selected_instance.clone().unwrap_or_default()
                        }) {
                            instance.play_seconds += seconds as u64;
                            self.dirty = true;
                        }
                    }
                },
                Event::InstallProgress {
                    tag,
                    done,
                    total,
                    message,
                } => {
                    if let Some(state) = &mut self.install {
                        if state.tag == tag {
                            state.done = done;
                            state.total = total;
                            state.message = message;
                        }
                    }
                }
                Event::InstallFinished { tag, error } => {
                    self.install = None;
                    match error {
                        None => {
                            self.refresh_installed();
                            self.selected_tag = Some(tag.clone());
                            self.config.settings.last_build = Some(tag.clone());
                            self.dirty = true;
                            self.line(format!("Installed {tag}"), Level::Info);
                            self.refresh_installed();
                            self.refresh_server_jar();
                            self.start_caps(tag);
                        }
                        Some(err) => {
                            self.line(err.clone(), Level::Error);
                            self.notice(err);
                        }
                    }
                }
                Event::BuildsFetched { builds, error } => {
                    self.pending_builds = false;
                    self.builds = builds;
                    self.builds_error = error.clone();
                    if let Some(err) = error {
                        self.line(format!("Could not list builds: {err}"), Level::Warn);
                    }
                }
                Event::DetectionDone { detection, error } => {
                    self.pending_detect = false;
                    if let Some(err) = error {
                        self.line(format!("Detection failed: {err}"), Level::Warn);
                    }
                    self.detection = Some(*detection);
                }
                Event::GameSpawned { pid, command } => {
                    self.game_pid = Some(pid);
                    self.line(format!("Started: {command}"), Level::Info);
                }
                Event::ServerJarProgress {
                    done,
                    total,
                    message,
                } => {
                    self.jar_fetch = Some(mojang::Progress {
                        done,
                        total,
                        message,
                    });
                }
                Event::ServerJarReady { path, error } => {
                    self.jar_fetch = None;
                    match (path, error) {
                        (Some(path), _) => {
                            self.line(format!("Server jar ready: {}", path.display()), Level::Info);
                            self.server_jar = Some(path);
                            if self.pending_prepare {
                                self.pending_prepare = false;
                                self.start_prepare();
                            }
                        }
                        (None, Some(err)) => {
                            self.pending_prepare = false;
                            self.line(err.clone(), Level::Error);
                            self.notice(err);
                        }
                        _ => {}
                    }
                }
                Event::SessionChecked {
                    session,
                    warning,
                    error,
                } => {
                    self.session_check = false;
                    self.sign_in = None;
                    match (session, error) {
                        (Some(session), _) => {
                            let name = session.name.clone();
                            if let Some(warning) = warning {
                                self.line(warning, Level::Warn);
                            }
                            self.config.settings.session = Some(*session);
                            self.config.settings.account = AccountMode::Microsoft;
                            self.dirty = true;
                            self.line(format!("Signed in as {name}"), Level::Info);
                            self.notice(format!("Signed in as {name}"));
                            if self.pending_play {
                                self.pending_play = false;
                                self.start_play();
                            }
                        }
                        (None, Some(err)) => {
                            self.pending_play = false;
                            self.line(err.clone(), Level::Error);
                            self.notice(err);
                        }
                        _ => {
                            self.pending_play = false;
                        }
                    }
                }
                Event::SignInCode { code, error } => match (code, error) {
                    (Some(code), _) => {
                        if let Some(sign_in) = &mut self.sign_in {
                            sign_in.user_code = code.user_code.clone();
                            sign_in.url = code.verification_uri.clone();
                        }
                        self.line(
                            format!(
                                "Sign in at {} with the code {}",
                                code.verification_uri, code.user_code
                            ),
                            Level::Info,
                        );
                    }
                    (None, Some(err)) => {
                        self.sign_in = None;
                        self.line(err.clone(), Level::Error);
                        self.notice(err);
                    }
                    _ => {}
                },
                Event::SignInEnded { error } => {
                    self.sign_in = None;
                    if let Some(err) = error {
                        self.line(err.clone(), Level::Error);
                        self.notice(err);
                    }
                }
                Event::CapsProbed { tag, caps, error } => {
                    self.pending_caps = false;
                    match (caps, error) {
                        (Some(caps), _) => {
                            self.line(
                                format!(
                                    "{} speaks Launch API {} (Minecraft {})",
                                    caps.horizon_label, caps.launch_api, caps.minecraft
                                ),
                                Level::Info,
                            );
                            self.caps = Some((tag, caps));
                        }
                        (None, Some(err)) => {
                            self.line(err.clone(), Level::Error);
                            self.notice(err);
                        }
                        (None, None) => {}
                    }
                }
            }
        }

        if self.dirty {
            self.save();
        }

        if let Some((_, left)) = &mut self.notice {
            if *left > 0.0 {
                *left -= ctx.input(|i| i.stable_dt).min(0.1) as f64;
                ctx.request_repaint_after(Duration::from_millis(80));
            }
        }
        if self.busy.is_some()
            || self.pending_builds
            || self.pending_detect
            || self.pending_caps
            || self.jar_fetch.is_some()
            || self.session_check
            || self.sign_in.is_some()
        {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

fn classify(line: &str) -> Level {
    let lower = line.to_ascii_lowercase();
    if lower.contains("error") || lower.contains("panic") || lower.contains("crash") {
        Level::Error
    } else if lower.contains("warn")
        || lower.contains("missing")
        || lower.contains("could not")
        || lower.contains("failed")
    {
        Level::Warn
    } else {
        Level::Info
    }
}
