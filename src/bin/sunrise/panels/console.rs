//! The Console tab: the launcher's own log, filling the panel.

use eframe::egui;
use egui::{Align, CornerRadius, Layout, Margin, RichText, Stroke};

use crate::app::{Action, App, Level};
use crate::theme::*;
use crate::widgets::small_button;

impl App {
    pub(crate) fn console_tab(&mut self, ui: &mut egui::Ui, body: f32) {
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
            .corner_radius(CornerRadius::ZERO)
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
                                    RichText::new(&line.text)
                                        .monospace()
                                        .size(10.0)
                                        .color(color),
                                )
                                .wrap_mode(egui::TextWrapMode::Wrap),
                            );
                        }
                    });
            });

        let ctx = ui.ctx().clone();
        self.do_action(action, &ctx);
    }
}
