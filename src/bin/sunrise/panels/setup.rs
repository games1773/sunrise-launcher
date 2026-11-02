//! The Setup tab: build, game files, directories and options.

use eframe::egui;
use egui::{Align, Layout, RichText};

use crate::app::{Action, App, Branch};
use crate::theme::*;
use crate::widgets::{glass, path_row, section_label, segmented, small_button, toggle};

impl App {
    pub(crate) fn setup_tab(&mut self, ui: &mut egui::Ui) {
        let mut action = Action::None;
        let setup = self.setup();
        let build = self.current_build().cloned();
        let detection = self.detection.clone();
        let server_jar = self.server_jar.clone();
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
                    ui.label(
                        RichText::new("Game files")
                            .size(11.5)
                            .strong()
                            .color(SLATE_100),
                    );
                    let (text, color) = if setup.prepared {
                        ("Prepared & synchronized".to_string(), EMERALD_400)
                    } else if setup.build_installed {
                        ("Not prepared yet".to_string(), AMBER_TEXT)
                    } else {
                        ("No build installed".to_string(), SLATE_400)
                    };
                    ui.label(
                        RichText::new(format!("Status: {text}"))
                            .size(10.0)
                            .color(color),
                    );
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
                    ui.label(
                        RichText::new("Horizon build")
                            .size(11.5)
                            .strong()
                            .color(SLATE_100),
                    );
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
                    if small_button(
                        ui,
                        if setup.build_installed {
                            "Change"
                        } else {
                            "Install"
                        },
                    ) {
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
        // No file to choose any more: the one jar the preparer needs comes from
        // Mojang, checked against the pin in the build's research-lock.json.
        ui.add_space(8.0);
        glass(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(RichText::new("Server jar").size(11.0).color(SLATE_300));
                    let (text, color) = match server_jar.as_ref().filter(|p| p.is_file()) {
                        Some(_) => ("Downloaded from Mojang, SHA-1 checked", EMERALD_400),
                        None => ("Fetched on the first prepare", SLATE_500),
                    };
                    ui.label(RichText::new(text).size(9.5).color(color));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new("26.3").monospace().size(9.5).color(SLATE_500));
                });
            });
        });
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
}
