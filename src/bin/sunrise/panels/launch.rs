//! The Launch tab: render distance, window, world and extra arguments.

use eframe::egui;
use egui::{Align, FontId, Layout, RichText};

use crate::app::App;
use crate::theme::*;
use crate::widgets::{glass, input_frame, section_label, text_input, toggle};

impl App {
    pub(crate) fn launch_tab(&mut self, ui: &mut egui::Ui) {
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
                .add(egui::Slider::new(&mut instance.render_distance, 2..=32).show_value(false))
                .changed()
            {
                self.dirty = true;
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new("2").monospace().size(9.0).color(SLATE_500));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new("32").monospace().size(9.0).color(SLATE_500));
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
            let presets: [(u32, u32); 4] = [(1280, 720), (1600, 900), (1920, 1080), (2560, 1440)];
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
}
