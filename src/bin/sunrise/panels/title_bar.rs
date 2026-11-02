//! The custom title bar: brand, build badge and window buttons.

use eframe::egui;
use egui::{Align, Color32, Layout, Margin, RichText, Sense};

use crate::app::{App, WindowButton};
use crate::theme::*;
use crate::widgets::{dot, logo, window_button};

impl App {
    pub(crate) fn title_bar(&mut self, ui: &mut egui::Ui) {
        let mut window_action: Option<WindowButton> = None;
        let frame = egui::Frame::new()
            .fill(Color32::from_rgba_unmultiplied(6, 7, 10, 240))
            .inner_margin(Margin::symmetric(16, 11));

        egui::Panel::top("titlebar")
            .exact_size(56.0)
            .frame(frame)
            .show(ui, |ui| {
                let bar = ui.max_rect();
                let drag = ui.interact(
                    bar,
                    ui.scope_id().with("titlebar-drag"),
                    Sense::click_and_drag(),
                );
                if drag.drag_started() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag.double_clicked() {
                    self.maximized = !self.maximized;
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(self.maximized));
                }

                ui.horizontal_centered(|ui| {
                    // 32 keeps the 16-pixel mark on whole pixels.
                    logo(ui, 32.0);
                    ui.add_space(9.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("SUNRISE")
                                .size(15.0)
                                .strong()
                                .color(SLATE_100),
                        );
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
                        // Right to left: buttons, commit, date, status dot.
                        if let Some((head, commit)) = self.build_badges() {
                            if let Some(commit) = commit {
                                ui.label(
                                    RichText::new(format!("commit: {commit}"))
                                        .monospace()
                                        .size(10.0)
                                        .color(SLATE_500),
                                );
                                ui.add_space(8.0);
                            }
                            ui.label(
                                RichText::new(head.trim_start_matches("Nightly "))
                                    .monospace()
                                    .size(10.0)
                                    .color(SLATE_300),
                            );
                            dot(ui, EMERALD_400, 7.0);
                        } else if let Some(build) = self.current_build() {
                            ui.label(
                                RichText::new(&build.tag)
                                    .monospace()
                                    .size(10.0)
                                    .color(SLATE_300),
                            );
                            dot(ui, SLATE_500, 7.0);
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
}
