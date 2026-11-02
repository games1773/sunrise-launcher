//! The painted controls the panels are built from: cards, inputs, buttons,
//! toggles, tags and icons. Every edge is square and bevelled the way the
//! game's own interface is; nothing here knows the app's state.

use eframe::egui;
use egui::{
    Align, Color32, CornerRadius, FontId, Layout, Margin, Rect, RichText, Sense, Shape, Stroke,
    StrokeKind, Vec2,
};

use crate::app::{Icon, WindowButton};
use crate::logo;
use crate::theme::*;

/// The light edge along a block's top and left, and the dark one opposite it.
fn bevel_light() -> Color32 {
    Color32::from_rgba_unmultiplied(255, 255, 255, 40)
}

fn bevel_dark() -> Color32 {
    Color32::from_rgba_unmultiplied(0, 0, 0, 140)
}

/// A square block with the 1px light-top, dark-bottom bevel a Minecraft button
/// has. Every custom control goes through here.
pub(crate) fn block(painter: &egui::Painter, rect: Rect, fill: Color32, outline: Color32) {
    painter.rect_filled(rect, CornerRadius::ZERO, fill);
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, outline),
        StrokeKind::Inside,
    );
    let inner = rect.shrink(1.0);
    if inner.width() > 3.0 && inner.height() > 3.0 {
        painter.hline(
            inner.x_range(),
            inner.top() + 0.5,
            Stroke::new(1.0, bevel_light()),
        );
        painter.vline(
            inner.left() + 0.5,
            inner.y_range(),
            Stroke::new(1.0, bevel_light()),
        );
        painter.hline(
            inner.x_range(),
            inner.bottom() - 0.5,
            Stroke::new(1.0, bevel_dark()),
        );
        painter.vline(
            inner.right() - 0.5,
            inner.y_range(),
            Stroke::new(1.0, bevel_dark()),
        );
    }
}

pub(crate) fn glass(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let response = egui::Frame::new()
        .fill(glass_fill())
        .corner_radius(CornerRadius::ZERO)
        .stroke(Stroke::new(1.0, white(16)))
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
    // One pixel of light across the top, so a card reads as raised.
    let rect = response.response.rect;
    ui.painter().hline(
        rect.x_range(),
        rect.top() + 0.5,
        Stroke::new(1.0, white(22)),
    );
}

pub(crate) fn input_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(6, 7, 10, 210))
        .corner_radius(CornerRadius::ZERO)
        .stroke(Stroke::new(1.0, white(24)))
        .inner_margin(Margin::symmetric(10, 7))
}

pub(crate) fn text_input(
    ui: &mut egui::Ui,
    text: &mut String,
    hint: &str,
    size: f32,
    mono: bool,
) -> egui::Response {
    let font = if mono {
        FontId::monospace(size)
    } else {
        FontId::proportional(size)
    };
    let response = ui.add(
        egui::TextEdit::singleline(text)
            .hint_text(hint)
            .font(font)
            .desired_width(f32::INFINITY)
            .frame(input_frame()),
    );
    // The reference lights the field it is typing in; a custom frame has to
    // draw that ring itself.
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect,
            CornerRadius::ZERO,
            Stroke::new(1.0, SUNRISE_400.gamma_multiply(0.65)),
            StrokeKind::Inside,
        );
    }
    response
}

pub(crate) fn field_label(ui: &mut egui::Ui, text: &str, right: Option<&str>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(text).size(9.5).strong().color(SLATE_500));
        if let Some(right) = right {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(right).size(9.5).color(SLATE_500));
            });
        }
    });
}

pub(crate) fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(10.0).strong().color(SLATE_500));
}

pub(crate) fn small_button(ui: &mut egui::Ui, label: &str) -> bool {
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), FontId::proportional(10.0), SLATE_300);
    let size = Vec2::new((galley.size().x + 18.0).max(50.0), 24.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = if response.is_pointer_button_down_on() {
        white(4)
    } else if response.hovered() {
        white(16)
    } else {
        white(8)
    };
    block(ui.painter(), rect, fill, white(26));
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, SLATE_300);
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

pub(crate) fn toggle(ui: &mut egui::Ui, value: &mut bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(38.0, 20.0), Sense::click());
    let on = *value;
    let track = if on { SUNRISE_500 } else { OBSIDIAN_950 };
    block(
        ui.painter(),
        rect,
        track,
        if on { SUNRISE_300 } else { white(28) },
    );
    // A square knob that slides between two stops, like a lever in the game.
    let knob = Rect::from_min_size(
        egui::pos2(
            if on {
                rect.right() - 17.0
            } else {
                rect.left() + 3.0
            },
            rect.top() + 3.0,
        ),
        Vec2::new(14.0, 14.0),
    );
    ui.painter().rect_filled(
        knob,
        CornerRadius::ZERO,
        if on { OBSIDIAN_950 } else { SLATE_400 },
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if response.clicked() {
        *value = !*value;
        return true;
    }
    false
}

pub(crate) fn segmented(ui: &mut egui::Ui, label: &str, selected: bool) -> bool {
    let color = if selected { OBSIDIAN_950 } else { SLATE_400 };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), FontId::proportional(10.0), color);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(galley.size().x + 18.0, 23.0), Sense::click());
    if selected {
        block(ui.painter(), rect, SUNRISE_500, SUNRISE_300);
    } else if response.hovered() {
        block(ui.painter(), rect, white(12), white(22));
    }
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, color);
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

pub(crate) fn path_row(
    ui: &mut egui::Ui,
    label: &str,
    value: Option<String>,
    action: &str,
) -> bool {
    let mut clicked = false;
    ui.label(RichText::new(label).size(10.5).color(SLATE_400));
    ui.add_space(3.0);
    ui.horizontal(|ui| {
        let button_width = 58.0;
        // The frame's own margins and stroke sit outside the width it is given,
        // so leave room for them or the button hangs off the panel.
        let field_width = (ui.available_width() - button_width - 8.0 - 22.0).max(80.0);
        egui::Frame::new()
            .fill(Color32::from_rgba_unmultiplied(6, 7, 10, 210))
            .corner_radius(CornerRadius::ZERO)
            .stroke(Stroke::new(1.0, white(20)))
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_width(field_width);
                ui.add(
                    egui::Label::new(
                        RichText::new(value.unwrap_or_else(|| "not set".to_string()))
                            .monospace()
                            .size(10.0)
                            .color(SLATE_300),
                    )
                    .wrap_mode(egui::TextWrapMode::Truncate),
                );
            });
        if small_button(ui, action) {
            clicked = true;
        }
    });
    clicked
}

pub(crate) fn progress_bar(ui: &mut egui::Ui, fraction: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 10.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, OBSIDIAN_950);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, white(18)),
        StrokeKind::Inside,
    );
    let width = (rect.width() * fraction.clamp(0.0, 1.0)).max(3.0);
    let filled = Rect::from_min_size(rect.min, Vec2::new(width, rect.height()));
    ui.painter()
        .rect_filled(filled, CornerRadius::ZERO, SUNRISE_500);
    ui.painter().hline(
        filled.x_range(),
        filled.top() + 1.5,
        Stroke::new(1.0, SUNRISE_300),
    );
}

pub(crate) fn dot(ui: &mut egui::Ui, color: Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, color);
}

pub(crate) fn pill(
    ui: &mut egui::Ui,
    text: &str,
    fg: Color32,
    bg: Color32,
    dot_color: Option<Color32>,
) {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), FontId::proportional(10.0), fg);
    let dot_width = if dot_color.is_some() { 9.0 } else { 0.0 };
    let size = Vec2::new(galley.size().x + dot_width + 16.0, galley.size().y + 9.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, bg);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, white(26)),
        StrokeKind::Inside,
    );
    let mut x = rect.left() + 8.0;
    if let Some(color) = dot_color {
        let top = rect.center().y - 2.0;
        ui.painter().rect_filled(
            Rect::from_min_size(egui::pos2(x, top), Vec2::splat(4.0)),
            CornerRadius::ZERO,
            color,
        );
        x += dot_width;
    }
    ui.painter().galley(
        egui::pos2(x, rect.center().y - galley.size().y * 0.5),
        galley,
        fg,
    );
}

/// A blocky player head, the way Minecraft draws one, in a square frame.
pub(crate) fn avatar(ui: &mut egui::Ui, _name: &str, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, OBSIDIAN_800);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(2.0, SUNRISE_400.gamma_multiply(0.8)),
        StrokeKind::Inside,
    );

    let clip = ui.painter().with_clip_rect(rect.shrink(3.0));
    let inner = rect.shrink(3.0);
    let cell = inner.width() / 8.0;
    const HAIR: Color32 = Color32::from_rgb(59, 36, 22);
    const SKIN: Color32 = Color32::from_rgb(201, 143, 99);
    const SHADE: Color32 = Color32::from_rgb(176, 121, 80);
    const EYE: Color32 = Color32::from_rgb(243, 240, 235);
    const MOUTH: Color32 = Color32::from_rgb(122, 70, 48);

    for row in 0..8u32 {
        for col in 0..8u32 {
            let color = match (row, col) {
                (0, _) | (1, _) => HAIR,
                (2, 0) | (2, 7) | (3, 0) | (3, 7) | (4, 0) | (4, 7) => HAIR,
                (3, 1) | (3, 6) => EYE,
                (3, 2) | (3, 5) => Color32::from_rgb(74, 90, 132),
                (5, 3) | (5, 4) => MOUTH,
                (6, 4) | (7, 4) => SHADE,
                _ => SKIN,
            };
            let pos = egui::pos2(
                inner.left() + col as f32 * cell,
                inner.top() + row as f32 * cell,
            );
            clip.rect_filled(
                Rect::from_min_size(pos, Vec2::splat(cell + 0.5)),
                CornerRadius::ZERO,
                color,
            );
        }
    }
}

pub(crate) fn big_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 54.0), Sense::click());
    let pressed = response.is_pointer_button_down_on();

    if enabled {
        let fill = if pressed {
            SUNRISE_500
        } else if response.hovered() {
            Color32::from_rgb(255, 138, 38)
        } else {
            SUNRISE_500
        };
        ui.painter().rect_filled(rect, CornerRadius::ZERO, fill);
        // A thick amber cap above and a burnt one below: the button reads as a
        // raised block, and sinks a couple of pixels while held.
        let cap = if pressed { 2.0 } else { 4.0 };
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min, Vec2::new(rect.width(), cap)),
            CornerRadius::ZERO,
            if pressed {
                Color32::from_rgb(198, 84, 8)
            } else {
                SUNRISE_300
            },
        );
        ui.painter().rect_filled(
            Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - cap), rect.max),
            CornerRadius::ZERO,
            Color32::from_rgb(198, 84, 8),
        );
        ui.painter().rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(1.0, Color32::from_rgb(255, 219, 170)),
            StrokeKind::Inside,
        );
    } else {
        block(ui.painter(), rect, OBSIDIAN_800, white(20));
    }
    if response.hovered() && enabled {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let color = if enabled { OBSIDIAN_950 } else { SLATE_500 };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), FontId::proportional(15.0), color);
    let total = 16.0 + 10.0 + galley.size().x;
    let start = rect.center().x - total * 0.5;
    play_icon(
        ui.painter(),
        egui::pos2(start + 7.0, rect.center().y),
        13.0,
        color,
    );
    ui.painter().galley(
        egui::pos2(start + 24.0, rect.center().y - galley.size().y * 0.5),
        galley,
        color,
    );

    response.clicked() && enabled
}

pub(crate) fn window_button(ui: &mut egui::Ui, kind: WindowButton) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    let hovered = response.hovered();
    if hovered {
        let fill = if kind == WindowButton::Close {
            Color32::from_rgba_unmultiplied(255, 71, 87, 40)
        } else {
            white(12)
        };
        ui.painter().rect_filled(rect, CornerRadius::ZERO, fill);
    }
    let color = if hovered && kind == WindowButton::Close {
        CORAL_500
    } else if hovered {
        SLATE_100
    } else {
        SLATE_400
    };
    let c = rect.center();
    let r = 4.0;
    let stroke = Stroke::new(1.2, color);
    match kind {
        WindowButton::Minimize => {
            ui.painter()
                .line_segment([egui::pos2(c.x - r, c.y), egui::pos2(c.x + r, c.y)], stroke);
        }
        WindowButton::Maximize => {
            ui.painter().rect_stroke(
                Rect::from_center_size(c, Vec2::splat(r * 1.8)),
                CornerRadius::ZERO,
                stroke,
                StrokeKind::Inside,
            );
        }
        WindowButton::Close => {
            ui.painter().line_segment(
                [egui::pos2(c.x - r, c.y - r), egui::pos2(c.x + r, c.y + r)],
                stroke,
            );
            ui.painter().line_segment(
                [egui::pos2(c.x - r, c.y + r), egui::pos2(c.x + r, c.y - r)],
                stroke,
            );
        }
    }
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

/// The brand mark: the same pixel art the .exe and the window icon carry.
pub(crate) fn logo(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    logo::paint(ui.painter(), rect);
}

pub(crate) fn play_icon(painter: &egui::Painter, center: egui::Pos2, size: f32, color: Color32) {
    let half = size * 0.5;
    painter.add(Shape::convex_polygon(
        vec![
            egui::pos2(center.x - half * 0.6, center.y - half),
            egui::pos2(center.x + half, center.y),
            egui::pos2(center.x - half * 0.6, center.y + half),
        ],
        color,
        Stroke::NONE,
    ));
}

pub(crate) fn check_icon(ui: &mut egui::Ui, color: Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let c = rect.center();
    let stroke = Stroke::new(1.8, color);
    ui.painter().line_segment(
        [
            egui::pos2(c.x - size * 0.42, c.y),
            egui::pos2(c.x - size * 0.1, c.y + size * 0.32),
        ],
        stroke,
    );
    ui.painter().line_segment(
        [
            egui::pos2(c.x - size * 0.1, c.y + size * 0.32),
            egui::pos2(c.x + size * 0.45, c.y - size * 0.34),
        ],
        stroke,
    );
}

pub(crate) fn terminal_icon(ui: &mut egui::Ui, color: Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let c = rect.center();
    let stroke = Stroke::new(1.2, color);
    ui.painter().line_segment(
        [
            egui::pos2(c.x - size * 0.35, c.y - size * 0.25),
            egui::pos2(c.x, c.y),
        ],
        stroke,
    );
    ui.painter().line_segment(
        [
            egui::pos2(c.x, c.y),
            egui::pos2(c.x - size * 0.35, c.y + size * 0.25),
        ],
        stroke,
    );
    ui.painter().line_segment(
        [
            egui::pos2(c.x + size * 0.1, c.y + size * 0.3),
            egui::pos2(c.x + size * 0.4, c.y + size * 0.3),
        ],
        stroke,
    );
}

pub(crate) fn draw_icon(
    painter: &egui::Painter,
    icon: Icon,
    center: egui::Pos2,
    size: f32,
    color: Color32,
) {
    let stroke = Stroke::new(1.1, color);
    let s = size * 0.5;
    match icon {
        Icon::Sliders => {
            for (index, offset) in [-0.5f32, 0.1, 0.6].iter().enumerate() {
                let y = center.y + s * offset;
                painter.line_segment(
                    [egui::pos2(center.x - s, y), egui::pos2(center.x + s, y)],
                    stroke,
                );
                let knob = center.x - s + size * (0.25 + index as f32 * 0.22);
                painter.rect_filled(
                    Rect::from_center_size(egui::pos2(knob, y), Vec2::splat(3.5)),
                    CornerRadius::ZERO,
                    color,
                );
            }
        }
        Icon::Play => {
            painter.add(Shape::convex_polygon(
                vec![
                    egui::pos2(center.x - s * 0.6, center.y - s),
                    egui::pos2(center.x + s, center.y),
                    egui::pos2(center.x - s * 0.6, center.y + s),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Terminal => {
            painter.line_segment(
                [
                    egui::pos2(center.x - s * 0.8, center.y - s * 0.6),
                    egui::pos2(center.x - s * 0.1, center.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(center.x - s * 0.1, center.y),
                    egui::pos2(center.x - s * 0.8, center.y + s * 0.6),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(center.x, center.y + s * 0.7),
                    egui::pos2(center.x + s * 0.9, center.y + s * 0.7),
                ],
                stroke,
            );
        }
    }
}
