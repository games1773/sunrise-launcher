//! The account and server row: an offline name, or the optional Microsoft
//! account — signed in in the browser, imported from a session file, and the
//! server to join at start.

use eframe::egui;
use egui::{Align, Layout, RichText};
use sunrise::config::valid_player_name;

use crate::app::{Action, App};
use crate::theme::*;
use crate::widgets::{dot, field_label, glass, segmented, small_button, text_input};

impl App {
    pub(crate) fn selectors(&mut self, ui: &mut egui::Ui) {
        let mut action = Action::None;
        ui.columns(2, |cols| {
            // The account: an offline name, or the optional Microsoft session.
            glass(&mut cols[0], |ui| {
                let online = self.online();
                ui.horizontal(|ui| {
                    ui.label(RichText::new("ACCOUNT").size(9.0).strong().color(SLATE_500));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if segmented(ui, "Online", online) && !online {
                            action = Action::UseOnlineAccount;
                        }
                        if segmented(ui, "Offline", !online) && online {
                            action = Action::UseOfflineAccount;
                        }
                    });
                });
                ui.add_space(4.0);

                if !online {
                    let mut name = self.config.settings.player_name.clone();
                    let response = text_input(ui, &mut name, "Player", 13.0, false);
                    if response.changed() {
                        self.config.settings.player_name = name.clone();
                        self.dirty = true;
                    }
                    let valid = valid_player_name(&name);
                    ui.label(
                        RichText::new(if valid {
                            "3-16 letters, digits, _"
                        } else {
                            "Needs 3-16 letters or digits"
                        })
                        .size(9.0)
                        .color(if valid { SLATE_500 } else { AMBER_TEXT }),
                    );
                } else if let Some(sign_in) = self.sign_in.clone() {
                    // The same code as the launch card, so it is never lost.
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            RichText::new("Waiting for the browser...")
                                .size(10.5)
                                .color(SLATE_400),
                        );
                    });
                    if sign_in.user_code.is_empty() {
                        ui.label(
                            RichText::new("Asking Microsoft for a code")
                                .size(9.0)
                                .color(SLATE_500),
                        );
                    } else {
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&sign_in.user_code)
                                        .monospace()
                                        .size(15.0)
                                        .strong()
                                        .color(SUNRISE_400),
                                )
                                .wrap_mode(egui::TextWrapMode::Truncate),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if small_button(ui, "Copy") {
                                    ui.ctx().copy_text(sign_in.user_code.clone());
                                }
                            });
                        });
                        ui.label(
                            RichText::new("The launch card has it too")
                                .size(9.0)
                                .color(SLATE_500),
                        );
                    }
                } else if let Some(session) = self.config.settings.session.clone() {
                    ui.horizontal(|ui| {
                        dot(ui, EMERALD_400, 5.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(&session.name)
                                    .size(12.5)
                                    .strong()
                                    .color(SLATE_100),
                            )
                            .wrap_mode(egui::TextWrapMode::Truncate),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if small_button(ui, "Sign out") {
                                action = Action::SignOut;
                            }
                        });
                    });
                    let hint = if session.renewable() {
                        "Online servers accept this account; it renews itself"
                    } else {
                        "Online servers accept this account"
                    };
                    ui.label(RichText::new(hint).size(9.0).color(SLATE_500));
                } else {
                    // Signed out: sign in here, or bring a session from elsewhere.
                    ui.horizontal(|ui| {
                        if small_button(ui, "Sign in") {
                            action = Action::SignIn;
                        }
                        if small_button(ui, "Import file") {
                            action = Action::ImportSession;
                        }
                        if small_button(ui, "Paste") {
                            action = Action::PasteSession;
                        }
                        if self.session_check {
                            ui.spinner();
                        }
                    });
                    ui.label(
                        RichText::new(if self.session_check {
                            "Checking with Mojang..."
                        } else {
                            "Sign in, or import a session JSON"
                        })
                        .size(9.0)
                        .color(SLATE_500),
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
                    let hint = if server.trim().is_empty() {
                        "Not joining a server"
                    } else if self.session().is_some() {
                        "Joining at start with your session"
                    } else {
                        "Offline-mode servers only"
                    };
                    ui.label(RichText::new(hint).size(9.5).color(SLATE_500));
                }
            });
        });
        let ctx = ui.ctx().clone();
        self.do_action(action, &ctx);
    }
}
