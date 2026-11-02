//! The Builds window: published builds, what is installed, and installing one.

use eframe::egui;
use egui::{Align, CornerRadius, Layout, Margin, RichText, Stroke};

use crate::app::{App, Branch};
use crate::theme::*;
use crate::widgets::small_button;

impl App {
    pub(crate) fn builds_window(&mut self, ctx: &egui::Context) {
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
                    .corner_radius(CornerRadius::ZERO)
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
                    RichText::new(
                        "From the horizon-rust/horizon GitHub releases, checksum-checked",
                    )
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
                                .corner_radius(CornerRadius::ZERO)
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
                                                if installing.as_deref() == Some(build.tag.as_str())
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
            self.refresh_server_jar();
            self.start_caps(tag);
        }
    }
}
