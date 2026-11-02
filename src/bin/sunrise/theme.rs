//! The palette, the egui style, and what is painted rather than laid out:
//! the ambient light and the hero's desert dusk.

use eframe::egui;
use egui::{Color32, CornerRadius, Rect, Shape, Stroke, Vec2};

pub(crate) const OBSIDIAN_950: Color32 = Color32::from_rgb(6, 7, 10);

pub(crate) const OBSIDIAN_850: Color32 = Color32::from_rgb(17, 20, 29);

pub(crate) const OBSIDIAN_800: Color32 = Color32::from_rgb(23, 27, 39);

pub(crate) const SUNRISE_300: Color32 = Color32::from_rgb(255, 193, 122);

pub(crate) const SUNRISE_400: Color32 = Color32::from_rgb(255, 158, 59);

pub(crate) const SUNRISE_500: Color32 = Color32::from_rgb(255, 119, 17);

pub(crate) const CORAL_500: Color32 = Color32::from_rgb(255, 71, 87);

pub(crate) const SLATE_100: Color32 = Color32::from_rgb(241, 245, 249);

pub(crate) const SLATE_300: Color32 = Color32::from_rgb(203, 213, 225);

pub(crate) const SLATE_400: Color32 = Color32::from_rgb(148, 163, 184);

pub(crate) const SLATE_500: Color32 = Color32::from_rgb(100, 116, 139);

pub(crate) const EMERALD_400: Color32 = Color32::from_rgb(52, 211, 153);

pub(crate) const AMBER_TEXT: Color32 = Color32::from_rgb(252, 211, 77);

pub(crate) fn white(alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(255, 255, 255, alpha)
}

pub(crate) fn glass_fill() -> Color32 {
    Color32::from_rgba_unmultiplied(17, 20, 29, 210)
}

pub(crate) fn glow(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    color: Color32,
    alpha: u8,
) {
    let rings = 10;
    for i in (1..=rings).rev() {
        let r = radius * i as f32 / rings as f32;
        painter.circle_filled(
            center,
            r,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha / 6),
        );
    }
}

/// The hero backdrop: a desert dusk, drawn rather than shipped as an image.
pub(crate) fn paint_sky(painter: &egui::Painter, rect: Rect) {
    let clip = painter.with_clip_rect(rect.shrink(1.0));
    let w = rect.width();
    let h = rect.height();
    let px = |f: f32| rect.left() + w * f;
    let py = |f: f32| rect.top() + h * f;

    let sky = [
        (0.00, Color32::from_rgb(42, 26, 37)),
        (0.14, Color32::from_rgb(88, 47, 55)),
        (0.30, Color32::from_rgb(142, 75, 56)),
        (0.44, Color32::from_rgb(198, 108, 60)),
        (0.54, Color32::from_rgb(233, 149, 82)),
        (0.61, Color32::from_rgb(247, 179, 111)),
        (0.68, Color32::from_rgb(206, 125, 76)),
        (0.80, Color32::from_rgb(146, 81, 54)),
        (0.92, Color32::from_rgb(92, 52, 40)),
        (1.00, Color32::from_rgb(52, 30, 28)),
    ];
    let bands = 110;
    for i in 0..bands {
        let t0 = i as f32 / bands as f32;
        let t1 = (i + 1) as f32 / bands as f32;
        clip.rect_filled(
            Rect::from_min_max(
                egui::pos2(rect.left(), rect.top() + h * t0),
                egui::pos2(rect.right(), rect.top() + h * t1 + 0.5),
            ),
            0,
            stops_color(&sky, t0),
        );
    }

    // The sun sits low and left of centre, hazed into the sky. It is drawn in
    // bands rather than as a circle, so its edge steps like a pixel sprite.
    let sun = egui::pos2(px(0.42), py(0.40));
    for ring in (1..=12).rev() {
        pixel_disc(
            &clip,
            sun,
            26.0 + ring as f32 * 11.0,
            Color32::from_rgba_unmultiplied(255, 202, 134, 6),
            11,
        );
    }
    pixel_disc(&clip, sun, 34.0, Color32::from_rgb(255, 206, 142), 9);
    pixel_disc(&clip, sun, 26.0, Color32::from_rgb(255, 244, 208), 9);

    // Far ridges on the horizon, then the dunes in front of them.
    ridge(
        &clip,
        rect,
        &[
            (0.00, 0.600),
            (0.09, 0.552),
            (0.17, 0.586),
            (0.27, 0.540),
            (0.36, 0.590),
            (0.47, 0.560),
            (0.58, 0.600),
            (0.68, 0.565),
            (0.80, 0.596),
            (0.90, 0.556),
            (1.00, 0.590),
        ],
        0.65,
        Color32::from_rgb(96, 51, 45),
    );
    dune(
        &clip,
        rect,
        0.860,
        0.020,
        0.4,
        1.4,
        Color32::from_rgb(92, 48, 37),
        0.95,
    );
    dune(
        &clip,
        rect,
        1.010,
        0.026,
        2.1,
        1.9,
        Color32::from_rgb(27, 15, 18),
        1.00,
    );

    // Cacti, thick enough to read at this size, as in the reference.
    for (fx, base, height, width) in [
        (0.050f32, 0.958f32, 0.255f32, 13.0f32),
        (0.106, 0.930, 0.135, 9.0),
        (0.266, 0.952, 0.190, 11.0),
        (0.545, 0.964, 0.225, 12.0),
        (0.628, 0.944, 0.120, 9.0),
        (0.876, 0.972, 0.165, 11.0),
    ] {
        cactus(&clip, egui::pos2(px(fx), py(base)), h * height, width);
    }

    // Dark scrim so the name and playtime stay readable over the scene.
    let scrim = 28;
    for i in 0..scrim {
        let t = i as f32 / scrim as f32;
        let alpha = (t * t * 210.0) as u8;
        clip.rect_filled(
            Rect::from_min_max(
                egui::pos2(rect.left(), rect.top() + h * 0.46 + t * h * 0.54),
                egui::pos2(
                    rect.right(),
                    rect.top() + h * 0.46 + (t + 1.0 / scrim as f32) * h * 0.54,
                ),
            ),
            0,
            Color32::from_rgba_unmultiplied(9, 8, 12, alpha),
        );
    }
}

/// A filled disc built from horizontal bands, so its edge steps instead of
/// curving: the blocky way the game draws a sun.
fn pixel_disc(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    color: Color32,
    bands: u32,
) {
    let step = radius * 2.0 / bands as f32;
    for band in 0..bands {
        let y = center.y - radius + band as f32 * step;
        let dy = (y + step * 0.5 - center.y) / radius;
        let half = radius * (1.0 - dy * dy).max(0.0).sqrt();
        painter.rect_filled(
            Rect::from_min_max(
                egui::pos2(center.x - half, y),
                egui::pos2(center.x + half, y + step + 0.5),
            ),
            CornerRadius::ZERO,
            color,
        );
    }
}

/// A silhouette from a run of heights, filled down to `baseline`.
pub(crate) fn ridge(
    painter: &egui::Painter,
    rect: Rect,
    points: &[(f32, f32)],
    baseline: f32,
    color: Color32,
) {
    let x = |f: f32| rect.left() + rect.width() * f;
    let y = |f: f32| rect.top() + rect.height() * f;
    for pair in points.windows(2) {
        let (x0, y0) = pair[0];
        let (x1, y1) = pair[1];
        painter.add(Shape::convex_polygon(
            vec![
                egui::pos2(x(x0), y(y0)),
                egui::pos2(x(x1) + 0.8, y(y1)),
                egui::pos2(x(x1) + 0.8, y(baseline)),
                egui::pos2(x(x0), y(baseline)),
            ],
            color,
            Stroke::NONE,
        ));
    }
}

/// A soft rolling dune, sampled so its edge curves instead of stepping.
pub(crate) fn dune(
    painter: &egui::Painter,
    rect: Rect,
    base: f32,
    amp: f32,
    phase: f32,
    freq: f32,
    color: Color32,
    baseline: f32,
) {
    let steps = 64;
    let w = rect.width();
    let h = rect.height();
    let top = |f: f32| {
        let a = (f * freq * std::f32::consts::TAU + phase).sin();
        let b = (f * freq * 2.3 * std::f32::consts::TAU + phase * 1.7).sin();
        // Snap the crest to five-pixel terraces: blocky terrain, not a curve.
        let step = 5.0 / h;
        ((base + amp * a + amp * 0.45 * b) / step).floor() * step
    };
    for i in 0..steps {
        let f0 = i as f32 / steps as f32;
        let f1 = (i + 1) as f32 / steps as f32;
        // Overlap the next slice by a hair: shared edges otherwise show as
        // vertical seams through the antialiasing.
        painter.add(Shape::convex_polygon(
            vec![
                egui::pos2(rect.left() + w * f0, rect.top() + h * top(f0)),
                egui::pos2(rect.left() + w * f1 + 0.8, rect.top() + h * top(f1)),
                egui::pos2(rect.left() + w * f1 + 0.8, rect.top() + h * baseline),
                egui::pos2(rect.left() + w * f0, rect.top() + h * baseline),
            ],
            color,
            Stroke::NONE,
        ));
    }
}

pub(crate) fn cactus(painter: &egui::Painter, base: egui::Pos2, height: f32, width: f32) {
    let color = Color32::from_rgb(24, 13, 15);
    let left = base.x - width * 0.5;
    painter.rect_filled(
        Rect::from_min_size(egui::pos2(left, base.y - height), Vec2::new(width, height)),
        CornerRadius::ZERO,
        color,
    );
    // Two arms, each a horizontal stub then a short upright, like the reference.
    let arm = width * 0.9;
    let left_y = base.y - height * 0.62;
    painter.rect_filled(
        Rect::from_min_size(
            egui::pos2(left - arm, left_y - height * 0.30),
            Vec2::new(arm + 1.0, height * 0.10),
        ),
        CornerRadius::ZERO,
        color,
    );
    painter.rect_filled(
        Rect::from_min_size(
            egui::pos2(left - arm, left_y - height * 0.30),
            Vec2::new(width * 0.45, height * 0.30),
        ),
        CornerRadius::ZERO,
        color,
    );
    let right_y = base.y - height * 0.48;
    painter.rect_filled(
        Rect::from_min_size(
            egui::pos2(left + width - 1.0, right_y - height * 0.22),
            Vec2::new(arm + 1.0, height * 0.09),
        ),
        CornerRadius::ZERO,
        color,
    );
    painter.rect_filled(
        Rect::from_min_size(
            egui::pos2(left + width + arm - width * 0.45, right_y - height * 0.22),
            Vec2::new(width * 0.45, height * 0.22),
        ),
        CornerRadius::ZERO,
        color,
    );
}

pub(crate) fn stops_color(stops: &[(f32, Color32)], t: f32) -> Color32 {
    if stops.len() < 2 {
        return stops.first().map(|s| s.1).unwrap_or(OBSIDIAN_850);
    }
    let t = t.clamp(0.0, 1.0);
    for pair in stops.windows(2) {
        let (from, a) = pair[0];
        let (to, b) = pair[1];
        if t <= to {
            let span = (to - from).max(f32::EPSILON);
            return lerp_color(a, b, ((t - from) / span).clamp(0.0, 1.0));
        }
    }
    stops[stops.len() - 1].1
}

pub(crate) fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

/// The window-wide look: the palette applied to egui once, at startup.
pub(crate) fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    let mut visuals = egui::Visuals::dark();
    // Minecraft's interface has no rounded corners anywhere; neither does this one.
    visuals.window_corner_radius = CornerRadius::ZERO;
    visuals.menu_corner_radius = CornerRadius::ZERO;
    visuals.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.6 };
    visuals.panel_fill = OBSIDIAN_950;
    visuals.window_fill = OBSIDIAN_850;
    visuals.window_stroke = Stroke::new(1.0, white(30));
    visuals.extreme_bg_color = OBSIDIAN_950;
    visuals.override_text_color = Some(SLATE_100);
    visuals.widgets.noninteractive.bg_fill = OBSIDIAN_850;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, white(20));
    visuals.widgets.noninteractive.corner_radius = CornerRadius::ZERO;
    visuals.widgets.inactive.bg_fill = white(8);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, white(22));
    visuals.widgets.inactive.corner_radius = CornerRadius::ZERO;
    visuals.widgets.hovered.bg_fill = white(16);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, white(45));
    visuals.widgets.hovered.corner_radius = CornerRadius::ZERO;
    visuals.widgets.active.bg_fill = white(24);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, SUNRISE_400.gamma_multiply(0.7));
    visuals.widgets.active.corner_radius = CornerRadius::ZERO;
    visuals.selection.bg_fill = SUNRISE_500.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, SUNRISE_400);
    visuals.hyperlink_color = SUNRISE_400;
    ctx.set_visuals(visuals);
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.button_padding = Vec2::new(10.0, 5.0);
    });
}

/// Monocraft (SIL OFL 1.1, see `assets/fonts/OFL.txt`): a Minecraft-flavoured
/// pixel font, the closest thing to the game's own lettering that can ship.
const MONOCRAFT: &[u8] = include_bytes!("../../../assets/fonts/Monocraft.ttf");

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "monocraft".to_owned(),
        std::sync::Arc::new(
            egui::FontData::from_static(MONOCRAFT).tweak(egui::FontTweak {
                // Monocraft sits small for its point size; nudge it back up.
                scale: 1.12,
                ..Default::default()
            }),
        ),
    );
    // Front of both families, so every label, button and log line is blocky;
    // egui's own fonts stay behind it for anything it cannot draw.
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, "monocraft".to_owned());
    }
    ctx.set_fonts(fonts);
}

pub(crate) fn paint_ambient(ctx: &egui::Context) {
    let rect = ctx.viewport_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    painter.rect_filled(rect, 0, OBSIDIAN_950);
    glow(
        &painter,
        egui::pos2(rect.left() + 40.0, rect.top() + 10.0),
        300.0,
        SUNRISE_500,
        22,
    );
    glow(
        &painter,
        egui::pos2(rect.right() - 60.0, rect.center().y),
        260.0,
        CORAL_500,
        14,
    );
}
