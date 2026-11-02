//! The window's layout: the left column, the management panel and its tabs.

mod account;
mod builds;
mod console;
mod hero;
mod launch;
mod play;
mod setup;
mod title_bar;
mod toast;

use eframe::egui;
use egui::{Color32, FontId, Margin, Rect, Sense, Stroke, Vec2};

use crate::app::{App, Icon, Tab};
use crate::theme::*;
use crate::widgets::draw_icon;

impl App {
    pub(crate) fn left_column(&mut self, ui: &mut egui::Ui) {
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

    pub(crate) fn right_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("management")
            .exact_size(424.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(15, 17, 25, 240))
                    .inner_margin(Margin::ZERO),
            )
            .show(ui, |ui| {
                self.tabs(ui);
                if self.tab == Tab::Console {
                    // The console keeps its own scroll area and fills the panel. The
                    // panel's content Ui shrink-wraps vertically, so the room left
                    // comes from the window rather than from available_height().
                    ui.add_space(14.0);
                    let body =
                        (ui.ctx().viewport_rect().bottom() - ui.cursor().top() - 18.0).max(240.0);
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

    pub(crate) fn tabs(&mut self, ui: &mut egui::Ui) {
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
            let response =
                ui.interact(tab_rect, ui.scope_id().with(("tab", index)), Sense::click());
            let active = self.tab == *tab;
            if active {
                ui.painter()
                    .rect_filled(tab_rect, 0, SUNRISE_500.gamma_multiply(0.10));
                ui.painter().rect_filled(
                    Rect::from_min_size(
                        egui::pos2(tab_rect.left(), tab_rect.bottom() - 2.0),
                        Vec2::new(width, 2.0),
                    ),
                    0,
                    SUNRISE_500,
                );
            } else if response.hovered() {
                ui.painter().rect_filled(
                    tab_rect,
                    0,
                    Color32::from_rgba_unmultiplied(255, 255, 255, 6),
                );
            }
            let color = if active { SLATE_100 } else { SLATE_400 };
            let icon = match tab {
                Tab::Setup => Icon::Sliders,
                Tab::Launch => Icon::Play,
                Tab::Console => Icon::Terminal,
            };
            let galley = ui.painter().layout_no_wrap(
                tab.label().to_string(),
                FontId::proportional(11.5),
                color,
            );
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
}
