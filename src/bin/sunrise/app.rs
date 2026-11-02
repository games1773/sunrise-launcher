//! The window's state, what each control asks for, and the frame loop.
//!
//! This is the only owner of state. Panels read it and hand back an `Action`;
//! nothing here draws, and the jobs an action starts live in `work`.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use eframe::egui;
use sunrise::auth::Session;
use sunrise::config::AccountMode;
use sunrise::{builds, detect, launch, mojang, paths, prepare, Config, Event};

use crate::theme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Level {
    Info,
    Warn,
    Error,
}

pub(crate) struct Line {
    pub(crate) text: String,
    pub(crate) level: Level,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Busy {
    Prepare,
    Sounds,
    Game,
}

/// A sign-in waiting for the player to finish in their browser.
#[derive(Clone)]
pub(crate) struct SignIn {
    /// The code to type; empty until Microsoft has answered.
    pub(crate) user_code: String,
    /// Where to type it.
    pub(crate) url: String,
    /// Set when the player cancels or the panel closes.
    pub(crate) cancel: Arc<AtomicBool>,
}

pub(crate) struct InstallState {
    pub(crate) tag: String,
    pub(crate) done: u64,
    pub(crate) total: u64,
    pub(crate) message: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Setup,
    Launch,
    Console,
}

impl Tab {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Setup => "Setup",
            Tab::Launch => "Launch",
            Tab::Console => "Console",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Branch {
    Nightly,
    Release,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    None,
    Play,
    Prepare,
    Builds,
    Verify,
    BrowseMinecraft,
    BrowseJava,
    OpenGameFolder,
    OpenLogs,
    CopyLogs,
    ClearLogs,
    UseOfflineAccount,
    UseOnlineAccount,
    ImportSession,
    PasteSession,
    SignIn,
    CancelSignIn,
    SignOut,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowButton {
    Minimize,
    Maximize,
    Close,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Icon {
    Sliders,
    Play,
    Terminal,
}

pub(crate) struct App {
    pub(crate) data: PathBuf,
    pub(crate) config: Config,
    pub(crate) detection: Option<detect::Detection>,
    pub(crate) builds: Vec<builds::RemoteBuild>,
    pub(crate) builds_error: Option<String>,
    pub(crate) installed: Vec<builds::InstalledBuild>,
    pub(crate) selected_tag: Option<String>,
    pub(crate) caps: Option<(String, launch::Caps)>,
    pub(crate) tx: Sender<Event>,
    pub(crate) rx: Receiver<Event>,
    pub(crate) log: Vec<Line>,
    pub(crate) busy: Option<Busy>,
    pub(crate) install: Option<InstallState>,
    /// The Mojang server-jar download, while it runs.
    pub(crate) jar_fetch: Option<mojang::Progress>,
    /// A prepare waiting for that download to finish.
    pub(crate) pending_prepare: bool,
    pub(crate) server_jar: Option<PathBuf>,
    /// A session import or renewal waiting for its answer from Mojang.
    pub(crate) session_check: bool,
    /// Sign in with Microsoft, from the code to the session.
    pub(crate) sign_in: Option<SignIn>,
    /// Play was pressed, and waits for the renewed session.
    pub(crate) pending_play: bool,
    /// Frames left to catch the session paste the button asked for.
    pub(crate) await_paste: u32,
    /// `SUNRISE_PREPARE=1` has already started its one preparation.
    pub(crate) auto_prepare_done: bool,
    pub(crate) sounds_after_prepare: bool,
    pub(crate) notice: Option<(String, f64)>,
    pub(crate) builds_window: bool,
    pub(crate) tab: Tab,
    pub(crate) branch: Branch,
    pub(crate) maximized: bool,
    pub(crate) pending_builds: bool,
    pub(crate) pending_detect: bool,
    pub(crate) pending_caps: bool,
    pub(crate) game_pid: Option<u32>,
    pub(crate) window_title: Option<String>,
    pub(crate) dirty: bool,
    pub(crate) frames: u32,
    pub(crate) capture_requested: bool,
}

impl App {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>, data: PathBuf) -> Self {
        crate::theme::install(&cc.egui_ctx);

        let config = Config::load(&data);
        let _ = paths::ensure_dir(&data);
        let _ = paths::ensure_dir(&config.selected().game_dir);

        let (tx, rx) = mpsc::channel();
        let mut app = Self {
            data,
            config,
            detection: None,
            builds: Vec::new(),
            builds_error: None,
            installed: Vec::new(),
            selected_tag: None,
            caps: None,
            tx,
            rx,
            log: Vec::new(),
            busy: None,
            install: None,
            jar_fetch: None,
            pending_prepare: false,
            server_jar: None,
            session_check: false,
            sign_in: None,
            pending_play: false,
            await_paste: 0,
            auto_prepare_done: false,
            sounds_after_prepare: false,
            notice: None,
            builds_window: false,
            tab: Tab::Setup,
            branch: Branch::Nightly,
            maximized: false,
            pending_builds: false,
            pending_detect: false,
            pending_caps: false,
            game_pid: None,
            window_title: None,
            dirty: false,
            frames: 0,
            capture_requested: false,
        };

        app.refresh_installed();
        app.selected_tag = app
            .config
            .settings
            .last_build
            .clone()
            .filter(|tag| app.installed.iter().any(|b| &b.tag == tag))
            .or_else(|| app.installed.last().map(|b| b.tag.clone()));

        if let Ok(tab) = std::env::var("SUNRISE_TAB") {
            app.tab = match tab.as_str() {
                "console" => Tab::Console,
                "launch" => Tab::Launch,
                _ => Tab::Setup,
            };
        }

        app.refresh_server_jar();
        app.line("Sunrise core ready.", Level::Info);
        app.start_detect();
        app.start_builds(false);
        if let Some(tag) = app.selected_tag.clone() {
            app.start_caps(tag);
        }
        app
    }

    // ---------------------------------------------------------------- state

    pub(crate) fn line(&mut self, text: impl Into<String>, level: Level) {
        self.log.push(Line {
            text: text.into(),
            level,
        });
        if self.log.len() > 2000 {
            self.log.drain(0..400);
        }
    }

    pub(crate) fn notice(&mut self, message: impl Into<String>) {
        self.notice = Some((message.into(), 8.0));
    }

    pub(crate) fn refresh_installed(&mut self) {
        self.installed = builds::installed_builds(&self.data);
        if let Some(tag) = &self.selected_tag {
            if !self.installed.iter().any(|b| &b.tag == tag) {
                self.selected_tag = self.installed.last().map(|b| b.tag.clone());
            }
        }
        if self.selected_tag.is_none() {
            self.selected_tag = self.installed.last().map(|b| b.tag.clone());
        }
    }

    /// The cached server jar for the selected build, when it is already here.
    pub(crate) fn refresh_server_jar(&mut self) {
        self.server_jar = self.current_build().and_then(|build| {
            let pinned = builds::pinned_server_sha1(&build.dir)?;
            mojang::cached(&self.data, &build.version, &pinned)
        });
    }

    pub(crate) fn current_build(&self) -> Option<&builds::InstalledBuild> {
        match &self.selected_tag {
            Some(tag) => self.installed.iter().find(|b| &b.tag == tag),
            None => self.installed.last(),
        }
    }

    pub(crate) fn build_dir(&self) -> Option<PathBuf> {
        self.current_build().map(|b| b.dir.clone())
    }

    pub(crate) fn setup(&self) -> prepare::SetupStatus {
        prepare::status(self.build_dir().as_deref(), self.detection.as_ref())
    }

    pub(crate) fn online(&self) -> bool {
        self.config.settings.account == AccountMode::Microsoft
    }

    /// The session the game would get, or `None` for the offline name.
    pub(crate) fn session(&self) -> Option<&Session> {
        if self.online() {
            self.config.settings.session.as_ref()
        } else {
            None
        }
    }

    pub(crate) fn save(&mut self) {
        self.dirty = false;
        if let Err(err) = self.config.save(&self.data) {
            self.line(err, Level::Error);
        }
    }

    pub(crate) fn playtime(&self) -> String {
        let seconds = self.config.selected().play_seconds;
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        if hours > 0 {
            format!("{hours} hrs {minutes} mins")
        } else {
            format!("{minutes} mins")
        }
    }

    /// "Nightly 2026-10-31" and its commit, from the build's own label.
    pub(crate) fn build_badges(&self) -> Option<(String, Option<String>)> {
        let (_, caps) = self.caps.as_ref()?;
        let label = caps.horizon_label.trim();
        if label.is_empty() {
            return None;
        }
        let mut parts = label.split('·');
        let head = parts.next().unwrap_or("").trim().to_string();
        let commit = parts.next().map(|s| s.trim().to_string());
        Some((head, commit))
    }

    // ----------------------------------------------------------------- events

    /// A paste asked for by the account's Paste button, not by a text field.
    pub(crate) fn handle_paste(&mut self, ctx: &egui::Context) {
        if self.await_paste == 0 {
            return;
        }
        let pasted = ctx.input(|i| {
            i.events.iter().find_map(|event| match event {
                egui::Event::Paste(text) => Some(text.clone()),
                _ => None,
            })
        });
        match pasted {
            Some(text) => {
                self.await_paste = 0;
                self.import_session_text(&text);
            }
            None => {
                self.await_paste -= 1;
                ctx.request_repaint();
            }
        }
    }

    pub(crate) fn do_action(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::Play => self.start_play(),
            Action::Prepare => self.start_prepare(),
            Action::Builds => self.builds_window = true,
            Action::Verify => {
                let prepared = self
                    .build_dir()
                    .map(|dir| prepare::is_prepared(&dir))
                    .unwrap_or(false);
                if prepared {
                    self.notice("Game files look complete.");
                } else {
                    self.start_prepare();
                }
            }
            Action::BrowseMinecraft => self.pick_minecraft(),
            Action::BrowseJava => self.pick_java(),
            Action::UseOfflineAccount => {
                self.config.settings.account = AccountMode::Offline;
                self.dirty = true;
            }
            Action::UseOnlineAccount => {
                self.config.settings.account = AccountMode::Microsoft;
                self.dirty = true;
            }
            Action::ImportSession => self.pick_session(),
            Action::SignIn => self.start_sign_in(),
            Action::CancelSignIn => {
                if let Some(sign_in) = self.sign_in.take() {
                    sign_in
                        .cancel
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                    self.notice("Sign-in cancelled.");
                }
            }
            Action::PasteSession => {
                self.await_paste = 30;
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            }
            Action::SignOut => {
                self.config.settings.session = None;
                self.config.settings.account = AccountMode::Offline;
                self.dirty = true;
                self.notice("Signed out; the offline name is the account again.");
            }
            Action::OpenGameFolder => {
                let _ = open::that(self.config.selected().game_dir.clone());
            }
            Action::OpenLogs => {
                let dir = self.config.selected().game_dir.join("logs");
                let _ = std::fs::create_dir_all(&dir);
                let _ = open::that(dir);
            }
            Action::CopyLogs => {
                let text: Vec<String> = self.log.iter().map(|l| l.text.clone()).collect();
                ctx.copy_text(text.join("\n"));
                self.notice("Console copied.");
            }
            Action::ClearLogs => self.log.clear(),
            Action::None => {}
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        self.handle_paste(&ctx);

        // SUNRISE_PREPARE=1 presses Prepare once detection is done. A test hook,
        // like SUNRISE_CAPTURE: it makes the download-then-prepare path run
        // without a person clicking.
        if !self.auto_prepare_done
            && self.detection.is_some()
            && std::env::var_os("SUNRISE_PREPARE").is_some()
        {
            self.auto_prepare_done = true;
            self.start_prepare();
        }

        theme::paint_ambient(&ctx);
        self.title_bar(ui);
        self.right_panel(ui);
        self.left_column(ui);
        self.builds_window(&ctx);
        self.paint_toast(&ctx);

        let title = if self.game_pid.is_some() {
            "Sunrise - playing"
        } else {
            "Sunrise"
        };
        if self.window_title.as_deref() != Some(title) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.to_string()));
            self.window_title = Some(title.to_string());
        }

        // SUNRISE_CAPTURE=path writes one raw frame and closes: used to check the layout.
        if let Ok(path) = std::env::var("SUNRISE_CAPTURE") {
            self.frames += 1;
            if self.frames == 60 && !self.capture_requested {
                self.capture_requested = true;
                ctx.request_screenshot(move |image| {
                    let mut out = format!("{} {}\n", image.width(), image.height()).into_bytes();
                    for pixel in &image.pixels {
                        out.extend_from_slice(&pixel.to_array());
                    }
                    let _ = std::fs::write(&path, out);
                });
            }
            if self.frames > 90 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint();
        }
    }
}
