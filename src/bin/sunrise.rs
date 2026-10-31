//! The Sunrise Launcher window.
//!
//! Visual translation of the supplied mock: obsidian surfaces, sunrise amber
//! accents, a custom title bar, a hero banner, and a tabbed management panel.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use eframe::egui;
use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Layout, Margin, Rect, RichText, Sense, Shape,
    Stroke, StrokeKind, UiBuilder, Vec2,
};
use sunrise::config::valid_player_name;
use sunrise::{builds, detect, launch, paths, prepare, Config, Event, Job};

// ---------------------------------------------------------------- palette

const OBSIDIAN_950: Color32 = Color32::from_rgb(6, 7, 10);
const OBSIDIAN_900: Color32 = Color32::from_rgb(11, 13, 19);
const OBSIDIAN_850: Color32 = Color32::from_rgb(17, 20, 29);
const OBSIDIAN_800: Color32 = Color32::from_rgb(23, 27, 39);
const SUNRISE_300: Color32 = Color32::from_rgb(255, 193, 122);
const SUNRISE_400: Color32 = Color32::from_rgb(255, 158, 59);
const SUNRISE_500: Color32 = Color32::from_rgb(255, 119, 17);
const CORAL_500: Color32 = Color32::from_rgb(255, 71, 87);
const SLATE_100: Color32 = Color32::from_rgb(241, 245, 249);
const SLATE_300: Color32 = Color32::from_rgb(203, 213, 225);
const SLATE_400: Color32 = Color32::from_rgb(148, 163, 184);
const SLATE_500: Color32 = Color32::from_rgb(100, 116, 139);
const EMERALD_400: Color32 = Color32::from_rgb(52, 211, 153);
const AMBER_TEXT: Color32 = Color32::from_rgb(252, 211, 77);

fn white(alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(255, 255, 255, alpha)
}

fn glass_fill() -> Color32 {
    Color32::from_rgba_unmultiplied(17, 20, 29, 210)
}

fn main() -> eframe::Result {
    let data = paths::data_dir();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Sunrise")
            .with_inner_size([1120.0, 720.0])
            .with_decorations(false)
            .with_resizable(false)
            .with_icon(std::sync::Arc::new(app_icon())),
        ..Default::default()
    };
    eframe::run_native(
        "Sunrise",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, data)))),
    )
}

fn app_icon() -> egui::IconData {
    let size = 64u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 / size as f32;
            let fy = y as f32 / size as f32;
            let i = ((y * size + x) * 4) as usize;
            rgba[i] = 6;
            rgba[i + 1] = 7;
            rgba[i + 2] = 10;
            rgba[i + 3] = 255;
            let d = ((fx - 0.5).powi(2) + (fy - 0.5).powi(2)).sqrt();
            if d < 0.26 {
                rgba[i] = 255;
                rgba[i + 1] = 158;
                rgba[i + 2] = 59;
            }
        }
    }
    egui::IconData {
        rgba,
        width: size,
        height: size,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Info,
    Warn,
    Error,
}

struct Line {
    text: String,
    level: Level,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Busy {
    Prepare,
    Sounds,
    Game,
}

struct InstallState {
    tag: String,
    done: u64,
    total: u64,
    message: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Setup,
    Launch,
    Console,
}

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::Setup => "Setup",
            Tab::Launch => "Launch",
            Tab::Console => "Console",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Branch {
    Nightly,
    Release,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    None,
    Play,
    Prepare,
    Builds,
    Verify,
    BrowseMinecraft,
    BrowseServerJar,
    BrowseJava,
    OpenGameFolder,
    OpenLogs,
    CopyLogs,
    ClearLogs,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WindowButton {
    Minimize,
    Maximize,
    Close,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Icon {
    Sliders,
    Play,
    Terminal,
}

struct App {
    data: PathBuf,
    config: Config,
    detection: Option<detect::Detection>,
    builds: Vec<builds::RemoteBuild>,
    builds_error: Option<String>,
    installed: Vec<builds::InstalledBuild>,
    selected_tag: Option<String>,
    caps: Option<(String, launch::Caps)>,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    log: Vec<Line>,
    busy: Option<Busy>,
    install: Option<InstallState>,
    sounds_after_prepare: bool,
    notice: Option<(String, f64)>,
    builds_window: bool,
    tab: Tab,
    branch: Branch,
    maximized: bool,
    pending_builds: bool,
    pending_detect: bool,
    pending_caps: bool,
    game_pid: Option<u32>,
    window_title: Option<String>,
    dirty: bool,
    frames: u32,
    capture_requested: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, data: PathBuf) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = OBSIDIAN_950;
        visuals.window_fill = OBSIDIAN_850;
        visuals.window_stroke = Stroke::new(1.0, white(30));
        visuals.extreme_bg_color = OBSIDIAN_950;
        visuals.override_text_color = Some(SLATE_100);
        visuals.widgets.noninteractive.bg_fill = OBSIDIAN_850;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, white(20));
        visuals.widgets.noninteractive.corner_radius = CornerRadius::same(8);
        visuals.widgets.inactive.bg_fill = white(8);
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, white(22));
        visuals.widgets.inactive.corner_radius = CornerRadius::same(8);
        visuals.widgets.hovered.bg_fill = white(16);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, white(45));
        visuals.widgets.hovered.corner_radius = CornerRadius::same(8);
        visuals.widgets.active.bg_fill = white(24);
        visuals.widgets.active.bg_stroke = Stroke::new(1.0, SUNRISE_400.gamma_multiply(0.7));
        visuals.widgets.active.corner_radius = CornerRadius::same(8);
        visuals.selection.bg_fill = SUNRISE_500.gamma_multiply(0.35);
        visuals.selection.stroke = Stroke::new(1.0, SUNRISE_400);
        visuals.hyperlink_color = SUNRISE_400;
        cc.egui_ctx.set_visuals(visuals);
        cc.egui_ctx.all_styles_mut(|style| {
            style.spacing.item_spacing = Vec2::new(8.0, 6.0);
            style.spacing.button_padding = Vec2::new(10.0, 5.0);
        });

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

        app.line("Sunrise core ready.", Level::Info);
        app.start_detect();
        app.start_builds(false);
        if let Some(tag) = app.selected_tag.clone() {
            app.start_caps(tag);
        }
        app
    }

    // ---------------------------------------------------------------- state

    fn line(&mut self, text: impl Into<String>, level: Level) {
        self.log.push(Line {
            text: text.into(),
            level,
        });
        if self.log.len() > 2000 {
            self.log.drain(0..400);
        }
    }

    fn notice(&mut self, message: impl Into<String>) {
        self.notice = Some((message.into(), 8.0));
    }

    fn refresh_installed(&mut self) {
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

    fn current_build(&self) -> Option<&builds::InstalledBuild> {
        match &self.selected_tag {
            Some(tag) => self.installed.iter().find(|b| &b.tag == tag),
            None => self.installed.last(),
        }
    }

    fn build_dir(&self) -> Option<PathBuf> {
        self.current_build().map(|b| b.dir.clone())
    }

    fn setup(&self) -> prepare::SetupStatus {
        prepare::status(
            self.build_dir().as_deref(),
            self.detection.as_ref(),
            self.config.settings.server_jar.as_deref(),
        )
    }

    fn save(&mut self) {
        self.dirty = false;
        if let Err(err) = self.config.save(&self.data) {
            self.line(err, Level::Error);
        }
    }

    fn playtime(&self) -> String {
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
    fn build_badges(&self) -> Option<(String, Option<String>)> {
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

    // ------------------------------------------------------------- background

    fn start_detect(&mut self) {
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

    fn start_builds(&mut self, force: bool) {
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

    fn start_caps(&mut self, tag: String) {
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

    fn start_install(&mut self, tag: String) {
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

    fn start_prepare(&mut self) {
        let Some(build) = self.current_build().cloned() else {
            self.builds_window = true;
            return;
        };
        if let Some(reason) = prepare::blocker(
            Some(&build.dir),
            self.detection.as_ref(),
            self.config.settings.server_jar.as_deref(),
        ) {
            self.notice(reason);
            return;
        }
        let minecraft = self
            .detection
            .as_ref()
            .and_then(|d| d.chosen.as_ref())
            .map(|c| c.dir.clone());
        let cmd = match prepare::prepare_command(
            &build.dir,
            minecraft.as_deref(),
            self.config.settings.server_jar.as_deref(),
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
        if let Err(err) = sunrise::spawn_streamed(cmd, Job::Prepare, self.tx.clone()) {
            self.busy = None;
            self.line(err.clone(), Level::Error);
            self.notice(err);
        }
    }

    fn start_sounds(&mut self) {
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
        let _ = sunrise::spawn_streamed(cmd, Job::Sounds, self.tx.clone());
    }

    fn start_play(&mut self) {
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
        if !valid_player_name(&self.config.settings.player_name) {
            self.notice("Pick a name: 3-16 letters, digits or _");
            self.tab = Tab::Setup;
            return;
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
            name: Some(self.config.settings.player_name.trim().to_string()),
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

    fn pick_minecraft(&mut self) {
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

    fn pick_server_jar(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("The 26.3 server jar")
            .add_filter("Minecraft server", &["jar"]);
        if let Some(dir) = dirs::download_dir() {
            dialog = dialog.set_directory(dir);
        }
        if let Some(file) = dialog.pick_file() {
            self.config.settings.server_jar = Some(file);
            self.dirty = true;
            self.start_detect();
        }
    }

    fn pick_java(&mut self) {
        if let Some(dir) = rfd::FileDialog::new()
            .set_title("A Java 25 home folder")
            .pick_folder()
        {
            self.config.settings.java_home = Some(dir);
            self.dirty = true;
            self.start_detect();
        }
    }

    // ----------------------------------------------------------------- events

    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Log { line, .. } => {
                    let level = classify(&line);
                    self.line(line, level);
                }
                Event::Finished {
                    job,
                    code,
                    seconds,
                } => match job {
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
        if self.busy.is_some() || self.pending_builds || self.pending_detect || self.pending_caps {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn do_action(&mut self, action: Action, ctx: &egui::Context) {
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
            Action::BrowseServerJar => self.pick_server_jar(),
            Action::BrowseJava => self.pick_java(),
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

    // -------------------------------------------------------------------- ui

    fn paint_ambient(&self, ctx: &egui::Context) {
        let rect = ctx.viewport_rect();
        let painter = ctx.layer_painter(egui::LayerId::background());
        painter.rect_filled(rect, 0, OBSIDIAN_950);
        glow(&painter, egui::pos2(rect.left() + 40.0, rect.top() + 10.0), 300.0, SUNRISE_500, 22);
        glow(
            &painter,
            egui::pos2(rect.right() - 60.0, rect.center().y),
            260.0,
            CORAL_500,
            14,
        );
    }

    fn title_bar(&mut self, ui: &mut egui::Ui) {
        let mut window_action: Option<WindowButton> = None;
        let frame = egui::Frame::new()
            .fill(Color32::from_rgba_unmultiplied(6, 7, 10, 240))
            .inner_margin(Margin::symmetric(16, 11));

        egui::Panel::top("titlebar")
            .exact_size(56.0)
            .frame(frame)
            .show(ui, |ui| {
                let bar = ui.max_rect();
                let drag =
                    ui.interact(bar, ui.scope_id().with("titlebar-drag"), Sense::click_and_drag());
                if drag.drag_started() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag.double_clicked() {
                    self.maximized = !self.maximized;
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(self.maximized));
                }

                ui.horizontal_centered(|ui| {
                    logo(ui, 30.0);
                    ui.add_space(9.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("SUNRISE").size(15.0).strong().color(SLATE_100));
                        ui.label(
                            RichText::new("LAUNCHER")
                                .monospace()
                                .size(9.0)
                                .strong()
                                .color(SUNRISE_400),
                        );
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if window_button(ui, WindowButton::Close) {
                            window_action = Some(WindowButton::Close);
                        }
                        if window_button(ui, WindowButton::Maximize) {
                            window_action = Some(WindowButton::Maximize);
                        }
                        if window_button(ui, WindowButton::Minimize) {
                            window_action = Some(WindowButton::Minimize);
                        }
                        ui.add_space(10.0);
                        if let Some((head, commit)) = self.build_badges() {
                            if let Some(commit) = commit {
                                ui.label(
                                    RichText::new(format!("commit: {commit}"))
                                        .monospace()
                                        .size(10.0)
                                        .color(SLATE_500),
                                );
                            }
                            pill(
                                ui,
                                &head,
                                SUNRISE_300,
                                Color32::from_rgba_unmultiplied(255, 255, 255, 12),
                                Some(EMERALD_400),
                            );
                        } else if let Some(build) = self.current_build() {
                            pill(
                                ui,
                                &build.tag,
                                SLATE_300,
                                Color32::from_rgba_unmultiplied(255, 255, 255, 12),
                                Some(EMERALD_400),
                            );
                        }
                    });
                });
            });

        match window_action {
            Some(WindowButton::Close) => {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Some(WindowButton::Minimize) => {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            Some(WindowButton::Maximize) => {
                self.maximized = !self.maximized;
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Maximized(self.maximized));
            }
            None => {}
        }
    }

    fn left_column(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default_margins()
            .frame(
                egui::Frame::new()
                    .fill(Color32::TRANSPARENT)
                    .inner_margin(Margin::symmetric(22, 18)),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.hero(ui);
                        ui.add_space(16.0);
                        self.selectors(ui);
                        ui.add_space(16.0);
                        self.play_panel(ui);
                        ui.add_space(16.0);
                        self.log_strip(ui);
                        ui.add_space(8.0);
                    });
            });
    }

    fn hero(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 262.0), Sense::hover());
        let radius = CornerRadius::same(16);

        ui.painter().rect_filled(rect, radius, OBSIDIAN_850);
        paint_sky(ui.painter(), rect);
        ui.painter()
            .rect_stroke(rect, radius, Stroke::new(1.0, white(28)), StrokeKind::Inside);

        let inner = rect.shrink2(Vec2::new(18.0, 15.0));
        let top_rect = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 24.0));
        let bottom_rect = Rect::from_min_max(
            egui::pos2(inner.left(), inner.bottom() - 62.0),
            inner.max,
        );

        let mut top =
            ui.new_child(UiBuilder::new().max_rect(top_rect).layout(Layout::left_to_right(Align::Center)));
        top.horizontal_centered(|ui| {
            pill(
                ui,
                "Minecraft 26.3 ready",
                SUNRISE_300,
                Color32::from_rgba_unmultiplied(6, 7, 10, 190),
                Some(SUNRISE_400),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let setup = self.setup();
                let (label, color) = if self.busy == Some(Busy::Game) {
                    ("Client online".to_string(), EMERALD_400)
                } else if setup.ready() {
                    ("Prepared".to_string(), EMERALD_400)
                } else if setup.build_installed {
                    ("Needs setup".to_string(), AMBER_TEXT)
                } else {
                    ("No build".to_string(), AMBER_TEXT)
                };
                pill(
                    ui,
                    &label,
                    color,
                    Color32::from_rgba_unmultiplied(6, 7, 10, 190),
                    None,
                );
            });
        });

        let mut bottom =
            ui.new_child(UiBuilder::new().max_rect(bottom_rect).layout(Layout::left_to_right(Align::Center)));
        bottom.horizontal(|ui| {
            let name = self.config.settings.player_name.trim().to_string();
            avatar(ui, &name, 58.0);
            ui.add_space(2.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new(&name).size(21.0).strong().color(SLATE_100));
                ui.horizontal(|ui| {
                    dot(ui, EMERALD_400, 5.0);
                    ui.label(RichText::new("Offline account").size(11.0).color(SLATE_400));
                });
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(
                        RichText::new("PLAYTIME")
                            .size(9.0)
                            .color(SLATE_500),
                    );
                    ui.label(
                        RichText::new(self.playtime())
                            .monospace()
                            .size(12.0)
                            .strong()
                            .color(SUNRISE_300),
                    );
                });
            });
        });
    }

    fn selectors(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |cols| {
            // Offline account name.
            glass(&mut cols[0], |ui| {
                field_label(ui, "OFFLINE ACCOUNT", None);
                ui.add_space(3.0);
                let mut name = self.config.settings.player_name.clone();
                let response = text_input(ui, &mut name, "Player", 13.0, false);
                if response.changed() {
                    self.config.settings.player_name = name.clone();
                    self.dirty = true;
                }
                if !valid_player_name(&name) {
                    ui.label(
                        RichText::new("3-16 letters, digits or _")
                            .size(9.5)
                            .color(AMBER_TEXT),
                    );
                }
            });

            // Server address.
            glass(&mut cols[1], |ui| {
                field_label(ui, "TARGET SERVER", None);
                ui.add_space(3.0);
                let mut server = self.config.selected().connect.clone().unwrap_or_default();
                let response = text_input(ui, &mut server, "play.example.net:25565", 11.5, true);
                let bad = server.trim().contains(char::is_whitespace);
                if response.changed() {
                    self.config.selected_mut().connect = if server.trim().is_empty() {
                        None
                    } else {
                        Some(server.trim().to_string())
                    };
                    self.dirty = true;
                }
                if bad {
                    ui.label(
                        RichText::new("A server address has no spaces")
                            .size(9.5)
                            .color(AMBER_TEXT),
                    );
                } else {
                    ui.label(
                        RichText::new(if server.trim().is_empty() {
                            "Not joining a server"
                        } else {
                            "Offline-mode servers only"
                        })
                        .size(9.5)
                        .color(SLATE_500),
                    );
                }
            });
        });
    }

    fn play_panel(&mut self, ui: &mut egui::Ui) {
        let setup = self.setup();
        let build = self.current_build().cloned();
        let mut action = Action::None;

        let frame = egui::Frame::new()
            .fill(glass_fill())
            .corner_radius(CornerRadius::same(16))
            .stroke(Stroke::new(1.0, SUNRISE_500.gamma_multiply(0.35)))
            .inner_margin(Margin::symmetric(16, 14));

        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                if setup.ready() {
                    check_icon(ui, EMERALD_400, 13.0);
                } else {
                    dot(ui, AMBER_TEXT, 7.0);
                }
                let text = if self.busy == Some(Busy::Game) {
                    "Minecraft 26.3 is running".to_string()
                } else if build.is_none() {
                    "No Horizon build installed".to_string()
                } else if !setup.minecraft_found {
                    "Minecraft 26.3 not found".to_string()
                } else if !setup.java_found {
                    "Java 25 not found".to_string()
                } else if !setup.prepared {
                    "Game files need preparing".to_string()
                } else {
                    "Ready to launch version 26.3".to_string()
                };
                ui.label(RichText::new(text).size(12.0).color(SLATE_300));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(build) = &build {
                        ui.label(
                            RichText::new(&build.tag)
                                .monospace()
                                .size(10.0)
                                .color(SLATE_500),
                        );
                    }
                });
            });

            if let Some(state) = &self.install {
                ui.add_space(8.0);
                let fraction = if state.total > 0 {
                    state.done as f32 / state.total as f32
                } else {
                    0.0
                };
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&state.message)
                            .monospace()
                            .size(10.0)
                            .color(SLATE_400),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{}%", (fraction * 100.0) as u32))
                                .monospace()
                                .size(10.0)
                                .color(SLATE_400),
                        );
                    });
                });
                progress_bar(ui, fraction);
            } else if self.busy.is_some() {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(
                        RichText::new(match self.busy {
                            Some(Busy::Prepare) => "Preparing game files...",
                            Some(Busy::Sounds) => "Copying sounds...",
                            _ => "Working...",
                        })
                        .monospace()
                        .size(10.0)
                        .color(SLATE_400),
                    );
                });
            }

            ui.add_space(10.0);
            let label = if self.busy == Some(Busy::Game) {
                "GAME RUNNING"
            } else if self.busy.is_some() {
                "WORKING..."
            } else if build.is_none() {
                "INSTALL A BUILD"
            } else if !setup.prepared {
                "PREPARE GAME FILES"
            } else {
                "PLAY MINECRAFT"
            };
            let enabled = self.busy.is_none()
                && valid_player_name(&self.config.settings.player_name);
            if big_button(ui, label, enabled) {
                action = if build.is_none() {
                    Action::Builds
                } else if !setup.prepared {
                    Action::Prepare
                } else {
                    Action::Play
                };
            }

            if let Some((message, left)) = &self.notice {
                if *left > 0.0 {
                    ui.add_space(7.0);
                    ui.add(
                        egui::Label::new(RichText::new(message).size(10.5).color(AMBER_TEXT))
                            .wrap_mode(egui::TextWrapMode::Wrap),
                    );
                }
            }
        });

        let ctx = ui.ctx().clone();
        self.do_action(action, &ctx);
    }

    fn log_strip(&mut self, ui: &mut egui::Ui) {
        let mut action = Action::None;
        let frame = egui::Frame::new()
            .fill(glass_fill())
            .corner_radius(CornerRadius::same(12))
            .stroke(Stroke::new(1.0, white(18)))
            .inner_margin(Margin::symmetric(13, 9));
        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                terminal_icon(ui, SUNRISE_400, 12.0);
                let last = self
                    .log
                    .last()
                    .map(|l| l.text.clone())
                    .unwrap_or_else(|| "Waiting for the first log line.".to_string());
                ui.add(
                    egui::Label::new(
                        RichText::new(last)
                            .monospace()
                            .size(10.0)
                            .color(SLATE_400),
                    )
                    .wrap_mode(egui::TextWrapMode::Truncate),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("View logs").size(10.0).color(SUNRISE_400),
                            )
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE),
                        )
                        .clicked()
                    {
                        action = Action::OpenLogs;
                    }
                });
            });
        });
        if action == Action::OpenLogs {
            self.tab = Tab::Console;
        }
    }

    fn right_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("management")
            .exact_size(424.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(11, 13, 19, 232))
                    .inner_margin(Margin::ZERO),
            )
            .show(ui, |ui| {
                self.tabs(ui);
                if self.tab == Tab::Console {
                    // The console keeps its own scroll area and fills the panel. The
                    // panel's content Ui shrink-wraps vertically, so the room left
                    // comes from the window rather than from available_height().
                    ui.add_space(14.0);
                    let body = (ui.ctx().viewport_rect().bottom()
                        - ui.cursor().top()
                        - 18.0)
                        .max(240.0);
                    ui.horizontal(|ui| {
                        ui.add_space(18.0);
                        ui.vertical(|ui| {
                            ui.set_width(424.0 - 36.0);
                            self.console_tab(ui, body);
                        });
                    });
                    ui.add_space(18.0);
                } else {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add_space(16.0);
                            ui.horizontal(|ui| {
                                ui.add_space(18.0);
                                ui.vertical(|ui| {
                                    ui.set_width(424.0 - 36.0);
                                    match self.tab {
                                        Tab::Setup => self.setup_tab(ui),
                                        Tab::Launch => self.launch_tab(ui),
                                        Tab::Console => {}
                                    }
                                });
                            });
                            ui.add_space(18.0);
                        });
                }
            });
    }

    fn tabs(&mut self, ui: &mut egui::Ui) {
        let height = 44.0;
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
        ui.painter()
            .rect_filled(rect, 0, Color32::from_rgba_unmultiplied(6, 7, 10, 150));
        ui.painter().hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            Stroke::new(1.0, white(20)),
        );

        let tabs = [Tab::Setup, Tab::Launch, Tab::Console];
        let width = rect.width() / tabs.len() as f32;
        for (index, tab) in tabs.iter().enumerate() {
            let tab_rect = Rect::from_min_size(
                egui::pos2(rect.left() + index as f32 * width, rect.top()),
                Vec2::new(width, rect.height()),
            );
            let response = ui.interact(
                tab_rect,
                ui.scope_id().with(("tab", index)),
                Sense::click(),
            );
            let active = self.tab == *tab;
            if active {
                ui.painter().rect_filled(
                    tab_rect,
                    0,
                    SUNRISE_500.gamma_multiply(0.10),
                );
                ui.painter().rect_filled(
                    Rect::from_min_size(
                        egui::pos2(tab_rect.left(), tab_rect.bottom() - 2.0),
                        Vec2::new(width, 2.0),
                    ),
                    0,
                    SUNRISE_500,
                );
            } else if response.hovered() {
                ui.painter()
                    .rect_filled(tab_rect, 0, Color32::from_rgba_unmultiplied(255, 255, 255, 6));
            }
            let color = if active { SLATE_100 } else { SLATE_400 };
            let icon = match tab {
                Tab::Setup => Icon::Sliders,
                Tab::Launch => Icon::Play,
                Tab::Console => Icon::Terminal,
            };
            let galley = ui
                .painter()
                .layout_no_wrap(tab.label().to_string(), FontId::proportional(11.5), color);
            let total = 14.0 + 6.0 + galley.size().x;
            let start = tab_rect.center().x - total * 0.5;
            draw_icon(
                ui.painter(),
                icon,
                egui::pos2(start + 7.0, tab_rect.center().y),
                12.0,
                color,
            );
            ui.painter().galley(
                egui::pos2(start + 20.0, tab_rect.center().y - galley.size().y * 0.5),
                galley,
                color,
            );
            if response.clicked() {
                self.tab = *tab;
            }
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        }
    }

    fn setup_tab(&mut self, ui: &mut egui::Ui) {
        let mut action = Action::None;
        let setup = self.setup();
        let build = self.current_build().cloned();
        let detection = self.detection.clone();
        let server_jar = self.config.settings.server_jar.clone();
        let game_dir = self.config.selected().game_dir.clone();

        section_label(ui, "ENVIRONMENT & DIRECTORIES");
        ui.add_space(8.0);

        glass(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Release branch").size(11.0).color(SLATE_300));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let nightly = self.branch == Branch::Nightly;
                    if segmented(ui, "Nightly", nightly) && !nightly {
                        self.branch = Branch::Nightly;
                    }
                    let release = self.branch == Branch::Release;
                    if segmented(ui, "Release", release) && !release {
                        self.branch = Branch::Release;
                    }
                });
            });
        });

        ui.add_space(8.0);
        glass(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Game files").size(11.5).strong().color(SLATE_100));
                    let (text, color) = if setup.prepared {
                        ("Prepared & synchronized".to_string(), EMERALD_400)
                    } else if setup.build_installed {
                        ("Not prepared yet".to_string(), AMBER_TEXT)
                    } else {
                        ("No build installed".to_string(), SLATE_400)
                    };
                    ui.label(RichText::new(format!("Status: {text}")).size(10.0).color(color));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if small_button(ui, "Verify") {
                        action = Action::Verify;
                    }
                });
            });
        });

        ui.add_space(8.0);
        glass(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Horizon build").size(11.5).strong().color(SLATE_100));
                    ui.label(
                        RichText::new(
                            build
                                .as_ref()
                                .map(|b| b.tag.clone())
                                .unwrap_or_else(|| "none installed".to_string()),
                        )
                        .monospace()
                        .size(10.0)
                        .color(SLATE_400),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if small_button(ui, if setup.build_installed { "Change" } else { "Install" }) {
                        action = Action::Builds;
                    }
                });
            });
        });

        ui.add_space(12.0);
        if path_row(
            ui,
            "Minecraft directory",
            detection
                .as_ref()
                .and_then(|d| d.chosen.as_ref())
                .map(|m| m.dir.display().to_string()),
            "Choose",
        ) {
            action = Action::BrowseMinecraft;
        }
        ui.add_space(8.0);
        if path_row(
            ui,
            "Server jar path",
            server_jar
                .as_ref()
                .filter(|p| p.is_file())
                .map(|p| p.display().to_string()),
            "Choose",
        ) {
            action = Action::BrowseServerJar;
        }
        ui.add_space(8.0);
        if path_row(
            ui,
            "Game instance folder",
            Some(game_dir.display().to_string()),
            "Open",
        ) {
            action = Action::OpenGameFolder;
        }

        if !setup.java_found {
            ui.add_space(8.0);
            if path_row(ui, "Java 25", None, "Choose") {
                action = Action::BrowseJava;
            }
        }

        ui.add_space(12.0);
        glass(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Cache sound files")
                            .size(11.5)
                            .strong()
                            .color(SLATE_100),
                    );
                    ui.label(
                        RichText::new("Copies audio into the instance (380 MB)")
                            .size(10.0)
                            .color(SLATE_400),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if toggle(ui, &mut self.config.settings.copy_sounds) {
                        self.dirty = true;
                    }
                });
            });
        });

        let ctx = ui.ctx().clone();
        self.do_action(action, &ctx);
    }

    fn launch_tab(&mut self, ui: &mut egui::Ui) {
        section_label(ui, "PERFORMANCE & DISPLAY");
        ui.add_space(8.0);

        glass(ui, |ui| {
            let instance = self.config.selected_mut();
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Render distance")
                        .size(11.5)
                        .strong()
                        .color(SLATE_100),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{} chunks", instance.render_distance))
                            .monospace()
                            .size(11.5)
                            .strong()
                            .color(SUNRISE_400),
                    );
                });
            });
            ui.add_space(4.0);
            if ui
                .add(
                    egui::Slider::new(&mut instance.render_distance, 2..=32)
                        .show_value(false),
                )
                .changed()
            {
                self.dirty = true;
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new("2").monospace().size(9.0).color(SLATE_500));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new("32")
                            .monospace()
                            .size(9.0)
                            .color(SLATE_500),
                    );
                });
            });
        });

        ui.add_space(8.0);
        glass(ui, |ui| {
            let instance = self.config.selected_mut();
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(
                        RichText::new("Game window")
                            .size(11.5)
                            .strong()
                            .color(SLATE_100),
                    );
                    ui.label(
                        RichText::new(format!("{} x {}", instance.width, instance.height))
                            .monospace()
                            .size(10.0)
                            .color(SLATE_400),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if toggle(ui, &mut instance.fullscreen) {
                        self.dirty = true;
                    }
                    ui.label(RichText::new("Fullscreen").size(10.5).color(SLATE_300));
                });
            });
            ui.add_space(4.0);
            let presets: [(u32, u32); 4] = [
                (1280, 720),
                (1600, 900),
                (1920, 1080),
                (2560, 1440),
            ];
            let current = format!("{} x {}", instance.width, instance.height);
            egui::ComboBox::from_id_salt("resolution")
                .selected_text(RichText::new(current).size(10.5).color(SLATE_300))
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for (w, h) in presets {
                        if ui
                            .selectable_label(
                                instance.width == w && instance.height == h,
                                format!("{w} x {h}"),
                            )
                            .clicked()
                        {
                            instance.width = w;
                            instance.height = h;
                            self.dirty = true;
                        }
                    }
                });
        });

        ui.add_space(8.0);
        glass(ui, |ui| {
            let instance = self.config.selected_mut();
            ui.label(
                RichText::new("World to open")
                    .size(11.5)
                    .strong()
                    .color(SLATE_100),
            );
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                let mut world = instance.world.clone().unwrap_or_default();
                if text_input(ui, &mut world, "New World", 11.0, true).changed() {
                    instance.world = if world.trim().is_empty() {
                        None
                    } else {
                        Some(world)
                    };
                    self.dirty = true;
                }
            });
            ui.add_space(3.0);
            let mut seed = instance.seed.clone().unwrap_or_default();
            if text_input(ui, &mut seed, "Seed (optional)", 11.0, true).changed() {
                instance.seed = if seed.trim().is_empty() {
                    None
                } else {
                    Some(seed)
                };
                self.dirty = true;
            }
        });

        ui.add_space(8.0);
        glass(ui, |ui| {
            let instance = self.config.selected_mut();
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Extra arguments")
                        .size(11.5)
                        .strong()
                        .color(SLATE_100),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if toggle(ui, &mut instance.creative) {
                        self.dirty = true;
                    }
                    ui.label(RichText::new("Creative").size(10.5).color(SLATE_300));
                });
            });
            ui.add_space(3.0);
            if ui
                .add(
                    egui::TextEdit::singleline(&mut instance.extra_args)
                        .hint_text("--pregen")
                        .font(FontId::monospace(10.5))
                        .desired_width(f32::INFINITY)
                        .frame(input_frame()),
                )
                .changed()
            {
                self.dirty = true;
            }
        });
    }

    fn console_tab(&mut self, ui: &mut egui::Ui, body: f32) {
        let mut action = Action::None;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Live client logs")
                    .monospace()
                    .size(10.0)
                    .color(SLATE_400),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if small_button(ui, "Copy") {
                    action = Action::CopyLogs;
                }
                if small_button(ui, "Folder") {
                    action = Action::OpenLogs;
                }
                if small_button(ui, "Clear") {
                    action = Action::ClearLogs;
                }
            });
        });
        ui.add_space(6.0);

        let height = (body - 28.0).max(200.0);
        egui::Frame::new()
            .fill(OBSIDIAN_950)
            .corner_radius(CornerRadius::same(12))
            .stroke(Stroke::new(1.0, white(16)))
            .inner_margin(Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_height(height);
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .max_height(height - 12.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for line in &self.log {
                            let color = match line.level {
                                Level::Info => SLATE_300,
                                Level::Warn => AMBER_TEXT,
                                Level::Error => CORAL_500,
                            };
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&line.text).monospace().size(10.0).color(color),
                                )
                                .wrap_mode(egui::TextWrapMode::Wrap),
                            );
                        }
                    });
            });

        let ctx = ui.ctx().clone();
        self.do_action(action, &ctx);
    }

    fn builds_window(&mut self, ctx: &egui::Context) {
        if !self.builds_window {
            return;
        }
        let mut open = self.builds_window;
        let mut install: Option<String> = None;
        let mut use_tag: Option<String> = None;
        let installed = self.installed.clone();
        let selected = self.selected_tag.clone();
        let installing = self.install.as_ref().map(|i| i.tag.clone());
        let branch = self.branch;
        let builds = self.builds.clone();
        let error = self.builds_error.clone();
        let pending = self.pending_builds;
        let mut refresh = false;

        egui::Window::new("Builds")
            .open(&mut open)
            .default_width(440.0)
            .max_height(470.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(OBSIDIAN_850)
                    .corner_radius(CornerRadius::same(14))
                    .stroke(Stroke::new(1.0, white(30)))
                    .inner_margin(Margin::symmetric(16, 14)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(match branch {
                            Branch::Nightly => "Nightly builds",
                            Branch::Release => "Release builds",
                        })
                        .size(11.0)
                        .strong()
                        .color(SLATE_100),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if small_button(ui, "Refresh") {
                            refresh = true;
                        }
                        if pending {
                            ui.spinner();
                        }
                    });
                });
                ui.label(
                    RichText::new("From the horizon-rust/horizon GitHub releases, checksum-checked")
                        .size(9.5)
                        .color(SLATE_500),
                );
                ui.add_space(8.0);

                if let Some(err) = &error {
                    ui.label(RichText::new(err).size(10.0).color(AMBER_TEXT));
                    ui.add_space(4.0);
                }

                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for build in builds.iter().filter(|b| match branch {
                            Branch::Nightly => b.is_nightly(),
                            Branch::Release => !b.is_nightly(),
                        }) {
                            let have = installed.iter().find(|b| b.tag == build.tag);
                            egui::Frame::new()
                                .fill(glass_fill())
                                .corner_radius(CornerRadius::same(10))
                                .stroke(Stroke::new(1.0, white(14)))
                                .inner_margin(Margin::symmetric(11, 9))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(build.short_name())
                                                    .size(11.5)
                                                    .strong()
                                                    .color(SLATE_100),
                                            );
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} - {} MB - {}",
                                                    build.tag,
                                                    build.size / 1_048_576,
                                                    build.published_at.get(..10).unwrap_or("")
                                                ))
                                                .monospace()
                                                .size(9.0)
                                                .color(SLATE_500),
                                            );
                                        });
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                if installing.as_deref()
                                                    == Some(build.tag.as_str())
                                                {
                                                    ui.spinner();
                                                } else if have.is_some() {
                                                    if selected.as_deref()
                                                        == Some(build.tag.as_str())
                                                    {
                                                        ui.label(
                                                            RichText::new("In use")
                                                                .size(10.0)
                                                                .color(EMERALD_400),
                                                        );
                                                    } else if small_button(ui, "Use") {
                                                        use_tag = Some(build.tag.clone());
                                                    }
                                                } else if small_button(ui, "Install") {
                                                    install = Some(build.tag.clone());
                                                }
                                            },
                                        );
                                    });
                                });
                            ui.add_space(5.0);
                        }
                    });
            });

        self.builds_window = open;
        if refresh {
            self.start_builds(true);
        }
        if let Some(tag) = install {
            self.start_install(tag);
        }
        if let Some(tag) = use_tag {
            self.selected_tag = Some(tag.clone());
            self.config.settings.last_build = Some(tag.clone());
            self.dirty = true;
            self.start_caps(tag);
        }
    }

    #[allow(deprecated)] // A window-global Area id has no salt-based constructor.
    fn paint_toast(&mut self, ctx: &egui::Context) {
        let Some((message, left)) = self.notice.clone() else {
            return;
        };
        if left <= 0.0 {
            return;
        }
        let alpha = (left.min(1.0) * 255.0) as u8;
        egui::Area::new(egui::Id::new("toast"))
            .anchor(egui::Align2::RIGHT_BOTTOM, Vec2::new(-18.0, -18.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(23, 27, 39, alpha))
                    .corner_radius(CornerRadius::same(12))
                    .stroke(Stroke::new(
                        1.0,
                        Color32::from_rgba_unmultiplied(255, 158, 59, alpha / 2),
                    ))
                    .inner_margin(Margin::symmetric(13, 9))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            dot(ui, SUNRISE_400, 6.0);
                            ui.set_max_width(360.0);
                            ui.label(
                                RichText::new(message)
                                    .size(10.5)
                                    .color(Color32::from_rgba_unmultiplied(241, 245, 249, alpha)),
                            );
                        });
                    });
            });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        self.paint_ambient(&ctx);
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

// ---------------------------------------------------------------- widgets

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

fn glass(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(glass_fill())
        .corner_radius(CornerRadius::same(12))
        .stroke(Stroke::new(1.0, white(16)))
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

fn input_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(6, 7, 10, 210))
        .corner_radius(CornerRadius::same(8))
        .stroke(Stroke::new(1.0, white(24)))
        .inner_margin(Margin::symmetric(10, 7))
}

fn text_input(
    ui: &mut egui::Ui,
    text: &mut String,
    hint: &str,
    size: f32,
    mono: bool,
) -> egui::Response {
    let font = if mono {
        FontId::monospace(size)
    } else {
        FontId::proportional(size)
    };
    ui.add(
        egui::TextEdit::singleline(text)
            .hint_text(hint)
            .font(font)
            .desired_width(f32::INFINITY)
            .frame(input_frame()),
    )
}

fn field_label(ui: &mut egui::Ui, text: &str, right: Option<&str>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(text).size(9.0).strong().color(SLATE_500));
        if let Some(right) = right {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(right).size(9.0).color(SLATE_500));
            });
        }
    });
}

fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(9.5).strong().color(SLATE_500));
}

fn small_button(ui: &mut egui::Ui, label: &str) -> bool {
    ui.add(
        egui::Button::new(RichText::new(label).size(10.0).color(SLATE_300))
            .fill(white(8))
            .stroke(Stroke::new(1.0, white(20)))
            .corner_radius(CornerRadius::same(7))
            .min_size(Vec2::new(0.0, 26.0)),
    )
    .clicked()
}

fn toggle(ui: &mut egui::Ui, value: &mut bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(34.0, 18.0), Sense::click());
    let on = *value;
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(9),
        if on { SUNRISE_500 } else { white(30) },
    );
    let knob_x = if on {
        rect.right() - 9.0
    } else {
        rect.left() + 9.0
    };
    ui.painter().circle_filled(
        egui::pos2(knob_x, rect.center().y),
        6.5,
        if on { OBSIDIAN_950 } else { SLATE_300 },
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if response.clicked() {
        *value = !*value;
        return true;
    }
    false
}

fn segmented(ui: &mut egui::Ui, label: &str, selected: bool) -> bool {
    let button = egui::Button::new(
        RichText::new(label)
            .size(9.5)
            .strong()
            .color(if selected { OBSIDIAN_950 } else { SLATE_400 }),
    )
    .fill(if selected {
        SUNRISE_500
    } else {
        Color32::TRANSPARENT
    })
    .stroke(Stroke::NONE)
    .corner_radius(CornerRadius::same(6))
    .min_size(Vec2::new(0.0, 22.0));
    ui.add(button).clicked()
}

fn path_row(ui: &mut egui::Ui, label: &str, value: Option<String>, action: &str) -> bool {
    let mut clicked = false;
    ui.label(RichText::new(label).size(10.0).color(SLATE_400));
    ui.add_space(3.0);
    ui.horizontal(|ui| {
        let button_width = 66.0;
        let field_width = (ui.available_width() - button_width - 8.0).max(80.0);
        egui::Frame::new()
            .fill(Color32::from_rgba_unmultiplied(6, 7, 10, 210))
            .corner_radius(CornerRadius::same(8))
            .stroke(Stroke::new(1.0, white(20)))
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_width(field_width);
                ui.add(
                    egui::Label::new(
                        RichText::new(value.unwrap_or_else(|| "not set".to_string()))
                            .monospace()
                            .size(9.5)
                            .color(SLATE_300),
                    )
                    .wrap_mode(egui::TextWrapMode::Truncate),
                );
            });
        if small_button(ui, action) {
            clicked = true;
        }
    });
    clicked
}

fn progress_bar(ui: &mut egui::Ui, fraction: f32) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 8.0), Sense::hover());
    let radius = CornerRadius::same(4);
    ui.painter().rect_filled(rect, radius, OBSIDIAN_950);
    let filled = Rect::from_min_size(
        rect.min,
        Vec2::new((rect.width() * fraction.clamp(0.0, 1.0)).max(4.0), rect.height()),
    );
    ui.painter().rect_filled(filled, radius, SUNRISE_500);
}

fn dot(ui: &mut egui::Ui, color: Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    ui.painter().circle_filled(rect.center(), size * 0.5, color);
}

fn pill(ui: &mut egui::Ui, text: &str, fg: Color32, bg: Color32, dot_color: Option<Color32>) {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), FontId::proportional(9.5), fg);
    let dot_width = if dot_color.is_some() { 10.0 } else { 0.0 };
    let size = Vec2::new(galley.size().x + dot_width + 20.0, galley.size().y + 8.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(99), bg);
    ui.painter()
        .rect_stroke(rect, CornerRadius::same(99), Stroke::new(1.0, white(26)), StrokeKind::Inside);
    let mut x = rect.left() + 10.0;
    if let Some(color) = dot_color {
        ui.painter()
            .circle_filled(egui::pos2(x + 2.0, rect.center().y), 2.5, color);
        x += dot_width;
    }
    ui.painter().galley(
        egui::pos2(x, rect.center().y - galley.size().y * 0.5),
        galley,
        fg,
    );
}

fn avatar(ui: &mut egui::Ui, name: &str, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let radius = CornerRadius::same(14);
    ui.painter().rect_filled(rect, radius, OBSIDIAN_800);
    ui.painter()
        .rect_stroke(rect, radius, Stroke::new(1.5, SUNRISE_400.gamma_multiply(0.8)), StrokeKind::Inside);
    let hue = name
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    let palette = [
        SUNRISE_400,
        CORAL_500,
        Color32::from_rgb(120, 170, 255),
        Color32::from_rgb(140, 210, 170),
    ];
    let tint = palette[(hue % palette.len() as u32) as usize];
    ui.painter()
        .circle_filled(rect.center(), size * 0.32, tint.gamma_multiply(0.30));
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        name.chars().next().unwrap_or('?').to_uppercase().to_string(),
        FontId::proportional(size * 0.36),
        SLATE_100,
    );
}

fn big_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), Sense::click());
    let radius = CornerRadius::same(12);

    if enabled {
        for ring in (1..4).rev() {
            ui.painter().rect_filled(
                rect.expand(ring as f32 * 3.0),
                CornerRadius::same(12 + ring as u8 * 3),
                Color32::from_rgba_unmultiplied(255, 119, 17, 5),
            );
        }
    }
    if enabled {
        // Rounded amber and coral caps, square gradient between them: the whole
        // button reads as one gradient without spilling past its corners.
        let cap = 16.0;
        let left_cap = Rect::from_min_size(rect.min, Vec2::new(cap, rect.height()));
        let right_cap = Rect::from_min_max(
            egui::pos2(rect.right() - cap, rect.top()),
            rect.max,
        );
        ui.painter().rect_filled(
            left_cap,
            CornerRadius {
                nw: 12,
                sw: 12,
                ne: 0,
                se: 0,
            },
            SUNRISE_500,
        );
        ui.painter().rect_filled(
            right_cap,
            CornerRadius {
                nw: 0,
                sw: 0,
                ne: 12,
                se: 12,
            },
            CORAL_500,
        );
        let middle = Rect::from_min_max(
            egui::pos2(left_cap.right(), rect.top()),
            egui::pos2(right_cap.left(), rect.bottom()),
        );
        let steps = 48;
        for i in 0..steps {
            let t0 = i as f32 / steps as f32;
            let t1 = (i + 1) as f32 / steps as f32;
            let color = lerp_color(SUNRISE_500, CORAL_500, t0);
            ui.painter().rect_filled(
                Rect::from_min_max(
                    egui::pos2(middle.left() + middle.width() * t0, middle.top()),
                    egui::pos2(middle.left() + middle.width() * t1 + 0.5, middle.bottom()),
                ),
                0,
                color,
            );
        }
    } else {
        ui.painter().rect_filled(rect, radius, OBSIDIAN_800);
    }
    if response.hovered() && enabled {
        ui.painter()
            .rect_filled(rect, radius, Color32::from_rgba_unmultiplied(255, 255, 255, 14));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let color = if enabled { OBSIDIAN_950 } else { SLATE_500 };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), FontId::proportional(14.0), color);
    let total = 14.0 + 10.0 + galley.size().x;
    let start = rect.center().x - total * 0.5;
    play_icon(
        ui.painter(),
        egui::pos2(start + 7.0, rect.center().y),
        13.0,
        color,
    );
    ui.painter().galley(
        egui::pos2(start + 24.0, rect.center().y - galley.size().y * 0.5),
        galley,
        color,
    );

    response.clicked() && enabled
}

fn window_button(ui: &mut egui::Ui, kind: WindowButton) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    let hovered = response.hovered();
    if hovered {
        let fill = if kind == WindowButton::Close {
            Color32::from_rgba_unmultiplied(255, 71, 87, 40)
        } else {
            white(12)
        };
        ui.painter().rect_filled(rect, CornerRadius::same(7), fill);
    }
    let color = if hovered && kind == WindowButton::Close {
        CORAL_500
    } else if hovered {
        SLATE_100
    } else {
        SLATE_400
    };
    let c = rect.center();
    let r = 4.0;
    let stroke = Stroke::new(1.2, color);
    match kind {
        WindowButton::Minimize => {
            ui.painter()
                .line_segment([egui::pos2(c.x - r, c.y), egui::pos2(c.x + r, c.y)], stroke);
        }
        WindowButton::Maximize => {
            ui.painter().rect_stroke(
                Rect::from_center_size(c, Vec2::splat(r * 1.8)),
                CornerRadius::same(1),
                stroke,
                StrokeKind::Inside,
            );
        }
        WindowButton::Close => {
            ui.painter().line_segment(
                [egui::pos2(c.x - r, c.y - r), egui::pos2(c.x + r, c.y + r)],
                stroke,
            );
            ui.painter().line_segment(
                [egui::pos2(c.x - r, c.y + r), egui::pos2(c.x + r, c.y - r)],
                stroke,
            );
        }
    }
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

fn logo(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let radius = CornerRadius::same(9);
    ui.painter().rect_filled(rect, radius, SUNRISE_500);
    let inner = rect.shrink(1.5);
    ui.painter().rect_filled(inner, CornerRadius::same(8), OBSIDIAN_900);
    sun_icon(ui.painter(), rect.center(), size * 0.24, SUNRISE_400);
}

fn sun_icon(painter: &egui::Painter, center: egui::Pos2, radius: f32, color: Color32) {
    for i in 0..8 {
        let angle = i as f32 * std::f32::consts::TAU / 8.0;
        let dir = egui::vec2(angle.cos(), angle.sin());
        painter.line_segment(
            [
                center + dir * (radius * 1.5),
                center + dir * (radius * 2.1),
            ],
            Stroke::new(1.2, color),
        );
    }
    painter.circle_stroke(center, radius, Stroke::new(1.6, color));
}

fn play_icon(painter: &egui::Painter, center: egui::Pos2, size: f32, color: Color32) {
    let half = size * 0.5;
    painter.add(Shape::convex_polygon(
        vec![
            egui::pos2(center.x - half * 0.6, center.y - half),
            egui::pos2(center.x + half, center.y),
            egui::pos2(center.x - half * 0.6, center.y + half),
        ],
        color,
        Stroke::NONE,
    ));
}

fn check_icon(ui: &mut egui::Ui, color: Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let c = rect.center();
    let stroke = Stroke::new(1.5, color);
    ui.painter().line_segment(
        [
            egui::pos2(c.x - size * 0.42, c.y),
            egui::pos2(c.x - size * 0.1, c.y + size * 0.32),
        ],
        stroke,
    );
    ui.painter().line_segment(
        [
            egui::pos2(c.x - size * 0.1, c.y + size * 0.32),
            egui::pos2(c.x + size * 0.45, c.y - size * 0.34),
        ],
        stroke,
    );
}

fn terminal_icon(ui: &mut egui::Ui, color: Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let c = rect.center();
    let stroke = Stroke::new(1.2, color);
    ui.painter().line_segment(
        [
            egui::pos2(c.x - size * 0.35, c.y - size * 0.25),
            egui::pos2(c.x, c.y),
        ],
        stroke,
    );
    ui.painter().line_segment(
        [
            egui::pos2(c.x, c.y),
            egui::pos2(c.x - size * 0.35, c.y + size * 0.25),
        ],
        stroke,
    );
    ui.painter().line_segment(
        [
            egui::pos2(c.x + size * 0.1, c.y + size * 0.3),
            egui::pos2(c.x + size * 0.4, c.y + size * 0.3),
        ],
        stroke,
    );
}

fn draw_icon(painter: &egui::Painter, icon: Icon, center: egui::Pos2, size: f32, color: Color32) {
    let stroke = Stroke::new(1.1, color);
    let s = size * 0.5;
    match icon {
        Icon::Sliders => {
            for (index, offset) in [-0.5f32, 0.1, 0.6].iter().enumerate() {
                let y = center.y + s * offset;
                painter.line_segment(
                    [egui::pos2(center.x - s, y), egui::pos2(center.x + s, y)],
                    stroke,
                );
                let knob = center.x - s + size * (0.25 + index as f32 * 0.22);
                painter.circle_filled(egui::pos2(knob, y), 1.6, color);
            }
        }
        Icon::Play => {
            painter.add(Shape::convex_polygon(
                vec![
                    egui::pos2(center.x - s * 0.6, center.y - s),
                    egui::pos2(center.x + s, center.y),
                    egui::pos2(center.x - s * 0.6, center.y + s),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Terminal => {
            painter.line_segment(
                [
                    egui::pos2(center.x - s * 0.8, center.y - s * 0.6),
                    egui::pos2(center.x - s * 0.1, center.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(center.x - s * 0.1, center.y),
                    egui::pos2(center.x - s * 0.8, center.y + s * 0.6),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(center.x, center.y + s * 0.7),
                    egui::pos2(center.x + s * 0.9, center.y + s * 0.7),
                ],
                stroke,
            );
        }
    }
}

fn glow(painter: &egui::Painter, center: egui::Pos2, radius: f32, color: Color32, alpha: u8) {
    let rings = 10;
    for i in (1..=rings).rev() {
        let r = radius * i as f32 / rings as f32;
        painter.circle_filled(
            center,
            r,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha / 6),
        );
    }
}

fn paint_sky(painter: &egui::Painter, rect: Rect) {
    let clip = painter.with_clip_rect(rect.shrink(1.0));
    let sky = [
        Color32::from_rgb(17, 20, 29),
        Color32::from_rgb(31, 26, 39),
        Color32::from_rgb(58, 34, 40),
        Color32::from_rgb(104, 52, 34),
        Color32::from_rgb(158, 80, 36),
        Color32::from_rgb(196, 110, 48),
    ];
    let bands = 72;
    for i in 0..bands {
        let t0 = i as f32 / bands as f32;
        let t1 = (i + 1) as f32 / bands as f32;
        let color = vertical_color(&sky, t0);
        clip.rect_filled(
            Rect::from_min_max(
                egui::pos2(rect.left(), rect.top() + rect.height() * t0),
                egui::pos2(rect.right(), rect.top() + rect.height() * t1 + 0.5),
            ),
            0,
            color,
        );
    }

    // Sun and its haze.
    let sun = egui::pos2(
        rect.left() + rect.width() * 0.72,
        rect.top() + rect.height() * 0.52,
    );
    for i in (1..=8).rev() {
        clip.circle_filled(
            sun,
            24.0 + i as f32 * 11.0,
            Color32::from_rgba_unmultiplied(255, 150, 60, 6),
        );
    }
    clip.circle_filled(sun, 30.0, Color32::from_rgb(255, 196, 120));
    clip.circle_filled(sun, 24.0, Color32::from_rgb(255, 232, 182));

    // Distant hills.
    let bottom = rect.bottom();
    clip.add(Shape::convex_polygon(
        vec![
            egui::pos2(rect.left() - 10.0, bottom),
            egui::pos2(rect.left() - 10.0, bottom - 42.0),
            egui::pos2(rect.left() + rect.width() * 0.3, bottom - 74.0),
            egui::pos2(rect.left() + rect.width() * 0.62, bottom),
        ],
        Color32::from_rgb(13, 15, 22),
        Stroke::NONE,
    ));
    clip.add(Shape::convex_polygon(
        vec![
            egui::pos2(rect.left() + rect.width() * 0.42, bottom),
            egui::pos2(rect.left() + rect.width() * 0.72, bottom - 58.0),
            egui::pos2(rect.right() + 10.0, bottom - 30.0),
            egui::pos2(rect.right() + 10.0, bottom),
        ],
        Color32::from_rgb(10, 11, 17),
        Stroke::NONE,
    ));

    // Dark scrim so the name stays readable.
    let scrim = 26;
    for i in 0..scrim {
        let t = i as f32 / scrim as f32;
        let alpha = (t * t * 235.0) as u8;
        clip.rect_filled(
            Rect::from_min_max(
                egui::pos2(rect.left(), rect.top() + rect.height() * 0.45 + t * rect.height() * 0.55),
                egui::pos2(
                    rect.right(),
                    rect.top() + rect.height() * 0.45 + (t + 1.0 / scrim as f32) * rect.height() * 0.55,
                ),
            ),
            0,
            Color32::from_rgba_unmultiplied(9, 11, 17, alpha),
        );
    }
}

fn vertical_color(stops: &[Color32], t: f32) -> Color32 {
    if stops.len() < 2 {
        return stops.first().copied().unwrap_or(OBSIDIAN_850);
    }
    let t = t.clamp(0.0, 1.0) * (stops.len() - 1) as f32;
    let index = (t.floor() as usize).min(stops.len() - 2);
    lerp_color(stops[index], stops[index + 1], t - index as f32)
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}
