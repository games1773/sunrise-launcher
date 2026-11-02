//! The transient notice in the bottom-right corner.

use eframe::egui;
use egui::{Color32, CornerRadius, Margin, RichText, Stroke, Vec2};

use crate::app::App;
use crate::theme::*;
use crate::widgets::dot;

impl App {
    #[allow(deprecated)] // A window-global Area id has no salt-based constructor.
    pub(crate) fn paint_toast(&mut self, ctx: &egui::Context) {
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
                    .corner_radius(CornerRadius::ZERO)
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
