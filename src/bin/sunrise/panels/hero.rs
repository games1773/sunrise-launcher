//! The hero banner: the painted scene, the build chips, the account and the
//! playtime.

use eframe::egui;
use egui::{
    Align, Color32, CornerRadius, Layout, Rect, RichText, Sense, Stroke, StrokeKind, UiBuilder,
    Vec2,
};

use crate::app::{App, Busy};
use crate::theme::*;
use crate::widgets::{avatar, dot, pill};

impl App {
    pub(crate) fn hero(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 262.0), Sense::hover());
        let radius = CornerRadius::ZERO;

        ui.painter().rect_filled(rect, radius, OBSIDIAN_850);
        paint_sky(ui.painter(), rect);
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, white(28)),
            StrokeKind::Inside,
        );

        let inner = rect.shrink2(Vec2::new(18.0, 15.0));
        let top_rect = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 24.0));
        let bottom_rect =
            Rect::from_min_max(egui::pos2(inner.left(), inner.bottom() - 62.0), inner.max);

        let mut top = ui.new_child(
            UiBuilder::new()
                .max_rect(top_rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
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

        let mut bottom = ui.new_child(
            UiBuilder::new()
                .max_rect(bottom_rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        bottom.horizontal(|ui| {
            let online = self.session().is_some();
            let name = self
                .session()
                .map(|s| s.name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| self.config.settings.player_name.trim().to_string());
            avatar(ui, &name, 58.0);
            ui.add_space(2.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new(&name).size(21.0).strong().color(SLATE_100));
                let (label, dot_color) = if online {
                    ("Microsoft account", EMERALD_400)
                } else if self.online() {
                    ("Not signed in", AMBER_TEXT)
                } else {
                    ("Offline account", SLATE_500)
                };
                ui.horizontal(|ui| {
                    dot(ui, dot_color, 5.0);
                    ui.label(RichText::new(label).size(11.0).color(SLATE_400));
                });
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(RichText::new("PLAYTIME").size(9.0).color(SLATE_500));
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
}
