//! The play card and the newest-log strip beneath it.

use eframe::egui;
use egui::{Align, Color32, CornerRadius, Layout, Margin, RichText, Stroke};

use sunrise::config::valid_player_name;

use crate::app::{Action, App, Busy, Tab};
use crate::theme::*;
use crate::widgets::{big_button, check_icon, dot, progress_bar, small_button, terminal_icon};

impl App {
    pub(crate) fn play_panel(&mut self, ui: &mut egui::Ui) {
        let setup = self.setup();
        let build = self.current_build().cloned();
        let mut action = Action::None;

        let frame = egui::Frame::new()
            .fill(glass_fill())
            .corner_radius(CornerRadius::ZERO)
            .stroke(Stroke::new(1.0, SUNRISE_500.gamma_multiply(0.35)))
            .inner_margin(Margin::symmetric(16, 14));

        frame.show(ui, |ui| {
            // Signing in takes over the launch card: the code is the biggest
            // thing on screen, in the card the eye is already on, and it stays
            // there until the session lands, Cancel is pressed, or an error.
            if let Some(sign_in) = self.sign_in.clone() {
                let waiting = !sign_in.user_code.is_empty();
                ui.horizontal(|ui| {
                    dot(ui, SUNRISE_400, 7.0);
                    ui.label(
                        RichText::new("Signing in with Microsoft")
                            .size(12.0)
                            .color(SLATE_300),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if small_button(ui, "Cancel") {
                            action = Action::CancelSignIn;
                        }
                        if waiting && small_button(ui, "Copy code") {
                            ui.ctx().copy_text(sign_in.user_code.clone());
                        }
                    });
                });
                ui.add_space(6.0);
                if waiting {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&sign_in.user_code)
                                .monospace()
                                .size(34.0)
                                .strong()
                                .color(SUNRISE_400),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("at microsoft.com/link")
                                .size(10.5)
                                .color(SLATE_500),
                        );
                    });
                    ui.add_space(6.0);
                    // One button does both: copy the code, then open the page.
                    if big_button(ui, "OPEN MICROSOFT.COM/LINK", true) {
                        ui.ctx().copy_text(sign_in.user_code.clone());
                        let _ = open::that(&sign_in.url);
                    }
                } else {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            RichText::new("Asking Microsoft for a code...")
                                .size(11.0)
                                .color(SLATE_400),
                        );
                    });
                }
                return;
            }

            let launchable =
                setup.ready() && (!self.online() || self.config.settings.session.is_some());
            ui.horizontal(|ui| {
                if launchable {
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
                } else if self.online() && self.config.settings.session.is_none() {
                    "Sign in, or switch to Offline".to_string()
                } else {
                    "Ready to launch 26.3".to_string()
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
            } else if let Some(fetch) = &self.jar_fetch {
                ui.add_space(8.0);
                let fraction = if fetch.total > 0 {
                    fetch.done as f32 / fetch.total as f32
                } else {
                    0.0
                };
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.add(
                        egui::Label::new(
                            RichText::new(&fetch.message)
                                .monospace()
                                .size(10.0)
                                .color(SLATE_400),
                        )
                        .wrap_mode(egui::TextWrapMode::Truncate),
                    );
                    if fetch.total > 0 {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("{}%", (fraction * 100.0) as u32))
                                    .monospace()
                                    .size(10.0)
                                    .color(SLATE_400),
                            );
                        });
                    }
                });
                if fetch.total > 0 {
                    progress_bar(ui, fraction);
                }
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
                && self.jar_fetch.is_none()
                && if self.online() {
                    self.config.settings.session.is_some()
                } else {
                    valid_player_name(&self.config.settings.player_name)
                };
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

    pub(crate) fn log_strip(&mut self, ui: &mut egui::Ui) {
        let mut action = Action::None;
        let frame = egui::Frame::new()
            .fill(glass_fill())
            .corner_radius(CornerRadius::ZERO)
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
                    egui::Label::new(RichText::new(last).monospace().size(10.0).color(SLATE_400))
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
}
